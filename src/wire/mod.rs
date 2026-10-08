//! The wire: length-prefixed JSON frames with an optional binary payload.
//!
//! This is all a plugin program needs. A plugin reads the host's requests with [`read_frame`]
//! and answers with [`write_frame`] on its standard streams; the host cuts replies out of a
//! non-blocking pipe with [`FrameDecoder`].

mod decoder;
mod error;
mod frame;

pub use decoder::FrameDecoder;
pub use error::WireError;
pub use frame::{Frame, MAX_JSON_BYTES, MAX_PAYLOAD_BYTES, encode_frame, read_frame, write_frame};

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    /// The message type the wire tests move.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", content = "v", rename_all = "snake_case")]
    pub(crate) enum Msg {
        Ask { text: String },
        Image { width: u32, height: u32 },
    }
}
