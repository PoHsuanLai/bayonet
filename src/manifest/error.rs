//! Why a manifest, or the program it names, cannot be used.

use std::fmt;
use std::path::PathBuf;

/// Why a manifest, or the program it names, cannot be used. `C` is the host's capability type
/// and `E` the host's own reason for refusing a `[[provides]]` entry.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError<C: fmt::Debug, E: std::error::Error> {
    /// The file cannot be read.
    #[error("cannot read the manifest: {kind}")]
    Unreadable {
        /// What the operating system reported.
        kind: std::io::ErrorKind,
    },
    /// The text is not TOML of the manifest's shape, or a `[[provides]]` entry is not of the
    /// host's shape.
    #[error("not a manifest: {reason}")]
    Syntax {
        /// The TOML parser's description.
        reason: String,
    },
    /// The id is empty, too long, or has a character outside `a-z 0-9 - _`.
    #[error("not a plugin id: {id:?}")]
    IdInvalid {
        /// The id as written.
        id: String,
    },
    /// The file is not named `<id>.toml`.
    #[error("the manifest names the id {id:?} but the file is called {file:?}")]
    IdFileMismatch {
        /// The id inside.
        id: String,
        /// The file's stem.
        file: String,
    },
    /// The name is empty.
    #[error("the manifest has no name")]
    NameEmpty,
    /// The protocol version is zero.
    #[error("protocol versions start at 1")]
    ProtocolZero,
    /// The manifest provides nothing.
    #[error("the manifest provides no capability")]
    NothingProvided,
    /// A capability is listed twice.
    #[error("the capability {capability:?} is listed twice")]
    CapabilityRepeated {
        /// The repeated one.
        capability: C,
    },
    /// A capability that is spoken over the wire, and no `program` to speak it.
    #[error("the capability {capability:?} needs a program")]
    ProgramMissing {
        /// What needs it.
        capability: C,
    },
    /// A path that must be absolute is not.
    #[error("the path {path:?} must be absolute")]
    PathNotAbsolute {
        /// The path as written.
        path: PathBuf,
    },
    /// The plugin speaks a protocol newer than the host.
    #[error("the plugin speaks protocol {protocol}, the host speaks up to {supported}")]
    ProtocolUnsupported {
        /// The plugin's.
        protocol: u32,
        /// The host's newest.
        supported: u32,
    },
    /// A program the manifest names is not there.
    #[error("the program {path:?} does not exist")]
    FileMissing {
        /// The path as written.
        path: PathBuf,
    },
    /// A program the manifest names cannot be run.
    #[error("the program {path:?} is not an executable file")]
    NotExecutable {
        /// The path as written.
        path: PathBuf,
    },
    /// The host refused a `[[provides]]` entry for its own reason.
    #[error(transparent)]
    Provision(E),
}

/// Prints each shared variant as a derived `Debug` would and a host's own refusal as itself, so a
/// log line or a test reads `HandlesNothing { .. }` and not `Provision(HandlesNothing { .. })`.
impl<C: fmt::Debug, E: std::error::Error> fmt::Debug for ManifestError<C, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Unreadable { kind } => {
                f.debug_struct("Unreadable").field("kind", kind).finish()
            }
            ManifestError::Syntax { reason } => {
                f.debug_struct("Syntax").field("reason", reason).finish()
            }
            ManifestError::IdInvalid { id } => f.debug_struct("IdInvalid").field("id", id).finish(),
            ManifestError::IdFileMismatch { id, file } => f
                .debug_struct("IdFileMismatch")
                .field("id", id)
                .field("file", file)
                .finish(),
            ManifestError::NameEmpty => f.write_str("NameEmpty"),
            ManifestError::ProtocolZero => f.write_str("ProtocolZero"),
            ManifestError::NothingProvided => f.write_str("NothingProvided"),
            ManifestError::CapabilityRepeated { capability } => f
                .debug_struct("CapabilityRepeated")
                .field("capability", capability)
                .finish(),
            ManifestError::ProgramMissing { capability } => f
                .debug_struct("ProgramMissing")
                .field("capability", capability)
                .finish(),
            ManifestError::PathNotAbsolute { path } => f
                .debug_struct("PathNotAbsolute")
                .field("path", path)
                .finish(),
            ManifestError::ProtocolUnsupported {
                protocol,
                supported,
            } => f
                .debug_struct("ProtocolUnsupported")
                .field("protocol", protocol)
                .field("supported", supported)
                .finish(),
            ManifestError::FileMissing { path } => {
                f.debug_struct("FileMissing").field("path", path).finish()
            }
            ManifestError::NotExecutable { path } => {
                f.debug_struct("NotExecutable").field("path", path).finish()
            }
            ManifestError::Provision(error) => fmt::Debug::fmt(error, f),
        }
    }
}
