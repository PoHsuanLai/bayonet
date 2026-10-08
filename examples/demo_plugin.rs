//! A plugin program for bayonet's tests and the shortest example of one: it answers the four
//! capabilities of `shapes`, and misbehaves on request, for the host's tests of timeouts,
//! crashes and cancellation.
//!
//! ```text
//! demo_plugin [--mode normal|mute|crash|old|lacks|hang|huge|orphan|ignore-cancel] [--pid-file PATH]
//! ```

// `shapes` is also read by the host side of the tests, which use what this program does not.
#[allow(dead_code)]
#[path = "shapes/mod.rs"]
mod shapes;

use bayonet::wire::{WireError, read_frame, write_frame};
use shapes::{
    Action, Activation, Batch, Cap, Choice, Fact, Icon, Item, Message, OpenTarget, Request,
};
use std::io::Write;
use std::process::{Command, ExitCode};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const TICK: Duration = Duration::from_millis(20);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let mode = flag("--mode").unwrap_or_else(|| "normal".to_owned());
    match run(&mode, flag("--pid-file")) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("demo_plugin: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(mode: &str, pid_file: Option<String>) -> Result<(), WireError> {
    match mode {
        "mute" => sleep_forever(),
        "crash" => {
            eprintln!("out of memory");
            std::process::exit(3);
        }
        _ => {}
    }
    let (protocol, provides) = match mode {
        "old" => (9, vec![Cap::RowFacts]),
        "lacks" => (1, vec![Cap::RowFacts]),
        _ => (
            1,
            vec![
                Cap::BlockLens,
                Cap::OpenProvider,
                Cap::AgentAdapter,
                Cap::RowFacts,
            ],
        ),
    };
    say(&Message::Hello { protocol, provides }, &[])?;
    let requests = read_requests();
    let Ok(first) = requests.recv() else {
        return Ok(());
    };
    match mode {
        "hang" => sleep_forever(),
        "orphan" => {
            // A program of its own that outlives this one unless the host kills the group.
            if let (Ok(child), Some(file)) = (Command::new("sleep").arg("60").spawn(), pid_file) {
                let _ = std::fs::write(file, child.id().to_string());
            }
            sleep_forever()
        }
        "huge" => {
            let mut header = Vec::new();
            header.extend_from_slice(&2u32.to_le_bytes());
            header.extend_from_slice(&(64u32 << 20).to_le_bytes());
            header.extend_from_slice(b"{}");
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(&header).and_then(|()| out.flush());
            sleep_forever()
        }
        _ => {}
    }
    let honours_cancel = mode != "ignore-cancel";
    serve(first, &requests, honours_cancel)
}

fn serve(
    request: Request,
    requests: &Receiver<Request>,
    honours_cancel: bool,
) -> Result<(), WireError> {
    match request {
        Request::Lens(block) => {
            let width = u32::try_from(block.text.len().max(1)).unwrap_or(1);
            let byte = block.kind.bytes().next().unwrap_or(0);
            let pixels = vec![byte; width as usize * 4];
            say(&Message::Drawn { width, height: 1 }, &pixels)
        }
        Request::Query { text, generation } => {
            if text.starts_with("slow") {
                let mut batch = 0;
                return until_cancelled(requests, honours_cancel, |_| {
                    batch += 1;
                    say(&Message::Batch(rows(generation, batch, &text)), &[])
                });
            }
            for batch in 1..=2 {
                say(&Message::Batch(rows(generation, batch, &text)), &[])?;
            }
            say(&Message::Complete, &[])
        }
        Request::Activate { item, choice } => {
            let activation = match choice {
                Choice::Primary => Activation::Copy { text: item.title },
                Choice::Action(id) => Activation::Open {
                    target: OpenTarget::Url(format!("https://example.org/{id}")),
                },
            };
            say(&Message::Activation(activation), &[])
        }
        Request::Adapt { task } => {
            if task.starts_with("slow") {
                return until_cancelled(requests, honours_cancel, |_| {
                    say(
                        &Message::Event {
                            text: "working".to_owned(),
                        },
                        &[],
                    )
                });
            }
            for step in 1..=3 {
                say(
                    &Message::Event {
                        text: format!("{task}: step {step}"),
                    },
                    &[],
                )?;
            }
            say(&Message::Finished, &[])
        }
        Request::Facts { table, key } => say(
            &Message::Facts(vec![Fact {
                label: table,
                value: key,
            }]),
            &[],
        ),
        Request::Cancel => say(
            &Message::Failed {
                message: "nothing to cancel".to_owned(),
            },
            &[],
        ),
    }
}

/// Calls `step` every tick until a `Cancel` arrives, which it answers with `Failed`; a plugin that
/// ignores cancels goes on until the host kills it.
fn until_cancelled(
    requests: &Receiver<Request>,
    honours_cancel: bool,
    mut step: impl FnMut(u32) -> Result<(), WireError>,
) -> Result<(), WireError> {
    let mut count = 0;
    loop {
        if honours_cancel && matches!(requests.try_recv(), Ok(Request::Cancel)) {
            return say(
                &Message::Failed {
                    message: "cancelled".to_owned(),
                },
                &[],
            );
        }
        count += 1;
        step(count)?;
        std::thread::sleep(TICK);
    }
}

fn rows(generation: u64, batch: u32, text: &str) -> Batch {
    let items = (0..2)
        .map(|n| Item {
            id: format!("row:{batch}:{n}"),
            title: format!("{text} {batch}.{n}"),
            subtitle: String::new(),
            icon: Icon::Glyph("search".to_owned()),
            score: 1000 - batch * 10 - n,
            actions: vec![Action {
                id: "open".to_owned(),
                label: "Open".to_owned(),
            }],
        })
        .collect();
    Batch { generation, items }
}

/// Reads requests on a thread of their own, so a cancel is seen while a stream is running.
fn read_requests() -> Receiver<Request> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        while let Ok(frame) = read_frame::<_, Request>(&mut input) {
            if send.send(frame.message).is_err() {
                break;
            }
        }
    });
    receive
}

fn say(message: &Message, payload: &[u8]) -> Result<(), WireError> {
    write_frame(&mut std::io::stdout().lock(), message, payload)
}

fn sleep_forever() -> ! {
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
