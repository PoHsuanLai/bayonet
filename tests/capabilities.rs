//! The capabilities of a second host (`block.lens`, `open.provider`, `agent.adapter`,
//! `row.facts`) on bayonet's generic API, against a real plugin program: discovery finds them,
//! the registry routes them, the runner asks them.

#![allow(clippy::unwrap_used)]
// A test handler names the messages it expects and refuses the rest alike.
#![allow(clippy::wildcard_enum_match_arm)]

mod support;

use bayonet::registry::Fit;
use bayonet::run::RunError;
use serde_json::{Value, json};
use std::cell::Cell;
use std::ops::ControlFlow;
use support::shapes::{
    Action, Activation, Batch, Block, Cap, Choice, Fact, Icon, Item, Message, OpenTarget,
    Provision, Request, Wire,
};
use support::{Host, PROVIDES, quick, runner};

fn installed() -> (Host, bayonet::registry::Installed<Provision>) {
    let host = Host::new();
    host.install("demo", &[], PROVIDES);
    let plugin = host.plugin("demo");
    (host, plugin)
}

#[test]
fn one_manifest_provides_all_four_capabilities_and_the_registry_routes_each() {
    let (host, _) = installed();
    let found = host.discover();
    assert!(found.rejected.is_empty());
    let registry = found.registry;
    // name, the capability asked, what the entry must name to fit
    let cases: &[(&str, Cap, &str)] = &[
        ("a chart lens", Cap::BlockLens, "chart"),
        ("a provider", Cap::OpenProvider, "gh "),
        ("an agent adapter", Cap::AgentAdapter, "claude"),
        ("row facts", Cap::RowFacts, "issues"),
    ];
    for (name, capability, wanted) in cases {
        let served = registry.serving(*capability, |provision| match provision {
            Provision::BlockLens(names) | Provision::RowFacts(names)
                if names.iter().any(|n| n == wanted) =>
            {
                Fit::Exact
            }
            Provision::OpenProvider(word) | Provision::AgentAdapter(word) if word == wanted => {
                Fit::Exact
            }
            Provision::BlockLens(_)
            | Provision::RowFacts(_)
            | Provision::OpenProvider(_)
            | Provision::AgentAdapter(_) => Fit::Miss,
        });
        assert_eq!(
            served.map(|p| p.manifest.id.as_str()),
            Some("demo"),
            "{name}"
        );
    }
    let unserved = registry.serving(Cap::BlockLens, |_| Fit::Miss);
    assert!(unserved.is_none());
}

#[test]
fn block_lens_returns_a_drawing_as_the_frames_payload() {
    let (_host, plugin) = installed();
    let lens = Request::Lens(Block {
        kind: "chart".to_owned(),
        text: "abc".to_owned(),
    });
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::BlockLens, &lens, 1 << 10)
        .unwrap();
    let frame = session.reply().unwrap();
    assert_eq!(
        frame.message,
        Message::Drawn {
            width: 3,
            height: 1
        }
    );
    // `c` is the first byte of "chart", repeated for 3 RGBA pixels.
    assert_eq!(frame.payload, vec![b'c'; 12]);
}

#[test]
fn row_facts_is_one_request_and_one_reply() {
    let (_host, plugin) = installed();
    let ask = Request::Facts {
        table: "issues".to_owned(),
        key: "42".to_owned(),
    };
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::RowFacts, &ask, 0)
        .unwrap();
    assert_eq!(
        session.reply().unwrap().message,
        Message::Facts(vec![Fact {
            label: "issues".to_owned(),
            value: "42".to_owned()
        }])
    );
}

#[test]
fn an_agent_adapter_streams_events_until_it_finishes() {
    let (_host, plugin) = installed();
    let task = Request::Adapt {
        task: "triage".to_owned(),
    };
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::AgentAdapter, &task, 0)
        .unwrap();
    let mut events = Vec::new();
    session
        .stream(
            |_| false,
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, RunError<Cap>> {
                match frame.message {
                    Message::Event { text } => {
                        events.push(text);
                        Ok(ControlFlow::Continue(()))
                    }
                    Message::Finished => Ok(ControlFlow::Break(())),
                    other => Err(RunError::unexpected("demo", &other)),
                }
            },
        )
        .unwrap();
    assert_eq!(
        events,
        ["triage: step 1", "triage: step 2", "triage: step 3"]
    );
}

/// What a launcher does with a provider plugin: batches go to its sink as they arrive, and a
/// newer keystroke cancels the query in flight.
#[test]
fn an_open_provider_feeds_a_launcher_sink_and_a_newer_keystroke_cancels_it() {
    let (_host, plugin) = installed();
    let query = Request::Query {
        text: "slow".to_owned(),
        generation: 7,
    };
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::OpenProvider, &query, 0)
        .unwrap();
    let keystroke = Cell::new(false);
    let mut sink: Vec<Batch> = Vec::new();
    let outcome = session
        .stream(
            |_| keystroke.get(),
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<String>, RunError<Cap>> {
                match frame.message {
                    Message::Batch(batch) => {
                        sink.push(batch);
                        // The third batch has arrived: the person types another letter.
                        keystroke.set(sink.len() >= 3);
                        Ok(ControlFlow::Continue(()))
                    }
                    Message::Failed { message } => Ok(ControlFlow::Break(message)),
                    other => Err(RunError::unexpected("demo", &other)),
                }
            },
        )
        .unwrap();
    assert_eq!(outcome, "cancelled", "the plugin acknowledged the cancel");
    assert!(sink.len() >= 3, "batches arrived while the query ran");
    assert!(sink.iter().all(|batch| batch.generation == 7));
}

#[test]
fn an_open_provider_answers_a_whole_query_then_says_what_choosing_a_row_does() {
    let (_host, plugin) = installed();
    let run = runner(quick());
    let query = Request::Query {
        text: "fix".to_owned(),
        generation: 1,
    };
    let mut session = run
        .open::<Wire, _>(&plugin, Cap::OpenProvider, &query, 0)
        .unwrap();
    let mut rows: Vec<Item> = Vec::new();
    session
        .stream(
            |_| false,
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, RunError<Cap>> {
                match frame.message {
                    Message::Batch(batch) => {
                        rows.extend(batch.items);
                        Ok(ControlFlow::Continue(()))
                    }
                    Message::Complete => Ok(ControlFlow::Break(())),
                    other => Err(RunError::unexpected("demo", &other)),
                }
            },
        )
        .unwrap();
    assert_eq!(rows.len(), 4);
    let first = rows[0].clone();
    for (choice, want) in [
        (
            Choice::Primary,
            Activation::Copy {
                text: first.title.clone(),
            },
        ),
        (
            Choice::Action("open".to_owned()),
            Activation::Open {
                target: OpenTarget::Url("https://example.org/open".to_owned()),
            },
        ),
    ] {
        let activate = Request::Activate {
            item: first.clone(),
            choice,
        };
        let mut session = run
            .open::<Wire, _>(&plugin, Cap::OpenProvider, &activate, 0)
            .unwrap();
        assert_eq!(session.reply().unwrap().message, Message::Activation(want));
    }
}

/// The wire shape of `open.provider` is the launcher's: the same JSON for a row, a choice and an
/// activation, so a launcher adopts a plugin by adding the provider kind to the row and nothing
/// else. The expected texts are what the launcher's own serde types write.
#[test]
fn the_open_provider_wire_is_the_launchers_provider_shape() {
    let launcher_row: Value = serde_json::from_str(
        r#"{"id":"calc:1","provider":"calculator","title":"= 42","subtitle":"","icon":{"kind":"glyph","v":"calculator"},"score":1000,"actions":[{"id":"copy","label":"Copy","keys":{"kind":"none"}}],"subject":{"kind":"none"}}"#,
    )
    .unwrap();
    // A launcher row read as a plugin's: the keys the plugin does not write are ignored.
    let item: Item = serde_json::from_value(launcher_row.clone()).unwrap();
    assert_eq!(
        item,
        Item {
            id: "calc:1".to_owned(),
            title: "= 42".to_owned(),
            subtitle: String::new(),
            icon: Icon::Glyph("calculator".to_owned()),
            score: 1000,
            actions: vec![Action {
                id: "copy".to_owned(),
                label: "Copy".to_owned()
            }],
        }
    );
    // A plugin's row read as a launcher's: the host adds the provider and the row is the same.
    let mut written = serde_json::to_value(&item).unwrap();
    written["provider"] = json!("calculator");
    let mut expected = launcher_row;
    expected.as_object_mut().unwrap().remove("subject");
    expected["actions"][0]
        .as_object_mut()
        .unwrap()
        .remove("keys");
    assert_eq!(written, expected);

    // name, the value, the launcher's text for it
    let pinned: Vec<(&str, Value, &str)> = vec![
        (
            "primary",
            serde_json::to_value(Choice::Primary).unwrap(),
            r#"{"kind":"primary"}"#,
        ),
        (
            "action",
            serde_json::to_value(Choice::Action("copy".to_owned())).unwrap(),
            r#"{"kind":"action","v":"copy"}"#,
        ),
        (
            "copy",
            serde_json::to_value(Activation::Copy {
                text: "42".to_owned(),
            })
            .unwrap(),
            r#"{"kind":"copy","v":{"text":"42"}}"#,
        ),
        (
            "open a file",
            serde_json::to_value(Activation::Open {
                target: OpenTarget::File("/home/p/a b.pdf".to_owned()),
            })
            .unwrap(),
            r#"{"kind":"open","v":{"target":{"kind":"file","v":"/home/p/a b.pdf"}}}"#,
        ),
        (
            "nothing",
            serde_json::to_value(Activation::Nothing).unwrap(),
            r#"{"kind":"nothing"}"#,
        ),
    ];
    for (name, value, text) in pinned {
        assert_eq!(
            value,
            serde_json::from_str::<Value>(text).unwrap(),
            "{name}"
        );
    }
}
