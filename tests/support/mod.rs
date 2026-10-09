//! A scratch host for the integration tests: manifests for the demo plugin in a scratch data
//! directory, and the registry that finding them makes.

// `shapes` is also read by the demo plugin, and each test file uses some of it.
#![allow(dead_code)]

#[path = "../../examples/shapes/mod.rs"]
pub mod shapes;

use bayonet::registry::Installed;
use bayonet::run::{Runner, Timeouts};
use bayonet::{Discovery, Search, discover};
use shapes::Provision;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Every capability of the demo host, as a manifest lists them.
pub const PROVIDES: &str = r#"
[[provides]]
capability = "block.lens"
kinds = ["chart"]

[[provides]]
capability = "open.provider"
trigger = "gh "

[[provides]]
capability = "agent.adapter"
agent = "claude"

[[provides]]
capability = "row.facts"
tables = ["issues"]
"#;

/// The demo plugin, built beside the tests as an example.
pub fn demo_program() -> PathBuf {
    example_program("demo_plugin")
}

/// The example called `name`, built beside the tests.
pub fn example_program(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("the test's own path");
    let program = exe
        .parent()
        .and_then(|deps| deps.parent())
        .map(|profile| profile.join("examples").join(name))
        .expect("a target directory");
    assert!(
        program.is_file(),
        "{} is missing: run the tests with `cargo test`, which builds the examples",
        program.display()
    );
    program
}

/// Timeouts short enough for a test of a hang, long enough for a loaded machine to start a
/// program.
pub fn quick() -> Timeouts {
    Timeouts::default()
        .with_hello(Duration::from_secs(10))
        .with_silence(Duration::from_millis(400))
        .with_cancel_grace(Duration::from_millis(300))
}

pub fn runner(timeouts: Timeouts) -> Runner {
    Runner::new("demo", timeouts).with_log(|_| {})
}

/// A scratch data directory for the host called `demo`.
pub struct Host {
    root: tempfile::TempDir,
}

impl Host {
    pub fn new() -> Host {
        Host {
            root: tempfile::tempdir().expect("a scratch directory"),
        }
    }

    /// Installs the demo plugin as `id`, started with `args`, providing `provides`.
    pub fn install(&self, id: &str, args: &[&str], provides: &str) {
        self.install_program(id, &demo_program(), args, provides);
    }

    /// Installs `program` as `id`, started with `args`, providing `provides`.
    pub fn install_program(&self, id: &str, program: &Path, args: &[&str], provides: &str) {
        let folder = self.root.path().join("user").join("demo").join("plugins");
        std::fs::create_dir_all(&folder).expect("the plugin folder");
        let text = format!(
            "id = \"{id}\"\nname = \"{id}\"\nprotocol = 1\n[program]\npath = {:?}\nargs = {:?}\n{provides}",
            program.display().to_string(),
            args
        );
        std::fs::write(folder.join(format!("{id}.toml")), text).expect("the manifest");
    }

    pub fn discover(&self) -> Discovery<Provision> {
        discover::<Provision>(&Search::new("demo", &self.root.path().join("user"), &[], 1))
    }

    /// The installed plugin `id`.
    pub fn plugin(&self, id: &str) -> Installed<Provision> {
        self.discover()
            .registry
            .installed()
            .iter()
            .find(|plugin| plugin.manifest.id.as_str() == id)
            .cloned()
            .expect("the plugin is installed and usable")
    }

    /// A pid file path in the scratch directory.
    pub fn pid_file(&self) -> PathBuf {
        self.root.path().join("child.pid")
    }
}
