//! Starting a plugin for a request.

use super::error::RunError;
use super::process::{Arrival, Log, Process};
use super::protocol::Protocol;
use super::session::Session;
use super::timeouts::Timeouts;
use crate::manifest::Provides;
use crate::registry::Installed;
use crate::wire::Frame;
use std::sync::Arc;

/// Runs requests against plugins. A request starts the plugin, takes its greeting, checks it and
/// sends the request; the [`Session`] it returns kills the process on every path out.
#[derive(Debug, Clone)]
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
                sink: Arc::new(|line| eprintln!("{line}")),
            },
            timeouts,
        }
    }

    /// The same runner with stderr lines handed to `sink` instead, already prefixed. The sink may
    /// capture state, and is called from the thread that is waiting on the plugin.
    ///
    /// ```
    /// use bayonet::run::{Runner, Timeouts};
    /// use std::sync::{Arc, Mutex};
    ///
    /// let lines = Arc::new(Mutex::new(Vec::new()));
    /// let kept = Arc::clone(&lines);
    /// let runner = Runner::new("myapp", Timeouts::default())
    ///     .with_log(move |line| kept.lock().unwrap().push(line.to_owned()));
    /// # drop(runner);
    /// ```
    pub fn with_log(self, sink: impl Fn(&str) + Send + Sync + 'static) -> Runner {
        Runner {
            log: Log {
                sink: Arc::new(sink),
                ..self.log
            },
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
    ///
    /// ```no_run
    /// use bayonet::manifest::{Entry, Provides, Refusal};
    /// use bayonet::registry::Installed;
    /// use bayonet::run::{Greeting, Protocol, RunError, Runner, Timeouts};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    /// enum Cap {
    ///     Facts,
    /// }
    ///
    /// // What the host reads from a `[[provides]]` table.
    /// #[derive(Debug, Clone, PartialEq, Eq)]
    /// struct Tool(Cap);
    /// impl Provides for Tool {
    ///     type Capability = Cap;
    ///     type Fault = std::convert::Infallible;
    ///     fn parse(_entry: Entry) -> Result<Tool, Refusal<Self::Fault>> {
    ///         Ok(Tool(Cap::Facts))
    ///     }
    ///     fn capability(&self) -> Cap {
    ///         self.0
    ///     }
    /// }
    ///
    /// // What the host and its plugins say to each other.
    /// #[derive(Serialize)]
    /// enum Ask {
    ///     Facts { key: String },
    /// }
    /// #[derive(Debug, Deserialize)]
    /// enum Say {
    ///     Hello { protocol: u32, provides: Vec<Cap> },
    ///     Facts(Vec<String>),
    /// }
    /// struct Wire;
    /// impl Protocol for Wire {
    ///     type Capability = Cap;
    ///     type Request = Ask;
    ///     type Message = Say;
    ///     const VERSION: u32 = 1;
    ///     fn greeting(message: &Say) -> Option<Greeting<'_, Cap>> {
    ///         match message {
    ///             Say::Hello { protocol, provides } => Some(Greeting { protocol: *protocol, provides }),
    ///             Say::Facts(_) => None,
    ///         }
    ///     }
    /// }
    ///
    /// fn facts_of(plugin: &Installed<Tool>) -> Result<Vec<String>, RunError<Cap>> {
    ///     let runner = Runner::new("myapp", Timeouts::default());
    ///     // The plugin is started and greets; the runner checks the greeting and sends the request.
    ///     let request = Ask::Facts { key: "42".to_owned() };
    ///     let mut session = runner.open::<Wire, _>(plugin, Cap::Facts, &request, 1 << 20)?;
    ///     // The one reply, within the silence timeout. The session kills the plugin when dropped.
    ///     match session.reply()?.message {
    ///         Say::Facts(rows) => Ok(rows),
    ///         other => Err(RunError::unexpected(session.id(), &other)),
    ///     }
    /// }
    /// ```
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
            self.log.clone(),
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
