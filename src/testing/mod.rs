//! A fake plugin for a host's tests, generic over the host's own messages.
//!
//! A host that wants to test its runner, its sessions or its handling of a plugin that misbehaves
//! needs a program that speaks its protocol and fails on request. [`Fake`] is that program minus
//! the host's vocabulary: it greets, hands every request to a closure the host supplies, and
//! misbehaves in one of the ways of [`Fault`] when asked to. What only the host can do wrong (a
//! short picture, a crash in the middle of its own export) stays in the host's closure, which
//! writes whatever it likes through its [`Conversation`] and ends the plugin with a [`Step`].
//!
//! ```no_run
//! use bayonet::testing::{Ending, Fake, Fault, Step, fault_argument};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize)]
//! enum Say {
//!     Hello { protocol: u32, provides: Vec<String> },
//!     Pong,
//! }
//! #[derive(Deserialize)]
//! enum Ask {
//!     Ping,
//!     Cancel,
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     let fault = fault_argument(std::env::args().skip(1)).and_then(|name| Fault::from_name(&name));
//!     Fake::new(
//!         1,
//!         vec!["ping".to_owned()],
//!         |protocol, provides| Say::Hello { protocol, provides: provides.to_vec() },
//!         |ask| matches!(ask, Ask::Cancel),
//!     )
//!     .with_fault(fault)
//!     .run(|_ask, conversation| {
//!         conversation.say(&Say::Pong, &[])?;
//!         Ok(Step::End(Ending::Finished))
//!     })
//! }
//! ```
//!
//! # No in-memory session
//!
//! [`Fake::serve`] takes any reader and writer, so the plugin's half of a conversation can be
//! driven in memory. There is no in-memory pipe for [`Session`](crate::run::Session): a session's
//! work is a child process in a group of its own, polled with a deadline and killed on every path
//! out, and a pipe in memory would test none of that. A test of a host's handling of a plugin
//! runs a real, small program, which is what this module makes cheap.

mod conversation;
mod fake;
mod fault;

pub use conversation::{Conversation, Ending, Step};
pub use fake::Fake;
pub use fault::{Fault, fault_argument};

#[cfg(test)]
mod tests;
