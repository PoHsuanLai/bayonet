//! A small host for the unit tests: three capabilities, one of which names a library instead of
//! speaking the wire.

use crate::manifest::{Entry, PathRole, Provides, Refusal, absolute};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The test host's capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Cap {
    Probe,
    Export,
    Play,
}

/// The test host's reasons to refuse an entry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ToolFault {
    #[error("handles nothing")]
    HandlesNothing(Cap),
}

/// One `[[provides]]` entry of the test host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tool {
    pub(crate) capability: Cap,
    pub(crate) kinds: Vec<String>,
    pub(crate) library: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Raw {
    capability: Cap,
    #[serde(default)]
    kinds: Vec<String>,
    library: Option<PathBuf>,
}

impl Provides for Tool {
    type Capability = Cap;
    type Fault = ToolFault;

    fn parse(entry: Entry) -> Result<Tool, Refusal<ToolFault>> {
        let raw: Raw = entry.read()?;
        if raw.kinds.is_empty() {
            return Err(Refusal::Fault(ToolFault::HandlesNothing(raw.capability)));
        }
        Ok(Tool {
            capability: raw.capability,
            kinds: raw.kinds,
            library: raw.library.map(absolute).transpose()?,
        })
    }

    fn capability(&self) -> Cap {
        self.capability
    }

    fn needs_program(&self) -> bool {
        self.capability != Cap::Play
    }

    fn paths(&self) -> Vec<(&Path, PathRole)> {
        self.library
            .iter()
            .map(|path| (path.as_path(), PathRole::Library))
            .collect()
    }
}

/// The text of a manifest for `id` with a program, speaking `protocol`, and these entries.
pub(crate) fn manifest_text(id: &str, protocol: u32, entries: &str) -> String {
    format!(
        "id = \"{id}\"\nname = \"{id}\"\nprotocol = {protocol}\n[program]\npath = \"/bin/{id}\"\n{entries}"
    )
}

/// One probe entry for these kinds.
pub(crate) fn probe(kinds: &str) -> String {
    format!("[[provides]]\ncapability = \"probe\"\nkinds = [{kinds}]\n")
}
