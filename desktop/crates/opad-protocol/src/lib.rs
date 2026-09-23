pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/osupad.rs"));
}

use bytes::{Buf, BytesMut};
use prost::Message;
use thiserror::Error;

/// Frame start marker of the current framing, `[0xAA, 0x55, len u16-LE, payload]`.
/// A parser that finds anything else slides forward one byte at a time until it
/// sees a frame start again, so stray bytes (ROM bootloader chatter, a text
/// command, a dropped byte) cost at most the frame they land in.
pub const FRAME_MAGIC: [u8; 2] = [0xAA, 0x55];
/// The pad protocol version (HelloAck `protocol_version`) this host speaks.
/// A pad reporting another one is left alone as incompatible.
pub const DEVICE_PROTOCOL_VERSION: u32 = 1;
pub const HEADER_BYTES: usize = 4;
/// Matches PROTOCOL_MAX_FRAME_SIZE (8192) on the device minus the header
pub const MAX_PAYLOAD_BYTES: usize = 8192 - HEADER_BYTES;
/// Every real message carries at least a sequence number or a payload tag.
/// Only legacy headers are held to it: it keeps runs of zero bytes from
/// reading as empty legacy frames.
const LEGACY_MIN_PAYLOAD: usize = 2;

/// How frames are delimited on the wire.
///
/// Pads and apps are updated independently, so both sides speak both: the
/// pad answers in the framing the host used, and the host tries each until
/// the pad answers. They cannot be confused — a legacy header starting AA 55
/// would claim at least 0x55AA bytes, more than any frame may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    /// `[0xAA, 0x55, len u16-LE, payload]`
    Marked,
    /// `[len u32-LE, payload]`, firmware and apps from before the marker
    Legacy,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Frame length {0} exceeds maximum limit of {MAX_PAYLOAD_BYTES} bytes")]
    FrameTooLarge(usize),
    #[error("Protobuf decode error: {0}")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Protobuf encode error: {0}")]
    EncodeError(#[from] prost::EncodeError),
}

fn encode_frame<M: Message>(msg: &M, framing: Framing) -> Result<Vec<u8>, ProtocolError> {
    let payload_len = msg.encoded_len();
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    let mut buf = Vec::with_capacity(HEADER_BYTES + payload_len);
    match framing {
        Framing::Marked => {
            buf.extend_from_slice(&FRAME_MAGIC);
            buf.extend_from_slice(&(payload_len as u16).to_le_bytes());
        }
        Framing::Legacy => buf.extend_from_slice(&(payload_len as u32).to_le_bytes()),
    }
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// The framing and payload length of the header at the front of `buf`, if it
/// is a plausible one that `accept` allows. `buf` holds at least 4 bytes.
fn header(buf: &[u8], accept: Option<Framing>) -> Option<(Framing, usize)> {
    if buf[..2] == FRAME_MAGIC {
        let len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
        let ok = accept != Some(Framing::Legacy) && len <= MAX_PAYLOAD_BYTES;
        return ok.then_some((Framing::Marked, len));
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let ok =
        accept != Some(Framing::Marked) && (LEGACY_MIN_PAYLOAD..=MAX_PAYLOAD_BYTES).contains(&len);
    ok.then_some((Framing::Legacy, len))
}

/// Drops bytes until `buf` starts with a plausible header, or is too short to
/// tell. Returns that header.
fn resync(buf: &mut BytesMut, accept: Option<Framing>) -> Option<(Framing, usize)> {
    loop {
        if accept == Some(Framing::Marked) {
            // Only a marker can start a frame: jump straight to the next one
            match buf.iter().position(|&b| b == FRAME_MAGIC[0]) {
                Some(start) => buf.advance(start),
                None => {
                    buf.clear();
                    return None;
                }
            }
        }
        if buf.len() < HEADER_BYTES {
            return None;
        }
        match header(buf, accept) {
            Some(h) => return Some(h),
            None => buf.advance(1),
        }
    }
}

fn decode_frame<M: Message + Default>(
    buf: &mut BytesMut,
    accept: Option<Framing>,
) -> Result<Option<(M, Framing)>, ProtocolError> {
    let Some((framing, payload_len)) = resync(buf, accept) else {
        return Ok(None);
    };
    if buf.len() < HEADER_BYTES + payload_len {
        return Ok(None);
    }

    match M::decode(&buf[HEADER_BYTES..HEADER_BYTES + payload_len]) {
        Ok(msg) => {
            buf.advance(HEADER_BYTES + payload_len);
            Ok(Some((msg, framing)))
        }
        Err(e) => {
            // Most likely a false start inside noise whose "payload" holds
            // real frames: skip only its first byte and rescan
            buf.advance(1);
            Err(e.into())
        }
    }
}

/// Encodes a HostToDevice message in the current (marked) framing.
pub fn encode_host_message(msg: &proto::HostToDevice) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg, Framing::Marked)
}

/// Encodes a HostToDevice message in the framing the pad speaks.
pub fn encode_host_message_as(
    msg: &proto::HostToDevice,
    framing: Framing,
) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg, framing)
}

/// Encodes a DeviceToHost message in the current (marked) framing.
pub fn encode_device_message(msg: &proto::DeviceToHost) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg, Framing::Marked)
}

/// Encodes a DeviceToHost message in the given framing.
pub fn encode_device_message_as(
    msg: &proto::DeviceToHost,
    framing: Framing,
) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg, framing)
}

/// Attempts to decode one DeviceToHost message, in either framing, from the
/// incoming stream buffer.
///
/// Garbage before a frame is discarded. Returns Ok(Some(msg)) for a complete
/// frame, Ok(None) when more data is needed, and Err for a frame whose payload
/// did not decode; the buffer has been advanced past it, so calling again
/// continues with the next frame.
pub fn decode_device_message(
    buf: &mut BytesMut,
) -> Result<Option<proto::DeviceToHost>, ProtocolError> {
    Ok(decode_frame(buf, None)?.map(|(m, _)| m))
}

/// Like [`decode_device_message`], also reporting the frame's framing.
/// `accept` limits it to one framing once the pad's is known; `None` takes both.
pub fn decode_device_message_framed(
    buf: &mut BytesMut,
    accept: Option<Framing>,
) -> Result<Option<(proto::DeviceToHost, Framing)>, ProtocolError> {
    decode_frame(buf, accept)
}

/// Attempts to decode one HostToDevice message, in either framing.
/// Same contract as [`decode_device_message`].
pub fn decode_host_message(
    buf: &mut BytesMut,
) -> Result<Option<proto::HostToDevice>, ProtocolError> {
    Ok(decode_frame(buf, None)?.map(|(m, _)| m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_to_device_roundtrip() {
        let msg = proto::HostToDevice {
            sequence_number: 42,
            payload: Some(proto::host_to_device::Payload::Hello(proto::Hello {
                protocol_version: 1,
                client_version: "1.0.0".to_string(),
            })),
        };

        let encoded = encode_host_message(&msg).expect("encode should succeed");
        let mut bytes_mut = BytesMut::from(&encoded[..]);

        let decoded = decode_host_message(&mut bytes_mut)
            .expect("decode should succeed")
            .expect("should produce message");

        assert_eq!(decoded.sequence_number, 42);
        match decoded.payload {
            Some(proto::host_to_device::Payload::Hello(h)) => {
                assert_eq!(h.protocol_version, 1);
                assert_eq!(h.client_version, "1.0.0");
            }
            _ => panic!("Unexpected payload"),
        }
        assert!(bytes_mut.is_empty());
    }

    #[test]
    fn test_device_to_host_roundtrip() {
        let msg = proto::DeviceToHost {
            sequence_number: 101,
            payload: Some(proto::device_to_host::Payload::Status(
                proto::DeviceStatus {
                    uptime_seconds: 3600,
                    state: proto::DeviceState::Idle as i32,
                    brightness: 80,
                    display_asleep: false,
                    lifetime_key1: 12345,
                    lifetime_key2: 67890,
                    map_key1: 50,
                    map_key2: 60,
                    ..Default::default()
                },
            )),
        };

        let encoded = encode_device_message(&msg).expect("encode should succeed");
        let mut bytes_mut = BytesMut::from(&encoded[..]);

        let decoded = decode_device_message(&mut bytes_mut)
            .expect("decode should succeed")
            .expect("should produce message");

        assert_eq!(decoded.sequence_number, 101);
        match decoded.payload {
            Some(proto::device_to_host::Payload::Status(s)) => {
                assert_eq!(s.uptime_seconds, 3600);
                assert_eq!(s.lifetime_key1, 12345);
                assert_eq!(s.lifetime_key2, 67890);
            }
            _ => panic!("Unexpected payload"),
        }
        assert!(bytes_mut.is_empty());
    }

    #[test]
    fn test_partial_frame_handling() {
        let msg = proto::HostToDevice {
            sequence_number: 1,
            payload: Some(proto::host_to_device::Payload::RequestStatus(true)),
        };

        let encoded = encode_host_message(&msg).unwrap();
        // Send only partial bytes
        let mut bytes_mut = BytesMut::from(&encoded[..2]);
        assert!(decode_host_message(&mut bytes_mut).unwrap().is_none());

        // Append rest of header
        bytes_mut.extend_from_slice(&encoded[2..4]);
        assert_eq!(&bytes_mut[..2], &FRAME_MAGIC);
        assert!(decode_host_message(&mut bytes_mut).unwrap().is_none());

        // Append rest of payload
        bytes_mut.extend_from_slice(&encoded[4..]);
        let res = decode_host_message(&mut bytes_mut).unwrap();
        assert!(res.is_some());
    }

    fn status(seq: u32) -> proto::DeviceToHost {
        proto::DeviceToHost {
            sequence_number: seq,
            payload: Some(proto::device_to_host::Payload::Status(
                proto::DeviceStatus {
                    uptime_seconds: seq,
                    ..Default::default()
                },
            )),
        }
    }

    fn drain(buf: &mut BytesMut) -> Vec<u32> {
        let mut seqs = Vec::new();
        loop {
            match decode_device_message(buf) {
                Ok(Some(m)) => seqs.push(m.sequence_number),
                Ok(None) => return seqs,
                Err(_) => continue,
            }
        }
    }

    #[test]
    fn header_is_magic_then_u16_le_length() {
        let encoded = encode_device_message(&status(7)).unwrap();
        assert_eq!(&encoded[..2], &[0xAA, 0x55]);
        let len = u16::from_le_bytes([encoded[2], encoded[3]]) as usize;
        assert_eq!(len, encoded.len() - HEADER_BYTES);
    }

    #[test]
    fn stray_bytes_between_frames_are_skipped() {
        let mut stream = Vec::new();
        stream.extend_from_slice(b"ESP-ROM:esp32s3-20210327\r\n");
        stream.extend(encode_device_message(&status(1)).unwrap());
        stream.extend_from_slice(&[0xAA, 0x00, 0xAA]);
        stream.extend(encode_device_message(&status(2)).unwrap());
        stream.extend_from_slice(b"FREAKY67\n");
        stream.extend(encode_device_message(&status(3)).unwrap());

        let mut buf = BytesMut::from(&stream[..]);
        assert_eq!(drain(&mut buf), vec![1, 2, 3]);
        assert!(buf.is_empty());
    }

    #[test]
    fn a_frame_cut_short_does_not_hide_the_next_one() {
        // A frame truncated mid-payload, then complete frames: the truncated
        // one decodes as garbage (or swallows bytes); every later frame whose
        // header lies outside its claimed length must survive
        let first = encode_device_message(&status(1)).unwrap();
        let mut stream = first[..first.len() - 3].to_vec();
        for seq in 2..=6 {
            stream.extend(encode_device_message(&status(seq)).unwrap());
        }
        let mut buf = BytesMut::from(&stream[..]);
        let seqs = drain(&mut buf);
        assert!(seqs.ends_with(&[3, 4, 5, 6]), "got {seqs:?}");
    }

    #[test]
    fn a_false_header_with_an_impossible_length_is_skipped() {
        let mut stream = vec![0xAA, 0x55, 0xFF, 0xFF];
        stream.extend(encode_device_message(&status(9)).unwrap());
        let mut buf = BytesMut::from(&stream[..]);
        assert_eq!(drain(&mut buf), vec![9]);
    }

    #[test]
    fn corrupted_stream_fuzz_recovers_every_clean_tail() {
        // Deterministic LCG noise: frames interleaved with random junk. Every
        // frame that follows a junk run which contains no marker byte is
        // recovered in order.
        let mut state: u32 = 0x1234_5678;
        let mut rnd = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (state >> 16) as u8
        };
        let mut stream = Vec::new();
        for seq in 1..=200u32 {
            let junk = (rnd() % 12) as usize;
            for _ in 0..junk {
                let b = rnd();
                stream.push(if b == 0xAA { 0x00 } else { b });
            }
            stream.extend(encode_device_message(&status(seq)).unwrap());
        }
        // Feed in odd-sized chunks, as a serial port would
        let mut buf = BytesMut::new();
        let mut seqs = Vec::new();
        for chunk in stream.chunks(37) {
            buf.extend_from_slice(chunk);
            seqs.extend(drain(&mut buf));
        }
        assert_eq!(seqs, (1..=200).collect::<Vec<_>>());
    }

    #[test]
    fn legacy_frames_decode_and_report_their_framing() {
        let msg = status(5);
        let legacy = encode_device_message_as(&msg, Framing::Legacy).unwrap();
        let len = u32::from_le_bytes(legacy[..4].try_into().unwrap()) as usize;
        assert_eq!(len, legacy.len() - HEADER_BYTES);

        let mut buf = BytesMut::from(&legacy[..]);
        let (m, framing) = decode_device_message_framed(&mut buf, None)
            .unwrap()
            .unwrap();
        assert_eq!((m.sequence_number, framing), (5, Framing::Legacy));
        assert!(buf.is_empty());
    }

    #[test]
    fn mixed_framings_in_one_stream_all_decode() {
        let mut stream = Vec::new();
        stream.extend(encode_device_message_as(&status(1), Framing::Legacy).unwrap());
        stream.extend(encode_device_message(&status(2)).unwrap());
        stream.extend_from_slice(b"junk");
        stream.extend(encode_device_message(&status(3)).unwrap());
        stream.extend(encode_device_message_as(&status(4), Framing::Legacy).unwrap());
        let mut buf = BytesMut::from(&stream[..]);
        assert_eq!(drain(&mut buf), vec![1, 2, 3, 4]);
    }

    /// The limit of legacy framing, pinned so nobody relies on more: junk
    /// right before a legacy frame can read as a legacy header and hold the
    /// parser waiting for bytes that never come. The host drops such a stale
    /// partial frame after 500 ms and re-sends Hello, and once the pad has
    /// answered it only accepts that pad's framing.
    #[test]
    fn junk_before_a_legacy_frame_can_stall_until_the_stale_timeout() {
        let mut stream = b"junk".to_vec();
        stream.extend(encode_device_message_as(&status(1), Framing::Legacy).unwrap());
        let mut buf = BytesMut::from(&stream[..]);
        assert!(drain(&mut buf).is_empty());
        assert!(
            !buf.is_empty(),
            "held as a partial frame, for the host to expire"
        );
    }

    #[test]
    fn a_locked_framing_ignores_the_other() {
        let legacy = encode_device_message_as(&status(1), Framing::Legacy).unwrap();
        let mut buf = BytesMut::from(&legacy[..]);
        assert!(
            decode_device_message_framed(&mut buf, Some(Framing::Marked))
                .unwrap()
                .is_none()
        );

        let marked = encode_device_message(&status(2)).unwrap();
        let mut buf = BytesMut::from(&marked[..]);
        assert!(
            decode_device_message_framed(&mut buf, Some(Framing::Legacy))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_legacy_length_with_an_aa_low_byte_is_not_mistaken_for_a_marker() {
        // A 170-byte (0xAA) payload: the legacy header reads AA 00 00 00.
        // Pad a real message to that size with an unknown bytes field (tag
        // 1000), which prost skips.
        let mut payload = status(9).encode_to_vec();
        let pad = 170 - payload.len() - 4;
        payload.extend([0xC2, 0x3E, (pad as u8 & 0x7F) | 0x80, (pad >> 7) as u8]);
        payload.resize(170, b'y');
        let mut frame = (payload.len() as u32).to_le_bytes().to_vec();
        frame.extend(payload);
        assert_eq!(&frame[..4], &[0xAA, 0, 0, 0]);

        let mut buf = BytesMut::from(&frame[..]);
        let (m, framing) = decode_device_message_framed(&mut buf, None)
            .unwrap()
            .unwrap();
        assert_eq!((m.sequence_number, framing), (9, Framing::Legacy));
    }
}
