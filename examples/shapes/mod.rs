//! A host with four capabilities, shared by the demo plugin and the tests: the shapes of
//! `block.lens`, `open.provider`, `agent.adapter` and `row.facts`, written on bayonet's generic
//! API. `open.provider` carries the wire shape of a launcher provider (`Query` in, `Batch` out,
//! `Choice` in, `Activation` out), so one plugin can feed a launcher and a document host alike.

use bayonet::manifest::{Entry, Provides, Refusal};
use bayonet::run::{Greeting, Protocol};
use serde::{Deserialize, Serialize};

/// The four things a plugin of this host can offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cap {
    /// Draws a block of a kind the host cannot draw itself.
    #[serde(rename = "block.lens")]
    BlockLens,
    /// Answers a typed query with rows, and says what choosing one does.
    #[serde(rename = "open.provider")]
    OpenProvider,
    /// Runs a task on behalf of an agent and reports events as they happen.
    #[serde(rename = "agent.adapter")]
    AgentAdapter,
    /// Adds facts to a row of a table.
    #[serde(rename = "row.facts")]
    RowFacts,
}

/// One `[[provides]]` entry: the capability and what it applies to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provision {
    /// The block kinds it draws.
    BlockLens(Vec<String>),
    /// The word that routes a query to it (`gh `), empty for every query.
    OpenProvider(String),
    /// The agent it adapts.
    AgentAdapter(String),
    /// The tables whose rows it knows.
    RowFacts(Vec<String>),
}

/// Why this host refuses an entry.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Fault {
    /// A lens or a facts entry that applies to nothing.
    #[error("the capability {0:?} applies to nothing")]
    AppliesToNothing(Cap),
}

#[derive(Deserialize)]
struct Raw {
    capability: Cap,
    #[serde(default)]
    kinds: Vec<String>,
    #[serde(default)]
    tables: Vec<String>,
    #[serde(default)]
    trigger: String,
    #[serde(default)]
    agent: String,
}

impl Provides for Provision {
    type Capability = Cap;
    type Fault = Fault;

    fn parse(entry: Entry) -> Result<Provision, Refusal<Fault>> {
        let raw: Raw = entry.read()?;
        let nothing = Refusal::Fault(Fault::AppliesToNothing(raw.capability));
        match raw.capability {
            Cap::BlockLens if raw.kinds.is_empty() => Err(nothing),
            Cap::BlockLens => Ok(Provision::BlockLens(raw.kinds)),
            Cap::OpenProvider => Ok(Provision::OpenProvider(raw.trigger)),
            Cap::AgentAdapter => Ok(Provision::AgentAdapter(raw.agent)),
            Cap::RowFacts if raw.tables.is_empty() => Err(nothing),
            Cap::RowFacts => Ok(Provision::RowFacts(raw.tables)),
        }
    }

    fn capability(&self) -> Cap {
        match self {
            Provision::BlockLens(_) => Cap::BlockLens,
            Provision::OpenProvider(_) => Cap::OpenProvider,
            Provision::AgentAdapter(_) => Cap::AgentAdapter,
            Provision::RowFacts(_) => Cap::RowFacts,
        }
    }
}

/// A block of a document: its kind and its source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// The kind the lens was chosen for.
    pub kind: String,
    /// The block's source.
    pub text: String,
}

/// What was chosen on a row, as a launcher's provider receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Choice {
    /// Enter or click.
    Primary,
    /// One of the row's own actions, by id.
    Action(String),
}

/// A row's icon, as a launcher's provider declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Icon {
    /// No icon.
    None,
    /// A file.
    File(String),
    /// A named glyph.
    Glyph(String),
}

/// A secondary action of a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// The action's id, handed back in [`Choice::Action`].
    pub id: String,
    /// What the menu says.
    pub label: String,
}

/// One result row. A launcher's own row has two more fields, which the host fills: the kind of
/// provider that produced it, and what the row is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Identity within the provider, stable across keystrokes.
    pub id: String,
    /// The row's title.
    pub title: String,
    /// The faint second line; empty for none.
    pub subtitle: String,
    /// Where the icon comes from.
    pub icon: Icon,
    /// The provider's rank for the row, in thousandths; higher sorts first.
    pub score: u32,
    /// Secondary actions, in menu order.
    pub actions: Vec<Action>,
}

/// Some results for one query generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    /// The keystroke the batch answers; a newer one makes it stale.
    pub generation: u64,
    /// The rows.
    pub items: Vec<Item>,
}

/// What choosing a row asks the host to do: a closed vocabulary the host matches exhaustively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Activation {
    /// Put text on the clipboard.
    Copy {
        /// The text.
        text: String,
    },
    /// Open an address.
    Open {
        /// What to open.
        target: OpenTarget,
    },
    /// Nothing to do.
    Nothing,
}

/// What an [`Activation::Open`] opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum OpenTarget {
    /// A file or folder.
    File(String),
    /// An address.
    Url(String),
}

/// One fact about a row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// The label.
    pub label: String,
    /// The value, formatted for a person.
    pub value: String,
}

/// What the host says to a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Request {
    /// `block.lens`: draw this block.
    Lens(Block),
    /// `open.provider`: results for the text typed.
    Query {
        /// The text as typed.
        text: String,
        /// The keystroke.
        generation: u64,
    },
    /// `open.provider`: what choosing `choice` on `item` does.
    Activate {
        /// The row.
        item: Item,
        /// What was chosen on it.
        choice: Choice,
    },
    /// `agent.adapter`: run this task.
    Adapt {
        /// What the agent asked for.
        task: String,
    },
    /// `row.facts`: facts about a row of a table.
    Facts {
        /// The table.
        table: String,
        /// The row's key.
        key: String,
    },
    /// Stop what is running.
    Cancel,
}

/// What a plugin says to the host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Message {
    /// The first message.
    Hello {
        /// The protocol version.
        protocol: u32,
        /// The capabilities it answers.
        provides: Vec<Cap>,
    },
    /// `block.lens`: the drawing is the frame's payload, RGBA8.
    Drawn {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// `open.provider`: some results.
    Batch(Batch),
    /// `open.provider`: the query is answered.
    Complete,
    /// `open.provider`: the answer to `Activate`.
    Activation(Activation),
    /// `agent.adapter`: something happened.
    Event {
        /// What a person can read.
        text: String,
    },
    /// `row.facts`: the facts.
    Facts(Vec<Fact>),
    /// `agent.adapter`: the task is finished.
    Finished,
    /// The request failed or was cancelled.
    Failed {
        /// What a person can read.
        message: String,
    },
}

/// The host's protocol.
#[derive(Debug, Clone, Copy)]
pub struct Wire;

impl Protocol for Wire {
    type Capability = Cap;
    type Request = Request;
    type Message = Message;
    const VERSION: u32 = 1;

    fn greeting(message: &Message) -> Option<Greeting<'_, Cap>> {
        match message {
            Message::Hello { protocol, provides } => Some(Greeting {
                protocol: *protocol,
                provides,
            }),
            Message::Drawn { .. }
            | Message::Batch(_)
            | Message::Complete
            | Message::Activation(_)
            | Message::Event { .. }
            | Message::Facts(_)
            | Message::Finished
            | Message::Failed { .. } => None,
        }
    }
}
