# bayonet

bayonet runs plugins as separate programs. A plugin is an executable a person installs; the host
never links it. The plugin describes itself in a TOML manifest, the host finds the manifest, starts
the program for one request, and talks to it over a pipe in length-prefixed JSON frames. A plugin
that crashes, hangs or lies costs one request and never takes the host down.

It is generic over the host's *capability type*, the things a plugin can offer (`thumbnail`,
`block.lens`, `open.provider`). bayonet owns the mechanics and none of the vocabulary:

| Module | What it does |
|---|---|
| `wire` | Frames (`u32 LE json length`, `u32 LE payload length`, JSON, payload), the blocking `read_frame` and `write_frame` a plugin uses, and the incremental `FrameDecoder` a host uses. A plugin program depends on this alone. |
| `manifest` | The envelope every manifest has (`id`, `name`, `protocol`, optional `version`, `licence`, `summary`, `[program]`), checked once. Each `[[provides]]` table goes to the host's own `Provides` type, or is kept as `Raw` TOML with its `capability` word for an owner to read later. `Version` is a dotted version that compares numerically. |
| `discover` | Finds `<data dir>/<app>/plugins/*.toml` (the flat `Layout`) or `<data dir>/<app>/<dir>/<id>/<manifest name>` (`Layout::Folders`, the folder name is the id and paths stay inside the folder) under the person's directory and the system's, with the app name a parameter, and checks that the programs they name are there. `scan` stops before ranking; `reparse` turns a `Candidate<Raw>` into the owner's `Candidate<P>`, reporting entries of words the owner does not own as skipped. |
| `registry` | Which plugin serves a request: newer protocol first, then the person's directory over the system's, then the newer `version`, then by id, so the order the disk listed files in never matters. |
| `suggest` | The package to name when no installed plugin serves a request. |
| `run` (Unix) | Starts the program in a process group of its own, takes its greeting, sends the request, reads the answer with a deadline, supports cancellation with a grace period, and kills and reaps the whole group on every path out. |

## Using it

A host implements two small traits.

```rust,ignore
// One `[[provides]]` entry of a manifest, parsed into the host's own type.
impl bayonet::manifest::Provides for Provision { /* Capability, Fault, parse, capability */ }

// The messages the host and its plugins exchange, and the greeting that starts them.
impl bayonet::run::Protocol for Wire { /* Provides, Request, Message, VERSION, greeting */ }
```

Then:

```rust,ignore
let found = bayonet::discover::<Provision>(&Search::new("myapp", user, system, Wire::VERSION));
let plugin = found.registry.serving(Cap::Thumbnail, |p| fit(p, &file)).ok_or(missing)?;
let mut session = Runner::new("myapp", Timeouts::default().with_silence(Duration::from_secs(10)))
    .open::<Wire>(plugin, Cap::Thumbnail, &request, payload_limit)?;
let reply = session.reply()?;           // one answer, or
session.stream(cancelled, &Request::Cancel, |id, frame| /* ... */)?;  // a stream of them
```

For a layout of one folder per plugin, with a vocabulary owned by several crates:

```rust,ignore
let search = Search::new("quire", user, system, 1)
    .with_layout(Layout::Folders { dir: "extensions", manifest: "extension.toml" });
let scanned = bayonet::scan::<Raw>(&search);            // knows no capability word
let typed = scanned.candidates.into_iter()
    .map(bayonet::reparse::<Mine>)                       // the owner's words; others are skipped
    /* ...the owner's own checks... */;
let registry = Registry::resolve(candidates, Wire::VERSION);
```

A plugin program is a loop over `read_frame` and `write_frame` on its standard streams. It speaks
first, with its greeting. `examples/demo_plugin.rs` is the shortest complete one.

## Capabilities of other hosts

`docs/capabilities.md` shows how four capabilities of a document host (`block.lens`, `open.provider`,
`agent.adapter`, `row.facts`) sit on this API, and how `open.provider` has the wire shape of a
launcher's provider, so one plugin can feed a launcher and a document host. `tests/bayonet/capabilities.rs` runs
them against a real plugin program.

## Features and portability

- `host` (default): everything but the wire. Without it the crate is `serde`, `serde_json` and
  `thiserror`, which is what a plugin program needs.
- `testing` (off by default): `bayonet::testing::Fake`, a fake plugin for a host's tests. It greets,
  hands each request to a closure of the host's, and misbehaves on the wire on request
  (`--fault mute`, `garbage`, `ignore-cancel`, and the rest of `Fault`). `examples/fake_plugin.rs` is
  one in a few lines, and `tests/bayonet/testing.rs` runs a `Runner` against it.
- The wire, manifests, discovery and registry build on any platform. `run` needs Unix (process
  groups, `poll`) and is absent elsewhere. There is no bus, no UI toolkit and no async runtime in
  the tree; `cargo deny` bans them.

## Run, never link

bayonet exists so that copyleft and codec code is never linked into an application. See
`CONVENTIONS.md`, section 15.

## Licence

MIT OR Apache-2.0, at your option. See `LICENSE-MIT` and `LICENSE-APACHE`.
