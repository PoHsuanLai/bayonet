//! Reading the envelope: the fields every manifest has, with each `[[provides]]` table handed to
//! the host.

use super::confine::inside;
use super::error::ManifestError;
use super::id::{InvalidId, PluginId};
use super::model::{Manifest, Program};
use super::provides::{Entry, Provides, Refusal, absolute};
use super::version::Version;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    protocol: u32,
    version: Option<String>,
    licence: Option<String>,
    summary: Option<String>,
    program: Option<RawProgram>,
    #[serde(default)]
    provides: Vec<toml::Table>,
}

#[derive(Deserialize)]
struct RawProgram {
    path: PathBuf,
    #[serde(default)]
    args: Vec<String>,
}

/// What reading a manifest can say no with.
type Failure<P> = ManifestError<<P as Provides>::Capability, <P as Provides>::Fault>;

/// Where the manifest being read lives, which decides the id and path rules.
#[derive(Clone, Copy)]
enum Place<'a> {
    /// A file among others: a plain id, absolute paths.
    Flat,
    /// The manifest of a folder: a reverse-DNS id, paths relative to the folder.
    Folder(&'a Path),
}

impl<P: Provides> Manifest<P> {
    /// Parses and checks the text of a manifest in the flat layout (`<id>.toml`). The shared
    /// fields are checked here; each `[[provides]]` table goes to [`Provides::parse`].
    pub fn parse(text: &str) -> Result<Manifest<P>, ManifestError<P::Capability, P::Fault>> {
        read(text, Place::Flat)
    }

    /// Parses and checks the text of the manifest in `folder`. The id is reverse DNS, and paths
    /// are relative to `folder` and must stay inside it ([`Entry::resolve`] does that for a
    /// host's entries; `[program] path` may also be absolute). Checking that the folder is
    /// named for the id is [`Manifest::check_folder_name`].
    pub fn parse_in_folder(
        text: &str,
        folder: &Path,
    ) -> Result<Manifest<P>, ManifestError<P::Capability, P::Fault>> {
        read(text, Place::Folder(folder))
    }
}

fn read<P: Provides>(text: &str, place: Place<'_>) -> Result<Manifest<P>, Failure<P>> {
    let raw: RawManifest = toml::from_str(text).map_err(|error| ManifestError::Syntax {
        reason: error.message().to_owned(),
    })?;
    let id = match place {
        Place::Flat => PluginId::parse(&raw.id),
        Place::Folder(_) => PluginId::parse_dotted(&raw.id),
    }
    .map_err(|InvalidId { id }| ManifestError::IdInvalid { id })?;
    if raw.name.trim().is_empty() {
        return Err(ManifestError::NameEmpty);
    }
    if raw.protocol == 0 {
        return Err(ManifestError::ProtocolZero);
    }
    let version = raw
        .version
        .map(|text| {
            Version::parse(&text).map_err(|invalid| ManifestError::VersionInvalid {
                version: invalid.version,
            })
        })
        .transpose()?;
    let program = raw
        .program
        .map(|program| program_in::<P>(program, place))
        .transpose()?;
    let folder = match place {
        Place::Flat => None,
        Place::Folder(folder) => Some(folder),
    };
    let entries = nonempty::<P>(raw.provides)?
        .into_iter()
        .map(|table| Entry::new(table, folder.map(Path::to_path_buf)))
        .collect();
    let provides = typed_entries::<P>(entries)?;
    check_program(program.as_ref(), &provides)?;
    Ok(Manifest {
        id,
        name: raw.name,
        protocol: raw.protocol,
        version,
        licence: raw.licence,
        summary: raw.summary,
        program,
        provides,
    })
}

fn program_in<P: Provides>(raw: RawProgram, place: Place<'_>) -> Result<Program, Failure<P>> {
    let path = match place {
        Place::Flat => absolute(raw.path),
        Place::Folder(_) if raw.path.is_absolute() => Ok(raw.path),
        Place::Folder(folder) => {
            inside(folder, &raw.path).ok_or(Refusal::PathEscapes { path: raw.path })
        }
    };
    Ok(Program {
        path: path.map_err(Refusal::into_error)?,
        args: raw.args,
    })
}

fn nonempty<P: Provides>(raw: Vec<toml::Table>) -> Result<Vec<toml::Table>, Failure<P>> {
    if raw.is_empty() {
        Err(ManifestError::NothingProvided)
    } else {
        Ok(raw)
    }
}

/// Parses each entry with the host's type and refuses a capability listed twice.
pub(crate) fn typed_entries<P: Provides>(entries: Vec<Entry>) -> Result<Vec<P>, Failure<P>> {
    let mut provides: Vec<P> = Vec::with_capacity(entries.len());
    for entry in entries {
        let provision = P::parse(entry).map_err(Refusal::into_error)?;
        if provides.iter().any(|seen| seen.repeats(&provision)) {
            return Err(ManifestError::CapabilityRepeated {
                capability: provision.capability(),
            });
        }
        provides.push(provision);
    }
    Ok(provides)
}

/// Refuses entries that speak the wire when there is no program to speak it.
pub(crate) fn check_program<P: Provides>(
    program: Option<&Program>,
    provides: &[P],
) -> Result<(), Failure<P>> {
    match (program, provides.iter().find(|p| p.needs_program())) {
        (None, Some(needing)) => Err(ManifestError::ProgramMissing {
            capability: needing.capability(),
        }),
        _ => Ok(()),
    }
}
