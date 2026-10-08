use super::*;
use crate::fixture::{Cap, Tool, manifest_text, probe};
use crate::manifest::{Manifest, ManifestError};

type Plugins = Registry<Tool>;

const SUPPORTED: u32 = 1;

/// A candidate whose manifest provides `probe` for `kinds` from `/bin/<id>`.
fn candidate(id: &str, protocol: u32, origin: Origin, kinds: &str) -> Candidate<Tool> {
    Candidate {
        manifest: Manifest::parse(&manifest_text(id, protocol, &probe(kinds))).unwrap(),
        origin,
        readiness: Readiness::Ready,
    }
}

/// How the test host fits a request for the kind `wanted`, the media type `exact` counting closer.
fn fit<'a>(wanted: &'a str, exact: &'a str) -> impl Fn(&Tool) -> Fit + 'a {
    move |tool| {
        if tool.kinds.iter().any(|kind| kind == exact) {
            Fit::Exact
        } else if tool.kinds.iter().any(|kind| kind == wanted) {
            Fit::Broad
        } else {
            Fit::Miss
        }
    }
}

fn serving_id(plugins: &Plugins, wanted: &str, exact: &str) -> Option<String> {
    plugins
        .serving(Cap::Probe, fit(wanted, exact))
        .map(|plugin| plugin.manifest.id.as_str().to_owned())
}

#[test]
fn the_user_directory_overrides_the_system_one_for_an_id() {
    let system = candidate("tool", 1, Origin::System, "\"video\"");
    let user = candidate("tool", 1, Origin::User, "\"audio\"");
    let plugins = Plugins::resolve(vec![system, user], SUPPORTED);
    assert_eq!(plugins.installed().len(), 1);
    assert_eq!(plugins.installed()[0].origin, Origin::User);
    // The user's copy handles audio only, so the system's video is gone with it.
    assert_eq!(serving_id(&plugins, "video", "-"), None);
    assert_eq!(serving_id(&plugins, "audio", "-").as_deref(), Some("tool"));
}

#[test]
fn a_protocol_beyond_the_hosts_is_set_aside_with_its_reason() {
    let newer = candidate("tool", 2, Origin::System, "\"video\"");
    let older = candidate("tool", 1, Origin::User, "\"video\"");
    let plugins = Plugins::resolve(vec![newer, older], SUPPORTED);
    assert_eq!(plugins.installed()[0].manifest.protocol, 1);
    assert_eq!(
        plugins.unusable()[0].reason,
        ManifestError::ProtocolUnsupported {
            protocol: 2,
            supported: SUPPORTED
        }
    );
}

#[test]
fn rank_prefers_a_higher_protocol_then_the_user() {
    let installed = |protocol, origin| Installed {
        manifest: candidate("a", protocol, origin, "\"video\"").manifest,
        origin,
    };
    let low_user = installed(1, Origin::User);
    let high_system = installed(3, Origin::System);
    let high_user = installed(3, Origin::User);
    let mut all = vec![low_user.clone(), high_system.clone(), high_user.clone()];
    all.sort_by_key(rank);
    assert_eq!(all, [high_user, high_system, low_user]);
}

#[test]
fn a_plugin_whose_program_is_unusable_never_shadows_a_working_one() {
    let mut broken = candidate("tool", 1, Origin::User, "\"video\"");
    broken.readiness = Readiness::Unready(ManifestError::FileMissing {
        path: "/bin/tool".into(),
    });
    let working = candidate("tool", 1, Origin::System, "\"video\"");
    let plugins = Plugins::resolve(vec![broken, working], SUPPORTED);
    assert_eq!(plugins.installed()[0].origin, Origin::System);
    assert_eq!(plugins.unusable().len(), 1);
    assert_eq!(plugins.unusable()[0].origin, Origin::User);
}

#[test]
fn an_exact_fit_beats_a_broad_one_and_ties_go_by_id() {
    let by_kind = candidate("a-kind", 1, Origin::System, "\"video\"");
    let by_type = candidate("z-type", 1, Origin::System, "\"video/x-odd\"");
    let plugins = Plugins::resolve(vec![by_type, by_kind], SUPPORTED);
    assert_eq!(
        serving_id(&plugins, "video", "video/x-odd").as_deref(),
        Some("z-type")
    );
    assert_eq!(
        serving_id(&plugins, "video", "-").as_deref(),
        Some("a-kind")
    );
    assert_eq!(serving_id(&plugins, "audio", "-"), None);
}

#[test]
fn the_order_discovery_listed_files_in_does_not_matter() {
    let a = candidate("alpha", 1, Origin::System, "\"video\"");
    let b = candidate("beta", 1, Origin::System, "\"video\"");
    let forward = Plugins::resolve(vec![a.clone(), b.clone()], SUPPORTED);
    let backward = Plugins::resolve(vec![b, a], SUPPORTED);
    assert_eq!(forward, backward);
    assert_eq!(serving_id(&forward, "video", "-").as_deref(), Some("alpha"));
}

#[test]
fn a_capability_the_plugin_does_not_provide_is_not_served() {
    let plugins = Plugins::resolve(
        vec![candidate("tool", 1, Origin::User, "\"video\"")],
        SUPPORTED,
    );
    assert!(plugins.serving(Cap::Probe, fit("video", "-")).is_some());
    assert!(plugins.serving(Cap::Export, fit("video", "-")).is_none());
    assert!(
        Plugins::empty()
            .serving(Cap::Probe, fit("video", "-"))
            .is_none()
    );
}

#[test]
fn a_plugin_set_aside_for_want_of_a_tool_says_which_tool() {
    let unready = |id: &str, reason: ManifestError<Cap, crate::fixture::ToolFault>| {
        let mut one = candidate(id, 1, Origin::System, "\"video\"");
        one.readiness = Readiness::Unready(reason);
        Plugins::resolve(vec![one], SUPPORTED)
    };
    let missing = |path: &str| ManifestError::FileMissing { path: path.into() };
    // name, the registry, the plugin and tool asked about, whether that tool is what is absent
    let cases: Vec<(&str, Plugins, &str, &str, bool)> = vec![
        (
            "the tool itself is absent",
            unready("play", missing("/usr/bin/mpv")),
            "play",
            "mpv",
            true,
        ),
        (
            "the tool is there but not executable",
            unready(
                "play",
                ManifestError::NotExecutable {
                    path: "/usr/bin/mpv".into(),
                },
            ),
            "play",
            "mpv",
            true,
        ),
        (
            "another file is absent: installing the tool would not help",
            unready("play", missing("/opt/play.so")),
            "play",
            "mpv",
            false,
        ),
        ("no such plugin", Plugins::empty(), "play", "mpv", false),
        (
            "another plugin's tool",
            unready("play", missing("/usr/bin/mpv")),
            "other",
            "mpv",
            false,
        ),
    ];
    for (name, plugins, id, tool, want) in cases {
        assert_eq!(plugins.tool_absent(id, tool), want, "{name}");
    }
}
