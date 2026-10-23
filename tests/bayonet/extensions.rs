//! The folder layout, the envelope's `version`, and the two-step read of raw entries, against
//! scratch directories.

use bayonet::manifest::{Entry, Manifest, ManifestError, PathRole, Provides, Raw, Refusal};
use bayonet::registry::{Origin, Registry};
use bayonet::{Discovery, Layout, Search, discover, reparse, scan};
use serde::Deserialize;
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

/// The one capability word the test owner has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Word {
    FileIcons,
}

/// An entry of the test owner: a file the Extension carries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Icons {
    file: PathBuf,
}

#[derive(Deserialize)]
struct Fields {
    #[allow(dead_code)]
    capability: Word,
    file: PathBuf,
}

impl Provides for Icons {
    type Capability = Word;
    type Fault = Infallible;

    fn parse(entry: Entry) -> Result<Icons, Refusal<Infallible>> {
        let fields: Fields = entry.clone().read()?;
        Ok(Icons {
            file: entry.resolve(fields.file)?,
        })
    }

    fn capability(&self) -> Word {
        Word::FileIcons
    }

    fn needs_program(&self) -> bool {
        false
    }

    fn owns(capability: &str) -> bool {
        capability == "file_icons"
    }

    fn paths(&self) -> Vec<(&Path, PathRole)> {
        vec![(self.file.as_path(), PathRole::Library)]
    }
}

const ICONS: &str = "[[provides]]\ncapability = \"file_icons\"\nfile = \"paper.png\"\n";

/// The text of an Extension manifest.
fn text(id: &str, extra: &str, entries: &str) -> String {
    format!("id = \"{id}\"\nname = \"{id}\"\nprotocol = 1\n{extra}\n{entries}")
}

/// A person's directory and system directories, all scratch.
struct Scratch {
    root: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Scratch {
        Scratch {
            root: tempfile::tempdir().unwrap(),
        }
    }

    fn user(&self) -> PathBuf {
        self.root.path().join("user")
    }

    fn system(&self, n: usize) -> PathBuf {
        self.root.path().join(format!("system{n}"))
    }

    /// Writes `<data>/quire/extensions/<folder>/extension.toml`, and returns the folder.
    fn extension(&self, data: PathBuf, folder: &str, manifest: &str) -> PathBuf {
        let dir = data.join("quire").join("extensions").join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("extension.toml"), manifest).unwrap();
        dir
    }

    /// A folder-layout search over the person's directory and these system directories.
    fn folders(&self, system: &[PathBuf]) -> Discovery<Icons> {
        discover::<Icons>(&search(&self.user(), system))
    }
}

fn search<'a>(user: &'a Path, system: &'a [PathBuf]) -> Search<'a> {
    Search::new("quire", user, system, 1).with_layout(Layout::Folders {
        dir: "extensions",
        manifest: "extension.toml",
    })
}

fn version_of(manifest: &Manifest<Icons>) -> Option<String> {
    manifest.version.as_ref().map(ToString::to_string)
}

fn installed(found: &Discovery<Icons>) -> Vec<(String, Origin)> {
    found
        .registry
        .installed()
        .iter()
        .map(|p| (p.manifest.id.as_str().to_owned(), p.origin))
        .collect()
}

#[test]
fn extensions_are_found_in_folders_named_for_their_id() {
    let s = Scratch::new();
    let folder = s.extension(
        s.user(),
        "org.example.Paper",
        &text(
            "org.example.Paper",
            "version = \"1.2.0\"\nlicence = \"MIT OR Apache-2.0\"\nsummary = \"Soft paper\"",
            ICONS,
        ),
    );
    fs::write(folder.join("paper.png"), "png").unwrap();
    let id = "org.example.Slate";
    let slate = s.extension(s.system(0), id, &text(id, "", ICONS));
    fs::write(slate.join("paper.png"), "png").unwrap();
    // A folder with no manifest is not an Extension; a folder named for another id is rejected.
    fs::create_dir_all(s.user().join("quire/extensions/notes")).unwrap();
    let wrong = s.extension(
        s.user(),
        "org.example.Other",
        &text("org.example.Nope", "", ICONS),
    );

    let found = s.folders(&[s.system(0)]);

    assert_eq!(
        installed(&found),
        [
            ("org.example.Paper".to_owned(), Origin::User),
            ("org.example.Slate".to_owned(), Origin::System),
        ]
    );
    let paper = &found.registry.installed()[0].manifest;
    assert_eq!(version_of(paper).as_deref(), Some("1.2.0"));
    assert_eq!(paper.licence.as_deref(), Some("MIT OR Apache-2.0"));
    assert_eq!(paper.summary.as_deref(), Some("Soft paper"));
    assert_eq!(paper.provides[0].file, folder.join("paper.png"));
    let slate = &found.registry.installed()[1].manifest;
    assert!(slate.version.is_none() && slate.licence.is_none() && slate.summary.is_none());
    let rejected: Vec<_> = found.rejected.iter().map(|r| r.file.clone()).collect();
    assert_eq!(rejected, [wrong.join("extension.toml")]);
    assert!(matches!(
        found.rejected[0].error,
        ManifestError::IdFolderMismatch { .. }
    ));
}

#[test]
fn a_path_that_leaves_the_folder_is_a_manifest_error() {
    let s = Scratch::new();
    let entry =
        |file: &str| format!("[[provides]]\ncapability = \"file_icons\"\nfile = \"{file}\"\n");
    // name, the path as written
    let cases: &[(&str, &str)] = &[
        ("climbing", "../outside.png"),
        ("climbing after a step", "icons/../../outside.png"),
        ("absolute", "/etc/hosts"),
    ];
    for (n, (name, file)) in cases.iter().enumerate() {
        let id = format!("org.example.Case{n}");
        s.extension(s.user(), &id, &text(&id, "", &entry(file)));
        let found = s.folders(&[]);
        let error = &found.rejected[n].error;
        assert!(
            matches!(error, ManifestError::PathEscapes { path } if path == Path::new(file)),
            "{name}: {error:?}"
        );
    }
    assert!(s.folders(&[]).registry.installed().is_empty());

    // The program of a raw scan may not climb either.
    let id = "org.example.Prog";
    let program = "[program]\npath = \"../bin/run\"\n";
    s.extension(s.system(0), id, &text(id, program, ICONS));
    let user = s.user();
    let system = [s.system(0)];
    let raw = scan::<Raw>(&search(&user, &system));
    assert!(matches!(
        raw.rejected[0].error,
        ManifestError::PathEscapes { .. }
    ));
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_folder_sets_the_extension_aside() {
    let s = Scratch::new();
    let outside = s.root.path().join("outside.png");
    fs::write(&outside, "png").unwrap();
    let id = "org.example.Link";
    let folder = s.extension(s.user(), id, &text(id, "", ICONS));
    std::os::unix::fs::symlink(&outside, folder.join("paper.png")).unwrap();

    let found = s.folders(&[]);

    assert!(found.registry.installed().is_empty());
    let unusable = found.registry.unusable();
    assert_eq!(unusable.len(), 1);
    let ManifestError::PathEscapes { path } = &unusable[0].reason else {
        panic!("wanted PathEscapes: {:?}", unusable[0].reason);
    };
    assert_eq!(
        path.canonicalize().unwrap(),
        outside.canonicalize().unwrap()
    );
}

#[test]
fn of_one_id_the_person_wins_then_the_newer_version() {
    let s = Scratch::new();
    let id = "org.example.Paper";
    let at = |data: PathBuf, version: &str| {
        let extra = if version.is_empty() {
            String::new()
        } else {
            format!("version = \"{version}\"")
        };
        let folder = s.extension(data, id, &text(id, &extra, ICONS));
        fs::write(folder.join("paper.png"), "png").unwrap();
        folder
    };
    at(s.system(0), "1.9");
    let new = at(s.system(1), "1.10");
    at(s.system(2), "");

    let order = |first: usize, second: usize, third: usize| {
        [s.system(first), s.system(second), s.system(third)]
    };
    // name, the system directories in search order
    let cases = [
        ("1.10 is newer than 1.9", order(0, 1, 2)),
        (
            "the order of the directories does not matter",
            order(2, 1, 0),
        ),
    ];
    for (name, system) in cases {
        let found = s.folders(&system);
        let kept = &found.registry.installed()[0].manifest;
        assert_eq!(kept.provides[0].file, new.join("paper.png"), "{name}");
        assert_eq!(version_of(kept).as_deref(), Some("1.10"), "{name}");
    }

    // The person's copy wins over a newer system one.
    let mine = at(s.user(), "1.0");
    let found = s.folders(&[s.system(0), s.system(1)]);
    assert_eq!(
        found.registry.installed()[0].manifest.provides[0].file,
        mine.join("paper.png")
    );
}

#[test]
fn raw_entries_become_the_owners_type_and_other_words_are_skipped() {
    let s = Scratch::new();
    let two = format!("{ICONS}[[provides]]\ncapability = \"look\"\nsheet = \"look.css\"\n");
    let id = "org.example.Both";
    let both = s.extension(s.user(), id, &text(id, "", &two));
    fs::write(both.join("paper.png"), "png").unwrap();
    let id = "org.example.LookOnly";
    s.extension(
        s.user(),
        id,
        &text(id, "", "[[provides]]\ncapability = \"look\"\n"),
    );
    let id = "org.example.Missing";
    s.extension(s.user(), id, &text(id, "", ICONS));
    let id = "org.example.Bad";
    s.extension(
        s.user(),
        id,
        &text(id, "", "[[provides]]\ncapability = \"file_icons\"\n"),
    );

    let user = s.user();
    let raw = scan::<Raw>(&search(&user, &[]));
    assert!(raw.rejected.is_empty());
    let words: Vec<(String, Vec<String>)> = raw
        .candidates
        .iter()
        .map(|c| {
            let words = c.manifest.provides.iter().map(|r| r.word().to_owned());
            (c.manifest.id.as_str().to_owned(), words.collect())
        })
        .collect();
    assert_eq!(words[1].1, ["file_icons", "look"], "{words:?}");

    let mut typed = Vec::new();
    let mut results = Vec::new();
    for candidate in raw.candidates {
        let id = candidate.manifest.id.as_str().to_owned();
        results.push((id, reparse::<Icons>(candidate)));
    }
    // org.example.Bad, Both, LookOnly, Missing: by name.
    let [bad, both_result, look_only, missing] = &results[..] else {
        panic!("four extensions: {results:?}");
    };
    assert!(
        matches!(bad.1, Err(ManifestError::Syntax { .. })),
        "an owned, malformed entry is an error"
    );
    let done = both_result.1.as_ref().unwrap();
    let skipped: Vec<_> = done.skipped.iter().map(|s| s.capability.as_str()).collect();
    assert_eq!(skipped, ["look"]);
    let candidate = done.candidate.clone().unwrap();
    assert_eq!(candidate.manifest.provides[0].file, both.join("paper.png"));
    let none = look_only.1.as_ref().unwrap();
    assert!(none.candidate.is_none());
    assert_eq!(none.skipped.len(), 1);

    // The checks the raw scan could not make run on the owner's entries: the file is missing.
    typed.push(candidate);
    typed.push(missing.1.as_ref().unwrap().candidate.clone().unwrap());
    let registry = Registry::resolve(typed, 1);
    let ids: Vec<_> = registry
        .installed()
        .iter()
        .map(|p| p.manifest.id.as_str())
        .collect();
    assert_eq!(ids, ["org.example.Both"]);
    assert!(matches!(
        registry.unusable()[0].reason,
        ManifestError::FileMissing { .. }
    ));
}

#[test]
fn the_flat_layout_is_unchanged() {
    let s = Scratch::new();
    let file = s.root.path().join("paper.png");
    fs::write(&file, "png").unwrap();
    let flat = s.user().join("host").join("plugins");
    fs::create_dir_all(&flat).unwrap();
    let entry = |path: &Path| {
        format!(
            "[[provides]]\ncapability = \"file_icons\"\nfile = \"{}\"\n",
            path.display()
        )
    };
    fs::write(
        flat.join("tool.toml"),
        text("tool", "version = \"2\"", &entry(&file)),
    )
    .unwrap();
    fs::write(
        flat.join("rel.toml"),
        text("rel", "", &entry(Path::new("paper.png"))),
    )
    .unwrap();
    fs::write(
        flat.join("Dotted.Id.toml"),
        text("Dotted.Id", "", &entry(&file)),
    )
    .unwrap();
    // A folder-layout Extension beside it is not read by a flat search.
    s.extension(
        s.user(),
        "org.example.Paper",
        &text("org.example.Paper", "", ICONS),
    );
    let user = s.user();

    let found = discover::<Icons>(&Search::new("host", &user, &[], 1));

    let ids: Vec<_> = found
        .registry
        .installed()
        .iter()
        .map(|p| p.manifest.id.as_str())
        .collect();
    assert_eq!(ids, ["tool"]);
    let errors: Vec<_> = found.rejected.iter().map(|r| &r.error).collect();
    assert!(
        matches!(errors[0], ManifestError::IdInvalid { .. }),
        "{errors:?}"
    );
    assert!(
        matches!(errors[1], ManifestError::PathNotAbsolute { .. }),
        "{errors:?}"
    );
    assert_eq!(errors.len(), 2);
}
