//! The package to suggest when no installed plugin serves a request.

/// A package a person installs with their system's package manager, and what it takes to make
/// it work. `H` is the host's own handle for the tool the package runs (the host decides what it
/// does with it, for instance offer to install it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Package<H> {
    name: &'static str,
    helper: H,
    tool: Option<&'static str>,
}

impl<H: Copy> Package<H> {
    /// A package called `name` whose plugin runs `helper`. `tool` is the file name of the
    /// system's program that the plugin's manifest names, when it names one: when that file is
    /// absent the plugin is installed and the tool is what is missing.
    pub const fn new(name: &'static str, helper: H, tool: Option<&'static str>) -> Package<H> {
        Package { name, helper, tool }
    }

    /// The package's name, as every distribution that ships it spells it.
    pub fn name(self) -> &'static str {
        self.name
    }

    /// The host's handle for the tool the package's plugin runs.
    pub fn helper(self) -> H {
        self.helper
    }

    /// The file name of the program of the system's that the plugin's manifest names, if it names
    /// one.
    pub fn tool(self) -> Option<&'static str> {
        self.tool
    }
}

/// One row of a host's table of packages: which package provides `capability` for requests that
/// match `key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suggestion<C, K, H> {
    /// What the package provides.
    pub capability: C,
    /// What the request must match, in the host's own terms (a kind of file, a block type).
    pub key: K,
    /// The package.
    pub package: Package<H>,
}

/// The package of the first row of `table` for `capability` whose key `matches`, or `None` when
/// no package is known.
///
/// ```
/// use bayonet::{Package, Suggestion, suggest};
///
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum Capability {
///     Play,
/// }
///
/// const TABLE: &[Suggestion<Capability, &str, ()>] = &[Suggestion {
///     capability: Capability::Play,
///     key: "video",
///     package: Package::new("mpv", (), Some("mpv")),
/// }];
///
/// let package = suggest(TABLE, Capability::Play, |kind| *kind == "video");
/// assert_eq!(package.map(Package::name), Some("mpv"));
/// assert!(suggest(TABLE, Capability::Play, |kind| *kind == "audio").is_none());
/// ```
pub fn suggest<C: PartialEq, K, H: Copy>(
    table: &[Suggestion<C, K, H>],
    capability: C,
    matches: impl Fn(&K) -> bool,
) -> Option<Package<H>> {
    table
        .iter()
        .find(|row| row.capability == capability && matches(&row.key))
        .map(|row| row.package)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAYER: Package<u8> = Package::new("app-player", 1, Some("player"));
    const LENS: Package<u8> = Package::new("app-lens", 2, None);

    const TABLE: &[Suggestion<&str, Option<&str>, u8>] = &[
        Suggestion {
            capability: "play",
            key: None,
            package: PLAYER,
        },
        Suggestion {
            capability: "lens",
            key: Some("text/x-odd"),
            package: LENS,
        },
    ];

    #[test]
    fn the_first_row_for_the_capability_whose_key_matches_names_its_package() {
        // name, capability, the type asked about, the package
        const CASES: &[(&str, &str, &str, Option<&str>)] = &[
            (
                "a row with no key matches any",
                "play",
                "-",
                Some("app-player"),
            ),
            (
                "a keyed row matches its key",
                "lens",
                "text/x-odd",
                Some("app-lens"),
            ),
            ("a keyed row skips another key", "lens", "text/plain", None),
            ("an unknown capability", "export", "-", None),
        ];
        for (name, capability, asked, want) in CASES {
            let got = suggest(TABLE, *capability, |key| key.is_none_or(|k| k == *asked))
                .map(Package::name);
            assert_eq!(got, *want, "{name}");
        }
    }

    #[test]
    fn a_package_names_the_tool_its_plugin_runs_and_the_hosts_handle() {
        assert_eq!(
            (PLAYER.tool(), PLAYER.helper(), LENS.tool()),
            (Some("player"), 1, None)
        );
    }
}
