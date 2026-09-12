pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/osupad.rs"));
}

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, BytesMut};
use prost::Message;
use std::io::Cursor;
use thiserror::Error;

/// Matches PROTOCOL_MAX_FRAME_SIZE (8192) on the device minus the 4-byte length prefix
pub const MAX_PAYLOAD_BYTES: usize = 8188;
pub const HEADER_BYTES: usize = 4;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Frame length {0} exceeds maximum limit of {MAX_PAYLOAD_BYTES} bytes")]
    FrameTooLarge(usize),
    #[error("Protobuf decode error: {0}")]
    DecodeError(#[from] prost::DecodeError),
    #[error("Protobuf encode error: {0}")]
    EncodeError(#[from] prost::EncodeError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Encodes a HostToDevice message into a framed byte buffer:
/// [4-byte little-endian length][protobuf payload]
pub fn encode_host_message(msg: &proto::HostToDevice) -> Result<Vec<u8>, ProtocolError> {
    let payload_len = msg.encoded_len();
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    let mut buf = Vec::with_capacity(HEADER_BYTES + payload_len);
    buf.write_u32::<LittleEndian>(payload_len as u32)?;
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// Encodes a DeviceToHost message into a framed byte buffer:
/// [4-byte little-endian length][protobuf payload]
pub fn encode_device_message(msg: &proto::DeviceToHost) -> Result<Vec<u8>, ProtocolError> {
    let payload_len = msg.encoded_len();
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    let mut buf = Vec::with_capacity(HEADER_BYTES + payload_len);
    buf.write_u32::<LittleEndian>(payload_len as u32)?;
    msg.encode(&mut buf)?;
    Ok(buf)
}

/// Attempts to decode one DeviceToHost message from the incoming stream buffer.
/// Returns Ok(Some(msg)) if a complete frame is available, Ok(None) if more data is needed,
/// or Err on invalid frames.
pub fn decode_device_message(buf: &mut BytesMut) -> Result<Option<proto::DeviceToHost>, ProtocolError> {
    if buf.len() < HEADER_BYTES {
        return Ok(None);
    }

    let mut rdr = Cursor::new(&buf[..HEADER_BYTES]);
    let payload_len = rdr.read_u32::<LittleEndian>()? as usize;

    if payload_len > MAX_PAYLOAD_BYTES {
        // Discard invalid header to avoid hanging
        buf.advance(HEADER_BYTES);
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    if buf.len() < HEADER_BYTES + payload_len {
        // Not enough data yet
        return Ok(None);
    }

    // Consume header
    buf.advance(HEADER_BYTES);
    let payload = buf.split_to(payload_len);

    let msg = proto::DeviceToHost::decode(payload)?;
    Ok(Some(msg))
}

/// Attempts to decode one HostToDevice message from the incoming stream buffer.
pub fn decode_host_message(buf: &mut BytesMut) -> Result<Option<proto::HostToDevice>, ProtocolError> {
    if buf.len() < HEADER_BYTES {
        return Ok(None);
    }

    let mut rdr = Cursor::new(&buf[..HEADER_BYTES]);
    let payload_len = rdr.read_u32::<LittleEndian>()? as usize;

    if payload_len > MAX_PAYLOAD_BYTES {
        buf.advance(HEADER_BYTES);
        return Err(ProtocolError::FrameTooLarge(payload_len));
    }

    if buf.len() < HEADER_BYTES + payload_len {
        return Ok(None);
    }

    buf.advance(HEADER_BYTES);
    let payload = buf.split_to(payload_len);

    let msg = proto::HostToDevice::decode(payload)?;
    Ok(Some(msg))
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
            payload: Some(proto::device_to_host::Payload::Status(proto::DeviceStatus {
                uptime_seconds: 3600,
                state: proto::DeviceState::Idle as i32,
                brightness: 80,
                display_asleep: false,
                lifetime_key1: 12345,
                lifetime_key2: 67890,
                map_key1: 50,
                map_key2: 60,
                ..Default::default()
            })),
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
        assert!(decode_host_message(&mut bytes_mut).unwrap().is_none());

        // Append rest of payload
        bytes_mut.extend_from_slice(&encoded[4..]);
        let res = decode_host_message(&mut bytes_mut).unwrap();
        assert!(res.is_some());
    }
}
