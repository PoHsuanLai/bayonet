//! The second step of a two-step read: raw entries to an owner's own type.

use super::ready::readiness;
use crate::manifest::{Manifest, ManifestError, Provides, Raw, check_program, typed_entries};
use crate::registry::Candidate;
use std::path::{Path, PathBuf};

type Failure<P> = ManifestError<<P as Provides>::Capability, <P as Provides>::Fault>;

/// An entry another owner's capability word names, which this owner left alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The `capability` word, as written.
    pub capability: String,
}

/// A candidate read for one owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reparsed<P: Provides> {
    /// The manifest with only this owner's entries, its programs and paths checked again; none
    /// when the manifest has no entry of this owner's.
    pub candidate: Option<Candidate<P>>,
    /// The entries set aside because the owner does not own their word.
    pub skipped: Vec<Skipped>,
}

/// Turns the entries of a scan that knew no vocabulary into the owner's own type `P`. Entries
/// whose word `P` does not own ([`Provides::owns`]) are reported in `skipped`, never as an error:
/// they belong to someone else. Entries it owns go to [`Provides::parse`], and the checks the
/// scan could not make run now: a capability listed twice, a wire capability with no program,
/// and the programs and paths of the owner's entries (missing, not executable, outside the
/// folder). The result is a [`Candidate<P>`] for
/// [`Registry::resolve`](crate::registry::Registry::resolve), which runs after the owner's own
/// checks.
pub fn reparse<P: Provides>(candidate: Candidate<Raw>) -> Result<Reparsed<P>, Failure<P>> {
    let Candidate {
        manifest, origin, ..
    } = candidate;
    let folder: Option<PathBuf> = manifest
        .provides
        .first()
        .and_then(Raw::folder)
        .map(Path::to_path_buf);
    let mut mine = Vec::new();
    let mut skipped = Vec::new();
    for raw in manifest.provides {
        if P::owns(raw.word()) {
            mine.push(raw.into_entry());
        } else {
            skipped.push(Skipped {
                capability: raw.word().to_owned(),
            });
        }
    }
    let provides = typed_entries::<P>(mine)?;
    if provides.is_empty() {
        return Ok(Reparsed {
            candidate: None,
            skipped,
        });
    }
    check_program(manifest.program.as_ref(), &provides)?;
    let manifest = Manifest {
        id: manifest.id,
        name: manifest.name,
        protocol: manifest.protocol,
        version: manifest.version,
        licence: manifest.licence,
        summary: manifest.summary,
        program: manifest.program,
        provides,
    };
    let readiness = readiness(&manifest, folder.as_deref());
    Ok(Reparsed {
        candidate: Some(Candidate {
            manifest,
            origin,
            readiness,
        }),
        skipped,
    })
}
