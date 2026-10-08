//! What a host calls the things a plugin can offer.

use std::fmt::Debug;
use std::hash::Hash;

/// The type a host uses to name one thing a plugin offers (`probe`, `block.lens`).
///
/// bayonet never lists capabilities: the host defines them, usually as one fieldless enum with a
/// serde spelling, and bayonet carries the value through manifests, the registry and the
/// handshake. Any small, comparable value qualifies, so the trait is implemented for every type
/// that meets the bounds.
pub trait Capability: Copy + Debug + Eq + Hash + Send + Sync + 'static {}

impl<T: Copy + Debug + Eq + Hash + Send + Sync + 'static> Capability for T {}
