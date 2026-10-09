//! The ways a fake plugin misbehaves on the wire, and the argument that asks for one.

/// A way a plugin goes wrong that needs nothing of the host's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Fault {
    /// It never says hello.
    Mute,
    /// It greets with a protocol version one past the one it speaks.
    OtherVersion,
    /// It greets and lists nothing it can do.
    ProvidesNothing,
    /// It writes a line to stderr and exits with status 3 when the first request arrives.
    CrashOnRequest,
    /// It reads the first request and never answers.
    HangOnRequest,
    /// It answers the first request with bytes that are not a frame.
    Garbage,
    /// It writes a megabyte to stderr with no newline, then serves as usual.
    StderrFlood,
    /// It does not see a cancel: [`Conversation::cancelled`](super::Conversation::cancelled) never
    /// says yes.
    IgnoreCancel,
}

impl Fault {
    /// Every fault, in the order of the variants.
    pub const ALL: [Fault; 8] = [
        Fault::Mute,
        Fault::OtherVersion,
        Fault::ProvidesNothing,
        Fault::CrashOnRequest,
        Fault::HangOnRequest,
        Fault::Garbage,
        Fault::StderrFlood,
        Fault::IgnoreCancel,
    ];

    /// The name the command line spells the fault with.
    pub const fn name(self) -> &'static str {
        match self {
            Fault::Mute => "mute",
            Fault::OtherVersion => "other-version",
            Fault::ProvidesNothing => "provides-nothing",
            Fault::CrashOnRequest => "crash-on-request",
            Fault::HangOnRequest => "hang-on-request",
            Fault::Garbage => "garbage",
            Fault::StderrFlood => "stderr-flood",
            Fault::IgnoreCancel => "ignore-cancel",
        }
    }

    /// The fault called `name`, or `None` for a name that is not one of these (a host's own fault,
    /// for instance).
    pub fn from_name(name: &str) -> Option<Fault> {
        Fault::ALL.into_iter().find(|fault| fault.name() == name)
    }
}

/// The value of `--fault <name>` among `args`, which are the program's arguments without its own
/// name. A host reads it once, tries [`Fault::from_name`], and takes the name for one of its own
/// faults when that finds nothing.
pub fn fault_argument(args: impl IntoIterator<Item = String>) -> Option<String> {
    let mut args = args.into_iter();
    args.find(|arg| arg == "--fault")?;
    args.next()
}
