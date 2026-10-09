//! The integration tests of bayonet, in one binary so they link once: the crash containment of
//! the runner, the capabilities of a second host, and (with the `testing` feature) a host's
//! runner against the fake plugin.

#![allow(clippy::unwrap_used)]
// A test handler names the messages it expects and refuses the rest alike.
#![allow(clippy::wildcard_enum_match_arm)]

mod capabilities;
mod guard;
mod run;
mod support;
#[cfg(feature = "testing")]
mod testing;
