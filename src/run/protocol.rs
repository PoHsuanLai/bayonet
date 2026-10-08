//! What a host teaches bayonet about its wire.

use crate::capability::Capability;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

/// What a plugin says about itself in its first message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Greeting<'a, C> {
    /// The protocol version the plugin speaks.
    pub protocol: u32,
    /// The capabilities it answers on the wire, on this machine.
    pub provides: &'a [C],
}

/// A host's protocol: the messages it and its plugins exchange. A host implements it for a
/// marker type, and [`Runner`](super::Runner) and [`Session`](super::Session) are generic over
/// that type.
///
/// A plugin is started for each request. It speaks first, with a message that carries a
/// [`Greeting`]; the host then sends one request; the plugin answers with one message or a
/// stream of them. The shape of the requests and answers is the host's.
pub trait Protocol {
    /// The host's capability type.
    type Capability: Capability;
    /// What the host says to a plugin.
    type Request: Serialize;
    /// What a plugin says to the host, the greeting included.
    type Message: DeserializeOwned + Debug;

    /// The one protocol version the host speaks. A plugin that greets with another is refused.
    const VERSION: u32;

    /// The greeting `message` carries, or `None` when it is not the greeting.
    fn greeting(message: &Self::Message) -> Option<Greeting<'_, Self::Capability>>;
}
