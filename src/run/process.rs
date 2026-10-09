//! One plugin process and the pipe to it: spawn, read frames with a deadline, write one, kill on
//! drop. Reading polls the pipe instead of blocking on it, so a plugin that stops talking costs
//! a timeout and no thread; the host needs none of its own.

use super::error::RunError;
use super::protocol::Protocol;
use crate::manifest::PluginId;
use crate::manifest::Program;
use crate::wire::{Frame, FrameDecoder, MAX_PAYLOAD_BYTES, WireError, encode_frame};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;
use std::fmt;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How many of a plugin's last stderr lines an error carries.
const TAIL_LINES: usize = 4;

/// The longest stderr line kept: a plugin that writes without a newline cannot grow the host. The
/// rest of a long line is dropped, and the line is logged with an ellipsis.
const STDERR_LINE: usize = 4096;

/// The most one read takes from a pipe (a pipe holds 64 KiB unless the plugin raised it).
const CHUNK: usize = 256 << 10;

/// What the wait for a message came to.
#[derive(Debug)]
pub(super) enum Arrival<M> {
    /// A message and its payload.
    Message(Frame<M>),
    /// Nothing came before the wait ran out.
    Quiet,
}

/// Where a plugin's stderr lines go, each prefixed with the host's name and the plugin's id.
#[derive(Clone)]
pub(super) struct Log {
    pub(super) app: &'static str,
    pub(super) sink: Arc<dyn Fn(&str) + Send + Sync>,
}

impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Log")
            .field("app", &self.app)
            .finish_non_exhaustive()
    }
}

/// A running plugin. Dropping it kills the process and reaps it.
#[derive(Debug)]
pub(super) struct Process<W: Protocol> {
    id: String,
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    stderr: Option<ChildStderr>,
    decoder: FrameDecoder,
    scratch: Box<[u8]>,
    stderr_line: Vec<u8>,
    stderr_cut: bool,
    payload_limit: u64,
    tail: Vec<String>,
    log: Log,
    wire: PhantomData<fn() -> W>,
}

impl<W: Protocol> Process<W> {
    /// Starts `program` with its manifest arguments, stdin and stdout piped and stderr captured
    /// to the log.
    pub(super) fn spawn(
        id: &PluginId,
        program: Option<&Program>,
        log: Log,
    ) -> Result<Process<W>, RunError<W::Capability>> {
        let id = id.as_str().to_owned();
        let program = program.ok_or_else(|| RunError::broke(&id, "the manifest has no program"))?;
        // A group of its own, so that killing the plugin kills what it started too.
        let mut child = Command::new(&program.path)
            .args(&program.args)
            .process_group(0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| RunError::Spawn {
                program: program.path.display().to_string(),
                kind: error.kind(),
            })?;
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RunError::Crashed {
                plugin: id,
                status: "no standard output".to_owned(),
            });
        };
        Ok(Process {
            id,
            child,
            stdin,
            stdout,
            stderr,
            decoder: FrameDecoder::new(),
            scratch: vec![0u8; CHUNK].into_boxed_slice(),
            stderr_line: Vec::new(),
            stderr_cut: false,
            payload_limit: u64::from(MAX_PAYLOAD_BYTES),
            tail: Vec::new(),
            log,
            wire: PhantomData,
        })
    }

    /// The plugin's id, for errors.
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    /// Limits the payload any later message may carry to `bytes`; a header announcing more is
    /// refused before the payload is buffered.
    pub(super) fn limit_payload(&mut self, bytes: u64) {
        self.payload_limit = bytes;
    }

    /// Sends one request. Stderr is drained first, so a plugin that logged while the host was not
    /// receiving is not blocked on a full pipe.
    pub(super) fn send(&mut self, message: &W::Request) -> Result<(), RunError<W::Capability>> {
        self.drain_stderr();
        let bytes = encode_frame(message, &[]).map_err(|error| self.wire_error(&error))?;
        let written = match self.stdin.as_mut() {
            Some(stdin) => stdin.write_all(&bytes).and_then(|()| stdin.flush()),
            None => Err(std::io::ErrorKind::BrokenPipe.into()),
        };
        written.map_err(|_| self.crashed())
    }

    /// Waits at most `wait` for the next message. Stderr is read meanwhile and logged.
    pub(super) fn receive(
        &mut self,
        wait: Duration,
    ) -> Result<Arrival<W::Message>, RunError<W::Capability>> {
        let end = Instant::now() + wait;
        loop {
            match self
                .decoder
                .next_frame_within::<W::Message>(self.payload_limit)
            {
                Ok(Some(frame)) => return Ok(Arrival::Message(frame)),
                Ok(None) => {}
                Err(error) => return Err(self.wire_error(&error)),
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(Arrival::Quiet);
            }
            self.pump(left)?;
        }
    }

    /// Reads what stderr holds now, without waiting.
    fn drain_stderr(&mut self) {
        while let Some(stderr) = &self.stderr {
            let mut fds = [PollFd::new(stderr, PollFlags::IN)];
            let none = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            match poll(&mut fds, Some(&none)) {
                Ok(0) | Err(_) => return,
                Ok(_) => self.read_stderr(),
            }
        }
    }

    /// Waits for readable output for at most `left`, reading what is there.
    fn pump(&mut self, left: Duration) -> Result<(), RunError<W::Capability>> {
        let timeout = Timespec {
            tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(left.subsec_nanos()),
        };
        let mut fds = vec![PollFd::new(&self.stdout, PollFlags::IN)];
        if let Some(stderr) = &self.stderr {
            fds.push(PollFd::new(stderr, PollFlags::IN));
        }
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => return Ok(()),
            Err(_) => return Err(self.crashed()),
        }
        let ready = |fd: &PollFd<'_>| {
            fd.revents()
                .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR)
        };
        let out_ready = ready(&fds[0]);
        let err_ready = fds.get(1).is_some_and(ready);
        if err_ready {
            self.read_stderr();
        }
        if out_ready {
            self.read_stdout()?;
        }
        Ok(())
    }

    fn read_stdout(&mut self) -> Result<(), RunError<W::Capability>> {
        match self.stdout.read(&mut self.scratch) {
            Ok(0) => {
                if self.decoder.is_mid_frame() {
                    return Err(self.wire_error(&WireError::Truncated));
                }
                Err(self.crashed())
            }
            Ok(n) => {
                self.decoder.push(&self.scratch[..n]);
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => Ok(()),
            Err(_) => Err(self.crashed()),
        }
    }

    /// Reads what stderr has and logs each finished line; closes it at the end of the stream.
    fn read_stderr(&mut self) {
        let Some(stderr) = self.stderr.as_mut() else {
            return;
        };
        let mut chunk = [0u8; 4096];
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => {
                self.stderr = None;
                self.finish_line();
            }
            Ok(n) => {
                for byte in &chunk[..n] {
                    if *byte == b'\n' {
                        self.finish_line();
                    } else if self.stderr_line.len() < STDERR_LINE {
                        self.stderr_line.push(*byte);
                    } else {
                        self.stderr_cut = true;
                    }
                }
            }
        }
    }

    fn finish_line(&mut self) {
        if self.stderr_line.is_empty() {
            return;
        }
        let mut line = String::from_utf8_lossy(&self.stderr_line).into_owned();
        if std::mem::take(&mut self.stderr_cut) {
            line.push('…');
        }
        self.stderr_line.clear();
        (self.log.sink)(&format!("{}: plugin {}: {line}", self.log.app, self.id));
        self.tail.push(line);
        if self.tail.len() > TAIL_LINES {
            self.tail.remove(0);
        }
    }

    /// The error for a plugin whose output ended: how it exited, and what it last said.
    pub(super) fn crashed(&mut self) -> RunError<W::Capability> {
        // Reading stderr to its end lets the last line it wrote before dying show up.
        while self.stderr.is_some() {
            self.read_stderr();
        }
        let exit = self.reap();
        let status = match self.tail.last() {
            Some(line) => format!("{exit}; last said: {line}"),
            None => exit,
        };
        RunError::Crashed {
            plugin: self.id.clone(),
            status,
        }
    }

    /// Waits a moment for the process to exit, then kills it, and says how it ended.
    fn reap(&mut self) -> String {
        let end = Instant::now() + Duration::from_millis(200);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return status.to_string(),
                Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(5)),
                Ok(None) => {
                    self.kill();
                    let _ = self.child.wait();
                    return "killed after it closed its output".to_owned();
                }
                Err(_) => return "its status could not be read".to_owned(),
            }
        }
    }

    /// A wire error as the plugin breaking the protocol.
    pub(super) fn wire_error(&self, error: &WireError) -> RunError<W::Capability> {
        RunError::broke(&self.id, error.to_string())
    }

    /// Kills the plugin and everything in its process group: a plugin that runs a program would
    /// otherwise leave it running when the plugin is killed.
    fn kill(&mut self) {
        let _ = rustix::process::kill_process_group(
            rustix::process::Pid::from_child(&self.child),
            rustix::process::Signal::KILL,
        );
        let _ = self.child.kill();
    }
}

impl<W: Protocol> Drop for Process<W> {
    fn drop(&mut self) {
        self.kill();
        let _ = self.child.wait();
    }
}
