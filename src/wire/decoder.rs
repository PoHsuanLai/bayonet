//! Cutting frames out of bytes as they arrive.

use super::error::WireError;
use super::frame::{Frame, HEADER, MAX_PAYLOAD_BYTES, lengths, parse};
use serde::de::DeserializeOwned;

/// Cuts frames out of bytes as they arrive, for a reader that must not block (the host polls the
/// pipe so it can time out). Push what was read, then pull frames until `None`.
#[derive(Debug, Clone, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    /// A decoder with nothing buffered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds bytes that arrived.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The next whole frame, or `None` until more bytes arrive. A header over the limits or JSON
    /// that is not a `T` is an error, and the decoder is then spent.
    pub fn next_frame<T: DeserializeOwned>(&mut self) -> Result<Option<Frame<T>>, WireError> {
        self.next_frame_within(u64::from(MAX_PAYLOAD_BYTES))
    }

    /// [`next_frame`](Self::next_frame) for a reply that may carry at most `max_payload` bytes of
    /// payload: a header that announces more is refused as soon as it is parsed, before any of the
    /// payload is buffered, so a plugin cannot make the host hold what it will then reject.
    pub fn next_frame_within<T: DeserializeOwned>(
        &mut self,
        max_payload: u64,
    ) -> Result<Option<Frame<T>>, WireError> {
        let Some(header) = self.buffer.first_chunk::<HEADER>() else {
            return Ok(None);
        };
        let (json_len, payload_len) = lengths(header)?;
        if payload_len as u64 > max_payload {
            return Err(WireError::PayloadTooLarge {
                len: payload_len as u64,
                limit: max_payload,
            });
        }
        let total = HEADER + json_len + payload_len;
        if self.buffer.len() < total {
            return Ok(None);
        }
        let message = parse(&self.buffer[HEADER..HEADER + json_len])?;
        // Whatever follows this frame stays; the frame's own bytes become its payload.
        let rest = self.buffer.split_off(total);
        let mut payload = std::mem::replace(&mut self.buffer, rest);
        payload.drain(..HEADER + json_len);
        Ok(Some(Frame { message, payload }))
    }

    /// Whether bytes of an unfinished frame are held: a stream that ends now was cut off.
    pub fn is_mid_frame(&self) -> bool {
        !self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::frame::{MAX_JSON_BYTES, encode_frame};
    use crate::wire::tests::Msg;

    fn ask() -> Msg {
        Msg::Ask { text: "a".into() }
    }

    #[test]
    fn the_decoder_waits_for_every_byte_and_cuts_back_to_back_frames() {
        let one = encode_frame(&ask(), &[]).unwrap();
        let two = encode_frame(&ask(), &[9; 5]).unwrap();
        let wire = [one.clone(), two].concat();
        let mut decoder = FrameDecoder::new();
        // One byte at a time: nothing is a frame until its last byte arrives.
        let mut got = Vec::new();
        for (at, byte) in wire.iter().enumerate() {
            decoder.push(&[*byte]);
            while let Some(frame) = decoder.next_frame::<Msg>().unwrap() {
                got.push((at + 1, frame.payload.len()));
            }
        }
        assert_eq!(got, [(one.len(), 0), (wire.len(), 5)]);
        assert!(!decoder.is_mid_frame());
    }

    #[test]
    fn a_header_over_the_limits_is_refused_before_buffering_its_body() {
        const CASES: &[(&str, u32, u32)] = &[
            ("json", MAX_JSON_BYTES + 1, 0),
            ("payload", 2, MAX_PAYLOAD_BYTES + 1),
        ];
        for (name, json, payload) in CASES {
            let mut header = Vec::new();
            header.extend_from_slice(&json.to_le_bytes());
            header.extend_from_slice(&payload.to_le_bytes());
            let mut decoder = FrameDecoder::new();
            decoder.push(&header);
            assert!(decoder.next_frame::<Msg>().is_err(), "{name}");
        }
    }

    #[test]
    fn a_header_over_the_replys_own_limit_is_refused_with_no_payload_buffered() {
        let mut header = Vec::new();
        header.extend_from_slice(&2u32.to_le_bytes());
        header.extend_from_slice(&(64u32 << 20).to_le_bytes());
        let mut decoder = FrameDecoder::new();
        decoder.push(&header);
        assert_eq!(
            decoder.next_frame_within::<Msg>(4).unwrap_err(),
            WireError::PayloadTooLarge {
                len: 64 << 20,
                limit: 4
            }
        );
        assert!(decoder.next_frame::<Msg>().unwrap().is_none());
    }
}
