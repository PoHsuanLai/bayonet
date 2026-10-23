//! Whether the files a manifest names are there, can be run, and stay in the plugin's folder.

use crate::manifest::{Manifest, ManifestError, PathReach, PathRole, Provides};
use crate::registry::Readiness;
use std::fs;
use std::path::Path;

type Failure<P> = ManifestError<<P as Provides>::Capability, <P as Provides>::Fault>;

/// The first path the manifest names that is missing, cannot be run or leaves its `folder`, or
/// `Ready`.
pub(super) fn readiness<P: Provides>(
    manifest: &Manifest<P>,
    folder: Option<&Path>,
) -> Readiness<P> {
    let program = manifest.program.iter().map(|program| {
        (
            program.path.as_path(),
            PathRole::Executable,
            PathReach::Anywhere,
        )
    });
    let entries = manifest.provides.iter().flat_map(|entry| {
        let reach = entry.reach();
        entry
            .paths()
            .into_iter()
            .map(move |(path, role)| (path, role, reach))
    });
    let problem = program
        .chain(entries)
        .find_map(|(path, role, reach)| check::<P>(path, role, reach, folder).err());
    match problem {
        Some(error) => Readiness::Unready(error),
        None => Readiness::Ready,
    }
}

fn check<P: Provides>(
    path: &Path,
    role: PathRole,
    reach: PathReach,
    folder: Option<&Path>,
) -> Result<(), Failure<P>> {
    if let Some(folder) = folder {
        confined::<P>(folder, path, reach)?;
    }
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

/// Refuses a path that is not in `folder`, or is but reaches out of it through a link. A path
/// that does not exist is left to the existence check.
fn confined<P: Provides>(folder: &Path, path: &Path, reach: PathReach) -> Result<(), Failure<P>> {
    if !path.starts_with(folder) {
        return match reach {
            PathReach::Anywhere => Ok(()),
            PathReach::Folder => Err(ManifestError::PathEscapes {
                path: path.to_path_buf(),
            }),
        };
    }
    match (fs::canonicalize(folder), fs::canonicalize(path)) {
        (Ok(home), Ok(real)) if !real.starts_with(&home) => {
            Err(ManifestError::PathEscapes { path: real })
        }
        _ => Ok(()),
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
