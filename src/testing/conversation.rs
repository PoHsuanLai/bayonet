//! What a host's handler can do while it serves a request, and how it ends the plugin.

use super::fault::Fault;
use crate::wire::{WireError, write_frame};
use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::mpsc::Receiver;

/// How the plugin's run ends.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ending {
    /// The plugin is done and exits cleanly.
    Finished,
    /// The plugin never greeted and stays silent.
    Mute,
    /// The plugin stays alive and says nothing more.
    Hung,
    /// The plugin dies with `code` after writing `said` to stderr.
    Crashed {
        /// The exit status.
        code: u8,
        /// The line written to stderr before dying; empty for none.
        said: String,
    },
}

/// What the plugin does after a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Wait for the next request.
    Next,
    /// Stop serving.
    End(Ending),
}

/// The plugin's side of one conversation, handed to the host's closure with each request.
#[derive(Debug)]
pub struct Conversation<'a, W, Q> {
    pub(super) out: &'a mut W,
    pub(super) inbox: &'a Receiver<Q>,
    pub(super) held: &'a mut VecDeque<Q>,
    pub(super) is_cancel: fn(&Q) -> bool,
    pub(super) fault: Option<Fault>,
}

impl<W: Write, Q> Conversation<'_, W, Q> {
    /// Writes one message and its payload as a frame.
    pub fn say<T: Serialize>(&mut self, message: &T, payload: &[u8]) -> Result<(), WireError> {
        write_frame(self.out, message, payload)
    }

    /// Writes `bytes` as they are, for a host's own kind of broken frame.
    pub fn say_raw(&mut self, bytes: &[u8]) -> Result<(), WireError> {
        self.out
            .write_all(bytes)
            .and_then(|()| self.out.flush())
            .map_err(|error| WireError::Io { kind: error.kind() })
    }

    /// Whether the host has asked to cancel since the last call, taking every cancel off the
    /// queue. Requests that arrive meanwhile are kept for the loop. Under
    /// [`Fault::IgnoreCancel`] the answer is always no.
    pub fn cancelled(&mut self) -> bool {
        while let Ok(request) = self.inbox.try_recv() {
            self.held.push_back(request);
        }
        let before = self.held.len();
        self.held.retain(|request| !(self.is_cancel)(request));
        let asked = self.held.len() < before;
        asked && self.fault != Some(Fault::IgnoreCancel)
    }

    /// The fault this plugin was started with, so a closure can honour one of its own kind.
    pub fn fault(&self) -> Option<Fault> {
        self.fault
    }
}
