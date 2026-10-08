# Changelog

All notable changes to bayonet. The format follows Keep a Changelog; versions follow semantic
versioning.

## Unreleased

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
