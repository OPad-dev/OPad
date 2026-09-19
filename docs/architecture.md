# OPad Architecture

## 1. Design Overview
OPad is a competitive two-key osu! keypad and telemetry display based on the Waveshare ESP32-S3-Touch-LCD-2 board.

The system is separated into two strictly decoupled layers:
1. **Low-Latency Keyboard Subsystem (ESP32-S3 firmware)**:
   - Targets a 1 ms (1000 Hz) USB HID polling interval.
   - Eager debounce: First edge accepted immediately in RAM, followed by lockout window (500–20,000 µs, default 5,000 µs).
   - Dedicated hardware GPIO interrupts waking the highest-priority input task on Core 0.
   - Operates fully independently from display, host software, CDC telemetry, tosu, or persistence.
2. **Auxiliary Telemetry & Management Subsystem (Host Daemon + Display)**:
   - Single-owner headless daemon (`osupad-daemon`) handling SQLite storage, tosu WebSocket connection, USB CDC protocol framing, and background synchronization.
   - Desktop application (`osupad-gui`) hosting the system tray, configuration controls, layout designer, and live diagnostics.
   - Command-line tool (`osupadctl`) for scripting, diagnostics, backups, and native USB firmware flashing.
   - Inter-process communication exclusively through local Unix domain socket IPC (`osupad-ipc`).

---

## 2. Invariant: Latency Always Wins
Per Section 3 of the technical specification:
> *"If a feature measurably worsens keyboard latency or latency jitter, the feature is reduced, deferred, frozen during gameplay, or removed."*

- **Zero Wait**: No code path handling key input may wait on LCD SPI transfers, NVS commits, CDC serialization, dynamic memory allocation, or host communication.
- **Zero Disk Writes**: When tosu reports `PLAYING` or `COOLDOWN`, all writes to SQLite on the host and NVS on the ESP32-S3 are strictly blocked and buffered in RAM.

---

## 3. Dual-Core Task & Interrupt Map

To shield the key input path from any display or communication stalls, tasks and interrupts are partitioned across the two Xtensa LX7 cores:

```text
+-----------------------------------------------------------------------+
|                                CORE 0                                 |
|  (Dedicated Low-Latency Input & USB HID Stack)                        |
|                                                                       |
|  - Keypad GPIO Edge ISR (Priority 3, ESP_INTR_FLAG_IRAM)              |
|  - Keypad Processing Task (Priority configMAX_PRIORITIES - 1)         |
|  - TinyUSB Stack Task (Priority configMAX_PRIORITIES - 2)             |
+-----------------------------------------------------------------------+
                                   | (Lock-free atomic state & RAM queues)
                                   v
+-----------------------------------------------------------------------+
|                                CORE 1                                 |
|  (Auxiliary Telemetry, Display, & Host Protocol)                      |
|                                                                       |
|  - USB CDC Protocol Task (Priority 5)                                 |
|  - Runtime Supervisor & Cooldown Timer (Priority 4, 100 Hz tick)      |
|  - LVGL Port Task (Priority 3, ST7789 SPI DMA transfers)              |
|  - Display SPI Interrupt Service Routine                              |
+-----------------------------------------------------------------------+
```

---

## 4. LVGL Display Subsystem (A1)

The telemetry display is driven by LVGL v8 via Espressif's `esp_lvgl_port` component:
- **Core Isolation**: The LVGL task and ST7789 SPI DMA controller run exclusively on Core 1. Display refreshes cannot preempt or introduce jitter into Core 0 input tasks.
- **Thread Safety**: All screen updates and data updates acquire `ui_lock()` before modifying LVGL object properties and release `ui_unlock()` immediately.
- **Gameplay Refresh Cap**: While in `PLAYING` mode, display rendering is throttled to 10 Hz (`gameplay_display_hz`), reducing SPI bus traffic by 83% while keeping gameplay animations crisp and fluid.
- **Inactivity Sleep**: If no keypress occurs for `display_sleep_seconds` (default 600 s / 10 min), the display backlight turns off to prevent panel burn-in. The very first keypress edge turns the backlight back on instantly without latency penalty.

---

## 5. Layout Designer & Data-Source Pipeline (A2)

The display interface is completely customizable through the PC layout designer:
1. **Layout Designer**: Built into `osupad-gui` using `iced`. Users can position labels, gauge arcs, hit counters, UR bars, and telemetry widgets on a 240x320 canvas.
2. **Preview Engine**: Powered by `osupad-ui-preview`, an embedded host-side C renderer mirroring firmware draw calls with 100% pixel-level parity.
3. **Data Sources**: Data sources are numbered `0..31` (`ui_source.h`). Sources include:
   - Live gameplay: Current PP, Combo, Accuracy, BPM, Map Progress, Star Rating, Hits (300/100/50/Miss), Key 1/2 Session & Map counts.
   - Hardware stats: Device uptime, Core temperature, Key latency min/max/avg.
4. **Binary Data Updates**: The host daemon polls tosu v2 WebSocket, extracts changed telemetry fields, and sends delta updates via `DataUpdate` over USB CDC.
5. **Storage**: Custom screen layouts are saved to the host SQLite database (`layouts` table) and uploaded to the ESP32-S3 over CDC (`SetLayout`), where they are persisted in NVS during `IDLE` state.

---

## 6. System Tray & Desktop Integration (A5)

The system tray icon is hosted by `osupad-gui` rather than the background daemon:
- **Headless Daemon**: `osupad-daemon` runs as a lightweight `systemd --user` service with zero X11, Wayland, DBus UI, or graphics dependencies.
- **Desktop StatusNotifierItem**: On Linux the GUI implements the modern Freedesktop StatusNotifierItem protocol via `ksni` with XEmbed fallback, integrating smoothly into GNOME (via AppIndicator), KDE Plasma, Sway, and Waybar. On Windows the same tray is `tray-icon` over `Shell_NotifyIcon`, behind the same `cfg` split.
- **Single Instance Control**: `osupad-gui` enforces single-instance execution via a dedicated Unix domain socket (`osupad-gui.sock`), or a named object on Windows. Launching a second instance sends a focus message to the existing window and exits cleanly.
- **Autostart**: On Linux the GUI installs `osupad-gui.desktop` to `~/.config/autostart/` with the `--minimized` flag. On Windows it writes `HKCU\...\Run` instead. Either way, startup is silent and straight to the tray.

---

## 6b. IPC transport abstraction (§W0-1)

The daemon owns the device and the database; the GUI and the CLI are clients.
Everything between them is one framed, transport-agnostic protocol:

```
[len: u32 LE][json]        IpcRequest / IpcResponse
```

The framing and the two enums are identical on every platform. **Only the
concrete stream type differs**, and it is a `#[cfg]`-selected alias rather than
a trait or a runtime choice:

| Alias | Unix | Windows |
|---|---|---|
| `IpcStream` | `tokio::net::UnixStream` | `NamedPipeClient` |
| `IpcServerStream` | `UnixStream` | `NamedPipeServer` |
| `IpcListener` | wraps `UnixListener` | one pipe instance per accept |

`$XDG_RUNTIME_DIR/osupad/daemon.sock` on Unix,
`\\.\pipe\osupad-ipc-<user SID>` on Windows. The address is per-user on both,
and so is the access control: `0700` plus a peer check on the socket, a
protected DACL granting only SYSTEM and the calling user on the pipe. If the
Windows side cannot determine the SID it refuses to listen rather than create an
unrestricted pipe.

Two rules that are easy to get wrong and are therefore load-bearing:

- **The accept loops are written per platform**, not shared. A `NamedPipeServer`
  is consumed on connect and a fresh instance must exist for the next client.
- **The handshake rejects a version mismatch.** The daemon, GUI and CLI always
  ship together, so differing versions mean an in-place update replaced the files
  while the old daemon is still running (§U-2). Failing loudly beats misbehaving
  subtly.

`docs/windows-portability.md` has the as-built detail.

---

## 6c. Device pairing / ownership (§W3)

The pad records which host install owns it. This exists to protect **counter
integrity** — a pad that silently changes hands takes 20,000 lifetime presses
somewhere unexpected — and it is not, and must not become, a lock:

- Nothing is cryptographic. No attestation, no signed handshake, no anti-tamper.
- The firmware stays flashable over USB by design.
- **The pad is a keyboard on any machine, with no software and no pairing.**
  Ownership lives entirely on the CDC side.

How it works:

1. Each install generates a **UUIDv4 on first run**, stored in `app_state`
   (§W3-1). No storage means no identity, which means it claims nothing and
   prompts about nothing.
2. The pad keeps a 16-byte `owner_id` in its NVS config blob (§W3-2). Absent or
   all zero means unclaimed.
3. On connect, `HelloAck` carries `owner_id`, and the daemon decides **before
   anything reconciles the counters** — that ordering is the whole point.
   Unclaimed is claimed silently; ours proceeds; someone else's raises the
   takeover prompt and blocks counter sync until it is answered.
4. `ClaimOwnership` is an NVS write, so the firmware honours it **only in
   IDLE** (P1-3). An all-zero claim is refused, so there is no wire path to
   unpairing.
5. The only unbind is a documented full reflash (§W3-4, `docs/recovery.md` §7).
   No GUI button, no on-device factory reset.

---

## 7. Host Runtime State Machine

The daemon tracks osu!lazer gameplay status through tosu's WebSocket API and advances through four states:
- `IDLE`: Normal operation. Telemetry HUD shows clock, total lifetime counters, and system health. Configuration changes and NVS saves execute immediately.
- `PLAYING`: Triggered when tosu reports active gameplay (`state: 2`). All SQLite and NVS disk writes are strictly prohibited. Display refresh is capped at 10 Hz.
- `COOLDOWN`: Exactly 5 seconds after map conclusion or retry. Quick retries instantly transition back to `PLAYING` without flushing to disk.
- `SYNC`: Triggered when cooldown safely expires in `IDLE`. The daemon reconciles lifetime counters, executes pending configuration changes, commits to SQLite, and updates device NVS.

---

## 8. Technical Decisions & Resolution Matrix (§39)

| Topic | Decision | Technical Rationale |
|---|---|---|
| **Switch Pinout** | Default `GPIO 14` (Key 1), `GPIO 9` (Key 2); configurable | Defaults are on Header P2 (pins 11 & 12) next to GND (pin 13). Other pins are chosen from a fixed list of header GPIOs (`config_validate.c`, `KEY_PINS` in `osupad-model`) that avoids strapping pins, USB, UART0, the I2C bus, and `GPIO 17` (board pull-down). |
| **Graphics Engine** | LVGL v8 (`esp_lvgl_port`) | Provides anti-aliased font rendering, robust object hierarchy, and native FreeRTOS SPI DMA task synchronization on Core 1. |
| **Core Affinity** | Core 0: Input & HID; Core 1: Display & Protocol | Guarantees hard physical isolation so display rendering and serial framing cannot interrupt or jitter the 1000 Hz USB HID loop. |
| **Gameplay Hz** | Default 10 Hz (configurable 1–60 Hz) | Eliminates bus contention on Core 1 during dense mania/standard maps while maintaining fluid visual feedback. |
| **Firmware Flashing** | Native `espflash` integration in `osupadctl` and the daemon | Enables single-binary flashing over USB CDC / ROM DFU without requiring Python, esptool, or external dependencies. One engine (`osupad_device::flash`) serves both, so the sequence that can stop the pad being a keyboard exists once. |
| **Partition Layout** | Two 2 MB OTA slots, `nvs` unmoved at `0x9000` (§U-3a) | The table at `0x8000` cannot be rewritten by an OTA update; it needs a serial reflash of every pad in the field. Done while the field is empty, and before the OTA code that will need it. |
| **Firmware Updates** | Host-driven flash of the app partition only, explicit consent every time (§U-3b) | A firmware update is the one operation that can stop the pad being a keyboard. Counters are synced to the host first, the image is verified against the signed manifest *and* its own chip ID, and `erase-flash` is never used because that is where the counters live. |
| **Device Pairing** | Ownership claim in NVS, no enforcement (§W3) | Protects counter integrity from a pad changing hands silently. Deliberately not DRM: anything baked into firmware plus app is extractable from either, so pretending otherwise would buy nothing and cost the user their own hardware. |
| **USB Serial** | Runtime MAC-derived (`OSUPAD-<MAC>`) | Ensures multiple connected pads get unique `/dev/serial/by-id/` entries with zero serial collisions. |

