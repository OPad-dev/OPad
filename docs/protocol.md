# osu!pad USB CDC Framing & Protocol

## 1. Framing Specification (§23)
USB CDC is a streaming byte-oriented transport. Framing is achieved by prepending each protobuf-encoded envelope with a 4-byte little-endian length prefix:

```text
+------------------------------------+--------------------------------+
| Length Prefix (4 bytes, uint32-LE) | Protobuf Envelope (N bytes)    |
+------------------------------------+--------------------------------+
```

### Protocol Constraints:
- Maximum frame size: **8192 bytes** (`PROTOCOL_MAX_FRAME_SIZE`).
- Messages exceeding 8192 bytes are safely dropped by the stream parser without crashing or dynamic allocation.
- Disconnections or serial port resets discard any partial buffer and re-synchronize on the next valid length-delimited boundary.

---

## 2. Protobuf Message Envelopes

All messages are defined in [`protocol/osupad.proto`](file:///home/paella/Documents/projects/esp32/osu-pad-esp32/protocol/osupad.proto) using NanoPB-compatible schemas.

Every message transmitted in either direction is wrapped in a top-level envelope containing a 32-bit sequence number:

```protobuf
message HostToDevice {
    uint32 sequence_number = 1;
    oneof payload { ... }
}

message DeviceToHost {
    uint32 sequence_number = 1;
    oneof payload { ... }
}
```

---

## 3. HostToDevice Payloads

| Tag | Message | Description |
|---|---|---|
| 1 | `Hello` | Initial handshake upon port discovery. Contains `protocol_version` (1) and `client_version`. |
| 2 | `TimeSync` | Real-time clock synchronization with year, month, day, hour, minute, second. |
| 3 | `SetConfig` | Applies device parameters: `key1_hid_usage`, `key2_hid_usage`, `debounce_us` (500–20,000 µs), `brightness` (0–100%), `display_sleep_seconds`, and `gameplay_display_hz` (1–60). *(Note: `press_color_rgb` is deprecated; highlight colors now come from layouts).* |
| 4 | `CounterSync` | Lifetime counter synchronization with `target_state` (`counter_generation`, `lifetime_key1`, `lifetime_key2`) and `force_restore` flag. |
| 5 | `HostStatus` | Host daemon status: `daemon_state` (`IDLE`, `PLAYING`, `COOLDOWN`) and `active_screen`. |
| 6 | `DataUpdate` | Real-time telemetry batch containing up to 32 `DataValue` elements (`source` ID `0..31`, variant of `number` or `text`). |
| 7 | `SetLayout` | Uploads custom screen layout JSON / binary definition for `screen` ID `0..3`. |
| 8 | `ResetLayout` | Resets specified screen to firmware default layout. |
| 9 | `RequestLogs` | Requests a batch of buffered diagnostic logs from device RAM. |
| 10 | `RequestStatus` | Requests instantaneous device status and hardware statistics. |
| 11 | `ResetLatencyStats` | Clears cumulative latency statistics (min, max, average, and histogram buckets). |
| 12 | `EnterBootloader` | Instructs device to restart immediately into native USB ROM DFU bootloader for flashing. |

---

## 4. DeviceToHost Payloads

| Tag | Message | Description |
|---|---|---|
| 1 | `HelloAck` | Handshake response. Returns `protocol_version`, `firmware_version`, `board_profile`, runtime MAC-derived `device_id`, `counter_generation`, and current lifetime presses. |
| 2 | `ConfigAck` | Acknowledges `SetConfig` with `success` boolean, descriptive status message, and current applied `ConfigPayload`. |
| 3 | `CounterSyncResp` | Acknowledges `CounterSync` with `success` boolean, error message, and confirmed synchronized `CounterState`. |
| 4 | `LayoutAck` | Acknowledges `SetLayout` or `ResetLayout` with `screen` ID, `success` boolean, and status message. |
| 5 | `Status` | Periodic or requested device health: `uptime_ms`, `runtime_state`, lifetime counters, and latency statistics (min, max, avg, samples, buckets). |
| 6 | `LogBatch` | Batch of diagnostic entries from the in-RAM ring buffer: `timestamp_ms`, severity `level`, `event_id`, and optional `arg0`/`arg1`. |

---

## 5. Version Negotiation Rules

1. When the host opens the USB CDC port, it sends `Hello` with `protocol_version = 1`.
2. The device responds with `HelloAck` containing its own `protocol_version`.
3. If `device.protocol_version != host.protocol_version`:
   - The device is marked as `IncompatibleDevice` in daemon runtime state.
   - All telemetry and configuration synchronization is safely suspended.
   - **USB HID Keyboard functionality continues to operate without interruption**, ensuring player input is never broken by an outdated daemon or firmware.
   - The GUI surfaces an incompatibility warning informing the user to update.

---

## 6. Firmware Update & Bootloader Entry

The device supports two non-contact methods to enter the ESP32-S3 ROM bootloader for firmware flashing:

1. **Protocol Command (`EnterBootloader`)**:
   - `osupadctl flash <firmware.bin>` sends `EnterBootloader` over CDC.
   - The firmware calls `esp_restart()` with ROM download mode flags set, immediately re-enumerating as an Espressif USB JTAG/serial DFU device (`VID: 0x303A, PID: 0x1001`).
2. **1200-Baud Touch (Fallback)**:
   - Setting serial line baud rate to 1200 baud and toggling DTR/RTS triggers ROM bootloader entry even if the application firmware is unresponsive or in an unexpected state.

