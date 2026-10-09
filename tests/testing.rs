//! A host's runner against a fake plugin built with `bayonet::testing`: each wire-level fault is
//! the error the runner names, and a host's own fault is expressed in its handler.

#![allow(clippy::unwrap_used)]
// A test handler names the messages it expects and refuses the rest alike.
#![allow(clippy::wildcard_enum_match_arm)]

mod support;

use bayonet::run::{RunError, Timeouts};
use bayonet::testing::Fault;
use std::ops::ControlFlow;
use std::time::Duration;
use support::shapes::{Cap, Message, Request, Wire};
use support::{Host, PROVIDES, example_program, quick, runner};

fn facts() -> Request {
    Request::Facts {
        table: "issues".to_owned(),
        key: "1".to_owned(),
    }
}

fn host_with(fault: &str) -> Host {
    let host = Host::new();
    let args: Vec<&str> = if fault.is_empty() {
        Vec::new()
    } else {
        vec!["--fault", fault]
    };
    host.install_program("fake", &example_program("fake_plugin"), &args, PROVIDES);
    host
}

/// The error of asking `row.facts` of the fake started with `fault`.
fn failure(fault: &str, timeouts: Timeouts, payload_limit: u64) -> RunError<Cap> {
    let host = host_with(fault);
    let plugin = host.plugin("fake");
    let mut session =
        match runner(timeouts).open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), payload_limit) {
            Ok(session) => session,
            Err(error) => return error,
        };
    match session.reply() {
        Err(error) => error,
        Ok(frame) => panic!("{fault}: the fake answered {frame:?}"),
    }
}

#[test]
fn a_fake_without_a_fault_answers_the_request() {
    let host = host_with("");
    let plugin = host.plugin("fake");
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), 0)
        .unwrap();
    let reply = session.reply().unwrap();
    assert!(matches!(reply.message, Message::Facts(rows) if rows.len() == 1));
}

#[test]
fn each_wire_fault_of_the_fake_is_the_error_the_runner_names() {
    type Check = fn(&RunError<Cap>) -> bool;
    let hello_fast = quick().with_hello(Duration::from_millis(300));
    let cases: Vec<(Fault, Timeouts, Check)> = vec![
        (Fault::Mute, hello_fast, |e| {
            matches!(e, RunError::Silent { .. })
        }),
        (Fault::OtherVersion, quick(), |e| {
            matches!(
                e,
                RunError::Version {
                    offered: 2,
                    supported: 1,
                    ..
                }
            )
        }),
        (Fault::ProvidesNothing, quick(), |e| {
            matches!(e, RunError::Lacks { .. })
        }),
        (
            Fault::CrashOnRequest,
            quick(),
            |e| matches!(e, RunError::Crashed { status, .. } if status.contains("last said: fake plugin: crashing on purpose")),
        ),
        (
            Fault::HangOnRequest,
            quick(),
            |e| matches!(e, RunError::Silent { waited, .. } if *waited == Duration::from_millis(400)),
        ),
        (Fault::Garbage, quick(), |e| {
            matches!(e, RunError::Protocol { .. })
        }),
    ];
    for (fault, timeouts, check) in cases {
        let error = failure(fault.name(), timeouts, 0);
        assert!(check(&error), "{}: {error:?}", fault.name());
    }
}

#[test]
fn a_fake_that_floods_stderr_still_answers() {
    let host = host_with(Fault::StderrFlood.name());
    let plugin = host.plugin("fake");
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), 0)
        .unwrap();
    assert!(matches!(
        session.reply().unwrap().message,
        Message::Facts(_)
    ));
}

fn adapt_slow(fault: &str) -> Result<(), RunError<Cap>> {
    let host = host_with(fault);
    let plugin = host.plugin("fake");
    let task = Request::Adapt {
        task: "slow".to_owned(),
    };
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::AgentAdapter, &task, 0)
        .unwrap();
    session.stream(
        |_| true,
        &Request::Cancel,
        |_, frame| -> Result<ControlFlow<()>, RunError<Cap>> {
            match frame.message {
                Message::Event { .. } => Ok(ControlFlow::Continue(())),
                Message::Failed { .. } => Ok(ControlFlow::Break(())),
                other => Err(RunError::unexpected("fake", &other)),
            }
        },
    )
}

#[test]
fn a_fake_stops_on_a_cancel_unless_it_ignores_it() {
    assert_eq!(adapt_slow(""), Ok(()));
    assert_eq!(
        adapt_slow(Fault::IgnoreCancel.name()),
        Err(RunError::Cancelled {
            plugin: "fake".to_owned()
        })
    );
}

#[test]
fn a_fault_only_the_host_knows_is_its_handlers_to_express() {
    let error = adapt_slow("crash-mid-stream").unwrap_err();
    assert!(
        matches!(&error, RunError::Crashed { status, .. } if status.contains("exit status: 4")),
        "{error:?}"
    );
}
