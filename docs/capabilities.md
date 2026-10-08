# Capabilities on bayonet

bayonet knows no capability by name. A host defines them, and bayonet carries the value through the
manifest, the registry and the greeting. This note shows what that looks like for a second host whose
capabilities are `block.lens`, `open.provider`, `agent.adapter` and `row.facts`. The code is
`examples/shapes/mod.rs`; `tests/capabilities.rs` runs it against `examples/demo_plugin.rs`, a real
program on a real pipe.

## What a host writes

1. **A capability type.** One `Copy` enum with a serde spelling (`"block.lens"`). Any `Copy + Eq +
   Hash + Debug` type qualifies; dots in the spelling are fine.
2. **A `Provides` type**, one value for each `[[provides]]` entry. `parse` reads the entry's own keys
   (`kinds`, `trigger`, `tables`, `agent`) with `Entry::read`, `capability` says which capability it
   is, and `Fault` is the host's reasons to refuse a well-formed entry.
3. **A `Protocol` marker**: the request and message types and the version. `greeting` says which
   message is the plugin's first.
4. **A fit function** when it routes: `Registry::serving(capability, |entry| Fit)` asks the host how
   well an entry fits a request, and bayonet orders the answers (exact over broad, serving order for
   ties).

## How each capability uses the wire

| Capability | Shape | bayonet API |
|---|---|---|
| `row.facts` | one request, one reply (`Facts`) | `open`, then `Session::reply` |
| `block.lens` | one request, one reply whose payload is the drawing (RGBA8) | `open` with a payload limit, then `reply`; the frame's `payload` holds the pixels |
| `agent.adapter` | one request, a stream of events, then `Finished`; the host may cancel | `Session::stream` with a cancel check |
| `open.provider` | a query, a stream of batches for its generation, then `Complete`; a newer keystroke cancels; a second request asks what choosing a row does | `Session::stream` for the query, a fresh `open` for the activation |

A plugin is started for each request, so `Query` and `Activate` are two runs. A plugin that keeps
state between them keeps it in a file or a daemon of its own; bayonet does not.

## `open.provider` and the launcher

A launcher's provider is `query(&Query, &mut dyn ResultSink)` and `activate(&Item, &Choice) ->
Activation`. `open.provider` is that trait across a pipe:

| Launcher | On the wire |
|---|---|
| `Query { text, generation, .. }` | `Request::Query { text, generation }` |
| `sink.push(Batch { generation, items, .. })` | `Message::Batch(Batch { generation, items })`, any number |
| the query is answered | `Message::Complete` |
| `Query::is_cancelled()` | the host's cancel check passed to `stream`, which sends `Request::Cancel` and gives the plugin the cancel grace to stop |
| `activate(item, choice)` | `Request::Activate { item, choice }`, answered by `Message::Activation(activation)` |

`Item`, `Action`, `Icon`, `Choice` and `Activation` are written with the launcher's serde
attributes: adjacently tagged enums (`{"kind":"copy","v":{"text":"42"}}`), transparent ids,
`score` in thousandths. A plugin's `Item` is the launcher's without the two fields the host fills:
`provider`, the kind of provider that produced the row, and `subject`, what the row is about
(defaulted). So adopting a plugin in a launcher is a provider that runs a `Session` in `query`,
forwards batches into the sink, stamps its own kind on each row, and maps `Activation` onto the
launcher's closed vocabulary. The activations a plugin may name are a subset of the launcher's
(`copy`, `open`, `nothing`), because a plugin must not be able to ask a daemon for anything the
person did not install it to do.

`the_open_provider_wire_is_the_launchers_provider_shape` pins this: it reads a row as the launcher
writes it, and checks that a plugin's row plus the host's `provider` is the launcher's row.

## What bayonet does not do

It does not decide what a capability may do. A capability that lets someone act (export a file,
send a message) is the host's to gate; an agent reaches a plugin only through the host's own
confirmed actions, never directly.
