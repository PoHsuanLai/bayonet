//! Finding the plugins: the manifests in the data directories, and whether the programs they
//! name are there.

use crate::manifest::{Manifest, ManifestError, PathRole, Provides};
use crate::registry::{Candidate, Origin, Readiness, Registry};
use std::fs;
use std::path::{Path, PathBuf};

/// Where to look, and for whom. Build it with [`Search::new`]; fields may be added without
/// breaking a caller.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct Search<'a> {
    /// The host's name: manifests are in `<data dir>/<app>/plugins`.
    pub app: &'a str,
    /// The person's data directory (`$XDG_DATA_HOME`).
    pub user: &'a Path,
    /// The system's data directories (`$XDG_DATA_DIRS`), most important first. The host reads
    /// the environment and passes the result in: bayonet reads none.
    pub system: &'a [PathBuf],
    /// The newest protocol version the host speaks.
    pub protocol: u32,
}

impl<'a> Search<'a> {
    /// A search for the host called `app`, in the person's directory `user` and the system's
    /// `system`, for plugins that speak up to `protocol`.
    pub fn new(app: &'a str, user: &'a Path, system: &'a [PathBuf], protocol: u32) -> Search<'a> {
        Search {
            app,
            user,
            system,
            protocol,
        }
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

/// The folder under a data directory that holds the manifests.
const FOLDER: &str = "plugins";

/// Reads every manifest in the person's `<user>/<app>/plugins` and in each system directory's.
/// A file that is not a usable manifest is listed in `rejected` and skipped; nothing here fails.
pub fn discover<P: Provides>(search: &Search<'_>) -> Discovery<P> {
    let dirs = std::iter::once((Origin::User, search.user)).chain(
        search
            .system
            .iter()
            .map(|dir| (Origin::System, dir.as_path())),
    );
    let mut candidates = Vec::new();
    let mut rejected = Vec::new();
    for (origin, dir) in dirs {
        for file in manifest_files(&dir.join(search.app).join(FOLDER)) {
            match read_candidate(&file, origin) {
                Ok(candidate) => candidates.push(candidate),
                Err(error) => rejected.push(Rejected::new(file, error)),
            }
        }
    }
    Discovery {
        registry: Registry::resolve(candidates, search.protocol),
        rejected,
    }
}

/// The `.toml` files directly inside `folder`, by name; none when it cannot be listed.
fn manifest_files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml") && path.is_file())
        .collect();
    files.sort();
    files
}

type Failure<P> = ManifestError<<P as Provides>::Capability, <P as Provides>::Fault>;

fn read_candidate<P: Provides>(file: &Path, origin: Origin) -> Result<Candidate<P>, Failure<P>> {
    let text = fs::read_to_string(file)
        .map_err(|error| ManifestError::Unreadable { kind: error.kind() })?;
    let manifest = Manifest::<P>::parse(&text)?;
    manifest.check_file_name(file)?;
    let readiness = readiness(&manifest);
    Ok(Candidate {
        manifest,
        origin,
        readiness,
    })
}

/// The first path the manifest names that is missing or cannot be run, or `Ready`.
fn readiness<P: Provides>(manifest: &Manifest<P>) -> Readiness<P> {
    let problem = manifest
        .paths()
        .into_iter()
        .find_map(|(path, role)| check::<P>(path, role).err());
    match problem {
        Some(error) => Readiness::Unready(error),
        None => Readiness::Ready,
    }
}

fn check<P: Provides>(path: &Path, role: PathRole) -> Result<(), Failure<P>> {
    let meta = fs::metadata(path).map_err(|_| ManifestError::FileMissing {
        path: path.to_path_buf(),
    })?;
    let runnable = meta.is_file() && is_executable(&meta);
    match role {
        PathRole::Executable if !runnable => Err(ManifestError::NotExecutable {
            path: path.to_path_buf(),
        }),
        PathRole::Library if !meta.is_file() => Err(ManifestError::FileMissing {
            path: path.to_path_buf(),
        }),
        PathRole::Executable | PathRole::Library => Ok(()),
    }
}

/// Whether the permission bits let anyone run the file. Where there are none to read, a file is
/// taken to be runnable and starting it says otherwise.
#[cfg(unix)]
fn is_executable(meta: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_meta: &fs::Metadata) -> bool {
    true
}
