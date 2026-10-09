//! A fake plugin built on `bayonet::testing`, for the tests of the demo host's runner: eleven
//! lines of host code plus one fault of the host's own.
//!
//! ```text
//! fake_plugin [--fault <name>]
//! ```
//!
//! The names are those of `bayonet::testing::Fault`, and `crash-mid-stream`, which only this host
//! knows how to do: two events, then it dies.

// `shapes` is also read by the host side of the tests, which use what this program does not.
#[allow(dead_code)]
#[path = "shapes/mod.rs"]
mod shapes;

use bayonet::testing::{Ending, Fake, Fault, Step, fault_argument};
use shapes::{Cap, Message, Request};
use std::process::ExitCode;
use std::time::Duration;

const TICK: Duration = Duration::from_millis(20);

fn main() -> ExitCode {
    let name = fault_argument(std::env::args().skip(1));
    let fault = name.as_deref().and_then(Fault::from_name);
    let crashes_mid_stream = name.as_deref() == Some("crash-mid-stream");
    let fake = Fake::new(
        1,
        vec![Cap::RowFacts, Cap::AgentAdapter],
        |protocol, provides| Message::Hello {
            protocol,
            provides: provides.to_vec(),
        },
        |request| matches!(request, Request::Cancel),
    )
    .with_fault(fault);
    fake.run(|request, conversation| {
        match request {
            Request::Facts { table, key } => {
                conversation.say(
                    &Message::Facts(vec![shapes::Fact {
                        label: table,
                        value: key,
                    }]),
                    &[],
                )?;
            }
            Request::Adapt { .. } => loop {
                conversation.say(
                    &Message::Event {
                        text: "working".to_owned(),
                    },
                    &[],
                )?;
                if crashes_mid_stream {
                    return Ok(Step::End(Ending::Crashed {
                        code: 4,
                        said: "the fake crashed mid stream".to_owned(),
                    }));
                }
                if conversation.cancelled() {
                    conversation.say(
                        &Message::Failed {
                            message: "cancelled".to_owned(),
                        },
                        &[],
                    )?;
                    break;
                }
                std::thread::sleep(TICK);
            },
            Request::Lens(_)
            | Request::Query { .. }
            | Request::Activate { .. }
            | Request::Cancel => {}
        }
        Ok(Step::End(Ending::Finished))
    })
}
