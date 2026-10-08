//! Reading the envelope: the fields every manifest has, with each `[[provides]]` table handed to
//! the host.

use super::error::ManifestError;
use super::id::PluginId;
use super::model::{Manifest, Program};
use super::provides::{Entry, Provides, absolute};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    protocol: u32,
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

impl<P: Provides> Manifest<P> {
    /// Parses and checks the text of a manifest. The shared fields are checked here; each
    /// `[[provides]]` table goes to [`Provides::parse`].
    pub fn parse(text: &str) -> Result<Manifest<P>, ManifestError<P::Capability, P::Fault>> {
        let raw: RawManifest = toml::from_str(text).map_err(|error| ManifestError::Syntax {
            reason: error.message().to_owned(),
        })?;
        let id = PluginId::parse(&raw.id)
            .map_err(|invalid| ManifestError::IdInvalid { id: invalid.id })?;
        if raw.name.trim().is_empty() {
            return Err(ManifestError::NameEmpty);
        }
        if raw.protocol == 0 {
            return Err(ManifestError::ProtocolZero);
        }
        let program = raw.program.map(program::<P>).transpose()?;
        let provides = provisions::<P>(raw.provides)?;
        if program.is_none()
            && let Some(needing) = provides.iter().find(|provision| provision.needs_program())
        {
            return Err(ManifestError::ProgramMissing {
                capability: needing.capability(),
            });
        }
        Ok(Manifest {
            id,
            name: raw.name,
            protocol: raw.protocol,
            program,
            provides,
        })
    }
}

fn program<P: Provides>(raw: RawProgram) -> Result<Program, Failure<P>> {
    Ok(Program {
        path: absolute(raw.path).map_err(|refusal| refusal.into_error())?,
        args: raw.args,
    })
}

fn provisions<P: Provides>(raw: Vec<toml::Table>) -> Result<Vec<P>, Failure<P>> {
    if raw.is_empty() {
        return Err(ManifestError::NothingProvided);
    }
    let mut provides: Vec<P> = Vec::with_capacity(raw.len());
    for table in raw {
        let provision = P::parse(Entry(table)).map_err(|refusal| refusal.into_error())?;
        let capability = provision.capability();
        if provides.iter().any(|seen| seen.capability() == capability) {
            return Err(ManifestError::CapabilityRepeated { capability });
        }
        provides.push(provision);
    }
    Ok(provides)
}
