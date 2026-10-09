//! The fake served in memory: no process, no pipe.

use super::*;
use crate::wire::{Frame, WireError, encode_frame, read_frame};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Say {
    Hello { protocol: u32, provides: Vec<u8> },
    Pong(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Ask {
    Ping(u8),
    Cancel,
}

fn fake(fault: Option<Fault>) -> Fake<u8, Say, Ask> {
    Fake::new(
        3,
        vec![1, 2],
        |protocol, provides| Say::Hello {
            protocol,
            provides: provides.to_vec(),
        },
        |ask| matches!(ask, Ask::Cancel),
    )
    .with_fault(fault)
}

/// The requests as the bytes a host would write.
fn requests(asks: &[Ask]) -> Cursor<Vec<u8>> {
    let mut bytes = Vec::new();
    for ask in asks {
        bytes.extend(encode_frame(ask, &[]).unwrap());
    }
    Cursor::new(bytes)
}

/// Serves `asks`, answering a ping with a pong, and returns the ending, what was written to
/// stdout and what to stderr.
fn serve(fault: Option<Fault>, asks: &[Ask]) -> (Ending, Vec<u8>, Vec<u8>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let ending = fake(fault)
        .serve(requests(asks), &mut out, &mut err, |ask, conversation| {
            if let Ask::Ping(n) = ask {
                conversation.say(&Say::Pong(n), &[])?;
            }
            Ok(Step::Next)
        })
        .unwrap();
    (ending, out, err)
}

fn said(mut bytes: &[u8]) -> Vec<Say> {
    let mut all = Vec::new();
    while let Ok(Frame { message, .. }) = read_frame::<_, Say>(&mut bytes) {
        all.push(message);
    }
    all
}

fn hello(protocol: u32, provides: &[u8]) -> Say {
    Say::Hello {
        protocol,
        provides: provides.to_vec(),
    }
}

#[test]
fn a_fault_name_round_trips_and_a_strange_one_is_none() {
    for fault in Fault::ALL {
        assert_eq!(Fault::from_name(fault.name()), Some(fault), "{fault:?}");
    }
    assert_eq!(Fault::from_name("short-picture"), None);
}

#[test]
fn the_fault_argument_is_the_word_after_the_flag() {
    let args = |words: &[&str]| {
        words
            .iter()
            .map(|word| (*word).to_owned())
            .collect::<Vec<_>>()
    };
    // name, the arguments, the fault named
    let cases: Vec<(&str, Vec<String>, Option<&str>)> = vec![
        ("none", args(&[]), None),
        ("named", args(&["--fault", "mute"]), Some("mute")),
        ("after another", args(&["-v", "--fault", "x"]), Some("x")),
        ("flag last", args(&["--fault"]), None),
    ];
    for (name, given, want) in cases {
        assert_eq!(fault_argument(given).as_deref(), want, "{name}");
    }
}

#[test]
fn a_well_behaved_fake_greets_and_answers_each_request() {
    let (ending, out, err) = serve(None, &[Ask::Ping(7), Ask::Ping(8)]);
    assert_eq!(ending, Ending::Finished);
    assert_eq!(
        said(&out),
        vec![hello(3, &[1, 2]), Say::Pong(7), Say::Pong(8)]
    );
    assert!(err.is_empty());
}

#[test]
fn each_fault_changes_what_the_fake_writes() {
    // name, the fault, the ending, the messages written, the stderr's length, and its bytes where
    // they are pinned
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Fault, Ending, Vec<Say>, usize, Option<&[u8]>)> = vec![
        ("mute", Fault::Mute, Ending::Mute, vec![], 0, None),
        (
            "other version",
            Fault::OtherVersion,
            Ending::Finished,
            vec![hello(4, &[1, 2]), Say::Pong(1)],
            0,
            None,
        ),
        (
            "provides nothing",
            Fault::ProvidesNothing,
            Ending::Finished,
            vec![hello(3, &[]), Say::Pong(1)],
            0,
            None,
        ),
        (
            "hang",
            Fault::HangOnRequest,
            Ending::Hung,
            vec![hello(3, &[1, 2])],
            0,
            None,
        ),
        (
            "crash: its line, then the code",
            Fault::CrashOnRequest,
            Ending::Crashed {
                code: 3,
                said: "fake plugin: crashing on purpose".to_owned(),
            },
            vec![hello(3, &[1, 2])],
            33,
            Some(b"fake plugin: crashing on purpose\n"),
        ),
        (
            "flood",
            Fault::StderrFlood,
            Ending::Finished,
            vec![hello(3, &[1, 2]), Say::Pong(1)],
            1 << 20,
            None,
        ),
    ];
    for (name, fault, ending, messages, stderr_len, stderr) in cases {
        let (got, out, err) = serve(Some(fault), &[Ask::Ping(1)]);
        assert_eq!(got, ending, "{name}");
        assert_eq!(said(&out), messages, "{name}");
        assert_eq!(err.len(), stderr_len, "{name}: stderr length");
        if let Some(stderr) = stderr {
            assert_eq!(err, stderr, "{name}: stderr");
        }
    }
}

#[test]
fn a_garbage_fake_writes_bytes_a_reader_refuses() {
    let (_, out, _) = serve(Some(Fault::Garbage), &[Ask::Ping(1)]);
    let mut after_hello = &out[encode_frame(&hello(3, &[1, 2]), &[]).unwrap().len()..];
    let error = read_frame::<_, Say>(&mut after_hello).unwrap_err();
    assert!(matches!(error, WireError::Malformed { .. }), "{error:?}");
}

/// Serves a ping, a cancel and a second ping; the first ping's handler polls `cancelled` for
/// `wait` and the result is what it saw, with the pongs written.
fn poll_for_cancel(fault: Option<Fault>, wait: Duration) -> (bool, Vec<Say>) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let asks = [Ask::Ping(1), Ask::Cancel, Ask::Ping(2)];
    let mut seen = false;
    fake(fault)
        .serve(requests(&asks), &mut out, &mut err, |ask, conversation| {
            if ask == Ask::Ping(1) {
                let end = Instant::now() + wait;
                while !seen && Instant::now() < end {
                    seen = conversation.cancelled();
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            if let Ask::Ping(n) = ask {
                conversation.say(&Say::Pong(n), &[])?;
            }
            Ok(Step::Next)
        })
        .unwrap();
    (seen, said(&out)[1..].to_vec())
}

#[test]
fn a_cancel_is_seen_by_a_fake_that_listens_and_never_by_one_that_ignores_it() {
    // name, the fault, how long the handler polls, whether it sees the cancel
    let cases = [
        ("a cancel that arrives while working", None, 5_000, true),
        (
            "a fake that ignores cancel",
            Some(Fault::IgnoreCancel),
            200,
            false,
        ),
    ];
    for (name, fault, wait_ms, want) in cases {
        let (seen, pongs) = poll_for_cancel(fault, Duration::from_millis(wait_ms));
        assert_eq!(seen, want, "{name}: seen");
        // Either way the next request is kept.
        assert_eq!(pongs, vec![Say::Pong(1), Say::Pong(2)], "{name}: pongs");
    }
}
