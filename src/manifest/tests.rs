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
fn a_full_manifest_parses_to_typed_values_and_names_its_paths_and_file() {
    let manifest = Parsed::parse(FULL).unwrap();

    // step: the typed values
    assert_eq!(manifest.id.as_str(), "tools", "step typed values: id");
    assert_eq!(manifest.name, "Tools", "step typed values: name");
    assert_eq!(manifest.protocol, 2, "step typed values: protocol");
    assert_eq!(
        manifest.program.as_ref().map(|p| p.args.clone()),
        Some(vec!["--serve".to_owned()]),
        "step typed values: program args"
    );
    let capabilities: Vec<_> = manifest.provides.iter().map(|p| p.capability).collect();
    assert_eq!(
        capabilities,
        [Cap::Probe, Cap::Play],
        "step typed values: capabilities"
    );
    assert_eq!(
        manifest.provision(Cap::Probe).map(|p| p.kinds.clone()),
        Some(vec!["video".to_owned(), "audio".to_owned()]),
        "step typed values: probe kinds"
    );
    assert!(
        manifest.provision(Cap::Export).is_none(),
        "step typed values: an unlisted capability"
    );

    // step: the paths to check are the program, then each entry's own
    assert_eq!(
        manifest.paths(),
        [
            (Path::new("/usr/libexec/app/tools"), PathRole::Executable),
            (Path::new("/usr/lib/app/play.so"), PathRole::Library),
        ],
        "step paths"
    );

    // step: the file must be named for the id
    assert!(
        manifest.check_file_name(Path::new("/x/tools.toml")).is_ok(),
        "step file name: the right name"
    );
    assert_eq!(
        manifest.check_file_name(Path::new("/x/other.toml")),
        Err(ManifestError::IdFileMismatch {
            id: "tools".to_owned(),
            file: "other".to_owned()
        }),
        "step file name: the wrong name"
    );
}

#[test]
fn a_manifest_whose_entries_need_no_program_needs_none() {
    let text = "id = \"p\"\nname = \"P\"\nprotocol = 1\n[[provides]]\ncapability = \"play\"\nkinds = [\"video\"]\n";
    assert!(Parsed::parse(text).unwrap().program.is_none());
}

/// What a bad manifest must come back as: the exact error, or any syntax error (its reason is
/// the TOML parser's text, which is not ours to pin).
enum Want {
    Exact(ManifestError<Cap, ToolFault>),
    Syntax,
}

#[test]
fn a_bad_manifest_is_a_typed_error() {
    use Want::{Exact, Syntax};
    let good = manifest_text("p", 1, &probe("\"video\""));
    let without_program = "id = \"p\"\nname = \"P\"\nprotocol = 1\n".to_owned();
    // name, text, the error wanted
    let cases: Vec<(&str, String, Want)> = vec![
        ("not toml", "id = ".to_owned(), Syntax),
        (
            "unknown capability",
            manifest_text(
                "p",
                1,
                "[[provides]]\ncapability = \"dance\"\nkinds = [\"a\"]\n",
            ),
            Syntax,
        ),
        (
            "bad id",
            good.replace("\"p\"", "\"P q\""),
            Exact(ManifestError::IdInvalid {
                id: "P q".to_owned(),
            }),
        ),
        (
            "empty name",
            good.replacen("name = \"p\"", "name = \" \"", 1),
            Exact(ManifestError::NameEmpty),
        ),
        (
            "protocol zero",
            good.replace("protocol = 1", "protocol = 0"),
            Exact(ManifestError::ProtocolZero),
        ),
        (
            "the host refuses an entry with no kinds",
            manifest_text("p", 1, "[[provides]]\ncapability = \"probe\"\n"),
            Exact(ManifestError::Provision(ToolFault::HandlesNothing(
                Cap::Probe,
            ))),
        ),
        (
            "a host's own refusal keeps its value",
            manifest_text("p", 1, "[[provides]]\ncapability = \"export\"\n"),
            Exact(ManifestError::Provision(ToolFault::HandlesNothing(
                Cap::Export,
            ))),
        ),
        (
            "relative program",
            good.replace("/bin/p", "bin/p"),
            Exact(ManifestError::PathNotAbsolute {
                path: "bin/p".into(),
            }),
        ),
        (
            "relative path inside an entry",
            manifest_text(
                "p",
                1,
                "[[provides]]\ncapability = \"play\"\nkinds = [\"a\"]\nlibrary = \"x.so\"\n",
            ),
            Exact(ManifestError::PathNotAbsolute {
                path: "x.so".into(),
            }),
        ),
        (
            "a wire capability with no program",
            without_program.clone() + &probe("\"video\""),
            Exact(ManifestError::ProgramMissing {
                capability: Cap::Probe,
            }),
        ),
        (
            "nothing provided",
            without_program,
            Exact(ManifestError::NothingProvided),
        ),
        (
            "a capability listed twice",
            manifest_text("p", 1, &(probe("\"video\"") + &probe("\"audio\""))),
            Exact(ManifestError::CapabilityRepeated {
                capability: Cap::Probe,
            }),
        ),
    ];
    for (name, text, want) in cases {
        let error = Parsed::parse(&text).expect_err(name);
        match want {
            Exact(want) => assert_eq!(error, want, "{name}"),
            Syntax => assert!(
                matches!(error, ManifestError::Syntax { .. }),
                "{name}: wanted a syntax error, got {error:?}"
            ),
        }
    }
}
