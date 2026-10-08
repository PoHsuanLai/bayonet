//! What discovery found, before and after the registry ranks it.

use crate::manifest::{Manifest, ManifestError, PluginId, Provides};

/// Which directory a manifest was found in. The person's own beats the system's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Origin {
    /// `$XDG_DATA_DIRS`: installed by the distribution.
    System,
    /// `$XDG_DATA_HOME`: installed by the person.
    User,
}

/// Whether the files a manifest names can be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Readiness<P: Provides> {
    /// Every program is there and runs.
    Ready,
    /// One is not, and this says which and why.
    Unready(ManifestError<P::Capability, P::Fault>),
}

/// A manifest as discovery found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate<P: Provides> {
    /// What the plugin says of itself.
    pub manifest: Manifest<P>,
    /// Where it was found.
    pub origin: Origin,
    /// Whether its programs are usable.
    pub readiness: Readiness<P>,
}

/// A plugin the host can use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed<P> {
    /// What the plugin says of itself.
    pub manifest: Manifest<P>,
    /// Where it was found.
    pub origin: Origin,
}

/// A plugin that was found and cannot be used, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unusable<P: Provides> {
    /// Which plugin.
    pub id: PluginId,
    /// Where it was found.
    pub origin: Origin,
    /// Why not.
    pub reason: ManifestError<P::Capability, P::Fault>,
}

/// How well a plugin's entry fits a request: the host decides, bayonet only orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fit {
    /// The entry does not serve the request.
    Miss,
    /// It serves it by a broad match (the request's kind).
    Broad,
    /// It serves it by a closer one (the request's media type), and beats a broad one.
    Exact,
}
