//! Keeps every integration test inside the one binary.

use std::fs;
use std::path::Path;

/// The directory of the one binary, under `tests/`.
const BINARY: &str = "bayonet";

#[test]
fn every_integration_test_file_is_part_of_the_binary() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut stray = Vec::new();
    for entry in fs::read_dir(&tests).expect("the tests directory") {
        let path = entry.expect("a directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_rs = path.extension().is_some_and(|ext| ext == "rs");
        if (path.is_file() && is_rs) || (path.is_dir() && name != BINARY) {
            stray.push(name.to_owned());
        }
    }
    assert!(
        stray.is_empty(),
        "these would be binaries of their own; move them into tests/{BINARY}/ as modules: {stray:?}"
    );

    let own = tests.join(BINARY);
    let main = fs::read_to_string(own.join("main.rs")).expect("main.rs");
    let mut unlisted = Vec::new();
    for entry in fs::read_dir(&own).expect("the binary's directory") {
        let path = entry.expect("a directory entry").path();
        let Some(module) = path.file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        if module != "main" && !main.contains(&format!("mod {module};")) {
            unlisted.push(module.to_owned());
        }
    }
    assert!(
        unlisted.is_empty(),
        "main.rs declares no `mod` for: {unlisted:?}"
    );
}
