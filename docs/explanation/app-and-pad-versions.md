---
title: App and Pad Versions Interoperability
description: Protocol negotiation, backward and forward compatibility rules, framing auto-detection, and version mismatch handling between desktop software and pad firmware.
---

A cornerstone of the OPad architecture is **independent updates**: the desktop application and pad firmware update independently (Product Rule 3). Users must never be told to "update both together". Every protocol, wire-format, or framing revision is designed to function bidirectionally: a brand new desktop suite works seamlessly with an older firmware release, and updated firmware remains fully functional with legacy desktop builds. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

## Compatibility Dimensions

Interoperability spans three distinct layers:
1. **Physical HID Keyboard Layer**: Fully independent of software. The pad enumerates as a standard 1000 Hz USB HID keyboard requiring no daemon, driver, or background processes on Linux or Windows (Product Rule 2).
2. **Serial Transport & Framing Layer**: Dynamic auto-negotiation between legacy (unmarked), marked (`0xAA, 0x55`), and checked (`0xAA, 0x5A` + CRC-16) frame encodings. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC --> <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->
3. **Application & Protobuf Layer**: Version identification, capability negotiation flags (`frame_crc`), and unknown protobuf field tolerance. <!-- src: /protocol/opad.proto#Hello -->

## Protocol Version Negotiation

Communication begins with the `Hello` / `HelloAck` handshake: <!-- src: /firmware/main/protocol/protocol.c#protocol_send_hello_ack -->

- **Desktop Handshake**:
  Upon detecting the pad's CDC serial port, `opad-daemon` transmits a `Hello` message wrapped in Marked framing (`0xAA, 0x55`). In this envelope, the desktop declares its version string (`client_version`) and requests checked framing (`frame_crc = true`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Firmware Response**:
  The firmware parses the incoming frame. It inspects the host's framing and responds in kind:
  - If the host sent a legacy frame, the pad replies with a legacy frame.
  - If the host sent a marked frame with `frame_crc = true`, the pad replies with `HelloAck.frame_crc = true` in a marked frame, then transitions both TX and RX engines to checked framing (`0xAA, 0x5A`) from the subsequent message onward. <!-- src: /firmware/main/protocol/protocol.c#s_crc_answer -->
  - If the host sends `frame_crc = false` (older desktop clients), the pad remains in marked framing (`0xAA, 0x55`) indefinitely.

## Protobuf Forward & Backward Compatibility

The protocol is defined with Google Protocol Buffers (proto3), which provides built-in wire evolution rules: <!-- src: /protocol/opad.proto -->

- **New Desktop with Old Firmware**:
  When a modern desktop sends messages containing new configuration fields (such as Hall Effect rapid trigger modes or customized swipe actions), older firmware simply skips unrecognized protobuf tags during nanopb decoding without failing or crashing. The pad applies the fields it understands and acknowledges the request.
- **Old Desktop with New Firmware**:
  When older desktop software connects to newer firmware, the pad generates `DeviceToHost` responses containing newer optional fields. The older desktop ignores unrecognized tags and safely extracts the standard status and counter payloads.
- **Static Allocations**:
  To protect embedded firmware memory bounds, fixed upper limits are compiled into nanopb via `protocol/opad.options` (e.g., maximum string lengths, fixed repeated arrays). Payloads exceeding these bounds are rejected cleanly before parsing. <!-- src: /protocol/opad.options -->

## Incompatible Devices & Version Mismatch Handling

The wire protocol version is tracked by `DEVICE_PROTOCOL_VERSION` (currently `1`): <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

- If a connected pad returns a `HelloAck` where `protocol_version` does not match the daemon's supported major version, the daemon flags the pad as an `IncompatibleDevice` (`desktop/crates/opad-model/src/lib.rs`): <!-- src: /desktop/crates/opad-model/src/lib.rs#IncompatibleDevice -->
  ```rust
  pub struct IncompatibleDevice {
      pub firmware_version: String,
      pub protocol_version: u32,
  }
  ```
- **Guaranteed Fallback**:
  An incompatible pad is **never blocked or bricked**. The daemon does not attempt to synchronize counters, overwrite configurations, or stream telemetry to it. However, the pad continues operating normally as a 1000 Hz USB HID keyboard. The GUI displays an informational notice advising the user of the version divergence without disrupting active gameplay.
