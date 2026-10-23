//! The manifest a plugin installs: the shared envelope, parsed and checked once, with the host's
//! own `[[provides]]` entries inside it.
//!
//! ```toml
//! id       = "ffmpeg"          # [a-z0-9_-]{1,64}; names the file and decides overrides
//! name     = "FFmpeg"          # what a person calls it
//! protocol = 1                 # the newest wire version the plugin speaks; at least 1
//! version  = "1.2.0"           # optional: dotted numbers, compared numerically
//! licence  = "MIT"             # optional: an SPDX expression, kept as text
//! summary  = "What it is for"  # optional: one line
//!
//! [program]                    # absent only when no entry needs one
//! path = "/usr/libexec/app/plugin"   # absolute
//! args = []
//!
//! [[provides]]                 # one table for each capability; its keys are the host's
//! capability = "probe"
//! ```
//!
//! In a folder layout (`<id>/<manifest name>`) the id is reverse DNS (`org.example.Paper`) and
//! must be the folder's name, and every path is relative to the folder and stays inside it; only
//! `[program] path` may also be absolute, because a package installs the program elsewhere.

mod confine;
mod error;
mod id;
mod model;
mod parse;
mod provides;
mod raw;
mod version;

pub use error::ManifestError;
pub use id::{InvalidId, PluginId};
pub use model::{Manifest, Program};
pub(crate) use parse::{check_program, typed_entries};
pub use provides::{Entry, PathRole, Provides, Refusal, absolute};
pub use raw::{Raw, RawCapability};
pub use version::{InvalidVersion, Version};

#[cfg(test)]
mod tests;
