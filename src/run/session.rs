//! One open request to one plugin.

use super::error::RunError;
use super::process::{Arrival, Process};
use super::protocol::Protocol;
use super::timeouts::Timeouts;
use crate::wire::{Frame, WireError};
use std::fmt;
use std::ops::ControlFlow;
use std::time::Instant;

/// How often a stream looks at its cancel check while the plugin is quiet.
const CANCEL_POLL: std::time::Duration = std::time::Duration::from_millis(50);

/// A plugin that has greeted, been checked, and been sent its request. Dropping the session
/// kills the plugin and everything it started, and reaps it, on every path out.
pub struct Session<W: Protocol> {
    process: Process<W>,
    timeouts: Timeouts,
}

impl<W: Protocol> fmt::Debug for Session<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("plugin", &self.process.id())
            .field("timeouts", &self.timeouts)
            .finish_non_exhaustive()
    }
}

impl<W: Protocol> Session<W> {
    pub(super) fn new(process: Process<W>, timeouts: Timeouts) -> Session<W> {
        Session { process, timeouts }
    }

    /// The plugin's id.
    pub fn id(&self) -> &str {
        self.process.id()
    }

    /// Sends another request on the same plugin.
    pub fn send(&mut self, request: &W::Request) -> Result<(), RunError<W::Capability>> {
        self.process.send(request)
    }

    /// The plugin broke the protocol for `reason`: an answer of the wrong shape, a picture that
    /// does not match its header.
    pub fn broke(&self, reason: impl Into<String>) -> RunError<W::Capability> {
        RunError::broke(self.id(), reason)
    }

    /// A wire error as the plugin breaking the protocol.
    pub fn wire_error(&self, error: &WireError) -> RunError<W::Capability> {
        self.process.wire_error(error)
    }

    /// The one message that answers a request that has no stream. Waits at most the silence
    /// timeout.
    pub fn reply(&mut self) -> Result<Frame<W::Message>, RunError<W::Capability>> {
        match self.process.receive(self.timeouts.silence)? {
            Arrival::Message(frame) => Ok(frame),
            Arrival::Quiet => Err(RunError::Silent {
                plugin: self.id().to_owned(),
                waited: self.timeouts.silence,
            }),
        }
    }

    /// Reads messages until `on_message` breaks with a value. Each message starts the silence
    /// wait again. When `cancel` first answers yes, `cancel_request` is sent and the plugin has
    /// the cancel grace to stop; after that it is killed and the result is
    /// [`RunError::Cancelled`] (unless the plugin finished first).
    ///
    /// `on_message` gets the plugin's id and the message, and says whether to go on. An error of
    /// its own ends the stream, and the plugin with it.
    pub fn stream<T, E>(
        &mut self,
        mut cancel: impl FnMut(Instant) -> bool,
        cancel_request: &W::Request,
        mut on_message: impl FnMut(&str, Frame<W::Message>) -> Result<ControlFlow<T>, E>,
    ) -> Result<T, E>
    where
        E: From<RunError<W::Capability>>,
    {
        let mut asked_to_cancel: Option<Instant> = None;
        let mut quiet_since = Instant::now();
        loop {
            let now = Instant::now();
            if let Some(since) = asked_to_cancel {
                if now.duration_since(since) >= self.timeouts.cancel_grace {
                    return Err(RunError::Cancelled {
                        plugin: self.id().to_owned(),
                    }
                    .into());
                }
            } else if cancel(now) {
                self.process.send(cancel_request)?;
                asked_to_cancel = Some(now);
            }
            if now.duration_since(quiet_since) >= self.timeouts.silence {
                return Err(RunError::Silent {
                    plugin: self.id().to_owned(),
                    waited: self.timeouts.silence,
                }
                .into());
            }
            match self.process.receive(CANCEL_POLL)? {
                Arrival::Quiet => {}
                Arrival::Message(frame) => {
                    quiet_since = Instant::now();
                    if let ControlFlow::Break(done) = on_message(self.process.id(), frame)? {
                        return Ok(done);
                    }
                }
            }
        }
    }
}
