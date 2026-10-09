//! The fake plugin: greet, then serve each request through the host's closure.

use super::conversation::{Conversation, Ending, Step};
use super::fault::Fault;
use crate::wire::{WireError, read_frame};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

/// How long a hung or mute plugin stays alive: longer than any timeout a test waits for.
const HANG: Duration = Duration::from_secs(60);

/// The line [`Fault::CrashOnRequest`] writes to stderr before dying.
const CRASH_LINE: &str = "fake plugin: crashing on purpose";

/// The exit status of [`Fault::CrashOnRequest`].
const CRASH_CODE: u8 = 3;

/// What [`Fault::StderrFlood`] writes: a megabyte with no newline.
const FLOOD_BYTES: usize = 1 << 20;

/// Not a frame: a header that announces five bytes of JSON and none of payload, then five bytes
/// that are not JSON.
const NOT_A_FRAME: &[u8] = b"\x05\0\0\0\0\0\0\0{nope";

/// Builds the host's greeting message from a protocol version and the capabilities to announce.
type Hello<C, M> = Box<dyn Fn(u32, &[C]) -> M>;

/// A plugin that speaks a host's protocol. `C` is the host's capability type, `M` what the plugin
/// says (the greeting included) and `Q` what the host says to it.
pub struct Fake<C, M, Q> {
    protocol: u32,
    provides: Vec<C>,
    hello: Hello<C, M>,
    is_cancel: fn(&Q) -> bool,
    fault: Option<Fault>,
}

impl<C, M, Q> std::fmt::Debug for Fake<C, M, Q> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fake")
            .field("protocol", &self.protocol)
            .field("fault", &self.fault)
            .finish_non_exhaustive()
    }
}

impl<C, M, Q> Fake<C, M, Q> {
    /// A fake that speaks `protocol` and offers `provides`. `hello` builds the host's greeting
    /// message from the protocol version and capabilities to announce (they differ from these
    /// under a fault), and `is_cancel` says which of the host's requests is the cancel.
    pub fn new(
        protocol: u32,
        provides: Vec<C>,
        hello: impl Fn(u32, &[C]) -> M + 'static,
        is_cancel: fn(&Q) -> bool,
    ) -> Fake<C, M, Q> {
        Fake {
            protocol,
            provides,
            hello: Box::new(hello),
            is_cancel,
            fault: None,
        }
    }

    /// The same fake, misbehaving as `fault` says; `None` for a well-behaved one.
    pub fn with_fault(self, fault: Option<Fault>) -> Fake<C, M, Q> {
        Fake { fault, ..self }
    }
}

impl<C, M: Serialize, Q: DeserializeOwned + Send + 'static> Fake<C, M, Q> {
    /// Serves the conversation on `input` and `out`, with `err` standing for stderr, and says how
    /// it ended. Nothing here exits, sleeps or panics: [`Fake::run`] turns the ending into what a
    /// process does, so a unit test can serve in memory.
    ///
    /// The plugin greets, unless it is mute, then gives each request to `handler`. A fault that
    /// acts on a request acts on the first one. Requests are read on a thread of their own, so
    /// the handler can see a cancel while it works ([`Conversation::cancelled`]).
    pub fn serve<R, W, E, H>(
        &self,
        input: R,
        out: &mut W,
        err: &mut E,
        mut handler: H,
    ) -> Result<Ending, WireError>
    where
        R: Read + Send + 'static,
        W: Write,
        E: Write,
        H: FnMut(Q, &mut Conversation<'_, W, Q>) -> Result<Step, WireError>,
    {
        if self.fault == Some(Fault::Mute) {
            return Ok(Ending::Mute);
        }
        self.greet(out)?;
        let inbox = read_requests(input);
        let mut held = VecDeque::new();
        let mut first = true;
        loop {
            let Some(request) = held.pop_front().or_else(|| inbox.recv().ok()) else {
                return Ok(Ending::Finished);
            };
            if std::mem::take(&mut first)
                && let Some(ending) = self.misbehave(out, err)?
            {
                return Ok(ending);
            }
            let mut conversation = Conversation {
                out,
                inbox: &inbox,
                held: &mut held,
                is_cancel: self.is_cancel,
                fault: self.fault,
            };
            match handler(request, &mut conversation)? {
                Step::Next => {}
                Step::End(ending) => return Ok(ending),
            }
        }
    }

    /// Serves on standard input and output and returns the exit code the ending calls for. A
    /// mute or hung plugin sleeps first, so the host sees a plugin that is alive and quiet.
    pub fn run<H>(&self, handler: H) -> ExitCode
    where
        H: FnMut(
            Q,
            &mut Conversation<'_, std::io::StdoutLock<'static>, Q>,
        ) -> Result<Step, WireError>,
    {
        let mut out = std::io::stdout().lock();
        let mut err = std::io::stderr();
        match self.serve(std::io::stdin(), &mut out, &mut err, handler) {
            Ok(Ending::Finished) => ExitCode::SUCCESS,
            Ok(Ending::Mute | Ending::Hung) => {
                std::thread::sleep(HANG);
                ExitCode::SUCCESS
            }
            Ok(Ending::Crashed { code, .. }) => ExitCode::from(code),
            Err(_) => ExitCode::FAILURE,
        }
    }

    fn greet<W: Write>(&self, out: &mut W) -> Result<(), WireError> {
        let (protocol, provides) = match self.fault {
            Some(Fault::OtherVersion) => (self.protocol.wrapping_add(1), &self.provides[..]),
            Some(Fault::ProvidesNothing) => (self.protocol, &self.provides[..0]),
            _ => (self.protocol, &self.provides[..]),
        };
        crate::wire::write_frame(out, &(self.hello)(protocol, provides), &[])
    }

    /// What the fault does when the first request arrives: `Some` ends the plugin, `None` lets it
    /// serve.
    fn misbehave<W: Write, E: Write>(
        &self,
        out: &mut W,
        err: &mut E,
    ) -> Result<Option<Ending>, WireError> {
        let io = |error: std::io::Error| WireError::Io { kind: error.kind() };
        match self.fault {
            Some(Fault::CrashOnRequest) => {
                writeln!(err, "{CRASH_LINE}").map_err(io)?;
                Ok(Some(Ending::Crashed {
                    code: CRASH_CODE,
                    said: CRASH_LINE.to_owned(),
                }))
            }
            Some(Fault::HangOnRequest) => Ok(Some(Ending::Hung)),
            Some(Fault::Garbage) => {
                out.write_all(NOT_A_FRAME)
                    .and_then(|()| out.flush())
                    .map_err(io)?;
                Ok(Some(Ending::Finished))
            }
            Some(Fault::StderrFlood) => {
                err.write_all(&vec![b'x'; FLOOD_BYTES]).map_err(io)?;
                Ok(None)
            }
            Some(
                Fault::Mute | Fault::OtherVersion | Fault::ProvidesNothing | Fault::IgnoreCancel,
            )
            | None => Ok(None),
        }
    }
}

/// Reads the host's requests on a thread of their own, until the stream ends or breaks.
fn read_requests<R, Q>(mut input: R) -> mpsc::Receiver<Q>
where
    R: Read + Send + 'static,
    Q: DeserializeOwned + Send + 'static,
{
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok(frame) = read_frame::<_, Q>(&mut input) {
            if send.send(frame.message).is_err() {
                break;
            }
        }
    });
    receive
}
