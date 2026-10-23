//! A `[[provides]]` entry read without knowing any capability word.

use super::provides::{Entry, Provides, Refusal};
use std::convert::Infallible;
use std::path::{Path, PathBuf};

/// The capability of a [`Raw`] entry. It is one value for every word, because the word is not
/// known to the type; ask [`Raw::word`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawCapability;

/// A `[[provides]]` entry kept as the TOML it was written in, with its `capability` word. A scan
/// that owns no vocabulary reads manifests as `Manifest<Raw>`; each owner then turns the entries
/// of its own words into its own [`Provides`] type with [`reparse`](crate::reparse).
#[derive(Debug, Clone)]
pub struct Raw {
    word: String,
    table: toml::Table,
    folder: Option<PathBuf>,
}

impl Raw {
    /// The `capability` word, as written.
    pub fn word(&self) -> &str {
        &self.word
    }

    /// The whole table, `capability` included.
    pub fn table(&self) -> &toml::Table {
        &self.table
    }

    /// The folder of the manifest this entry was read from, when it has one.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// The entry as an owner's [`Provides::parse`] takes it.
    pub(crate) fn into_entry(self) -> Entry {
        Entry::new(self.table, self.folder)
    }
}

/// Two entries are equal when they print as the same TOML, which keeps `Eq` honest for a table
/// that holds a float.
impl PartialEq for Raw {
    fn eq(&self, other: &Raw) -> bool {
        self.word == other.word
            && self.folder == other.folder
            && self.table.to_string() == other.table.to_string()
    }
}

impl Eq for Raw {}

impl Provides for Raw {
    type Capability = RawCapability;
    type Fault = Infallible;

    fn parse(entry: Entry) -> Result<Raw, Refusal<Infallible>> {
        let (table, folder) = entry.into_parts();
        let word = table
            .get("capability")
            .and_then(toml::Value::as_str)
            .filter(|word| !word.trim().is_empty())
            .map(str::to_owned);
        let Some(word) = word else {
            return Err(Refusal::Syntax {
                reason: "an entry needs a `capability` word".to_owned(),
            });
        };
        Ok(Raw {
            word,
            table,
            folder,
        })
    }

    fn capability(&self) -> RawCapability {
        RawCapability
    }

    fn needs_program(&self) -> bool {
        false
    }

    fn repeats(&self, other: &Raw) -> bool {
        self.word == other.word
    }
}
