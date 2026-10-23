//! Finding the plugins: the manifests in the data directories, and whether the programs they
//! name are there.

use super::scan::{read_candidate, sources};
use crate::manifest::{ManifestError, Provides};
use crate::registry::{Candidate, Origin, Registry};
use std::path::{Path, PathBuf};

/// Where in a data directory the manifests are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout<'a> {
    /// One file for each plugin, `<data dir>/<app>/plugins/<id>.toml`; the file name is the id.
    #[default]
    Flat,
    /// One folder for each plugin, `<data dir>/<app>/<dir>/<id>/<manifest>`; the folder name is
    /// the id, and the plugin's own files sit beside the manifest. A folder without the
    /// manifest file is not a plugin and is not listed anywhere.
    Folders {
        /// The folder of plugin folders under the app's (`extensions`).
        dir: &'a str,
        /// The manifest's file name in each plugin folder (`extension.toml`).
        manifest: &'a str,
    },
}

/// Where to look, and for whom. Build it with [`Search::new`]; fields may be added without
/// breaking a caller.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Search<'a> {
    /// The host's name: manifests are in `<data dir>/<app>`, in the places `layout` says.
    pub app: &'a str,
    /// The person's data directory (`$XDG_DATA_HOME`).
    pub user: &'a Path,
    /// The system's data directories (`$XDG_DATA_DIRS`), most important first. The host reads
    /// the environment and passes the result in: bayonet reads none.
    pub system: &'a [PathBuf],
    /// The newest protocol version the host speaks.
    pub protocol: u32,
    /// Where the manifests are under the app's folder.
    pub layout: Layout<'a>,
}

impl<'a> Search<'a> {
    /// A search for the host called `app`, in the person's directory `user` and the system's
    /// `system`, for plugins that speak up to `protocol`, in the flat layout
    /// (`<app>/plugins/*.toml`).
    pub fn new(app: &'a str, user: &'a Path, system: &'a [PathBuf], protocol: u32) -> Search<'a> {
        Search {
            app,
            user,
            system,
            protocol,
            layout: Layout::Flat,
        }
    }

    /// The same search in another layout.
    pub fn with_layout(self, layout: Layout<'a>) -> Search<'a> {
        Search { layout, ..self }
    }
}

/// What was found: the registry, and each file that could not become a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery<P: Provides> {
    /// The plugins that can be used, and the ones that were found and cannot be.
    pub registry: Registry<P>,
    /// Manifest files that were unreadable, malformed or misnamed.
    pub rejected: Vec<Rejected<P>>,
}

/// What a scan found, before anything is ranked: every candidate of every id, in the order of
/// the directories and then of the names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan<P: Provides> {
    /// Each manifest that was read, with where it was found. Two copies of one id are both here.
    pub candidates: Vec<Candidate<P>>,
    /// Manifest files that were unreadable, malformed or misnamed.
    pub rejected: Vec<Rejected<P>>,
}

/// A manifest file that is not a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rejected<P: Provides> {
    /// The file.
    pub file: PathBuf,
    /// Why not.
    pub error: ManifestError<P::Capability, P::Fault>,
}

impl<P: Provides> Rejected<P> {
    /// The manifest `file`, which is not a plugin because of `error`.
    pub fn new(file: PathBuf, error: ManifestError<P::Capability, P::Fault>) -> Rejected<P> {
        Rejected { file, error }
    }
}

/// Reads every manifest the search's layout names, in the person's directory and in each system
/// directory, and ranks them. A file that is not a usable manifest is listed in `rejected` and
/// skipped; nothing here fails.
pub fn discover<P: Provides>(search: &Search<'_>) -> Discovery<P> {
    let Scan {
        candidates,
        rejected,
    } = scan::<P>(search);
    Discovery {
        registry: Registry::resolve(candidates, search.protocol),
        rejected,
    }
}

/// Reads every manifest as [`discover`] does and stops there: no candidate is set aside or
/// ranked, so a host can make its own checks first and call [`Registry::resolve`] after.
/// Read with [`Raw`](crate::manifest::Raw) as `P` when the host owns no vocabulary of its own
/// and hands the entries on to [`reparse`](crate::reparse).
pub fn scan<P: Provides>(search: &Search<'_>) -> Scan<P> {
    let dirs = std::iter::once((Origin::User, search.user)).chain(
        search
            .system
            .iter()
            .map(|dir| (Origin::System, dir.as_path())),
    );
    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for (origin, dir) in dirs {
        for source in sources(dir, search.app, search.layout) {
            match read_candidate(&source, origin) {
                Ok(candidate) => candidates.push(candidate),
                Err(error) => rejected.push(Rejected::new(source.file, error)),
            }
        }
    }
    Scan {
        candidates,
        rejected,
    }
}
