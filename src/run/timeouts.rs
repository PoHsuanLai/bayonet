//! How long the host waits on a plugin.

use std::time::Duration;

/// How long the host waits on a plugin. Start from [`Timeouts::default`] and set what differs
/// with the `with_` methods; fields may be added without breaking a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Timeouts {
    /// From starting the plugin to its greeting.
    pub hello: Duration,
    /// The longest a plugin may say nothing while a request is open; each message starts the
    /// wait again, so a stream that reports progress may run as long as it likes.
    pub silence: Duration,
    /// After the cancel request, how long the plugin has to stop before it is killed.
    pub cancel_grace: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Timeouts {
            hello: Duration::from_secs(5),
            silence: Duration::from_secs(30),
            cancel_grace: Duration::from_secs(2),
        }
    }
}

impl Timeouts {
    /// The same with `hello` as the wait for the greeting.
    pub fn with_hello(self, hello: Duration) -> Timeouts {
        Timeouts { hello, ..self }
    }

    /// The same with `silence` as the longest a plugin may say nothing.
    pub fn with_silence(self, silence: Duration) -> Timeouts {
        Timeouts { silence, ..self }
    }

    /// The same with `cancel_grace` as the time a plugin has to stop after a cancel.
    pub fn with_cancel_grace(self, cancel_grace: Duration) -> Timeouts {
        Timeouts {
            cancel_grace,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_with_method_changes_its_own_wait_only() {
        let base = Timeouts::default();
        let one = Duration::from_secs(1);
        assert_eq!(base.with_hello(one), Timeouts { hello: one, ..base });
        assert_eq!(base.with_silence(one).silence, one);
        assert_eq!(base.with_cancel_grace(one).cancel_grace, one);
        assert_eq!(base.with_silence(one).hello, base.hello);
    }
}
