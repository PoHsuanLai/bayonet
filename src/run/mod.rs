//! Running a plugin: start its program, take its greeting, send one request, read the answer
//! with a deadline, and kill it and everything it started on every path out.
//!
//! A crash, a hang or a lie of a plugin is an event and costs one request: it is a [`RunError`],
//! never a panic and never a stuck thread. Reading polls the pipe, so the host needs no thread
//! of its own. This half is Unix only (process groups, `poll`); the wire, the manifests and the
//! registry build everywhere.

mod error;
mod process;
mod protocol;
mod runner;
mod session;
mod timeouts;

pub use error::RunError;
pub use protocol::{Greeting, Protocol};
pub use runner::Runner;
pub use session::Session;
pub use timeouts::Timeouts;
