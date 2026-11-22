---
title: Serial Protocol Reference
description: Specification of the CDC serial framing, negotiation, and Protocol Buffers wire format between host and pad.
category: reference
source_files:
  - /protocol/opad.proto
  - /protocol/opad.options
  - /firmware/main/protocol/protocol.c
  - /firmware/main/protocol/frame_parser.c
  - /desktop/crates/opad-protocol/src/lib.rs
---

The OPad desktop suite communicates with the pad over USB CDC-ACM serial at 115200 baud (8N1). All bidirectional control messages are encoded using Google Protocol Buffers (proto3) encapsulated in length-delimited wire frames. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

## Framing Layer

The protocol defines three distinct framing variants to maintain backward and forward compatibility between firmware and desktop clients. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

### Framing Variants

| Variant | Magic Bytes | Length Field | Payload Field | Checksum Field | Total Header |
|---|---|---|---|---|---|
| **Legacy** | None | `uint32` (little-endian) | Raw protobuf payload | None | 4 bytes |
| **Marked** | `0xAA, 0x55` | `uint16` (little-endian) | Raw protobuf payload | None | 4 bytes |
| **Checked** | `0xAA, 0x5A` | `uint16` (little-endian) | Raw protobuf payload | `uint16` (CRC-16 LE) | 4 bytes (+ 2 trailer) |

<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->
<!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

- **Legacy Framing**: Used by initial firmware builds. The length is a 32-bit unsigned little-endian integer immediately followed by the protobuf bytes. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->
- **Marked Framing**: Starts with synchronization magic bytes `[0xAA, 0x55]` (`FRAME_MAGIC`) followed by a 16-bit little-endian length. This protects against garbage byte streams when attaching to an active serial port. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Checked Framing**: Starts with magic bytes `[0xAA, 0x5A]` (`FRAME_MAGIC_CHECKED`) followed by a 16-bit little-endian length, the protobuf payload, and a 2-byte CRC-16 trailer. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->

### CRC-16 Specification

Checked frames employ standard CRC-16/CCITT-FALSE: <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 -->
- **Polynomial**: `0x1021` ($x^{16} + x^{12} + x^5 + 1$)
- **Initial Value**: `0xFFFF`
- **RefIn**: `false` (no bit reflection)
- **RefOut**: `false`
- **XorOut**: `0x0000`
- **Coverage**: Calculated over the 2-byte payload length field concatenated with the payload bytes (header magic is excluded). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 --> <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->
- **Encoding**: Appended to the end of the frame in little-endian order. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#encode_host_message -->

### Frame Size Limits

- **Header overhead**: 4 bytes (`HEADER_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#HEADER_BYTES -->
- **CRC trailer overhead**: 2 bytes (`CRC_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#CRC_BYTES -->
- **Maximum payload size**: 8186 bytes (`MAX_PAYLOAD_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#MAX_PAYLOAD_BYTES -->

Frames exceeding 8186 bytes payload or failing CRC validation are dropped, resetting the parser to search for the next sync magic. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

## Framing Negotiation

To maintain strict interoperability (Product Rule 3), the communication begins in marked framing and negotiates checked framing dynamically: <!-- src: /firmware/main/protocol/protocol.c#s_crc_answer -->

1. The host connects and sends a `HostToDevice` frame containing a `Hello` message wrapped in a **Marked frame** (`0xAA, 0x55`). In the message, `Hello.frame_crc` is set to `true`. <!-- src: /protocol/opad.proto#Hello -->
2. The pad parses the marked frame. If the firmware supports checked framing, it sets `HelloAck.frame_crc = true` and replies with a `DeviceToHost` containing `HelloAck` encapsulated in a **Marked frame** (`0xAA, 0x55`). <!-- src: /firmware/main/protocol/protocol.c#protocol_send_hello_ack -->
3. After transmitting the `HelloAck` marked frame, both host and pad switch their transmit and receive states to **Checked framing** (`0xAA, 0x5A`). <!-- src: /firmware/main/protocol/protocol.c#s_tx_crc -->
4. If an older pad does not support CRC or an older host sends `frame_crc = false`, marked framing remains active for the entire session. If legacy frames arrive, the pad answers using legacy framing. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

## Top-Level Envelopes

All protocol communication uses two root protobuf envelopes containing sequential tracking numbers and `oneof` payload fields. <!-- src: /protocol/opad.proto#HostToDevice --> <!-- src: /protocol/opad.proto#DeviceToHost -->

```protobuf
message HostToDevice {
    uint32 sequence_number = 1;
    oneof payload {
        Hello hello = 2;
        SetConfig set_config = 3;
        TimeSync time_sync = 4;
        GameplayDisplayState gameplay_state = 5;
        CounterSyncRequest counter_sync = 6;
        bool request_status = 7;
        HostStatus host_status = 8;
        bool reset_latency_stats = 9;
        DataUpdate data_update = 10;
        SetLayout set_layout = 11;
        uint32 reset_layout = 12;
        bool request_logs = 13;
        ClaimOwnership claim_ownership = 14;
        DetectPinRequest detect_pin = 15;
        bool run_board_test = 16;
        PlayAnimation play_animation = 17;
        bool restart = 18;
        bool start_he_calibration = 19;
        bool cancel_he_calibration = 20;
        bool he_live_view = 21;
    }
}

message DeviceToHost {
    uint32 sequence_number = 1;
    oneof payload {
        HelloAck hello_ack = 2;
        DeviceStatus status = 3;
        ConfigAck config_ack = 4;
        CounterSyncResponse counter_sync_resp = 5;
        LogEventBatch log_batch = 6;
        LayoutAck layout_ack = 7;
        DetectPinResponse detect_pin_resp = 8;
        KeyPressBatch key_presses = 9;
        BoardTestResult board_test = 10;
        HeCalibrationProgress he_calibration_progress = 11;
        HeLiveSample he_live_sample = 12;
    }
}
```

## Protocol Message Catalog

### Session & Identity Messages

- **`Hello` / `HelloAck`**: Establishes protocol version (`ProtocolVersion`), client version string, pad firmware version, device serial identifier, running partition slot (`ota_0` / `ota_1`), and owner identifier. <!-- src: /protocol/opad.proto#Hello --> <!-- src: /protocol/opad.proto#HelloAck -->
- **`ClaimOwnership`**: Pairs a host identity (`owner_id`) with the pad. Pad persists owner metadata in NVS. <!-- src: /protocol/opad.proto#ClaimOwnership -->
- **`DeviceStatus`**: Sent in response to `request_status = true`. Reports device state (`DeviceState`), active input module type (`InputModule`), keypress counts, uptime, and Hall Effect status. <!-- src: /protocol/opad.proto#DeviceStatus -->
- **`HostStatus`**: Sent periodically by daemon to keep the pad informed of active host profile, audio volume, and playback state. <!-- src: /protocol/opad.proto#HostStatus -->
- **`restart`**: Flags the pad to perform a clean restart once idle (never mid-map). <!-- src: /protocol/opad.proto#HostToDevice -->

### Configuration Messages

- **`SetConfig` / `ConfigAck`**: Transmits an updated `ConfigPayload` payload containing debounce algorithms, LED settings, display layouts, and Hall Effect parameters (`HeKeySettings`). In accordance with latency requirements, writing to NVS is deferred if the device is currently playing a map (`PLAYING` / `COOLDOWN`). <!-- src: /protocol/opad.proto#SetConfig --> <!-- src: /protocol/opad.proto#ConfigAck --> <!-- src: /firmware/main/protocol/protocol.c#apply_he_settings -->
- **`ConfigPayload`**: Contains full settings structure including `key1_gpio`, `key2_gpio`, `debounce_ms`, `led_brightness`, `screen_timeout_s`, and game key definitions. <!-- src: /protocol/opad.proto#ConfigPayload -->

### Gameplay & Screen Messages

- **`TimeSync`**: Synchronizes pad clock timestamp with host system monotonic time. <!-- src: /protocol/opad.proto#TimeSync -->
- **`GameplayDisplayState`**: Transmits metadata for the currently active map (song title, artist, difficulty name, map duration) to update the LCD display. <!-- src: /protocol/opad.proto#GameplayDisplayState -->
- **`DataUpdate`**: High-frequency gameplay statistics streaming (UR, combo, accuracy, PPM, grade) composed of `DataValue` arrays. <!-- src: /protocol/opad.proto#DataUpdate -->
- **`SetLayout` / `LayoutAck`**: Dynamically defines custom screen layouts composed of up to 32 `UiWidget` elements. <!-- src: /protocol/opad.proto#SetLayout --> <!-- src: /protocol/opad.proto#LayoutAck -->
- **`reset_layout`**: Resets the specified screen identifier back to the built-in default layout. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`PlayAnimation`**: Triggers a grade splash or celebration screen animation on the pad display. <!-- src: /protocol/opad.proto#PlayAnimation -->

### Hall Effect Calibration & Telemetry Messages

- **`HeKeySettings`**: Per-key actuation point, reset point, deadzone, and rapid trigger mode (`HeRtMode`). <!-- src: /protocol/opad.proto#HeKeySettings --> <!-- src: /protocol/opad.proto#HeRtMode -->
- **`HeCalibrationStatus`**: Current calibration state (`HeCalStage`), rested ADC values, and bottom travel extremes. <!-- src: /protocol/opad.proto#HeCalibrationStatus --> <!-- src: /protocol/opad.proto#HeCalStage -->
- **`start_he_calibration` / `cancel_he_calibration`**: Host commands to initiate or abort the interactive calibration sequence. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`HeCalibrationProgress`**: Broadcasts the current step of guided calibration and reports any hardware calibration errors (`HeCalError`). <!-- src: /protocol/opad.proto#HeCalibrationProgress --> <!-- src: /protocol/opad.proto#HeCalError -->
- **`he_live_view` / `HeLiveSample`**: Streams real-time normalized switch travel distance and raw magnetic readings (~50 Hz) during the desktop Hall Effect visualizer view. <!-- src: /protocol/opad.proto#HeLiveSample -->

### Counters, Hardware Diagnostics & Self-Test Messages

- **`CounterSyncRequest` / `CounterSyncResponse`**: Bi-directional reconciliation of persistent keypress lifetime totals (`CounterState`). <!-- src: /protocol/opad.proto#CounterSyncRequest --> <!-- src: /protocol/opad.proto#CounterSyncResponse -->
- **`DetectPinRequest` / `DetectPinResponse`**: Interactive pin test tool where the host asks the pad to monitor a GPIO pin for a physical switch press. <!-- src: /protocol/opad.proto#DetectPinRequest --> <!-- src: /protocol/opad.proto#DetectPinResponse -->
- **`run_board_test` / `BoardTestResult`**: Executes pad hardware self-tests (I2C bus scan, display communication, touch controller response, and Hall Effect ADC channels). <!-- src: /protocol/opad.proto#BoardTestResult -->
- **`KeyPress` / `KeyPressBatch`**: Diagnostic batch of recent keypress events including press/release timestamps and durations. <!-- src: /protocol/opad.proto#KeyPressBatch -->
- **`LogEvent` / `LogEventBatch`**: Diagnostic log events drained from the pad ring buffer to the host daemon. <!-- src: /protocol/opad.proto#LogEventBatch -->

## Nanopb Buffer and Field Limits

Field lengths and fixed array bounds are specified in `protocol/opad.options`: <!-- src: /protocol/opad.options -->

| Field | Max Size / Limit | Allocation Type |
|---|---|---|
| `Hello.client_version` | 32 characters | Fixed string |
| `HelloAck.firmware_version` | 32 characters | Fixed string |
| `HelloAck.board_profile` | 32 characters | Fixed string |
| `HelloAck.device_id` | 32 characters | Fixed string |
| `HelloAck.owner_id` | 16 characters | Fixed string |
| `HelloAck.running_partition` | 17 characters | Fixed string |
| `ClaimOwnership.owner_id` | 16 characters | Fixed string |
| `ConfigAck.message` | 64 characters | Fixed string |
| `GameplayDisplayState.title` | 64 characters | Fixed string |
| `GameplayDisplayState.artist` | 64 characters | Fixed string |
| `GameplayDisplayState.difficulty` | 32 characters | Fixed string |
| `DataValue.text` | 64 characters | Fixed string |
| `DataUpdate.values` | 32 elements | Repeated message |
| `UiWidget.label` | 32 characters | Fixed string |
| `UiWidget.suffix` | 12 characters | Fixed string |
| `SetLayout.widgets` | 32 elements | Repeated message |
| `LayoutAck.message` | 64 characters | Fixed string |
| `PlayAnimation.grade` | 4 characters | Fixed string |
| `CounterState.device_id` | 32 characters | Fixed string |
| `CounterSyncResponse.message` | 64 characters | Fixed string |
| `LogEvent.tag` | 16 characters | Fixed string |
| `LogEvent.message` | 64 characters | Fixed string |
| `LogEventBatch.events` | 8 elements | Repeated message |
| `KeyPressBatch.presses` | 32 elements | Repeated message |
| `BoardTestResult.message` | 64 characters | Fixed string |
| `HeAdcTest.message` | 48 characters | Fixed string |
| `HeAdcTest.speeds` | 6 elements | Repeated message |
| `HeAdcTest.channels` | 3 elements | Repeated message |
| `HeCalibrationProgress.error_message` | 48 characters | Fixed string |

These limits guarantee static allocations on the ESP32-S3 firmware, preventing dynamic heap fragmentation during runtime execution. <!-- src: /firmware/main/protocol/protocol.c#protocol_encode_device_message -->
