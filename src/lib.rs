//! Run plugins as separate programs.
//!
//! A plugin is an executable a person installs. The host never links it: it describes itself in a
//! TOML manifest, the host finds the manifest, starts the program for a request, and talks to it
//! over a pipe in length-prefixed JSON frames. A plugin that crashes, hangs or lies costs one
//! request and never takes the host down.
//!
//! bayonet is generic over the host's *capability type*, the things a plugin can offer. It owns
//! the mechanics and none of the vocabulary:
//!
//! - [`wire`]: frames, the blocking reader and writer a plugin uses, and the incremental
//!   decoder a host uses. Always built; a plugin program depends on this alone.
//! - [`manifest`]: the envelope every manifest has (`id`, `name`, `protocol`, `[program]`), with
//!   each `[[provides]]` table handed to the host's own [`manifest::Provides`] type.
//! - [`discover`]: finding manifests in `<data dir>/<app>/plugins`, with the app name a
//!   parameter, and checking the programs they name.
//! - [`registry`]: which plugin serves a request, independent of the order the disk listed files
//!   in.
//! - [`suggest`]: the package to name when none does.
//! - [`run`] (Unix): spawning, greeting, timeouts, cancellation, crash containment.
//!
//! - [`testing`] (feature `testing`): a fake plugin a host's tests build in a few lines.
//!
//! The `host` feature (on by default) builds everything but the wire.

mod capability;
#[cfg(feature = "host")]
mod discover;
#[cfg(all(test, feature = "host"))]
mod fixture;
#[cfg(feature = "host")]
pub mod manifest;
#[cfg(feature = "host")]
pub mod registry;
#[cfg(all(feature = "host", unix))]
pub mod run;
#[cfg(feature = "host")]
mod suggest;
#[cfg(feature = "testing")]
pub mod testing;
pub mod wire;

pub use capability::Capability;
#[cfg(feature = "host")]
pub use discover::{Discovery, Rejected, Search, discover};
#[cfg(feature = "host")]
pub use suggest::{Package, Suggestion, suggest};
