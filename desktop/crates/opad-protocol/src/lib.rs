pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/osupad.rs"));
}

use bytes::{Buf, BytesMut};
use prost::Message;
use thiserror::Error;

/// Frame start marker: every frame is `[0xAA, 0x55, len u16-LE, payload]`.
/// A parser that finds anything else slides forward one byte at a time until it
/// sees the marker again, so stray bytes (ROM bootloader chatter, a text
/// command, a dropped byte) cost at most the frame they land in.
pub const FRAME_MAGIC: [u8; 2] = [0xAA, 0x55];
pub const HEADER_BYTES: usize = 4;
/// Matches PROTOCOL_MAX_FRAME_SIZE (8192) on the device minus the header
pub const MAX_PAYLOAD_BYTES: usize = 8192 - HEADER_BYTES;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Frame length {0} exceeds maximum limit of {MAX_PAYLOAD_BYTES} bytes")]
    FrameTooLarge(usize),
    #[error("Protobuf decode error: {0}")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Protobuf encode error: {0}")]
    EncodeError(#[from] prost::EncodeError),
}

fn encode_frame<M: Message>(msg: &M) -> Result<Vec<u8>, ProtocolError> {
    let payload_len = msg.encoded_len();
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    let mut buf = Vec::with_capacity(HEADER_BYTES + payload_len);
    buf.extend_from_slice(&FRAME_MAGIC);
    buf.extend_from_slice(&(payload_len as u16).to_le_bytes());
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// Drops bytes until `buf` starts with a plausible frame header, or holds only
/// a prefix of one. Returns the number of bytes skipped.
fn resync(buf: &mut BytesMut) -> usize {
    let mut skipped = 0;
    loop {
        let Some(start) = buf.iter().position(|&b| b == FRAME_MAGIC[0]) else {
            skipped += buf.len();
            buf.clear();
            return skipped;
        };
        buf.advance(start);
        skipped += start;
        if buf.len() < 2 {
            return skipped;
        }
        if buf[1] != FRAME_MAGIC[1] {
            buf.advance(1);
            skipped += 1;
            continue;
        }
        if buf.len() < HEADER_BYTES {
            return skipped;
        }
        let len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
        if len > MAX_PAYLOAD_BYTES {
            // AA 55 inside other bytes, not a header
            buf.advance(1);
            skipped += 1;
            continue;
        }
        return skipped;
    }
}

fn decode_frame<M: Message + Default>(buf: &mut BytesMut) -> Result<Option<M>, ProtocolError> {
    resync(buf);
    if buf.len() < HEADER_BYTES {
        return Ok(None);
    }
    let payload_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    if buf.len() < HEADER_BYTES + payload_len {
        return Ok(None);
    }

    match M::decode(&buf[HEADER_BYTES..HEADER_BYTES + payload_len]) {
        Ok(msg) => {
            buf.advance(HEADER_BYTES + payload_len);
            Ok(Some(msg))
        }
        Err(e) => {
            // Most likely a false start (AA 55 in noise) whose "payload" holds
            // real frames: skip only the marker's first byte and rescan
            buf.advance(1);
            Err(e.into())
        }
    }
}

/// Encodes a HostToDevice message into a framed byte buffer.
pub fn encode_host_message(msg: &proto::HostToDevice) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg)
}

/// Encodes a DeviceToHost message into a framed byte buffer.
pub fn encode_device_message(msg: &proto::DeviceToHost) -> Result<Vec<u8>, ProtocolError> {
    encode_frame(msg)
}

/// Attempts to decode one DeviceToHost message from the incoming stream buffer.
///
/// Garbage before a frame is discarded. Returns Ok(Some(msg)) for a complete
/// frame, Ok(None) when more data is needed, and Err for a frame whose payload
/// did not decode; the buffer has been advanced past it, so calling again
/// continues with the next frame.
pub fn decode_device_message(
    buf: &mut BytesMut,
) -> Result<Option<proto::DeviceToHost>, ProtocolError> {
    decode_frame(buf)
}

/// Attempts to decode one HostToDevice message from the incoming stream buffer.
/// Same contract as [`decode_device_message`].
pub fn decode_host_message(
    buf: &mut BytesMut,
) -> Result<Option<proto::HostToDevice>, ProtocolError> {
    decode_frame(buf)
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
}
