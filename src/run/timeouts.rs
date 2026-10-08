//! How long the host waits on a plugin.

use std::time::Duration;

/// How long the host waits on a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
