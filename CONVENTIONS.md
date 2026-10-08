# Conventions

bayonet follows the conventions the author's other Rust repositories share. This file carries the
ones that apply to a small, portable library, with the section numbers of the shared text, so a
reference such as "section 15" means the same thing here as there. The sections about user
interfaces, services and dev scripts are not carried because bayonet has none.

The aim is code that reads as one deliberate design. Every rule below exists because the code once
broke it. If a rule blocks you, say so in the change; never deviate quietly.

---

## 1. Before writing anything

1. **Find the home.** Each concept has one file. Extend it; never write a second one. The wire is
   `wire`, the manifest envelope is `manifest`, ranking is `registry`, finding files is `discover`,
   starting programs is `run`.
2. **Find the layer.** `wire` knows nothing of the rest. `manifest` knows `wire` not at all.
   `registry` and `discover` know `manifest`. `run` knows all of them. A lower layer never names a
   higher one; if what you need lives above you, move the shared piece down or pass it in.
3. **Stay generic.** bayonet names no host, no capability, no file kind and no message. A host's
   vocabulary enters through `Provides` and `Protocol`; if a change needs bayonet to know a word of
   one host, the abstraction is wrong. Say so rather than add the word.

## 2. Files and modules

- **One concept per file,** named after the concept, never after the work that produced it.
  Forbidden in file, module, test and function names: `v2`, `new_`, `old_`, `fixes`, `followups`,
  `polish`, `gaps`, `round`, `wave`, a ticket id, another project's name.
- **Size.** Aim for under 300 lines; at 400, split by concept, never by halves. A function over
  about 50 lines, or one that needs a comment to separate its phases, is two functions.
- **`mod.rs` and `lib.rs` hold declarations and re-exports only,** plus at most the one type the
  module is named after.
- **No `unsafe`** (`unsafe_code = "deny"`). No `std::env::set_var`.

## 3. Public API

- **Private by default.** Items are `pub(crate)` unless a caller outside uses them today.
- **One path per public item,** re-exported once. No glob re-exports.
- **Unique, specific names.** No bare generic names at the crate root.
- **Test helpers are not API.** They live in `tests/support/`, `examples/` or `#[cfg(test)]`.
- **An API change updates every caller in the same change.** No deprecated alias.

## 4. Types

- **Data apart from logic.** A type's own `impl` holds construction, derivation and accessors.
- **No `bool`** in a struct field, a parameter or a message: use a two-variant enum. Predicates
  may return `bool`.
- **Newtype every identifier** (`PluginId`). Parse at the boundary, once: a manifest becomes typed
  values where it is read, and nothing past it checks again.
- **Model absence and failure in the state.** A plugin that cannot be used is in
  `Registry::unusable` with its reason, never silently missing.

## 5. Traits

- A trait exists when **two or more implementations** are swapped at that boundary, or a **generic
  consumer** runs over many. `Provides` and `Protocol` are the two seams a host fills; there are no
  others. Keep traits small.
- **Closed sets stay enums** (`RunError`, `ManifestError`, `WireError`). A trait there hides the
  compile error you want when a variant is added.

## 6. State, effects and dependencies

- **Effects show in the signature.** Reading the disk, spawning and waiting are in `discover` and
  `run`, and the signatures say `Result`. `manifest`, `registry`, `suggest` and `wire`'s framing
  are values and functions of values.
- **Everything ambient is injected.** bayonet reads no environment variable and no clock for a
  decision: `discover` takes the directories, `Runner` takes the timeouts and the log sink.
- **No globals.** No `static` mutable state, no `OnceLock`, no `thread_local!`.
- **Dependencies are few and settled.** A new dependency is a decision recorded in the change that
  adds it. Never write what a dependency already does; never add a dependency for something ten
  lines do. No bus, no UI toolkit, no async runtime: `deny.toml` bans them.

## 7. Errors and logging

- An error exists so a caller can **act**, not so a string can be logged. One `thiserror` enum per
  concern, with variants a caller can act on. No `anyhow`.
- No `unwrap` or `expect` outside tests unless a comment on the same line proves the invariant.
- Malformed input from outside (a manifest, a pipe, a plugin's output) is never a panic.
- A plugin's stderr is the only log bayonet writes, through the runner's sink. No stray `eprintln!`.

## 8. Tests

- **Name tests after behaviour:** `a_plugin_that_ignores_a_cancel_is_killed_after_the_grace_period`.
- **Unit tests beside the code** (`#[cfg(test)] mod tests`), **integration tests** in `tests/`.
- **Pure functions get table tests.** One table per function and one loop; each row names its case.
- **Tests never touch the real system:** scratch directories (`tempfile`), no real `$HOME`, no
  system data directory. A plugin under test is `examples/demo_plugin.rs`.
- **An assertion must be able to fail.** Assert the difference the action makes, not a state the
  action happens to be compatible with. Ask the program for the thing under test.

## 9. Comments and docs

- **Comments say why,** in the present tense, about the code as it is. They never narrate history
  and never cite tickets, rounds or dates.
- **Public items carry a doc comment** saying what they mean, not what they are
  (`missing_docs` warns, and clippy runs with `-D warnings`).
- **No "for now", "until", "later", "temporary"** in code or docs.
- **Docs describe the present.** `README.md` is the map, this file the rules, `CHANGELOG.md` what
  changed. A backticked path in a doc names a file that is there.

## 10. Change discipline

- **Replace, don't add beside.** When something supersedes old code, delete the old code in the
  same change.
- **Small commits,** one concern each, compiling at every commit, message in the imperative.

## 12. Derives and serde

- **Public types derive, in this order, what holds:** `Debug, Clone, PartialEq, Eq, Serialize,
  Deserialize`. `Copy` only for types one word or smaller; `Hash` for every identifier and map
  key. Data is `Eq`, so floats stay out of it.
- **`Serialize` and `Deserialize` only on a type that is stored or crosses a wire.**
- **Never `deny_unknown_fields`** on a stored type: an unknown key does not stop a load.
- **Every wire type has a round-trip test.**

## 13. Names

- Enums read as values at the use site. No `get_` prefix.
- Name a value for what it does, not for where it came from. Wire vocabulary stays at the wire.

## 14. Verification

The gate passes before work is reported complete, and CI runs the same:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo clippy --no-default-features --all-targets --locked -- -D warnings
cargo test --locked          # builds examples/demo_plugin.rs, which the tests start
cargo doc --no-deps          # with RUSTDOCFLAGS="-D warnings"
cargo deny check advisories bans licenses sources
cargo check --lib --target x86_64-pc-windows-msvc   # the wire, manifests and registry build off Unix
```

Check every exit code, never a piped summary. **Substrings are not tokens:** match a thing that has
a grammar at its boundaries (compare whole labels, split on the separator, use the parser).
**Run it the way its user would** before concluding anything about it. **Report honestly.**

## 15. Run, never link

Every repo is `MIT OR Apache-2.0`. Copyleft and codec code is never linked into what we ship and
never pasted into it: it lives in separate-process plugins that use the person's own distro tools.
The licence of what you read or depend on decides which:

- **Permissive** (MIT, Apache-2.0, BSD, 0BSD): code may be copied, with attribution, and linked.
- **MPL** (file-level copyleft): a crate may be depended on, unmodified, from the registry; never
  paste. MPL's unit is the file: paste one function and that file becomes MPL, and the crate's
  licence claim becomes false.
- **LGPL, GPL, AGPL, EUPL** and anything whose licence cannot be verified: never link, never
  paste, read for facts only. "LGPL, so linking is fine" is not a position we hold: upstream libav
  and libmpv are LGPL, but the builds distributions ship are GPL, and a binding to a codec library
  also puts the patent-encumbered codecs in our binary.

**A thing we may not link is a plugin.** It is a separate executable the person installs, which
uses their own distro's tools and talks to the host over arm's-length IPC: a pipe, or a socket and
shared memory. Never a `dlopen`ed library of ours or theirs. A crash, a hang or a lie of a plugin
is an event and costs one request; it never takes the host down. Which package adds a missing
plugin is named by the host, never silently absent. bayonet is the mechanism: `run` starts the
program in a process group and kills it and everything it started on every path out, and
`suggest` names the package.

**The boundary is mechanical.** `deny.toml` bans `rsmpv`, `rsmpv-sys`, `libmpv`, `libmpv-sys`,
`ffmpeg-next`, `ffmpeg-sys-next`, `libheif-rs`, `libheif-sys`, `libraw-rs`, `libraw-sys`, `rsraw`,
`rawloader` and `rawler` from the dependency tree, and allows no copyleft licence. A host repo
holds the same line in its own boundary check and reads `ldd` of what it ships. Pure-Rust parsing
of a container is not linking a codec.

**Note-and-close.** Read the source, reduce what you learned to a sentence about observable
behaviour, close the file, implement from the sentence. If it fits in that sentence, it is a fact
and it is yours; if you need the source open to reproduce it, it is expression and it is theirs.
Matching identifiers, comments, branch order, magic constants, error strings or bugs mean you
copied.

**Never put GPL, AGPL or MPL source into a model's context** and ask for an equivalent. Put the
spec section and our own fixture in the prompt instead.

**Cite the behaviour, not the source.** `// taken from <gpl-project>/file.c:412` is a pointer to
copyleft code that reads as an admission. Write what the behaviour is and point at the test or
trace that proves it; a quirk without a trace is a rumour.
