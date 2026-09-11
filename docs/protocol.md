# osu!pad USB CDC Framing & Protocol

## 1. Framing Specification (§23)
USB CDC is a streaming byte oriented transport. Framing is achieved by prepending each protobuf-encoded message with a 4-byte little-endian length prefix:

```text
+------------------------------------+--------------------------------+
| Length Prefix (4 bytes, uint32-LE) | Protobuf Payload (N bytes)     |
+------------------------------------+--------------------------------+
```

### Constraints:
- Maximum payload length: **4096 bytes** (`MAX_PAYLOAD_BYTES`).
- Packets exceeding this size cause the stream buffer to be safely flushed.
- Disconnections or USB resets discard any pending partial frame.

## 2. Message Overview
The canonical definition resides in [`protocol/osupad.proto`](file:///home/paella/Documents/projects/esp32/osu-pad-esp32/protocol/osupad.proto).

### Handshake
- `HostToDevice.hello`: Sent by host upon port open. Contains protocol version and client version.
- `DeviceToHost.hello_ack`: Returned by device. Contains firmware version, board profile, device ID, counter generation, and current lifetime presses.

### Time Synchronization
- `HostToDevice.time_sync`: Host sends current local calendar time (year, month, day, hour, minute, second). The ESP advances time locally via hardware timer.

### Configuration
- `HostToDevice.set_config`: Updates key HID usages, eager debounce lockout (µs), display brightness (0-100), and display sleep timeout.
- `DeviceToHost.config_ack`: Acknowledges configuration update.

### Gameplay Display Stream
- `HostToDevice.gameplay_state`: Transmitted only during `PLAYING` mode. Contains map title, artist, current PP, song progress ratio (0.0 to 1.0), and map press counts.
- Rate-limited to ~5 Hz to guarantee zero input jitter.

### Counters & Reconciliation
- `HostToDevice.counter_sync`: Transmits target counters and generation for synchronization.
- `DeviceToHost.status`: Reports current device uptime, state, and lifetime counters.
- `DeviceToHost.log_batch`: Transmits diagnostic log events collected in ESP RAM ring buffer.
