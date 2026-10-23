//! Keeping the paths of a folder-layout manifest inside its folder.

use std::path::{Component, Path, PathBuf};

/// `path` placed in `folder`, or `None` when it is absolute or climbs out with `..`. Only the
/// text is read; a symbolic link out of the folder is for discovery to find.
pub(crate) fn inside(folder: &Path, path: &Path) -> Option<PathBuf> {
    let mut placed = folder.to_path_buf();
    for part in path.components() {
        match part {
            Component::Normal(name) => placed.push(name),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_stays_inside_the_folder_only_if_it_is_relative_and_never_climbs() {
        let folder = Path::new("/data/ext");
        // name, the path, where it lands
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("plain", "icons/a.png", Some("/data/ext/icons/a.png")),
            ("current dir", "./icons", Some("/data/ext/icons")),
            ("climbing", "../other", None),
            ("climbing after a step", "icons/../../other", None),
            ("absolute", "/etc/passwd", None),
        ];
        for (name, path, want) in cases {
            assert_eq!(
                inside(folder, Path::new(path)),
                want.map(PathBuf::from),
                "{name}"
            );
        }
    }
}
