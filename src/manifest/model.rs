//! A manifest as a value.

use super::error::ManifestError;
use super::id::PluginId;
use super::provides::{PathRole, Provides};
use std::path::{Path, PathBuf};

/// The executable that speaks the wire, and the arguments it is started with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// An absolute path.
    pub path: PathBuf,
    /// Arguments placed before anything the host adds.
    pub args: Vec<String>,
}

/// A plugin as it describes itself. `P` is the host's [`Provides`] type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest<P> {
    /// Names the file and the plugin.
    pub id: PluginId,
    /// What a person calls it.
    pub name: String,
    /// The newest protocol version it speaks.
    pub protocol: u32,
    /// What speaks the wire; absent for a plugin whose capabilities need no program.
    pub program: Option<Program>,
    /// What it provides, one entry for each capability.
    pub provides: Vec<P>,
}

impl<P: Provides> Manifest<P> {
    /// Checks that the file the manifest was read from is called `<id>.toml`.
    pub fn check_file_name(
        &self,
        file: &Path,
    ) -> Result<(), ManifestError<P::Capability, P::Fault>> {
        let stem = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem == self.id.as_str() {
            Ok(())
        } else {
            Err(ManifestError::IdFileMismatch {
                id: self.id.as_str().to_owned(),
                file: stem.to_owned(),
            })
        }
    }

    /// The entry of `capability`, if the plugin has one.
    pub fn provision(&self, capability: P::Capability) -> Option<&P> {
        self.provides
            .iter()
            .find(|provision| provision.capability() == capability)
    }

    /// Every path the manifest names that must be there before the plugin is usable, with what
    /// each must be: the program first, then the paths of each entry in order.
    pub fn paths(&self) -> Vec<(&Path, PathRole)> {
        let program = self
            .program
            .iter()
            .map(|program| (program.path.as_path(), PathRole::Executable));
        program
            .chain(self.provides.iter().flat_map(Provides::paths))
            .collect()
    }
}
