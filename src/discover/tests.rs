use super::*;
use crate::fixture::{Tool, manifest_text, probe};
use crate::manifest::ManifestError;
use crate::registry::Origin;
use std::fs;
use std::path::{Path, PathBuf};

/// A user directory and one system directory, both scratch.
struct Dirs {
    root: tempfile::TempDir,
}

impl Dirs {
    fn new() -> Dirs {
        Dirs {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn user(&self) -> PathBuf {
        self.root.path().join("user")
    }

    fn system(&self) -> Vec<PathBuf> {
        vec![self.root.path().join("system")]
    }

    fn write(&self, base: PathBuf, file: &str, text: &str) -> PathBuf {
        let folder = base.join("host").join("plugins");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(file);
        fs::write(&path, text).unwrap();
        path
    }

    fn discover(&self) -> Discovery<Tool> {
        let user = self.user();
        let system = self.system();
        discover::<Tool>(&Search {
            app: "host",
            user: &user,
            system: &system,
            protocol: 1,
        })
    }
}

/// A program that runs, in the scratch root.
fn program(dirs: &Dirs, name: &str, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dirs.root.path().join(name);
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn manifest_for(id: &str, program: &Path) -> String {
    manifest_text(id, 1, &probe("\"video\""))
        .replace(&format!("/bin/{id}"), &program.display().to_string())
}

#[test]
fn manifests_are_found_under_the_apps_folder_in_both_directories() {
    let dirs = Dirs::new();
    let run = program(&dirs, "run", 0o755);
    dirs.write(dirs.user(), "mine.toml", &manifest_for("mine", &run));
    dirs.write(
        dirs.system()[0].clone(),
        "theirs.toml",
        &manifest_for("theirs", &run),
    );
    // Another app's folder is not read.
    let other = dirs.user().join("other").join("plugins");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("x.toml"), manifest_for("x", &run)).unwrap();
    let found = dirs.discover();
    let ids: Vec<_> = found
        .registry
        .installed()
        .iter()
        .map(|plugin| (plugin.manifest.id.as_str().to_owned(), plugin.origin))
        .collect();
    assert_eq!(
        ids,
        [
            ("mine".to_owned(), Origin::User),
            ("theirs".to_owned(), Origin::System)
        ]
    );
    assert!(found.rejected.is_empty());
}

#[test]
fn malformed_and_misnamed_manifests_are_rejected_without_hiding_the_good_ones() {
    let dirs = Dirs::new();
    let run = program(&dirs, "run", 0o755);
    dirs.write(dirs.user(), "good.toml", &manifest_for("good", &run));
    let bad = dirs.write(dirs.user(), "bad.toml", "id = \n");
    let misnamed = dirs.write(dirs.user(), "other.toml", &manifest_for("not-other", &run));
    dirs.write(dirs.user(), "notes.txt", "not a manifest, not read");
    let found = dirs.discover();
    assert_eq!(found.registry.installed().len(), 1);
    let rejected: Vec<_> = found
        .rejected
        .iter()
        .map(|r| (r.file.clone(), format!("{:?}", r.error)))
        .collect();
    assert_eq!(rejected.len(), 2);
    assert_eq!(rejected[0].0, bad);
    assert!(rejected[0].1.starts_with("Syntax"), "{}", rejected[0].1);
    assert_eq!(rejected[1].0, misnamed);
    assert!(
        rejected[1].1.starts_with("IdFileMismatch"),
        "{}",
        rejected[1].1
    );
}

#[test]
fn a_program_that_is_missing_or_not_executable_sets_the_plugin_aside() {
    let dirs = Dirs::new();
    let plain = program(&dirs, "plain", 0o644);
    let gone = dirs.root.path().join("gone");
    dirs.write(dirs.user(), "plain.toml", &manifest_for("plain", &plain));
    dirs.write(dirs.user(), "gone.toml", &manifest_for("gone", &gone));
    let found = dirs.discover();
    assert!(found.registry.installed().is_empty());
    let reasons: Vec<_> = found
        .registry
        .unusable()
        .iter()
        .map(|unusable| (unusable.id.as_str().to_owned(), unusable.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        [
            ("gone".to_owned(), ManifestError::FileMissing { path: gone }),
            (
                "plain".to_owned(),
                ManifestError::NotExecutable { path: plain }
            ),
        ]
    );
}

#[test]
fn a_missing_data_directory_finds_nothing_and_does_not_fail() {
    let dirs = Dirs::new();
    let found = dirs.discover();
    assert!(found.registry.installed().is_empty());
    assert!(found.rejected.is_empty());
}
