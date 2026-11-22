---
title: Serial Protocol Reference
description: Specification of the CDC serial framing, negotiation, and Protocol Buffers wire format between host and pad.
sidebar:
  order: 5
---

The OPad desktop suite communicates with the pad over USB CDC-ACM serial at 115200 baud. Control and telemetry messages are encoded as Protocol Buffers v3 (`proto3`) inside length-delimited wire frames. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing, /desktop/crates/opad-device/src/lib.rs -->

## Framing Layer

Three framing variants exist to maintain bidirectional compatibility between old and new desktop apps and firmware. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

### Framing Variants

| Variant | Magic Bytes | Length Field | Payload Field | Checksum Field | Header / Trailer |
|---|---|---|---|---|---|
| **Legacy** | None | `uint32` (little-endian, `>= 2`) | Raw protobuf payload | None | 4 bytes / 0 bytes |
| **Marked** | `0xAA, 0x55` | `uint16` (little-endian) | Raw protobuf payload | None | 4 bytes / 0 bytes |
| **Checked** | `0xAA, 0x5A` | `uint16` (little-endian) | Raw protobuf payload | `uint16` (CRC-16 LE) | 4 bytes / 2 bytes |

<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC, /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED, /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

- **Legacy Framing**: Used by firmware and apps prior to marked frames. A 32-bit unsigned little-endian length (`2..=8186`, `LEGACY_MIN_PAYLOAD = 2` so runs of zeros are not parsed as empty frames) followed immediately by the protobuf payload. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#LEGACY_MIN_PAYLOAD -->
- **Marked Framing**: Starts with magic bytes `[0xAA, 0x55]` (`FRAME_MAGIC`) followed by a 16-bit little-endian length and the protobuf payload. Allows fast resynchronization past stray serial bytes. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Checked Framing**: Starts with magic bytes `[0xAA, 0x5A]` (`FRAME_MAGIC_CHECKED`) followed by a 16-bit little-endian length, the protobuf payload, and a 2-byte little-endian CRC-16 trailer. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->

### CRC-16 Specification

Checked frames use CRC-16/CCITT-FALSE (check value `0x29B1` for `"123456789"`): <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16, /firmware/main/protocol/frame_parser.c#frame_crc16 -->

- **Polynomial**: `0x1021`
- **Initial Value**: `0xFFFF`
- **RefIn / RefOut**: `false` / `false`
- **XorOut**: `0x0000`
- **Coverage**: Computed over the 2-byte length field concatenated with the payload bytes (the 2 magic bytes are excluded). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#checked_crc, /firmware/main/protocol/frame_parser.c#checked_crc -->
- **Encoding**: Appended after the payload in little-endian byte order. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#encode_frame, /firmware/main/protocol/frame_parser.c#frame_write_crc -->

### Frame Size Limits

- **Header overhead**: 4 bytes (`HEADER_BYTES` / `FRAME_HEADER_SIZE`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#HEADER_BYTES -->
- **CRC trailer overhead**: 2 bytes (`CRC_BYTES` / `FRAME_CRC_SIZE`) on checked frames only. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#CRC_BYTES -->
- **Maximum payload size**: 8186 bytes (`MAX_PAYLOAD_BYTES = 8192 - 4 - 2` / `FRAME_MAX_PAYLOAD`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#MAX_PAYLOAD_BYTES -->
- **Stale partial frame timeout**: 500 ms (`RX_STALE_US = 500000` on firmware, `STALE_PARTIAL_FRAME = 500 ms` on host). <!-- src: /firmware/main/protocol/protocol.c#RX_STALE_US, /desktop/crates/opad-device/src/lib.rs#STALE_PARTIAL_FRAME -->

Frames with lengths exceeding 8186 bytes or failing CRC validation are rejected by advancing the scan position by one byte so any real frame glued immediately after a truncated frame is still recovered. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed, /desktop/crates/opad-protocol/src/lib.rs#decode_frame -->

## Framing Negotiation

To preserve bidirectional compatibility (AGENTS.md §1.3), framing is negotiated on every connection: <!-- src: /desktop/crates/opad-device/src/lib.rs#Handshake, /firmware/main/protocol/protocol.c#s_crc_answer -->

1. While the pad's framing is unknown, the host sends `Hello` (`frame_crc = true`, `he_calibration = true`) every 400 ms (`HELLO_RETRY`), alternating between **Marked** (`0xAA, 0x55`) and **Legacy** framing until the pad replies. <!-- src: /desktop/crates/opad-device/src/lib.rs#hello_framing, /desktop/crates/opad-device/src/lib.rs#hello_message -->
2. On the first decoded host message, the pad latches that framing family (`s_host_framing`) and answers in the same framing. <!-- src: /firmware/main/protocol/protocol.c#s_host_framing -->
3. If the host sent a Marked `Hello` with `frame_crc = true` and the firmware supports checked frames, the pad sends `HelloAck` with `frame_crc = true` as a **Marked** frame, then switches TX to **Checked** framing (`0xAA, 0x5A` + CRC-16) for all subsequent messages (`s_tx_crc = true`). On receiving `HelloAck.frame_crc = true`, the host also switches TX to Checked framing. <!-- src: /firmware/main/protocol/protocol.c#s_tx_crc, /desktop/crates/opad-device/src/lib.rs#Handshake -->
4. If either side does not support CRC (or if Legacy framing was negotiated), both sides stay on Marked or Legacy framing for the session. Dropping DTR (closing the port) resets framing and CRC state on the pad. <!-- src: /firmware/main/usb/usb_cdc.c#tud_cdc_line_state_cb, /firmware/main/protocol/protocol.c -->

## Top-Level Envelopes

All protocol messages use two root protobuf envelopes with a `sequence_number` and a `oneof payload`: <!-- src: /protocol/opad.proto#HostToDevice, /protocol/opad.proto#DeviceToHost -->

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

- **`Hello` / `HelloAck`**: Handshake carrying `protocol_version`, `client_version`, `firmware_version`, `board_profile`, `device_id`, counter generation and Key 1/2 totals, 16-byte `owner_id`, `running_partition` (`ota_0` / `ota_1` / `factory`), `current_config`, `input_module` (`InputModule`), `key_count`, and capability flags (`key_press_times`, `board_test`, `builtin_animations`, `restart`, `he_input`, `he_calibration`, `he_live`, `tap_rate_per_second`, `frame_crc`). <!-- src: /protocol/opad.proto#Hello, /protocol/opad.proto#HelloAck -->
- **`ClaimOwnership`**: Sends a 16-byte `owner_id` to pair the pad with a host install in NVS. Honored only in `IDLE` and rejects empty or all-zero IDs. <!-- src: /protocol/opad.proto#ClaimOwnership, /firmware/main/config/device_config.c#device_config_claim_owner -->
- **`DeviceStatus`**: Returned in response to `request_status = true`. Reports `uptime_seconds`, `state` (`DeviceState`), `brightness`, `display_asleep`, `lifetime_key1`/`2`/`3`, `map_key1`/`2`/`3`, on-device latency stats (`latency_samples`, `p50`, `p99`, `p999`, `max`, `hid_deferred_reports`), `display_ok`, `nvs_ok`, `nvs_writes`, and `he_calibration` (`HeCalibrationStatus`). <!-- src: /protocol/opad.proto#DeviceStatus -->
- **`HostStatus`**: Sent every 1 second and on state transitions to inform the pad of `tosu_connected`, `playing`, `play_id` (resetting map press counters when changed), and `osu_active` (whether volume swipes target osu! or system volume). <!-- src: /protocol/opad.proto#HostStatus, /desktop/daemon/src/runtime.rs#HOST_STATUS_INTERVAL -->
- **`restart`**: Requests a clean firmware restart (`esp_restart`) once the pad is `IDLE` and its OTA image is confirmed; saves dirty counters, config, layouts, and calibration before restarting. <!-- src: /protocol/opad.proto#HostToDevice, /firmware/main/runtime/runtime.c#restart_now -->

### Configuration Messages

- **`SetConfig` / `ConfigAck`**: Updates device settings via `ConfigPayload` and returns success status, message, and the active `current_config`. Persistence to NVS is deferred until `IDLE` if received during `PLAYING` or `COOLDOWN`. <!-- src: /protocol/opad.proto#SetConfig, /protocol/opad.proto#ConfigAck, /firmware/main/config/device_config.c#write_to_nvs -->
- **`ConfigPayload`**: Carries `key1_hid_usage`, `key2_hid_usage`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, deprecated `press_color_rgb`, `key1_gpio`, `key2_gpio`, per-direction `SwipeAction` / key / modifier fields (`swipe_{left,right,up,down}_{action,key,modifiers}`), per-key Hall Effect settings (`key1_he`, `key2_he`, `key3_he`), optional `key3_hid_usage`, and optional `game_keys` bitmask. <!-- src: /protocol/opad.proto#ConfigPayload -->

### Gameplay & Screen Messages

- **`TimeSync`**: Synchronizes the pad's wall-clock date and time (`year`, `month`, `day`, `hour`, `minute`, `second`). <!-- src: /protocol/opad.proto#TimeSync -->
- **`GameplayDisplayState`**: Legacy beatmap display message carrying `title`, `artist`, `current_pp`, `progress_ratio` (`0.0`–`1.0`), `current_map_presses_k1`/`k2`, `play_id`, `difficulty`, and `stars`. <!-- src: /protocol/opad.proto#GameplayDisplayState -->
- **`DataUpdate`**: Streams changed UI data source values (`repeated DataValue values`, up to 32 per message), each carrying a `source` ID (`ui_source_t`) and `oneof value { double number; string text; bool clear; }`. <!-- src: /protocol/opad.proto#DataUpdate, /protocol/opad.proto#DataValue -->
- **`SetLayout` / `LayoutAck`**: Pushes a custom screen layout (`screen`, `background` RGB, and up to 32 `UiWidget` entries) and confirms acceptance or reports validation errors. <!-- src: /protocol/opad.proto#SetLayout, /protocol/opad.proto#LayoutAck -->
- **`reset_layout`**: Resets the given screen ID (`uint32`) back to its built-in default layout. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`PlayAnimation`**: Plays a result animation (`animation` ID: `0` stops, `1..127` built-in, `128+n` reserved) with the grade string (`"SS"`, `"S"`, `"A"`, ...). Ignored during a map and stopped immediately when a map starts. <!-- src: /protocol/opad.proto#PlayAnimation -->

### Hall Effect Calibration & Telemetry Messages

- **`HeKeySettings`**: Per-key settings in 0.1% of calibrated span (`1..1000`): `actuation`, `release_hyst`, `top_reset`, `rt_press`, `rt_release`, optional `bottom_dead_zone`, `rt_mode` (`HeRtMode`), and optional `allow_below_noise_floor`. <!-- src: /protocol/opad.proto#HeKeySettings, /protocol/opad.proto#HeRtMode -->
- **`HeCalibrationStatus`**: Embedded in `DeviceStatus`. Reports per-key calibrated flags and rest/full raw counts for Keys 1–3, `is_locked`, `is_calibrating`, `noise_floor_rest`, and `noise_floor_movement`. <!-- src: /protocol/opad.proto#HeCalibrationStatus -->
- **`start_he_calibration` / `cancel_he_calibration`**: Starts or cancels interactive Hall Effect calibration. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`HeCalibrationProgress`**: Reports active calibration stage (`HeCalStage`), `key_index` (`1`–`3`, `0` during rest/idle/complete), `presses_done` (`0..5`), `error` (`HeCalError`), `error_message`, live `raw_value`, and saved `key{1,2,3}_{rest,full}` counts on success. <!-- src: /protocol/opad.proto#HeCalibrationProgress, /protocol/opad.proto#HeCalStage, /protocol/opad.proto#HeCalError -->
- **`he_live_view` / `HeLiveSample`**: Sending `he_live_view = true` streams raw ADC counts (`key1_raw`, `key2_raw`, `key3_raw`) and boolean pressed states (`key1_down`, `key2_down`, `key3_down`) at ~50 Hz (`HE_LIVE_PERIOD_US = 20000`) for 2 seconds (`HE_LIVE_KEEPALIVE_US = 2000000`); the host refreshes it to keep streaming and converts raw counts to travel using `HeCalibrationStatus`. Never streamed during `PLAYING` or `COOLDOWN`. <!-- src: /protocol/opad.proto#HeLiveSample, /firmware/main/protocol/protocol.c#HE_LIVE_PERIOD_US -->

### Counters, Hardware Diagnostics & Self-Test Messages

- **`CounterSyncRequest` / `CounterSyncResponse`**: Reconciles persistent lifetime counts (`CounterState`, with optional `lifetime_key3` and `force_restore` flag) and returns the pad's synchronized state. <!-- src: /protocol/opad.proto#CounterSyncRequest, /protocol/opad.proto#CounterSyncResponse -->
- **`DetectPinRequest` / `DetectPinResponse`**: Scans candidate switch GPIOs for `timeout_ms` (ignoring `exclude_gpio`) to detect which pin `key_id` is wired to. <!-- src: /protocol/opad.proto#DetectPinRequest, /protocol/opad.proto#DetectPinResponse -->
- **`run_board_test` / `BoardTestResult`**: Runs the carrier and input module PCB test in `IDLE`: measures module ID voltages (`id_mv`, `id_loaded_mv`, `reverse_probe_mv`), reports `boot_module`/`module` (`InputModule`), `cable_reversed`, key GPIOs and pull-up/pull-down/bridged pin masks, and on Hall Effect V3 includes `HeAdcTest` (ADS7028 reset status, register walk, offset calibration, SCLK speed sweep with `0xA5A`, and 3 sensor channel bursts). <!-- src: /protocol/opad.proto#BoardTestResult, /protocol/opad.proto#HeAdcTest -->
- **`KeyPress` / `KeyPressBatch`**: Batches accepted game-key press timestamps (`key`, `t_us` in pad microseconds, plus batch `now_us` and `dropped` count) during `HostStatus.playing` for accurate KPM tap-rate computation. <!-- src: /protocol/opad.proto#KeyPressBatch, /firmware/main/protocol/protocol.c#protocol_send_key_presses -->
- **`reset_latency_stats`**: Clears the pad's on-device key-edge-to-HID latency histogram and deferred report counter. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`request_logs` / `LogEvent` / `LogEventBatch`**: Drains diagnostic ring-buffer events (up to 8 per batch) from the pad to the daemon. <!-- src: /protocol/opad.proto#LogEventBatch, /firmware/main/protocol/protocol.c#protocol_send_log_batch -->

## Nanopb Buffer and Field Limits

Fixed string, byte, and repeated array bounds are defined in `protocol/opad.options`: <!-- src: /protocol/opad.options -->

| Field | Max Size / Limit | Allocation Type |
|---|---|---|
| `Hello.client_version` | 32 bytes | Fixed string |
| `HelloAck.firmware_version` | 32 bytes | Fixed string |
| `HelloAck.board_profile` | 32 bytes | Fixed string |
| `HelloAck.device_id` | 32 bytes | Fixed string |
| `HelloAck.owner_id` | 16 bytes | Fixed byte array (`bytes`) |
| `HelloAck.running_partition` | 17 bytes | Fixed string |
| `ClaimOwnership.owner_id` | 16 bytes | Fixed byte array (`bytes`) |
| `ConfigAck.message` | 64 bytes | Fixed string |
| `GameplayDisplayState.title` | 64 bytes | Fixed string |
| `GameplayDisplayState.artist` | 64 bytes | Fixed string |
| `GameplayDisplayState.difficulty` | 32 bytes | Fixed string |
| `DataValue.text` | 64 bytes | Fixed string |
| `DataUpdate.values` | 32 elements | Repeated message |
| `UiWidget.label` | 32 bytes | Fixed string |
| `UiWidget.suffix` | 12 bytes | Fixed string |
| `SetLayout.widgets` | 32 elements | Repeated message |
| `LayoutAck.message` | 64 bytes | Fixed string |
| `PlayAnimation.grade` | 4 bytes | Fixed string |
| `CounterState.device_id` | 32 bytes | Fixed string |
| `CounterSyncResponse.message` | 64 bytes | Fixed string |
| `LogEvent.tag` | 16 bytes | Fixed string |
| `LogEvent.message` | 64 bytes | Fixed string |
| `LogEventBatch.events` | 8 elements | Repeated message |
| `KeyPressBatch.presses` | 32 elements | Repeated message |
| `BoardTestResult.message` | 64 bytes | Fixed string |
| `HeAdcTest.message` | 48 bytes | Fixed string |
| `HeAdcTest.speeds` | 6 elements | Repeated message |
| `HeAdcTest.channels` | 3 elements | Repeated message |
| `HeCalibrationProgress.error_message` | 48 bytes | Fixed string |

These static bounds allow the ESP32-S3 firmware to encode and decode protobuf messages without dynamic heap allocation. <!-- src: /firmware/main/protocol/protocol.c#protocol_encode_device_message -->
