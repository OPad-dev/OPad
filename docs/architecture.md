# osu!pad Architecture

## 1. Design Overview
osu!pad is a competitive two-key osu! keypad and telemetry display based on the Waveshare ESP32-S3-Touch-LCD-2 board.

The system is separated into two strictly decoupled layers:
1. **Low-Latency Keyboard Subsystem (ESP32-S3 firmware)**:
   - Targets a 1 ms (1000 Hz) USB HID polling interval.
   - Eager debounce: First edge accepted immediately in RAM, followed by lockout window (500–20,000 µs, default 3,000 µs).
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
- **Desktop StatusNotifierItem**: The GUI implements the modern Freedesktop StatusNotifierItem protocol via `ksni` with XEmbed fallback, integrating smoothly into GNOME (via AppIndicator), KDE Plasma, Sway, and Waybar.
- **Single Instance Control**: `osupad-gui` enforces single-instance execution via a dedicated Unix domain socket (`osupad-gui.sock`). Launching a second instance sends a focus message to the existing window and exits cleanly.
- **Autostart**: The GUI installs `osupad-gui.desktop` to `~/.config/autostart/` with the `--minimized` flag, enabling silent background startup directly to the system tray.

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
| **Switch Pinout** | `GPIO 14` (Key 1), `GPIO 9` (Key 2) | Directly exposed on Header P2 (pins 11 & 12) next to GND (pin 13). Neither GPIO interferes with S3 strapping pins, USB JTAG, or octal PSRAM/Flash. |
| **Graphics Engine** | LVGL v8 (`esp_lvgl_port`) | Provides anti-aliased font rendering, robust object hierarchy, and native FreeRTOS SPI DMA task synchronization on Core 1. |
| **Core Affinity** | Core 0: Input & HID; Core 1: Display & Protocol | Guarantees hard physical isolation so display rendering and serial framing cannot interrupt or jitter the 1000 Hz USB HID loop. |
| **Gameplay Hz** | Default 10 Hz (configurable 1–60 Hz) | Eliminates bus contention on Core 1 during dense mania/standard maps while maintaining fluid visual feedback. |
| **Firmware Flashing** | Native `espflash` integration in `osupadctl` | Enables single-binary flashing over USB CDC / ROM DFU without requiring Python, esptool, or external dependencies. |
| **USB Serial** | Runtime MAC-derived (`OSUPAD-<MAC>`) | Ensures multiple connected pads get unique `/dev/serial/by-id/` entries with zero serial collisions. |

