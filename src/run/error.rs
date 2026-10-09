//! Why a run failed.

use std::fmt::Debug;
use std::time::Duration;

/// Why a plugin could not answer. Every variant names the plugin by id; none of them takes the
/// host down, and the process is gone by the time the host sees one.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RunError<C: Debug> {
    /// The program could not be started.
    #[error("cannot start {program:?}: {kind}")]
    Spawn {
        /// The program as named.
        program: String,
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// The plugin sent nothing for too long and was killed.
    #[error("the plugin {plugin} was silent for {waited:?}")]
    Silent {
        /// The plugin's id.
        plugin: String,
        /// How long the host waited.
        waited: Duration,
    },
    /// The plugin ended without answering: it crashed, was killed, or exited early.
    #[error("the plugin {plugin} ended without an answer ({status})")]
    Crashed {
        /// The plugin's id.
        plugin: String,
        /// How it ended, with the last line it wrote to stderr when it wrote one.
        status: String,
    },
    /// The plugin broke the protocol: bytes that are not a message, a message out of turn.
    #[error("the plugin {plugin} broke the protocol: {reason}")]
    Protocol {
        /// The plugin's id.
        plugin: String,
        /// What was wrong.
        reason: String,
    },
    /// The plugin speaks a version of the protocol the host does not.
    #[error("the plugin {plugin} speaks protocol {offered}, the host speaks {supported}")]
    Version {
        /// The plugin's id.
        plugin: String,
        /// What it said in its greeting.
        offered: u32,
        /// What the host speaks.
        supported: u32,
    },
    /// The plugin's greeting does not list the capability that was asked of it.
    #[error("the plugin {plugin} does not answer {capability:?}")]
    Lacks {
        /// The plugin's id.
        plugin: String,
        /// What was asked.
        capability: C,
    },
    /// The host asked the plugin to stop and it did (or was killed after the grace period).
    #[error("the plugin {plugin} was stopped")]
    Cancelled {
        /// The plugin's id.
        plugin: String,
    },
}

impl<C: Debug> RunError<C> {
    /// The plugin `plugin` broke the protocol for `reason`.
    pub fn broke(plugin: &str, reason: impl Into<String>) -> RunError<C> {
        RunError::Protocol {
            plugin: plugin.to_owned(),
            reason: reason.into(),
        }
    }

    /// The plugin `plugin` sent `message`, which the host did not expect at that point.
    pub fn unexpected(plugin: &str, message: &impl Debug) -> RunError<C> {
        RunError::broke(plugin, format!("did not expect {message:?}"))
    }
}
