//! Starting a plugin for a request.

use super::error::RunError;
use super::process::{Arrival, Log, Process};
use super::protocol::Protocol;
use super::session::Session;
use super::timeouts::Timeouts;
use crate::manifest::Provides;
use crate::registry::Installed;
use crate::wire::Frame;

/// Runs requests against plugins. A request starts the plugin, takes its greeting, checks it and
/// sends the request; the [`Session`] it returns kills the process on every path out.
#[derive(Debug, Clone, Copy)]
pub struct Runner {
    log: Log,
    timeouts: Timeouts,
}

impl Runner {
    /// A runner for the host called `app`, which waits as `timeouts` say. The plugin's stderr
    /// lines go to standard error as `<app>: plugin <id>: <line>`.
    pub fn new(app: &'static str, timeouts: Timeouts) -> Runner {
        Runner {
            log: Log {
                app,
                sink: |line| eprintln!("{line}"),
            },
            timeouts,
        }
    }

    /// The same runner with stderr lines handed to `sink` instead, already prefixed.
    pub fn with_log(self, sink: fn(&str)) -> Runner {
        Runner {
            log: Log { sink, ..self.log },
            ..self
        }
    }

    /// What `plugin` says of itself when started: its greeting message, checked against the
    /// protocol the host speaks. Blocking: it starts the plugin and kills it.
    pub fn hello<W, P>(
        &self,
        plugin: &Installed<P>,
    ) -> Result<Frame<W::Message>, RunError<W::Capability>>
    where
        W: Protocol,
        P: Provides<Capability = W::Capability>,
    {
        let mut process = self.start::<W, P>(plugin)?;
        self.greeting(&mut process)
    }

    /// Starts `plugin`, takes its greeting, checks that it answers `capability`, and sends
    /// `request`. No reply may carry more than `payload_limit` bytes of payload.
    pub fn open<W, P>(
        &self,
        plugin: &Installed<P>,
        capability: W::Capability,
        request: &W::Request,
        payload_limit: u64,
    ) -> Result<Session<W>, RunError<W::Capability>>
    where
        W: Protocol,
        P: Provides<Capability = W::Capability>,
    {
        let mut process = self.start::<W, P>(plugin)?;
        let hello = self.greeting(&mut process)?;
        let answers = W::greeting(&hello.message)
            .is_some_and(|greeting| greeting.provides.contains(&capability));
        if !answers {
            return Err(RunError::Lacks {
                plugin: process.id().to_owned(),
                capability,
            });
        }
        process.limit_payload(payload_limit);
        process.send(request)?;
        Ok(Session::new(process, self.timeouts))
    }

    fn start<W, P>(&self, plugin: &Installed<P>) -> Result<Process<W>, RunError<W::Capability>>
    where
        W: Protocol,
        P: Provides<Capability = W::Capability>,
    {
        Process::spawn(
            &plugin.manifest.id,
            plugin.manifest.program.as_ref(),
            self.log,
        )
    }

    /// The plugin's first message, checked against the protocol this host speaks.
    fn greeting<W: Protocol>(
        &self,
        process: &mut Process<W>,
    ) -> Result<Frame<W::Message>, RunError<W::Capability>> {
        let frame = match process.receive(self.timeouts.hello)? {
            Arrival::Message(frame) => frame,
            Arrival::Quiet => {
                return Err(RunError::Silent {
                    plugin: process.id().to_owned(),
                    waited: self.timeouts.hello,
                });
            }
        };
        let Some(greeting) = W::greeting(&frame.message) else {
            return Err(RunError::unexpected(process.id(), &frame.message));
        };
        if greeting.protocol != W::VERSION {
            return Err(RunError::Version {
                plugin: process.id().to_owned(),
                offered: greeting.protocol,
                supported: W::VERSION,
            });
        }
        Ok(frame)
    }
}
