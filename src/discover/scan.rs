//! Listing the manifests of one data directory and reading each into a candidate.

use super::ready::readiness;
use super::search::Layout;
use crate::manifest::{Manifest, ManifestError, Provides};
use crate::registry::{Candidate, Origin};
use std::fs;
use std::path::{Path, PathBuf};

/// The folder under a data directory that holds the flat layout's manifests.
const FLAT_FOLDER: &str = "plugins";

type Failure<P> = ManifestError<<P as Provides>::Capability, <P as Provides>::Fault>;

/// A manifest file, and the folder it is the manifest of in the folder layout.
pub(super) struct Source {
    pub(super) file: PathBuf,
    folder: Option<PathBuf>,
}

/// The manifests of the host `app` under the data directory `data`, in the order of their names;
/// none when the directory cannot be listed.
pub(super) fn sources(data: &Path, app: &str, layout: Layout<'_>) -> Vec<Source> {
    let base = data.join(app);
    match layout {
        Layout::Flat => flat(&base.join(FLAT_FOLDER)),
        Layout::Folders { dir, manifest } => folders(&base.join(dir), manifest),
    }
}

/// The `.toml` files directly inside `folder`, by name.
fn flat(folder: &Path) -> Vec<Source> {
    let mut files: Vec<PathBuf> = children(folder)
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml") && path.is_file())
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|file| Source { file, folder: None })
        .collect()
}

/// The folders directly inside `parent` that hold a file called `manifest`, by name.
fn folders(parent: &Path, manifest: &str) -> Vec<Source> {
    let mut dirs: Vec<PathBuf> = children(parent)
        .into_iter()
        .filter(|path| path.join(manifest).is_file())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| Source {
            file: dir.join(manifest),
            folder: Some(dir),
        })
        .collect()
}

fn children(folder: &Path) -> Vec<PathBuf> {
    fs::read_dir(folder)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads and checks the manifest of `source`.
pub(super) fn read_candidate<P: Provides>(
    source: &Source,
    origin: Origin,
) -> Result<Candidate<P>, Failure<P>> {
    let text = fs::read_to_string(&source.file)
        .map_err(|error| ManifestError::Unreadable { kind: error.kind() })?;
    let folder = source.folder.as_deref();
    let manifest = match folder {
        None => Manifest::<P>::parse(&text)?,
        Some(folder) => Manifest::<P>::parse_in_folder(&text, folder)?,
    };
    match folder {
        None => manifest.check_file_name(&source.file)?,
        Some(folder) => manifest.check_folder_name(folder)?,
    }
    let readiness = readiness(&manifest, folder);
    Ok(Candidate {
        manifest,
        origin,
        readiness,
    })
}
