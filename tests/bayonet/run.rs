//! Crash containment: a plugin that hangs, dies, lies or ignores a cancel costs one request and
//! leaves nothing running.

use crate::support::shapes::{Cap, Message, Request, Wire};
use crate::support::{Host, PROVIDES, facts, quick, runner};
use bayonet::run::{RunError, Runner, Timeouts};
use std::ops::ControlFlow;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// The error of asking `row.facts` of the demo plugin started in `mode`, with `timeouts`.
fn failure(mode: &str, timeouts: Timeouts, payload_limit: u64) -> RunError<Cap> {
    let host = Host::new();
    host.install("demo", &["--mode", mode], PROVIDES);
    let plugin = host.plugin("demo");
    let mut session =
        match runner(timeouts).open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), payload_limit) {
            Ok(session) => session,
            Err(error) => return error,
        };
    match session.reply() {
        Err(error) => error,
        Ok(frame) => panic!("{mode}: the plugin answered {frame:?}"),
    }
}

#[test]
fn a_plugin_that_fails_in_some_way_is_an_error_naming_it() {
    let hello_fast = quick().with_hello(Duration::from_millis(300));
    // name, mode, timeouts, payload limit, a check on the error
    type Check = fn(&RunError<Cap>) -> bool;
    let cases: Vec<(&str, &str, Timeouts, u64, Check)> = vec![
        (
            "never greets",
            "mute",
            hello_fast,
            0,
            |e| matches!(e, RunError::Silent { plugin, waited } if plugin == "demo" && *waited == Duration::from_millis(300)),
        ),
        (
            "dies before greeting",
            "crash",
            quick(),
            0,
            |e| matches!(e, RunError::Crashed { status, .. } if status.contains("last said: out of memory")),
        ),
        ("speaks a newer protocol", "old", quick(), 0, |e| {
            matches!(
                e,
                RunError::Version {
                    offered: 9,
                    supported: 1,
                    ..
                }
            )
        }),
        (
            "greets then says nothing",
            "hang",
            quick(),
            0,
            |e| matches!(e, RunError::Silent { waited, .. } if *waited == Duration::from_millis(400)),
        ),
        (
            "announces more payload than allowed",
            "huge",
            quick(),
            1024,
            |e| matches!(e, RunError::Protocol { reason, .. } if reason.contains("exceeds the limit of 1024")),
        ),
    ];
    for (name, mode, timeouts, limit, check) in cases {
        let error = failure(mode, timeouts, limit);
        assert!(check(&error), "{name}: {error:?}");
    }
}

#[test]
fn a_plugin_that_does_not_list_the_capability_is_not_asked() {
    let host = Host::new();
    host.install("demo", &["--mode", "lacks"], PROVIDES);
    let plugin = host.plugin("demo");
    let error = runner(quick())
        .open::<Wire, _>(&plugin, Cap::BlockLens, &facts(), 0)
        .unwrap_err();
    assert_eq!(
        error,
        RunError::Lacks {
            plugin: "demo".to_owned(),
            capability: Cap::BlockLens
        }
    );
}

#[test]
fn a_program_that_cannot_start_is_a_spawn_error() {
    let host = Host::new();
    host.install("demo", &[], PROVIDES);
    let mut plugin = host.plugin("demo");
    if let Some(program) = plugin.manifest.program.as_mut() {
        program.path = "/nonexistent/plugin".into();
    }
    let error = runner(quick())
        .open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), 0)
        .unwrap_err();
    assert!(
        matches!(&error, RunError::Spawn { program, .. } if program == "/nonexistent/plugin"),
        "{error:?}"
    );
}

#[test]
fn a_plugin_that_ignores_a_cancel_is_killed_after_the_grace_period() {
    let host = Host::new();
    host.install("demo", &["--mode", "ignore-cancel"], PROVIDES);
    let plugin = host.plugin("demo");
    let task = Request::Adapt {
        task: "slow".to_owned(),
    };
    let mut session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::AgentAdapter, &task, 0)
        .unwrap();
    let started = Instant::now();
    let error = session
        .stream(
            |_| true,
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, RunError<Cap>> {
                match frame.message {
                    Message::Event { .. } => Ok(ControlFlow::Continue(())),
                    other => Err(RunError::unexpected("demo", &other)),
                }
            },
        )
        .unwrap_err();
    assert_eq!(
        error,
        RunError::Cancelled {
            plugin: "demo".to_owned()
        }
    );
    assert!(
        started.elapsed() >= quick().cancel_grace,
        "the plugin had its grace period"
    );
}

#[test]
fn a_stream_that_reports_progress_may_outlast_the_silence_timeout() {
    let host = Host::new();
    host.install("demo", &[], PROVIDES);
    let plugin = host.plugin("demo");
    let task = Request::Adapt {
        task: "slow".to_owned(),
    };
    let timeouts = quick().with_silence(Duration::from_millis(250));
    let mut session = runner(timeouts)
        .open::<Wire, _>(&plugin, Cap::AgentAdapter, &task, 0)
        .unwrap();
    let started = Instant::now();
    session
        .stream(
            |_| false,
            &Request::Cancel,
            |_, frame| -> Result<ControlFlow<()>, RunError<Cap>> {
                match frame.message {
                    Message::Event { .. } => {
                        // The silence wait starts again with every message; stop after it has
                        // been outlasted twice over.
                        let done = started.elapsed() > timeouts.silence * 2;
                        Ok(if done {
                            ControlFlow::Break(())
                        } else {
                            ControlFlow::Continue(())
                        })
                    }
                    other => Err(RunError::unexpected("demo", &other)),
                }
            },
        )
        .unwrap();
    assert!(
        started.elapsed() > timeouts.silence * 2,
        "the stream outlasted the silence timeout twice over"
    );
}

#[test]
fn dropping_a_session_kills_what_the_plugin_started() {
    let host = Host::new();
    host.install(
        "demo",
        &[
            "--mode",
            "orphan",
            "--pid-file",
            &host.pid_file().display().to_string(),
        ],
        PROVIDES,
    );
    let plugin = host.plugin("demo");
    let session = runner(quick())
        .open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), 0)
        .unwrap();
    let pid = wait_for_pid(&host.pid_file());
    assert!(
        running(&pid),
        "the plugin's own child is alive while the session is"
    );
    drop(session);
    let end = Instant::now() + Duration::from_secs(10);
    while running(&pid) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!running(&pid), "the child died with the plugin's group");
}

fn wait_for_pid(file: &Path) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if let Ok(text) = std::fs::read_to_string(file)
            && !text.is_empty()
        {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the plugin never wrote its child's pid");
}

/// Whether a process with this pid is alive (a zombie awaiting its parent is not).
fn running(pid: &str) -> bool {
    let out = Command::new("ps")
        .args(["-o", "stat=", "-p", pid.trim()])
        .output()
        .expect("ps runs");
    let stat = String::from_utf8_lossy(&out.stdout);
    let stat = stat.trim();
    !stat.is_empty() && !stat.starts_with('Z')
}

#[test]
fn a_log_sink_that_captures_state_receives_the_plugins_stderr_lines() {
    use std::sync::{Arc, Mutex};
    let host = Host::new();
    host.install("demo", &["--mode", "crash"], PROVIDES);
    let plugin = host.plugin("demo");
    let lines = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&lines);
    let runner = Runner::new("myapp", quick()).with_log(move |line| {
        kept.lock().unwrap().push(line.to_owned());
    });
    let error = runner
        .open::<Wire, _>(&plugin, Cap::RowFacts, &facts(), 0)
        .unwrap_err();
    assert!(matches!(error, RunError::Crashed { .. }), "{error:?}");
    assert_eq!(
        *lines.lock().unwrap(),
        vec!["myapp: plugin demo: out of memory".to_owned()]
    );
}
