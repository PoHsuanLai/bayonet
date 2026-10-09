//! Framing: a message is a header, its JSON and an optional binary payload.
//!
//! ```text
//! u32 LE json length | u32 LE payload length | JSON | payload
//! ```
//!
//! The payload carries bytes that should not be encoded (pixels, audio), so a 24-megapixel
//! picture crosses the pipe as 96 MB with no encoding on either side.

use super::error::WireError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::io::{Read, Write};

/// The most JSON a message may carry: 1 MiB.
pub const MAX_JSON_BYTES: u32 = 1 << 20;

/// The most payload a message may carry: 512 MiB, 128 megapixels of RGBA8.
pub const MAX_PAYLOAD_BYTES: u32 = 512 << 20;

pub(super) const HEADER: usize = 8;

/// A message and the bytes that came with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<T> {
    /// The decoded message.
    pub message: T,
    /// The payload; empty for a message that has none.
    pub payload: Vec<u8>,
}

/// `message` and `payload` as the bytes of one frame.
pub fn encode_frame<T: Serialize>(message: &T, payload: &[u8]) -> Result<Vec<u8>, WireError> {
    let json = serde_json::to_vec(message).map_err(|error| WireError::Unwritable {
        reason: error.to_string(),
    })?;
    let json_len = checked_json(json.len() as u64)?;
    let payload_len = checked_payload(payload.len() as u64)?;
    let mut bytes = Vec::with_capacity(HEADER + json.len() + payload.len());
    bytes.extend_from_slice(&json_len.to_le_bytes());
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&json);
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

fn checked_json(len: u64) -> Result<u32, WireError> {
    u32::try_from(len)
        .ok()
        .filter(|len| *len <= MAX_JSON_BYTES)
        .ok_or(WireError::JsonTooLarge {
            len,
            limit: u64::from(MAX_JSON_BYTES),
        })
}

fn checked_payload(len: u64) -> Result<u32, WireError> {
    u32::try_from(len)
        .ok()
        .filter(|len| *len <= MAX_PAYLOAD_BYTES)
        .ok_or(WireError::PayloadTooLarge {
            len,
            limit: u64::from(MAX_PAYLOAD_BYTES),
        })
}

/// Writes one frame and flushes: what a plugin's main loop calls.
pub fn write_frame<W: Write, T: Serialize>(
    out: &mut W,
    message: &T,
    payload: &[u8],
) -> Result<(), WireError> {
    let bytes = encode_frame(message, payload)?;
    out.write_all(&bytes)
        .and_then(|()| out.flush())
        .map_err(|error| WireError::Io { kind: error.kind() })
}

/// Reads one frame, blocking: what a plugin's main loop calls. `Closed` means the stream ended
/// between frames.
pub fn read_frame<R: Read, T: DeserializeOwned>(input: &mut R) -> Result<Frame<T>, WireError> {
    let mut header = [0u8; HEADER];
    read_exact(input, &mut header, WireError::Closed)?;
    let (json_len, payload_len) = lengths(&header)?;
    let mut json = vec![0u8; json_len];
    read_exact(input, &mut json, WireError::Truncated)?;
    let mut payload = vec![0u8; payload_len];
    read_exact(input, &mut payload, WireError::Truncated)?;
    Ok(Frame {
        message: parse(&json)?,
        payload,
    })
}

fn read_exact<R: Read>(
    input: &mut R,
    into: &mut [u8],
    on_empty: WireError,
) -> Result<(), WireError> {
    let mut filled = 0;
    while filled < into.len() {
        match input.read(&mut into[filled..]) {
            Ok(0) if filled == 0 => return Err(on_empty),
            Ok(0) => return Err(WireError::Truncated),
            Ok(n) => filled += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(WireError::Io { kind: error.kind() }),
        }
    }
    Ok(())
}

/// The two lengths a header announces, inside the limits.
pub(super) fn lengths(header: &[u8; HEADER]) -> Result<(usize, usize), WireError> {
    let word = |at: usize| {
        u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
    };
    let json = checked_json(u64::from(word(0)))?;
    let payload = checked_payload(u64::from(word(4)))?;
    Ok((json as usize, payload as usize))
}

pub(super) fn parse<T: DeserializeOwned>(json: &[u8]) -> Result<T, WireError> {
    serde_json::from_slice(json).map_err(|error| WireError::Malformed {
        reason: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::tests::Msg;

    #[test]
    fn a_frame_survives_a_blocking_round_trip() {
        let image = Msg::Image {
            width: 2,
            height: 1,
        };
        let mut wire = Vec::new();
        write_frame(&mut wire, &image, &[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        let frame: Frame<Msg> = read_frame(&mut wire.as_slice()).unwrap();
        assert_eq!(frame.message, image);
        assert_eq!(frame.payload, [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn garbage_json_and_cut_streams_are_typed_errors() {
        let mut wire = Vec::new();
        wire.extend_from_slice(&3u32.to_le_bytes());
        wire.extend_from_slice(&0u32.to_le_bytes());
        wire.extend_from_slice(b"{no");
        assert!(matches!(
            read_frame::<_, Msg>(&mut wire.as_slice()),
            Err(WireError::Malformed { .. })
        ));
        let whole = encode_frame(&Msg::Ask { text: "a".into() }, &[]).unwrap();
        assert_eq!(
            read_frame::<_, Msg>(&mut &whole[..whole.len() - 1]).unwrap_err(),
            WireError::Truncated
        );
        assert_eq!(
            read_frame::<_, Msg>(&mut &[][..]).unwrap_err(),
            WireError::Closed
        );
    }

    #[test]
    fn a_message_over_the_limit_is_not_written() {
        let long = Msg::Ask {
            text: "x".repeat(MAX_JSON_BYTES as usize),
        };
        assert!(matches!(
            encode_frame(&long, &[]),
            Err(WireError::JsonTooLarge { .. })
        ));
    }

    /// The wire format as bytes: two little-endian u32 lengths (JSON, then payload), the JSON,
    /// the payload. Other programs read these bytes, so they are written out and not computed.
    #[test]
    fn a_frame_is_exactly_these_bytes_on_the_wire() {
        const FRAME: &[u8] =
            b"\x20\0\0\0\x03\0\0\0{\"kind\":\"ask\",\"v\":{\"text\":\"hi\"}}\xde\xad\xbe";
        let ask = Msg::Ask { text: "hi".into() };
        assert_eq!(encode_frame(&ask, &[0xde, 0xad, 0xbe]).unwrap(), FRAME);
        let frame: Frame<Msg> = read_frame(&mut &FRAME[..]).unwrap();
        assert_eq!(frame.message, ask);
        assert_eq!(frame.payload, [0xde, 0xad, 0xbe]);
    }
}
