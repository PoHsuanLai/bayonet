use super::*;
use crate::fixture::{Cap, Tool, ToolFault, manifest_text, probe};
use std::path::Path;

type Parsed = Manifest<Tool>;

const FULL: &str = r#"
id = "tools"
name = "Tools"
protocol = 2

[program]
path = "/usr/libexec/app/tools"
args = ["--serve"]

[[provides]]
capability = "probe"
kinds = ["video", "audio"]

[[provides]]
capability = "play"
kinds = ["video"]
library = "/usr/lib/app/play.so"
"#;

#[test]
fn a_full_manifest_parses_to_typed_values() {
    let manifest = Parsed::parse(FULL).unwrap();
    assert_eq!(manifest.id.as_str(), "tools");
    assert_eq!(manifest.name, "Tools");
    assert_eq!(manifest.protocol, 2);
    assert_eq!(
        manifest.program.as_ref().map(|p| p.args.clone()),
        Some(vec!["--serve".to_owned()])
    );
    let capabilities: Vec<_> = manifest.provides.iter().map(|p| p.capability).collect();
    assert_eq!(capabilities, [Cap::Probe, Cap::Play]);
    assert_eq!(
        manifest.provision(Cap::Probe).map(|p| p.kinds.clone()),
        Some(vec!["video".to_owned(), "audio".to_owned()])
    );
    assert!(manifest.provision(Cap::Export).is_none());
}

#[test]
fn the_paths_to_check_are_the_program_then_each_entrys_own() {
    let manifest = Parsed::parse(FULL).unwrap();
    assert_eq!(
        manifest.paths(),
        [
            (Path::new("/usr/libexec/app/tools"), PathRole::Executable),
            (Path::new("/usr/lib/app/play.so"), PathRole::Library),
        ]
    );
}

#[test]
fn a_manifest_whose_entries_need_no_program_needs_none() {
    let text = "id = \"p\"\nname = \"P\"\nprotocol = 1\n[[provides]]\ncapability = \"play\"\nkinds = [\"video\"]\n";
    assert!(Parsed::parse(text).unwrap().program.is_none());
}

#[test]
fn a_bad_manifest_is_a_typed_error() {
    let good = manifest_text("p", 1, &probe("\"video\""));
    // name, text, the error's variant name
    let cases: Vec<(&str, String, &str)> = vec![
        ("not toml", "id = ".to_owned(), "Syntax"),
        (
            "unknown capability",
            manifest_text(
                "p",
                1,
                "[[provides]]\ncapability = \"dance\"\nkinds = [\"a\"]\n",
            ),
            "Syntax",
        ),
        ("bad id", good.replace("\"p\"", "\"P q\""), "IdInvalid"),
        (
            "empty name",
            good.replacen("name = \"p\"", "name = \" \"", 1),
            "NameEmpty",
        ),
        (
            "protocol zero",
            good.replace("protocol = 1", "protocol = 0"),
            "ProtocolZero",
        ),
        (
            "the host refuses an entry",
            manifest_text("p", 1, "[[provides]]\ncapability = \"probe\"\n"),
            "HandlesNothing",
        ),
        (
            "relative program",
            good.replace("/bin/p", "bin/p"),
            "PathNotAbsolute",
        ),
        (
            "relative path inside an entry",
            manifest_text(
                "p",
                1,
                "[[provides]]\ncapability = \"play\"\nkinds = [\"a\"]\nlibrary = \"x.so\"\n",
            ),
            "PathNotAbsolute",
        ),
        (
            "a wire capability with no program",
            "id = \"p\"\nname = \"P\"\nprotocol = 1\n".to_owned() + &probe("\"video\""),
            "ProgramMissing",
        ),
        (
            "nothing provided",
            "id = \"p\"\nname = \"P\"\nprotocol = 1\n".to_owned(),
            "NothingProvided",
        ),
    ];
    for (name, text, want) in cases {
        let error = Parsed::parse(&text).expect_err(name);
        let got = format!("{error:?}");
        assert!(got.starts_with(want), "{name}: wanted {want}, got {got}");
    }
}

#[test]
fn a_hosts_own_refusal_keeps_its_value() {
    let text = manifest_text("p", 1, "[[provides]]\ncapability = \"export\"\n");
    assert_eq!(
        Parsed::parse(&text).unwrap_err(),
        ManifestError::Provision(ToolFault::HandlesNothing(Cap::Export))
    );
}

#[test]
fn a_capability_listed_twice_is_refused() {
    let text = manifest_text("p", 1, &(probe("\"video\"") + &probe("\"audio\"")));
    assert_eq!(
        Parsed::parse(&text).unwrap_err(),
        ManifestError::CapabilityRepeated {
            capability: Cap::Probe
        }
    );
}

#[test]
fn the_file_must_be_named_for_the_id() {
    let manifest = Parsed::parse(FULL).unwrap();
    assert!(manifest.check_file_name(Path::new("/x/tools.toml")).is_ok());
    assert_eq!(
        manifest.check_file_name(Path::new("/x/other.toml")),
        Err(ManifestError::IdFileMismatch {
            id: "tools".to_owned(),
            file: "other".to_owned()
        })
    );
}
