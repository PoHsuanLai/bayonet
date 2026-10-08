//! The manifest a plugin installs: the shared envelope, parsed and checked once, with the host's
//! own `[[provides]]` entries inside it.
//!
//! ```toml
//! id       = "ffmpeg"          # [a-z0-9_-]{1,64}; names the file and decides overrides
//! name     = "FFmpeg"          # what a person calls it
//! protocol = 1                 # the newest wire version the plugin speaks; at least 1
//!
//! [program]                    # absent only when no entry needs one
//! path = "/usr/libexec/app/plugin"   # absolute
//! args = []
//!
//! [[provides]]                 # one table for each capability; its keys are the host's
//! capability = "probe"
//! ```

mod error;
mod id;
mod model;
mod parse;
mod provides;

pub use error::ManifestError;
pub use id::{InvalidId, PluginId};
pub use model::{Manifest, Program};
pub use provides::{Entry, PathRole, Provides, Refusal, absolute};

#[cfg(test)]
mod tests;
