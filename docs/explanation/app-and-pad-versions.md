---
title: App and Pad Versions Interoperability
description: Protocol negotiation, backward and forward compatibility rules, framing auto-detection, and version mismatch handling between desktop software and pad firmware.
sidebar:
  order: 4
---

OPad's desktop suite and pad firmware update independently (AGENTS.md §1.3). Users are never required to update both together. Wire framing, capability flags, and protobuf fields are designed to interoperate in both directions: a new desktop app works with older firmware, and new firmware works with older desktop builds. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

## Compatibility Dimensions

Interoperability operates across three layers:

1. **Physical HID Keyboard Layer**: Independent of host software. The pad enumerates as a standard 1000 Hz USB HID keyboard requiring no daemon, GUI, or driver on Linux or Windows (AGENTS.md §1.2). <!-- src: /firmware/main/usb/usb_descriptors.c -->
2. **Serial Transport & Framing Layer**: Dynamic negotiation across legacy (unmarked), marked (`0xAA, 0x55`), and checked (`0xAA, 0x5A` + CRC-16) frame formats ([`Framing`](/desktop/crates/opad-protocol/src/lib.rs), [`frame_parser_feed()`](/firmware/main/protocol/frame_parser.c)).
3. **Application & Protobuf Layer**: Major protocol version check (`DEVICE_PROTOCOL_VERSION`), handshake capability flags in [`Hello`](/protocol/opad.proto) and [`HelloAck`](/protocol/opad.proto), and proto3 unknown-field skipping.

## Protocol Version & Framing Negotiation

Every USB CDC session begins with a `Hello` / `HelloAck` handshake ([`Handshake`](/desktop/crates/opad-device/src/lib.rs), [`protocol_send_hello_ack()`](/firmware/main/protocol/protocol.c)):

- **Host Probe (`Hello`)**:
  While the pad's framing is unknown, the host sends a `Hello` every 400 ms (`HELLO_RETRY`), alternating between marked framing (`0xAA, 0x55`) on even attempts and legacy framing on odd attempts ([`hello_framing()`](/desktop/crates/opad-device/src/lib.rs)). The `Hello` payload sets `protocol_version = 1`, `client_version`, `he_calibration = true`, and `frame_crc = true` ([`hello_message()`](/desktop/crates/opad-device/src/lib.rs)).
- **Pad Framing Latch & Reply (`HelloAck`)**:
  Until the first valid frame decodes, the firmware sends nothing (`s_host_framing_known = false`) so unsolicited frames cannot desynchronize an older host ([`protocol.c`](/firmware/main/protocol/protocol.c)). On the first decoded message, the firmware latches `s_host_framing` and locks its parser to that family:
  - If the host sent a **legacy** frame, the pad answers in legacy framing and ignores marked/checked headers for the rest of the connection.
  - If the host sent a **marked** frame with `Hello.frame_crc = true`, the pad replies with `HelloAck` (`frame_crc = true`) in a **marked** frame, then enables checked framing (`s_tx_crc = true`, `0xAA, 0x5A` + CRC-16) for every subsequent frame. Both sides' marked parsers accept both marked and checked frames.
  - If the host sent `Hello.frame_crc = false` (or the pad is older firmware that ignores `Hello.frame_crc` and answers `frame_crc = false`), both sides stay on marked framing (`0xAA, 0x55`).
- **Capability Negotiation**:
  [`HelloAck`](/protocol/opad.proto) advertises optional firmware features (such as `running_partition`, `key_press_times`, `board_test`, `builtin_animations`, `restart`, `he_input`, `he_calibration`, `he_live`, `tap_rate_per_second`, `key_count`, `input_module`, and `frame_crc`). Older firmware omits newer fields, which decode to `false`/`0`/empty on the host so the app gracefully degrades (for example, falling back to tosu for tap-rate timing when `key_press_times` is false).

## Protobuf Forward & Backward Compatibility

The wire schema uses Protocol Buffers v3 ([`protocol/opad.proto`](/protocol/opad.proto)):

- **New Desktop with Old Firmware**:
  When a newer app sends fields that older firmware does not recognize (for example Hall Effect settings or swipe modifiers), nanopb skips unknown wire tags during `pb_decode` and applies the fields it understands.
- **Old Desktop with New Firmware**:
  When an older app connects to newer firmware, prost ignores unknown fields in `DeviceToHost` messages and extracts the fields it knows.
- **Static Bounds in Firmware**:
  [`protocol/opad.options`](/protocol/opad.options) sets fixed `max_size` and `max_count` bounds for nanopb so firmware decodes into static buffers with zero heap allocation. Frames longer than `FRAME_MAX_PAYLOAD` (8186 bytes) are skipped by [`frame_parser_feed()`](/firmware/main/protocol/frame_parser.c); payloads that overflow a field limit fail cleanly inside [`protocol_decode_host_message()`](/firmware/main/protocol/protocol.c) and record `DIAG_EVENT_DECODE_FAILED`.

## Incompatible Devices & Version Mismatch Handling

The major wire protocol version is [`DEVICE_PROTOCOL_VERSION`](/desktop/crates/opad-protocol/src/lib.rs) (`1`):

- If `HelloAck.protocol_version` does not equal `DEVICE_PROTOCOL_VERSION`, the daemon records [`IncompatibleDevice`](/desktop/crates/opad-model/src/lib.rs) in [`RuntimeController::on_event()`](/desktop/daemon/src/runtime.rs), restores its pre-handshake counter state, and leaves `device_connected = false`:
  ```rust
  pub struct IncompatibleDevice {
      pub firmware_version: String,
      pub protocol_version: u32,
  }
  ```
- **Safe Fallback**:
  An incompatible pad keeps working as a 1000 Hz USB HID keyboard. The daemon does not sync counters, overwrite configuration, or stream telemetry to it. `opadctl status` prints an `Incompatible:` warning line ([`desktop/cli/src/main.rs`](/desktop/cli/src/main.rs)), and `opad-gui` displays a warning banner ([`desktop/gui/src/main.rs`](/desktop/gui/src/main.rs)).
