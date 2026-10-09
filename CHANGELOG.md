# Changelog

All notable changes to bayonet. The format follows Keep a Changelog; versions follow semantic
versioning.

## 0.2.0

### Added

- The `testing` feature (off by default) and `bayonet::testing`: `Fake`, a fake plugin generic over
  the host's messages. It greets, hands each request to the host's closure through a
  `Conversation`, and can misbehave on the wire in the ways of `Fault` (`Mute`, `OtherVersion`,
  `ProvidesNothing`, `CrashOnRequest`, `HangOnRequest`, `Garbage`, `StderrFlood`, `IgnoreCancel`),
  named on the command line as `--fault <name>` (`fault_argument`, `Fault::from_name`). A host's own
  faults are written in its closure, which ends the plugin with a `Step`. `Fake::serve` works on any
  reader and writer, so the plugin's half can be driven in memory; `Fake::run` serves on the
  standard streams. `examples/fake_plugin.rs` and `tests/testing.rs` run a `Runner` against one.
- Doctests on `Runner::open`, `Registry::resolve`, `suggest` and `Runner::with_log`.

### Changed (breaking)

- `Runner::with_log` takes `impl Fn(&str) + Send + Sync + 'static` instead of `fn(&str)`, so the sink
  can capture state. A closure that captures nothing, or a function, is passed as before. `Runner`
  is no longer `Copy` (it holds the sink in an `Arc`): clone it where it was copied.
- `#[non_exhaustive]` on `RunError`, `WireError`, `ManifestError`, `Refusal`, `Readiness`, `Fit`,
  `Origin` and `Rejected`. A `match` on one of the enums needs a wildcard arm; constructing a variant
  is unchanged. `Rejected` can no longer be built by literal: use `Rejected::new(file, error)`. Reading
  its public fields is unchanged.
- `Suggestion` and `Greeting` stay plain structs: hosts write them as rows of constant tables.

## 0.1.0

### Added

- The wire: length-prefixed JSON frames with a binary payload (`read_frame`, `write_frame`,
  `encode_frame`, `FrameDecoder`), with limits of 1 MiB of JSON and 512 MiB of payload checked before
  anything is buffered.
- Manifests: the shared envelope (`id`, `name`, `protocol`, `[program]`) parsed once into
  `Manifest<P>`, with each `[[provides]]` table read by the host's own `Provides` type.
- Discovery in `<data dir>/<app>/plugins`, with the app name a parameter and the person's directory
  over the system's.
- A registry that ranks plugins independent of the order the disk listed them in: newer protocol,
  then the person's directory, then id; `Registry::serving` takes the host's own fit.
- The missing-package suggestion: `Package`, `Suggestion`, `suggest`, and `Registry::tool_absent` for
  a plugin that is installed while the system's tool is not.
- `Search` and `Timeouts` are `#[non_exhaustive]`: built with `Search::new` and `Timeouts::default().with_*`.
- Running (Unix): `Runner`, `Session` and `Protocol`. A plugin gets a process group of its own, a
  greeting timeout, a silence timeout reset by every message, a cancel grace, and is killed and reaped
  on every path out. Stderr is read and logged; the last line is in a crash's error.
- A demo plugin and tests that run four capabilities of another host on the API, one of them with the
  wire shape of a launcher's provider.
