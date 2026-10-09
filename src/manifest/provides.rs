//! What a host teaches bayonet about its `[[provides]]` entries.

use super::error::ManifestError;
use crate::capability::Capability;
use serde::de::DeserializeOwned;
use std::fmt;
use std::path::{Path, PathBuf};

/// What a path a manifest names must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRole {
    /// A file that can be run.
    Executable,
    /// A file that is loaded, not run.
    Library,
}

/// One `[[provides]]` table, as the manifest wrote it, for the host to read.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry(pub(super) toml::Table);

impl Entry {
    /// Reads the entry as the host's own raw shape. A table that is not of that shape is a
    /// [`Refusal::Syntax`].
    pub fn read<T: DeserializeOwned, E>(self) -> Result<T, Refusal<E>> {
        self.0
            .try_into()
            .map_err(|error: toml::de::Error| Refusal::Syntax {
                reason: error.message().to_owned(),
            })
    }
}

/// Why a host will not take a `[[provides]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal<E> {
    /// The entry is not of the host's shape (a capability or a key that does not exist).
    Syntax {
        /// The TOML parser's description.
        reason: String,
    },
    /// A path the entry names must be absolute and is not.
    PathNotAbsolute {
        /// The path as written.
        path: PathBuf,
    },
    /// The entry is well formed and the host will not have it, for a reason of its own.
    Fault(E),
}

/// `path` if it is absolute, else a [`Refusal::PathNotAbsolute`].
pub fn absolute<E>(path: PathBuf) -> Result<PathBuf, Refusal<E>> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(Refusal::PathNotAbsolute { path })
    }
}

/// One `[[provides]]` entry of a manifest, parsed into the host's own type. A host implements it
/// for one type (usually an enum with a variant for each capability), and bayonet's [`Manifest`]
/// and [`Registry`] are generic over it.
///
/// [`Manifest`]: super::Manifest
/// [`Registry`]: crate::registry::Registry
pub trait Provides: Sized + fmt::Debug + Clone + Eq {
    /// The host's capability type.
    type Capability: Capability;
    /// The host's reasons to refuse an entry that is well formed but wrong: a capability with
    /// nothing to handle, a key that does not belong to it.
    type Fault: std::error::Error + Clone + Eq + 'static;

    /// Parses one entry. The `capability` key is the host's to read, with every key beside it.
    fn parse(entry: Entry) -> Result<Self, Refusal<Self::Fault>>;

    /// Which capability this entry provides.
    fn capability(&self) -> Self::Capability;

    /// Whether the plugin's `program` speaks this capability over the wire. A capability that
    /// only names files (a player the host starts itself) answers no.
    fn needs_program(&self) -> bool {
        true
    }

    /// Every path the entry names that must be there before the plugin is usable.
    fn paths(&self) -> Vec<(&Path, PathRole)> {
        Vec::new()
    }
}

impl<E: std::error::Error> Refusal<E> {
    /// The manifest error this refusal is.
    pub(super) fn into_error<C: fmt::Debug>(self) -> ManifestError<C, E> {
        match self {
            Refusal::Syntax { reason } => ManifestError::Syntax { reason },
            Refusal::PathNotAbsolute { path } => ManifestError::PathNotAbsolute { path },
            Refusal::Fault(error) => ManifestError::Provision(error),
        }
    }
}
