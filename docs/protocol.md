# OPad USB CDC Framing & Protocol

## 1. Framing Specification (§23)
USB CDC is a streaming byte-oriented transport. Each protobuf-encoded envelope is
prefixed with a 2-byte start marker and a 2-byte little-endian payload length:

```text
+------+------+---------------------+--------------------------------+
| 0xAA | 0x55 | Length (uint16-LE)  | Protobuf Envelope (N bytes)    |
+------+------+---------------------+--------------------------------+
```

### Protocol Constraints:
- Maximum frame size: **8192 bytes** (`PROTOCOL_MAX_FRAME_SIZE`), so N ≤ 8188.
- **Resynchronisation.** Both parsers (`firmware/main/protocol/frame_parser.c`,
  `opad-protocol`) slide forward one byte at a time until they see `AA 55`
  followed by a length ≤ 8188. Stray bytes — ROM bootloader chatter, a
  plain-text command, a dropped byte — cost at most the frame they land in.
- The host skips a frame whose payload fails to decode by one byte and rescans,
  in case the `AA 55` was a false start inside noise.
- **Stale partial frames.** Frames are written whole on both sides, so a partial
  frame that sits for 500 ms is dropped: the pad simply discards it; the host
  discards it and re-sends `Hello`.
- The pad queues a device frame only when the whole of it fits the CDC TX FIFO;
  otherwise it drops the frame and records `DIAG_EVENT_CDC_WRITE_DROPPED`.
- Disconnections or serial port resets discard any partial buffer.

---

## 2. Protobuf Message Envelopes

All messages are defined in [`protocol/osupad.proto`](file:///home/paella/Documents/projects/esp32/OPad-esp32/protocol/osupad.proto) using NanoPB-compatible schemas.

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
| 3 | `SetConfig` | Applies device parameters: `key1_hid_usage`, `key2_hid_usage`, `debounce_us` (500–20,000 µs), `brightness` (0–100%), `display_sleep_seconds`, `gameplay_display_hz` (1–60), and `key1_gpio` / `key2_gpio` (switch pins; 0 keeps the current pin). Pins must be one of the supported header GPIOs 2, 4, 6–16, 18, 21 and differ; a pin move is applied by the keypad task once both keys are released. *(Note: `press_color_rgb` is deprecated; highlight colors now come from layouts).* |
| 4 | `CounterSync` | Lifetime counter synchronization with `target_state` (`counter_generation`, `lifetime_key1`, `lifetime_key2`) and `force_restore` flag. |
| 5 | `HostStatus` | Host daemon status: `daemon_state` (`IDLE`, `PLAYING`, `COOLDOWN`) and `active_screen`. |
| 6 | `DataUpdate` | Real-time telemetry batch containing up to 32 `DataValue` elements (`source` ID `0..31`, variant of `number` or `text`). |
| 7 | `SetLayout` | Uploads custom screen layout JSON / binary definition for `screen` ID `0..3`. |
| 8 | `ResetLayout` | Resets specified screen to firmware default layout. |
| 9 | `RequestLogs` | Requests a batch of buffered diagnostic logs from device RAM. |
| 10 | `RequestStatus` | Requests instantaneous device status and hardware statistics. |
| 11 | `ResetLatencyStats` | Clears cumulative latency statistics (min, max, average, and histogram buckets). |
| 12 | `EnterBootloader` | Instructs device to restart immediately into native USB ROM DFU bootloader for flashing. |
| 14 | `ClaimOwnership` | Records which host install owns this pad (§W3-1, §W3-2). Carries a 16-byte `owner_id`. It is an NVS write, so the firmware honours it **only in IDLE**, never during `PLAYING` or `COOLDOWN` (P1-3); the host only ever sends it at connect time, which is already an IDLE-only moment. An all-zero `owner_id` is **refused**, so there is no wire path to unpairing — that is a documented reflash (§W3-4, `docs/recovery.md` §7). Re-claiming by the current owner writes nothing. |

---

## 4. DeviceToHost Payloads

| Tag | Message | Description |
|---|---|---|
| 1 | `HelloAck` | Handshake response. Returns `protocol_version`, `firmware_version`, `board_profile`, runtime MAC-derived `device_id`, `counter_generation`, current lifetime presses, `owner_id` (field 8) and `running_partition` (field 9). |
| 2 | `ConfigAck` | Acknowledges `SetConfig` with `success` boolean, descriptive status message, and current applied `ConfigPayload`. |
| 3 | `CounterSyncResp` | Acknowledges `CounterSync` with `success` boolean, error message, and confirmed synchronized `CounterState`. |
| 4 | `LayoutAck` | Acknowledges `SetLayout` or `ResetLayout` with `screen` ID, `success` boolean, and status message. |
| 5 | `Status` | Periodic or requested device health: `uptime_ms`, `runtime_state`, lifetime counters, and latency statistics (min, max, avg, samples, buckets). |
| 6 | `LogBatch` | Batch of diagnostic entries from the in-RAM ring buffer: `timestamp_ms`, severity `level`, `event_id`, and optional `arg0`/`arg1`. |

### `HelloAck.owner_id` (field 8, §W3-1 / §W3-2)

16 bytes. Empty or all zero means **unclaimed**, which is also what firmware
predating W3-2 sends — the two are deliberately indistinguishable, so an older
pad is treated as unclaimed rather than as belonging to nobody in particular.

The host decides from this and its own install identity:

| `owner_id` | Host action |
|---|---|
| Absent / all zero | Claim it silently with `ClaimOwnership`. The common case, and it is not worth a prompt. |
| This install's | Nothing. Proceed normally. |
| A different install's | **Prompt** (§W3-3). Counter sync is blocked until the user answers. The pad keeps working as a keyboard throughout — that was never in question. |
| Any of the above, but this install has no identity | Claim nothing, prompt about nothing. A daemon with no storage has no identity (§W3-1). |

The prompt offers three answers: take the pad over keeping its counters, take it
over keeping this PC's, or leave it alone. "Leave it alone" suppresses config,
layouts, telemetry and all syncing, and the pad is still a keyboard.

This is an **ownership model, not DRM**. Nothing here is cryptographic and
nothing is enforced; it protects counter integrity from a pad silently changing
hands, and that is all it is for.

### `HelloAck.running_partition` (field 9, §U-3a)

The label of the app partition the running image booted from: `ota_0`, `ota_1`,
or `factory` for a pad still on the pre-OTA single-app table. An **empty string**
means firmware older than the field, which the host reads as unknown rather than
guessing a slot. Surfaced by `osupadctl status` as `Running Slot`.

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
   - Setting serial line baud rate to 1200 baud arms download mode; the firmware reboots when DTR drops, i.e. when the host closes the port.
The esptool DTR/RTS pattern (RTS falling while DTR stays high) is deliberately
**not** a trigger: ModemManager and other serial probes produce it when they open
any tty, and each one used to reboot the pad into download mode.

`osupadctl` tries both in that order, setting DTR and RTS explicitly rather
than relying on what the platform does at open — Linux asserts DTR when a tty is
opened and Windows does not, so an implicit sequence means different things on
the two platforms (§W1-3).

### Partition layout (§U-3a)

Since the two-slot table, the **app image is written at `0x20000`** (`ota_0`),
not at `0x10000`. `nvs` stays at `0x9000` at its original `0x6000` size, which
is why the lifetime counters and `owner_id` survive a firmware update.

A host-driven update writes the **app partition only** and never `erase-flash`
(§U-3b); `erase-flash` takes NVS with it and is the documented unbind path, not
an update mechanism (`docs/recovery.md` §7).

