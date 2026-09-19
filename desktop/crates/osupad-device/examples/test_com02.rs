use bytes::BytesMut;
use osupad_protocol::proto::{self, host_to_device::Payload, HostToDevice};
use osupad_protocol::{decode_device_message, encode_host_message};
use std::io::{Read, Write};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port_name = osupad_device::find_target_port().expect("OPad not found on USB!");
    println!("Testing COM-02 against device on {}", port_name);

    let mut port = serialport::new(&port_name, 115_200)
        .timeout(Duration::from_millis(2000))
        .open()?;

    println!("Phase 1: Injecting 512 bytes of random garbage noise...");
    let garbage: Vec<u8> = (0..512).map(|i| (i * 37 + 11) as u8).collect();
    port.write_all(&garbage)?;
    port.flush()?;
    std::thread::sleep(Duration::from_millis(100));

    println!("Phase 2: Injecting invalid frame with oversized length prefix (0x7FFFFFFF)...");
    let oversized = vec![0xFF, 0xFF, 0xFF, 0x7F, 0xAA, 0xBB, 0xCC, 0xDD];
    port.write_all(&oversized)?;
    port.flush()?;
    std::thread::sleep(Duration::from_millis(100));

    println!("Phase 3: Injecting invalid protobuf bytes with short length (16 bytes)...");
    let bad_proto = vec![
        0x10, 0x00, 0x00, 0x00, // length 16
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF,
    ];
    port.write_all(&bad_proto)?;
    port.flush()?;
    std::thread::sleep(Duration::from_millis(200));

    println!("Phase 4: Sending valid Hello envelope to verify recovery...");
    let hello_msg = HostToDevice {
        sequence_number: 101,
        payload: Some(Payload::Hello(proto::Hello {
            protocol_version: 1,
            client_version: "com02-test-v1".to_string(),
        })),
    };
    let encoded = encode_host_message(&hello_msg)?;
    port.write_all(&encoded)?;
    port.flush()?;

    println!("Phase 5: Reading back response from device...");
    let mut in_buf = BytesMut::with_capacity(1024);
    let mut temp = [0u8; 256];
    let start = std::time::Instant::now();
    let mut received_ack = None;

    while start.elapsed() < Duration::from_secs(3) {
        match port.read(&mut temp) {
            Ok(n) if n > 0 => {
                in_buf.extend_from_slice(&temp[..n]);
                while let Ok(Some(msg)) = decode_device_message(&mut in_buf) {
                    if let Some(proto::device_to_host::Payload::HelloAck(ack)) = msg.payload {
                        received_ack = Some(ack);
                        break;
                    }
                }
                if received_ack.is_some() {
                    break;
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
        }
    }

    if let Some(ack) = received_ack {
        println!(
            "✓ COM-02 PASS: Device recovered cleanly from malformed frames and responded to Hello!"
        );
        println!("  Device ID:        {}", ack.device_id);
        println!("  Board Profile:    {}", ack.board_profile);
        println!("  Firmware Version: {}", ack.firmware_version);
        println!(
            "  Counters:         {} / {}",
            ack.lifetime_key1, ack.lifetime_key2
        );
        Ok(())
    } else {
        panic!("✗ COM-02 FAIL: Device did not respond with HelloAck after malformed frames (possible hang/crash)");
    }
}
