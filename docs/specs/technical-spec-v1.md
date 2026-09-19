# OPad - ESP32-S3 Low-Latency osu! Keypad and Telemetry Display

**Technical Specification / Implementation Contract - v1.0**  
**Primary target:** Waveshare ESP32-S3-Touch-LCD-2 + Linux + osu!lazer AppImage  
**Secondary target:** Windows host support without firmware redesign  
**Working binary names:** `osupad-daemon`, `osupad-gui`, `osupadctl`

---

## 0. Instructions for Antigravity and other coding agents

Read this document completely before implementing anything. Treat it as the project contract.

The project is intentionally small in product scope but strict in architecture. The single most important requirement is **input latency**. Every design choice must preserve the keyboard path. Convenience features are optional if they interfere with input. The keypad must remain a good two-key USB keyboard even if every host-side component is missing or broken.

Interpret requirement keywords as follows:

- **MUST**: required for v1.0; do not change without an explicit project decision.
- **MUST NOT**: prohibited.
- **SHOULD**: preferred unless there is a concrete technical reason not to do it.
- **MAY**: optional; do not implement merely because it is possible.

Before implementing a subsystem, first check whether ESP-IDF, a mature Rust crate, or an established external tool already solves it. **Do not reimplement solved infrastructure.** Prefer a dependency or external tool unless it measurably compromises input latency, reliability, portability, or maintainability.

Do not expand scope. In particular, do not turn this into a general macro pad, RGB keyboard, configurable key matrix, touch UI, plugin framework, or replacement for tosu.

---

# 1. Project summary

OPad is a two-key osu! keypad based on an ESP32-S3 board with an integrated 2-inch LCD. It has two responsibilities that must remain architecturally independent:

1. **Low-latency keyboard:** two physical MX-compatible switches are exposed to the host as USB HID keys, defaulting to `Z` and `X`.
2. **Small telemetry display:** when osu!lazer is being played, the LCD shows a minimal set of live data from tosu. When osu! is not being played, it behaves as a small desk clock and shows lifetime key-press counters.

The same USB-C connection is used for power, USB HID, and host-to-device communication.

The device also keeps lifetime press counters for both physical keys. The counters exist on both the ESP32 and the PC so either side can restore the other after a failure. The PC stores its persistent state in SQLite. Manual import/export is JSON.

The host architecture is deliberately split into a persistent background daemon and a separate configuration GUI. The GUI may remain closed indefinitely; the daemon owns the hardware, tosu connection, SQLite database, tray icon, synchronization, and logs.

---

# 2. Product goals

## 2.1 Required goals

The finished v1.0 MUST:

- Act as a two-key USB keyboard with default mappings `Z` and `X`.
- Work as a keyboard with **no daemon, no GUI, no tosu, and no osu! running**.
- Use the ESP32-S3 native USB peripheral as a USB Full-Speed composite device.
- Target a 1 ms HID polling interval (1000 Hz USB HID endpoint interval).
- Prioritize minimum and deterministic input latency over all display/statistics features.
- Keep all storage, LCD rendering, host communication, logging, and synchronization out of the key input critical path.
- Count accepted key-down presses independently for physical key 1 and key 2.
- Persist lifetime counters across normal power cycles.
- Use tosu as the osu!lazer integration layer rather than implementing osu! memory/process reading.
- Show live gameplay information on the ESP display: current PP, song progress, title, artist, and current-map press counts.
- Show a desk-clock idle screen when not playing, including lifetime key counters.
- Allow display brightness and display sleep timeout to be configured from the desktop GUI.
- Put the display to sleep after the configured idle interval and wake it when a physical key is pressed. The key event MUST be serviced before display wake work.
- Keep a PC-side persistent copy of lifetime counters and configuration in SQLite.
- Provide JSON import/export for portable backups.
- Provide a desktop monitor view that shows host logs and delayed ESP diagnostic events.
- Provide a system tray item owned by the daemon with only essential ESP/device information and actions to open Settings or Monitor.
- Use a separate iced desktop GUI for configuration and inspection.
- Detect whether the daemon is running by IPC connectivity, not by process-name scanning.
- Be designed so Windows support can be added without changing the firmware protocol or core application model.

## 2.2 Non-goals

v1.0 MUST NOT attempt to provide:

- More than two physical gameplay keys.
- Keyboard layers, macros, rapid trigger, Hall-effect support, RGB, key matrices, or QMK/VIA compatibility.
- A touchscreen UI on the ESP.
- Wi-Fi, Bluetooth, cloud accounts, telemetry upload, or Internet dependency.
- NTP on the ESP.
- A custom replacement for tosu.
- A general-purpose ESP32-S3 board autodetection framework.
- Runtime support for arbitrary vendor boards.
- Continuous historical analytics of every gameplay session unless required later by an explicit scope change.
- Per-keypress disk or flash writes.
- Per-keypress log lines.
- A web UI or Electron-based GUI.

---

# 3. Core invariant: latency always wins

The project has one overriding rule:

> **If a feature measurably worsens keyboard latency or latency jitter, the feature is reduced, deferred, frozen during gameplay, or removed.**

This rule overrides display refresh rate, PP freshness, logging detail, persistence immediacy, tray behavior, and convenience.

No code path handling a key edge may wait for:

- LCD/SPI activity
- NVS/flash
- SQLite
- CDC serial output
- tosu
- logging
- memory allocation that can block unpredictably
- filesystem operations
- GUI state
- IPC
- Wi-Fi/Bluetooth

The baseline for all later performance comparisons is **HID-only firmware**. Each secondary subsystem is added only after the keyboard path is benchmarked.

---

# 4. Hardware target

## 4.1 Primary board

The officially supported v1.0 board is:

**Waveshare ESP32-S3-Touch-LCD-2**

The project uses the board for:

- ESP32-S3 MCU
- native USB
- integrated LCD
- backlight control
- nonvolatile flash

The touchscreen is intentionally unused.

The implementation MUST verify the Waveshare schematic/pinout before assigning gameplay GPIOs. Two exposed GPIOs that do not conflict with USB, LCD, PSRAM, bootstrapping, or other required board functions MUST be selected in the board-support layer.

## 4.2 Switch wiring

The mechanical switches are ordinary MX-compatible digital switches. Firmware must not depend on a specific switch vendor.

Electrical model:

```text
GPIO_KEY_1 ---- switch ---- GND
GPIO_KEY_2 ---- switch ---- GND

GPIO input mode: pull-up
Pressed state: LOW
Released state: HIGH
```

No custom PCB is planned. Dupont wiring is acceptable for v1.0.

## 4.3 Mechanical enclosure

The physical design is expected to place the LCD above the two switches with the LCD tilted toward the user. The project SHOULD reuse and edit an existing enclosure model if a suitable one exists rather than designing from zero.

Mechanical requirements:

- USB-C remains accessible.
- Two MX-compatible switch positions only.
- Screen mounted above and angled toward the user.
- Wiring should not place mechanical load on the board connectors.
- No requirement for touch access.

The enclosure is independent from the firmware except for final GPIO/wiring documentation.

---

# 5. Board portability policy

The firmware SHOULD have a small Board Support Package (BSP) abstraction, but v1.0 MUST support only the Waveshare board.

Do **not** implement runtime board autodetection. Similar ESP32-S3 LCD boards can differ in LCD controller, pin assignment, backlight polarity, reset wiring, USB routing, PSRAM, and power circuitry; a generic runtime detector is not worth the maintenance burden.

Recommended source organization:

```text
firmware/
  main/
    input/
    usb/
    display/
    counters/
    protocol/
    runtime/
  boards/
    waveshare_esp32s3_touch_lcd_2/
      board.c
      board.h
      board_display.c
      board_pins.h
```

Core firmware MUST access board-specific functionality through a narrow board API such as:

```text
board_init()
board_get_key1_gpio()
board_get_key2_gpio()
board_display_init()
board_display_set_brightness()
board_display_sleep()
board_display_wake()
```

If another developer wants to support another board, they may add another compile-time board profile or fork the project. The core HID/counter/protocol logic should not require modification.

**Decision:** portable architecture yes; universal board support no.

---

# 6. High-level system architecture

```text
                       +-------------------+
                       |    osu!lazer      |
                       |    AppImage       |
                       +---------+---------+
                                 |
                                 | observed by
                                 v
                       +-------------------+
                       |       tosu        |
                       +---------+---------+
                                 |
                           WebSocket v2
                                 |
                                 v
+----------------------+   +-------------------------+
|     osupad-gui       |   |     osupad-daemon      |
|      iced/Rust       |<->|         Rust            |
|                      |IPC|                         |
| configuration        |   | tosu manager            |
| device status        |   | device manager          |
| import/export JSON   |   | state machine           |
| monitor              |   | SQLite owner            |
| firmware actions     |   | synchronization         |
+----------------------+   | clock sync              |
                           | tray                     |
                           | log buffers              |
                           +------------+------------+
                                        |
                             USB CDC    |    USB HID
                                        |
                                        v
                           +-------------------------+
                           |        ESP32-S3         |
                           |                         |
                           | key GPIO -> HID         |
                           | RAM counters            |
                           | NVS lifetime backup     |
                           | LCD                     |
                           | CDC protocol            |
                           +-------------------------+
```

The daemon is the single owner of:

- the serial/CDC connection to the ESP
- the tosu WebSocket
- the SQLite database
- runtime state
- device synchronization
- tray state
- log aggregation

The GUI MUST NOT open the serial device or SQLite database directly.

---

# 7. Firmware technology choices

## 7.1 Language and SDK

Firmware MUST use **C/C++ with ESP-IDF**.

Use existing ESP-IDF components whenever possible:

- Native USB device stack / TinyUSB integration for HID and CDC.
- NVS for nonvolatile counters/configuration that must live on device.
- `esp_lcd` and existing LCD panel support where compatible.
- FreeRTOS facilities provided by ESP-IDF.
- `esp_timer` / monotonic timing facilities.

Do not replace these with custom implementations unless latency testing proves a specific upstream component unsuitable.

## 7.2 Features intentionally disabled

The following should not be initialized in normal firmware:

- Wi-Fi
- Bluetooth
- touch controller
- unnecessary sensors/peripherals
- microSD, if present

The goal is a small deterministic firmware, not a showcase of every peripheral on the board.

---

# 8. USB architecture

The ESP MUST enumerate as one composite USB device containing at least:

1. **USB HID Keyboard interface** - time-critical gameplay input.
2. **USB CDC ACM interface** - configuration, telemetry, synchronization, clock, diagnostics, and host-to-display data.

The HID endpoint SHOULD request a 1 ms polling interval.

The keyboard interface MUST work independently from the CDC interface. If the CDC side is unopened, disconnected, or in an error state, HID must continue functioning normally.

The firmware MUST NOT require the host daemon to complete a handshake before enabling HID.

Boot priority:

```text
1. Essential board/GPIO initialization
2. USB HID usable
3. CDC available
4. Load counters/configuration
5. LCD initialization
6. Nonessential runtime features
```

An LCD failure MUST NOT prevent HID operation.

---

# 9. Key input critical path

## 9.1 Event model

Each physical key uses GPIO edge notification/interruption. The ISR must do the minimum work required to capture the event and wake/notify the high-priority input task.

Conceptual path:

```text
MX switch edge
    -> GPIO ISR
    -> high-priority input task notification
    -> update logical key state
    -> submit HID report
```

The ISR MUST NOT:

- render the LCD
- write NVS
- communicate with the host over CDC
- log strings
- allocate heap memory
- synchronize with SQLite or any host feature

## 9.2 Debounce strategy

Traditional "wait N milliseconds before accepting a press" debounce is prohibited because it adds deliberate input latency.

Use **eager debounce**:

1. The first eligible edge is accepted immediately.
2. HID state is updated immediately.
3. A short configurable lockout window suppresses bounce transitions.
4. At the end of the lockout, the physical GPIO may be re-sampled and corrected if necessary.

Default debounce value is expected around 2-3 ms, but final default MUST be determined by real MX switch testing. The GUI MAY expose debounce as an advanced setting.

Both press and release must remain responsive. Counters increment only on accepted press transitions, never on release.

## 9.3 Key mapping

Internally call the physical inputs `KEY_1` and `KEY_2` rather than hardcoding osu! semantics throughout the firmware.

Defaults:

```text
KEY_1 -> Z
KEY_2 -> X
```

The desktop configuration MAY allow changing their HID usages. The product still supports exactly two gameplay keys.

---

# 10. Firmware scheduling and priority

The exact ESP-IDF task/core arrangement should follow current USB stack constraints, but conceptually the firmware MUST separate critical input work from auxiliary work.

Desired priority model:

```text
Highest priority:
  GPIO/input state
  HID submission

Medium/low priority:
  CDC receive/transmit
  gameplay display updates
  clock display
  counter synchronization
  diagnostics

Lowest / deferred:
  NVS writes
  log draining
  configuration persistence
```

If core affinity is used, input/HID work should be isolated from display/storage work where this is compatible with ESP-IDF/TinyUSB operation.

Do not assume dual-core separation alone guarantees low latency. Benchmark the final implementation.

---

# 11. Runtime states

The host daemon uses **tosu's state** as the source of truth for whether osu! is actively playing. Do not infer gameplay from key presses or inactivity.

Required host/device behavior is organized around four logical modes:

```text
IDLE
  |
  | tosu state -> PLAYING
  v
PLAYING
  |
  | tosu leaves PLAYING
  v
COOLDOWN (5 seconds)
  | \
  |  \ tosu returns to PLAYING
  |   +-----------------------> PLAYING
  |
  | 5 s expires
  v
SYNC/IDLE
```

## 11.1 PLAYING

This is the competitive/minimal-latency mode.

Allowed:

- key input/HID
- RAM counter increments
- tosu reception on the PC
- a rate-limited host-to-LCD gameplay state stream
- minimal state needed for correctness

Prohibited/deferred:

- NVS writes
- SQLite writes
- backup writes
- firmware checks
- log-to-disk flushing
- counter reconciliation
- unnecessary discovery/scanning
- verbose ESP logging

## 11.2 COOLDOWN

Default duration: **5 seconds after tosu leaves PLAYING**.

During cooldown, behave approximately like PLAYING. This prevents a quick retry or next map from colliding with flash/database/synchronization work.

If tosu returns to PLAYING before the timer expires, immediately return to PLAYING without doing deferred work.

The 5-second value is a design constant for v1.0 unless testing demonstrates a concrete reason to change it. It does not need to be a user-facing setting.

## 11.3 SYNC/IDLE

After cooldown expires:

- synchronize lifetime counters
- commit pending SQLite changes
- write ESP NVS checkpoints when required
- sync clock
- apply deferred configuration
- drain buffered ESP diagnostic events
- perform safe maintenance work

Once synchronization finishes, remain in IDLE. Do not create artificial continuous work just because more tasks are permitted.

---

# 12. Lifetime and map counters

The ESP maintains at least:

```c
uint64_t lifetime_key1;
uint64_t lifetime_key2;
uint32_t map_key1;
uint32_t map_key2;
```

Requirements:

- `lifetime_*` survive normal reboots/power cycles through NVS checkpointing.
- `map_*` reset when a new gameplay session/map begins.
- `map_*` are temporary and do not need persistent storage.
- Lifetime counters increment in RAM in the low-latency input path after accepting a press.
- There MUST NOT be a flash/NVS write per press.

## 12.1 Power-loss trade-off

True zero-loss persistence for every press would require synchronous nonvolatile writes or additional hardware such as FRAM. Both conflict with the current hardware/scope and latency requirement.

Therefore the explicit v1.0 policy is:

- During PLAYING and COOLDOWN, counters live in RAM.
- After the cooldown, lifetime counters are synchronized to PC SQLite and checkpointed to ESP NVS.
- A sudden power loss during a map can lose lifetime presses since the last completed synchronization.
- This is accepted because **latency has higher priority than perfect mid-map persistence**.

Do not silently "fix" this by adding per-press flash writes.

---

# 13. Counter reconciliation and recovery

Both the ESP and PC store a device identity and counter generation.

Logical persistent fields:

```text
device_id
counter_generation
lifetime_key1
lifetime_key2
```

`counter_generation` changes when the user intentionally resets/imports a new counter baseline. It prevents an old backup from being mistaken for a newer counter value simply because its numeric count is larger.

For the same `device_id` and `counter_generation`, reconciliation SHOULD choose the highest valid lifetime value for each key unless the operation is an explicit user-directed restore/import.

Supported recovery cases:

1. **PC database lost, ESP intact:** import current device state from ESP to SQLite.
2. **ESP replaced/reflashed, PC intact:** restore lifetime state from SQLite/JSON to the ESP.
3. **Both present but one side stale:** reconcile after cooldown and make both sides equal.
4. **Manual JSON import:** require explicit confirmation; update generation to prevent accidental rollback ambiguity.

Resetting lifetime counters MUST require deliberate confirmation in the GUI. There is no reset menu on the ESP itself.

---

# 14. LCD behavior

## 14.1 Rendering philosophy

The display is secondary. It MUST NOT compromise input.

Use the simplest practical display stack. Do not use LVGL unless a hard requirement appears later that cannot be met cleanly without it. For v1.0, direct rendering through ESP-IDF LCD facilities is preferred.

Avoid full-screen redraws during gameplay. Update only regions whose values changed.

Use DMA/nonblocking SPI support where the board/display driver supports it, but still benchmark HID jitter.

## 14.2 Gameplay screen

Required information:

- current PP
- song progress
- title
- artist
- current-map physical key press counts

Example layout only; visual design may be refined:

```text
+----------------------------+
| Freedom Dive               |
| xi                         |
|                            |
|          286.4 PP          |
|                            |
| [###############---] 82%   |
|                            |
| K1 2381        K2 2197     |
+----------------------------+
```

The project does not need animations.

Gameplay LCD refresh must be rate limited. Start with approximately 5 Hz. The final shipped default should be the **highest refresh rate that passes the latency regression gate**. It is acceptable for PP/progress to update less frequently than tosu produces data.

If LCD updates measurably degrade input latency or jitter even after optimization/rate reduction, freeze or disable gameplay LCD updates. The keyboard wins.

## 14.3 Idle clock screen

When not playing, show a minimal clock and lifetime counters, for example:

```text
+----------------------------+
|                            |
|          00:53             |
|       12 September         |
|                            |
| K1  1,284,391              |
| K2  1,176,822              |
|                            |
+----------------------------+
```

The ESP does not use NTP or Wi-Fi. The PC daemon sends local wall-clock time. The ESP advances the displayed time using its local monotonic/timekeeping facilities until the next host sync.

The host SHOULD send time:

- when the device connects
- after entering idle/sync
- periodically while idle if appropriate

Do not perform time synchronization during PLAYING merely for clock accuracy.

## 14.4 Brightness and display sleep

Configurable from the desktop GUI:

- brightness, default 100%
- display sleep timeout

When the sleep timer expires:

- turn off/backlight-sleep the display
- stop unnecessary display updates

On a key press while the display sleeps:

1. service the key/HID event first
2. schedule/display wake second

Entering PLAYING MAY also wake the display from the host/display task.

---

# 15. tosu integration

The project MUST use tosu for osu!lazer integration. Do not read osu! process memory, files, IPC internals, or databases directly to reproduce information already exposed by tosu.

The daemon connects to tosu's supported local WebSocket API and extracts only the data the device needs:

- gameplay state / whether currently playing
- beatmap title
- artist
- current PP
- current beatmap time/progress information required to derive progress

Keep the tosu adapter behind a small interface so upstream API changes are isolated.

The daemon should be able to reconnect if tosu restarts.

If tosu is unavailable:

- HID remains unaffected
- the ESP returns/remains in non-gameplay display mode
- the tray may indicate the device only; it does not need osu status
- the GUI may expose tosu diagnostics in a detailed status/monitor view

Do not make the tray menu about osu/tosu state.

---

# 16. Host daemon

## 16.1 Purpose

`osupad-daemon` is the persistent background component. The GUI is not required for normal operation.

The daemon owns:

- ESP device discovery/connection
- CDC protocol
- tosu WebSocket
- runtime state machine
- SQLite storage
- counter synchronization
- clock synchronization
- tray icon/menu
- log aggregation
- IPC server for GUI/CLI
- invocation/orchestration of external flashing tools when requested

## 16.2 Language

Host software should be Rust unless a specific subsystem is materially better served by an external tool in another language. Do not force everything into one language if doing so means reimplementing mature functionality.

Expected Rust ecosystem building blocks include, subject to final dependency review:

- Tokio for async runtime/tasks
- tokio-tungstenite or equivalent for tosu WebSocket
- tokio-serial / serialport for CDC
- rusqlite for SQLite
- serde / serde_json for JSON
- tracing / tracing-subscriber for structured host logging
- interprocess or another mature local IPC abstraction
- iced for GUI
- tray-icon for system tray
- clap for command-line tooling
- prost for host-side protobuf if protobuf is selected as specified below

Pin dependency versions in Cargo.lock for releases. Do not update dependencies merely because newer versions exist after v1.0 is stable.

---

# 17. Daemon deployment and lifecycle

## 17.1 Linux

Primary Linux behavior:

- daemon starts automatically in the user's graphical login session
- no root privileges for normal runtime
- use a `systemd --user` service where supported
- provide a fallback XDG autostart/manual mechanism if the target environment does not provide a usable systemd user session
- tray integration must not be allowed to crash the core daemon

Because the daemon owns a graphical tray icon, installation must ensure the process receives the graphical session environment required by the tray backend.

## 17.2 Windows

Windows support is a later platform target but MUST be considered in protocol/core design.

Expected differences:

- COM port rather than `/dev/ttyACM*`
- named pipe rather than Unix domain socket
- startup-at-login rather than systemd user service

Core application models, JSON format, SQLite schema, device protocol, and firmware must remain unchanged.

---

# 18. Desktop GUI

## 18.1 Framework and role

The configuration application uses **Rust + iced**.

The GUI is not the runtime owner. It is a client of the daemon and may be closed at any time.

On launch it MUST:

1. Attempt the daemon IPC connection.
2. Perform a protocol/version handshake.
3. Show whether the daemon is available.
4. If unavailable, provide a clear recovery action such as Start Daemon / Install or Repair Daemon, where platform support allows.

Do not detect the daemon by searching process names or PID files alone.

## 18.2 Suggested navigation

```text
Overview
Statistics
Input
Display
Device
Backup
Monitor
```

### Overview

Show:

- daemon running/offline
- ESP connected/disconnected
- firmware version
- board profile/device ID
- lifetime key 1/key 2 counters
- last successful synchronization

Detailed osu status is not necessary here unless needed for troubleshooting.

### Statistics

Show at least:

- lifetime key 1
- lifetime key 2
- total lifetime presses
- PC-stored values vs ESP-stored values when useful
- last synchronization time

No requirement for historical charts in v1.0.

### Input

Allow:

- mapping KEY_1 (default Z)
- mapping KEY_2 (default X)
- advanced debounce configuration

Show 1000 Hz target as informational, not as a normal user slider.

### Display

Allow:

- brightness
- sleep timeout
- gameplay refresh rate only if exposing it remains useful after benchmark tuning

Keep UI simple. The device does not need theme/rich animation settings.

### Device

Show:

- device ID
- board profile
- firmware version
- connection state
- ESP lifetime counters
- PC lifetime counters
- sync state

Actions:

- synchronize now, only when safe
- restore ESP from PC
- import PC state from ESP
- update firmware
- reset lifetime counters with strong confirmation

### Backup

Actions:

- export JSON
- import JSON
- validate backup before applying it
- show a summary/diff before destructive restore

### Monitor

See section 24.

---

# 19. System tray

The tray belongs to the daemon, not the GUI.

The tray is intentionally device-focused. Do not fill it with osu status.

Suggested native menu:

```text
OPad
--------------------------
ESP32: Connected
Firmware: 1.0.0
Key 1: 1,284,391
Key 2: 1,176,822
Last sync: 00:47
--------------------------
Open Settings
Open Monitor
--------------------------
Quit Daemon
```

The exact rendering is platform-dependent. Use a mature tray library instead of implementing Win32/DBus/AppIndicator plumbing manually.

Tray failure MUST NOT terminate HID, synchronization, or the daemon core. Treat tray as an optional frontend layer.

`Open Settings` launches or focuses `osupad-gui`. `Open Monitor` launches/focuses the GUI directly on the Monitor section.

---

# 20. SQLite storage

## 20.1 Decision

SQLite is the canonical PC-side persistent store. This is appropriate because the project now has multiple related persistent concerns: device identity, counters, settings, synchronization metadata, and migrations.

Use `rusqlite` unless a concrete requirement appears for an async database abstraction. This application does not need a connection pool or database server.

## 20.2 Ownership

Only the daemon accesses SQLite.

The GUI and CLI communicate with the daemon through local IPC. This avoids multiple writers and keeps runtime policy centralized.

## 20.3 Gameplay write policy

During PLAYING and COOLDOWN:

**SQLite writes MUST be zero.**

Runtime changes live in memory and are committed after the system enters SYNC/IDLE.

The database may remain open, but no gameplay-triggered transaction may touch disk.

## 20.4 Suggested schema

Keep the schema minimal and migration-friendly. Example:

```sql
CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE device_state (
    device_id TEXT PRIMARY KEY,
    board_profile TEXT NOT NULL,
    firmware_version TEXT,
    counter_generation INTEGER NOT NULL,
    lifetime_key1 INTEGER NOT NULL,
    lifetime_key2 INTEGER NOT NULL,
    last_seen_at TEXT,
    last_sync_at TEXT
);

CREATE TABLE config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    key1_hid_usage INTEGER NOT NULL,
    key2_hid_usage INTEGER NOT NULL,
    debounce_us INTEGER NOT NULL,
    brightness INTEGER NOT NULL,
    display_sleep_seconds INTEGER NOT NULL,
    gameplay_display_hz INTEGER NOT NULL,
    tosu_endpoint TEXT NOT NULL
);
```

Additional tables should only be added for a concrete requirement.

WAL mode MAY be used for robustness/concurrent read characteristics, but it does not relax the no-writes-during-gameplay rule.

---

# 21. JSON backup/import/export

JSON is the public portable backup format. SQLite files are implementation details and should not be the user-facing interchange format.

The JSON format MUST be versioned.

Example:

```json
{
  "format_version": 1,
  "exported_at": "2026-09-12T00:53:00+02:00",
  "device": {
    "device_id": "a81f...",
    "board_profile": "waveshare_esp32s3_touch_lcd_2",
    "counter_generation": 4
  },
  "stats": {
    "lifetime_key1": 1284391,
    "lifetime_key2": 1176822
  },
  "config": {
    "key1": "Z",
    "key2": "X",
    "debounce_us": 5000,
    "brightness": 100,
    "display_sleep_seconds": 600,
    "gameplay_display_hz": 5
  }
}
```

Import requirements:

- validate schema/version before applying
- reject malformed/overflowing values
- show the user what will change
- require confirmation for counter rollback/reset
- perform import via daemon, never by the GUI editing SQLite directly

---

# 22. Local IPC between GUI/CLI and daemon

Use a mature local IPC abstraction rather than hand-writing platform-specific socket/pipe code.

Preferred conceptual mapping:

```text
Linux/Unix -> Unix domain socket under XDG_RUNTIME_DIR
Windows    -> named pipe
```

The GUI checks daemon health by connecting and performing a versioned handshake.

Suggested handshake data:

```text
client protocol version
daemon protocol version
daemon application version
device connected flag
```

IPC should use typed serialized messages rather than ad-hoc text parsing.

The daemon remains the authority for whether an operation is safe while PLAYING. If the GUI requests a persistence-sensitive operation during gameplay, the daemon may reject it or queue it for idle, depending on the operation.

---

# 23. ESP <-> daemon protocol

## 23.1 Do not hand-roll serialization

Because the two sides are C/C++ and Rust, use generated serialization rather than maintaining parallel manual parsers.

Recommended v1.0 choice:

- **Protocol Buffers schema** as the single message definition.
- **nanopb** generated C/C++ code on ESP32.
- **prost** generated Rust code on the host.

The `.proto` files live in a shared `protocol/` directory and are the source of truth.

## 23.2 Framing

USB CDC is a byte stream, so message boundaries still require framing. Keep framing deliberately small.

Recommended frame:

```text
uint32 little-endian payload_length
protobuf payload bytes
```

Rules:

- maximum payload length is bounded (for example 4 KiB unless a concrete feature needs more)
- invalid lengths cause the stream/session to be reset safely
- no custom checksum is required unless testing reveals a need; USB already supplies link-level integrity
- reconnect/USB reset discards any partial frame

Avoid inventing a feature-rich binary protocol.

## 23.3 Message categories

The protocol should cover at least:

```text
Hello / HelloAck
DeviceStatus
HostConfig
SetConfig
TimeSync
GameplayDisplayState
Counters
CounterSync
LogEventBatch
PrepareForFlash / RebootToBootloader if supported safely
Ack / Error
```

Do not transmit one message for every accepted keypress during gameplay merely for statistics. Map/lifetime counters can be included in rate-limited state snapshots or synchronized after gameplay.

## 23.4 Versioning

Every protocol session must negotiate/declare a protocol version. The daemon should produce a clear diagnostic if firmware and host are incompatible.

---

# 24. Logging and Monitor

## 24.1 Host logging

Use structured logging, preferably `tracing`.

The daemon maintains an in-memory ring buffer that the GUI Monitor can subscribe to. Logs MAY also be persisted while idle if useful, but gameplay must not cause continuous disk logging.

## 24.2 ESP logging

Do not emit a log line for every press/release.

During PLAYING:

- normal/debug logging is disabled or minimized
- important diagnostic events may be encoded into a small fixed-size RAM ring buffer
- avoid expensive string formatting in the critical path

During SYNC/IDLE:

- buffered diagnostic events may be sent to the daemon in a batch
- the daemon converts event IDs/arguments into human-readable monitor entries if appropriate

## 24.3 Monitor GUI

The Monitor page should show both host and device diagnostics, for example:

```text
00:48:01 HOST INFO  ESP connected
00:48:01 ESP  INFO  HID ready
00:48:03 HOST INFO  counters synchronized
00:51:12 HOST INFO  state -> PLAYING
00:54:40 HOST INFO  state -> COOLDOWN
00:54:45 HOST INFO  state -> SYNC
```

Recommended controls:

- Clear
- Copy
- Save log
- severity filter
- source filter (HOST / ESP)

The GUI MUST NOT open the serial port itself to implement Monitor. It subscribes to daemon-provided logs over IPC.

---

# 25. Clock synchronization

The PC is the time authority.

Do not add Wi-Fi/NTP merely for the clock.

The daemon sends local date/time to the ESP. To keep the ESP free from timezone/DST rules, the message SHOULD contain already-resolved local calendar fields or an equivalent representation that does not require the ESP to understand timezone databases.

The ESP tracks elapsed seconds locally until another sync.

If the PC time changes, it is corrected at the next safe sync.

Clock accuracy is never allowed to trigger work during PLAYING.

---

# 26. Firmware update and setup tooling

The project should not implement Espressif flashing protocols itself.

Use an established tool such as **espflash** or the official ESP-IDF flashing tooling. Prefer shipping/invoking a known tested binary where licensing and packaging allow rather than requiring end users to set up a Python environment.

Expected CLI surface:

```text
osupadctl status
osupadctl setup
osupadctl flash <firmware>
osupadctl export <file.json>
osupadctl import <file.json>
osupadctl monitor
```

The GUI may call daemon/CLI operations for firmware updates, but it does not implement the bootloader protocol.

Flashing flow must coordinate device ownership:

1. GUI/CLI requests flash.
2. daemon verifies not PLAYING.
3. daemon releases/coordinates the CDC device.
4. external flashing tool performs update.
5. daemon rediscovers the device and validates firmware/protocol version.

Firmware update failure must not overwrite PC lifetime counters.

Whether A/B application OTA partitions are useful should be evaluated against flash layout and the fact that updates are local over USB. Do not build a custom OTA subsystem if external flashing already provides a safer/simpler solution.

---

# 27. Dependency/reuse policy

The project explicitly prefers mature dependencies over custom infrastructure.

Examples of functionality that SHOULD be reused:

| Need | Preferred existing solution |
|---|---|
| ESP USB HID/CDC | ESP-IDF / TinyUSB integration |
| ESP flash persistence | ESP-IDF NVS |
| LCD bus/panel | ESP-IDF `esp_lcd` / existing panel driver |
| osu!lazer telemetry | tosu |
| Rust async | Tokio |
| WebSocket | tokio-tungstenite or equivalent |
| serial | tokio-serial / serialport |
| SQLite | rusqlite |
| JSON | serde + serde_json |
| structured logs | tracing |
| GUI | iced |
| tray | tray-icon |
| local IPC | interprocess or equivalent |
| C/Rust message serialization | protobuf + nanopb + prost |
| ESP flashing | espflash / official tool |
| CLI parsing | clap |

The team/agent MUST first look for an existing maintained solution before implementing a protocol, parser, platform abstraction, database layer, logger, WebSocket stack, or flashing tool.

Exceptions are allowed only when:

- the dependency adds measurable input latency/jitter
- the dependency is substantially less reliable than a small local implementation
- packaging it would make the product materially harder to install
- licensing/security makes it unsuitable

Document any such exception.

---

# 28. Suggested repository layout

```text
osupad/
  README.md
  LICENSE
  docs/
    architecture.md
    latency-testing.md
    protocol.md
    recovery.md

  protocol/
    osupad.proto

  firmware/
    CMakeLists.txt
    main/
      app_main.c
      input/
      usb/
      display/
      counters/
      protocol/
      runtime/
    boards/
      waveshare_esp32s3_touch_lcd_2/
    components/
      nanopb/                 # preferably dependency-managed

  desktop/
    Cargo.toml                # workspace
    Cargo.lock
    crates/
      osupad-model/
      osupad-protocol/
      osupad-ipc/
      osupad-storage/
      osupad-device/
      osupad-tosu/
    daemon/
    gui/
    cli/

  packaging/
    linux/
      systemd-user/
      xdg-autostart/
      udev/
    windows/

  scripts/
    dev/
    release/

  tests/
    protocol/
    integration/
```

Keep shared models/protocol code in reusable crates so daemon, GUI, and CLI do not duplicate structures or validation logic.

---

# 29. Daemon internal modules

Recommended boundaries:

## `DeviceManager`

- discover target ESP
- own serial/CDC port
- protocol handshake
- send display/config/time messages
- receive device state/counters/log batches
- reconnect

## `TosuManager`

- connect/reconnect to tosu
- normalize the small subset of osu state used by the project
- publish `PLAYING` transitions and display data

## `RuntimeController`

- owns PLAYING/COOLDOWN/SYNC/IDLE state machine
- enforces "no persistence while playing"
- controls when sync/maintenance may run

## `Storage`

- only module that knows SQLite details
- migrations
- atomic counter/config transactions
- never called for writes while PLAYING/COOLDOWN

## `SyncManager`

- reconcile ESP/PC counters
- send config
- time sync
- perform deferred work after cooldown

## `TrayController`

- device-only status
- launch/focus GUI
- must be failure-isolated from daemon core

## `IpcServer`

- GUI/CLI requests
- status subscriptions
- log subscriptions
- operation validation

## `LogHub`

- host ring buffer
- ESP diagnostic ring/batches
- monitor subscribers

---

# 30. GUI-to-daemon operation policy

All mutating operations go through the daemon. The daemon decides whether to apply now, defer, or reject.

Examples:

| Operation | While PLAYING |
|---|---|
| Read status | Allow |
| Open monitor | Allow |
| Read lifetime counters | Allow from RAM |
| Change brightness | Prefer defer/apply only if proven harmless |
| Change key mapping | Defer until not playing |
| Change debounce | Defer until not playing |
| Reset counters | Reject/defer with explicit message |
| Import JSON | Reject/defer |
| Flash firmware | Reject |
| Force sync | Reject/defer |

This prevents the GUI from accidentally violating the latency contract.

---

# 31. Error handling and failure isolation

## ESP-side

- LCD init failure -> keyboard still works.
- CDC failure -> keyboard still works.
- malformed host frame -> reset CDC protocol session; keyboard still works.
- NVS read failure -> use safe defaults, expose diagnostic, keyboard still works.
- missing host -> idle standalone behavior; keyboard still works.

## Host-side

- tosu unavailable -> daemon/device connection remains alive; keyboard unaffected.
- ESP disconnected -> daemon/tray indicate disconnected; GUI remains usable for stored backup/config.
- SQLite failure -> do not modify ESP counters destructively; surface error.
- tray failure -> daemon continues.
- GUI crash -> daemon continues.
- daemon crash -> ESP continues as standalone HID keyboard.

---

# 32. Security and privacy

The design is local-first:

- no cloud service
- no account
- no remote telemetry
- no Internet dependency for normal operation
- tosu communication is local
- GUI-to-daemon IPC is local
- ESP communication is USB local

Local IPC should use filesystem/socket permissions appropriate to the logged-in user so another local user cannot casually issue destructive commands.

Firmware update files should be validated where practical. Release artifacts should have checksums/signatures if a public distribution workflow is created.

---

# 33. Performance and latency testing

Latency testing is a release gate, not an optional optimization pass.

## 33.1 Benchmark stages

Test at least:

**A. HID-only firmware**  
GPIO + HID, no display, no CDC runtime activity beyond enumeration.

**B. HID + CDC**  
Daemon connected, no active display rendering.

**C. HID + CDC + gameplay display**  
Normal PP/progress/title updates at candidate refresh rate.

**D. Full release configuration**  
Counters, daemon, tosu, tray, idle/sync transitions.

## 33.2 Instrumentation

Prefer hardware-assisted measurement where possible:

- toggle a spare debug GPIO immediately on accepted input event
- use a logic analyzer to correlate switch edge/debug/HID-host observations where practical
- collect internal timestamps for ISR-to-HID-submit timing in a special benchmark build
- never ship verbose benchmark logging in normal gameplay firmware

## 33.3 Gate

The final release MUST show no meaningful degradation from the HID-only baseline attributable to display/storage/CDC tasks.

A practical engineering gate should compare high percentiles, not only averages. Suggested initial gate:

- no missed accepted presses
- no stuck keys
- no periodic spikes correlated with LCD or sync work during PLAYING
- p99.9 internal input-to-HID-submit latency increase from secondary features should remain below approximately 0.1 ms relative to baseline, or be justified by measurement limitations
- no extra >1 ms-class outliers caused by display/logging/storage activity

If the gate fails, first reduce display refresh/work. If it still fails, disable gameplay display updates before compromising keyboard latency.

---

# 34. Functional tests

Firmware tests/checks should cover:

- key 1 press/release HID correctness
- key 2 press/release HID correctness
- simultaneous keys
- rapid alternating streams
- debounce bounce patterns
- display asleep then key press: HID first, wake second
- CDC absent
- malformed CDC frame
- LCD init failure simulation where feasible
- NVS missing/corrupt defaults

Host integration tests should cover:

- daemon with no ESP
- daemon with ESP but no tosu
- tosu reconnect
- PLAYING -> COOLDOWN -> PLAYING
- PLAYING -> COOLDOWN -> SYNC/IDLE
- no SQLite writes during PLAYING/COOLDOWN
- counter reconciliation both directions
- stale generation handling
- JSON export/import validation
- GUI detects absent daemon
- GUI reconnect after daemon restart
- monitor subscribe/unsubscribe
- tray failure does not stop daemon core

---

# 35. Suggested implementation phases

Each phase should end with tests; latency-sensitive phases must end with measurement before moving on.

## Phase 0 - repository and protocol skeleton

- repository layout
- ESP-IDF project boots
- Rust workspace builds
- protobuf generation path established
- CI/build scripts

## Phase 1 - HID-only keypad

- choose verified safe Waveshare GPIOs
- two switches
- eager debounce
- native USB HID
- 1 ms endpoint interval
- lifetime counters in RAM only
- latency baseline measurement

Do not continue until this is reliable.

## Phase 2 - standalone device persistence

- NVS lifetime checkpoint/load
- no per-press writes
- boot with lifetime values
- safe reset/import primitives internally

## Phase 3 - display foundation

- board LCD driver
- idle clock placeholder
- brightness/sleep/wake
- verify no input regression

## Phase 4 - CDC + generated protocol

- composite HID + CDC
- protobuf/nanopb/prost
- hello/status/config/time messages
- daemon device discovery/handshake
- verify no input regression

## Phase 5 - daemon + tosu + gameplay display

- tosu adapter
- PLAYING state from tosu
- gameplay screen data
- rate limiting/coalescing
- cooldown state machine
- tune display refresh using latency tests

## Phase 6 - SQLite + synchronization

- migrations
- PC lifetime/config persistence
- reconciliation/generation logic
- enforce zero DB writes in PLAYING/COOLDOWN

## Phase 7 - tray + IPC

- daemon tray
- Unix local IPC
- status/query operations
- launch/focus GUI actions

## Phase 8 - iced GUI

- overview/statistics/input/display/device/backup/monitor
- daemon status handling
- destructive confirmation flows

## Phase 9 - JSON backup + flashing/setup

- JSON export/import
- CLI
- external espflash integration
- install/service scripts

## Phase 10 - Windows portability pass

- named pipe backend
- COM discovery
- startup at login
- same daemon/GUI/protocol models

## Phase 11 - release hardening

- long gameplay tests
- reconnect tests
- power-cycle tests
- recovery tests
- latency regression suite
- dependency pinning
- release build/package

---

# 36. Installation/setup experience

The project does not need a fancy setup wizard, but setup should be reproducible and simple.

Desired initial Linux flow:

```text
1. Install/copy osupad-daemon, osupad-gui, osupadctl.
2. Install user service/autostart integration.
3. Install any required udev permission rule if necessary.
4. Flash supported firmware using osupadctl/espflash.
5. Start daemon.
6. Connect ESP.
7. Verify handshake and counters.
8. Configure tosu endpoint if default discovery is insufficient.
```

`osupadctl setup` MAY automate safe parts of this sequence. Do not create a large custom installer framework unless packaging needs it.

---

# 37. Configuration defaults

Initial defaults, subject to final hardware tests:

```text
KEY_1 mapping: Z
KEY_2 mapping: X
HID interval: 1 ms (not user-configurable)
Debounce: 2-3 ms eager lockout, tuned by testing
Brightness: 100%
Display sleep: 10 minutes
Gameplay display: start testing at 5 Hz
Cooldown after leaving PLAYING: 5 seconds
Wi-Fi: disabled
Bluetooth: disabled
Touch: disabled
```

If a default changes after benchmarking, document the measured reason.

---

# 38. Decisions explicitly closed

The following decisions should not be reopened during implementation without a real blocker:

- Exactly two gameplay keys.
- MX-compatible digital switches.
- One USB-C connection.
- Native USB HID + CDC composite.
- Latency outranks every other feature.
- tosu is the osu! integration.
- tosu's PLAYING state controls gameplay mode.
- 5-second post-play cooldown before persistence/sync work.
- ESP-IDF C/C++ firmware.
- Rust host stack.
- Separate daemon and iced GUI.
- Daemon may run while GUI is closed.
- Daemon owns tray, ESP, tosu, SQLite, and IPC.
- SQLite on PC.
- JSON for import/export.
- No NTP/Wi-Fi clock; PC sends time.
- No touch UI.
- No custom PCB.
- BSP abstraction but only Waveshare board officially supported in v1.0.
- No runtime generic-board autodetection.
- Reuse existing tools/libraries instead of reimplementing infrastructure.
- Monitor exists for diagnostics, but no per-keypress gameplay logging.

---

# 39. Open implementation details that agents may decide

These are intentionally not fully fixed and can be selected based on current upstream APIs and measurement:

- exact safe GPIO numbers for the Waveshare switches, after schematic verification
- exact LCD drawing primitive/library within ESP-IDF-compatible options
- exact FreeRTOS task/core affinity compatible with TinyUSB
- exact gameplay display refresh default after benchmarking
- exact protobuf message layout, provided it follows the required semantics
- exact local IPC crate if `interprocess` is unsuitable
- exact Linux tray integration details required by iced/winit/desktop environment
- exact SQLite migration mechanism
- exact packaging format(s)
- whether host runtime uses one or multiple Tokio runtimes/threads
- whether external flashing uses espflash or an official equivalent

When choosing among these, optimize in this order:

1. input latency/determinism
2. reliability
3. low maintenance
4. reuse of mature code
5. portability
6. developer convenience

---

# 40. Definition of Done for v1.0

v1.0 is complete when all of the following are true:

### Keyboard

- ESP enumerates reliably as a keyboard without host software.
- KEY_1/KEY_2 correctly produce configured HID usages.
- 1 ms HID endpoint interval is confirmed.
- Fast streams and simultaneous presses do not miss/stick.
- Display/CDC/full-stack operation passes latency regression testing.

### Device display

- Idle clock works from host-provided time.
- Lifetime counters are shown.
- Display sleeps after configured interval.
- A sleeping display wakes after a physical key press without delaying that key.
- Gameplay view shows title, artist, current PP, progress, and map key counts.

### Persistence/recovery

- ESP lifetime counters survive normal restart.
- PC lifetime counters/config survive restart in SQLite.
- No NVS/SQLite writes occur during PLAYING/COOLDOWN.
- ESP can restore PC and PC can restore ESP.
- JSON export/import works and is version validated.

### Daemon

- Runs without GUI.
- Automatically connects/reconnects to device and tosu.
- Implements PLAYING/COOLDOWN/SYNC/IDLE correctly.
- Owns tray and IPC.
- Tray failure does not kill core behavior.

### GUI

- Detects daemon via IPC.
- Allows required configuration.
- Shows lifetime/device synchronization state.
- Performs backup/recovery actions through daemon.
- Monitor displays host and ESP diagnostic events.

### Maintenance

- README contains build/install instructions.
- Architecture and protocol are documented.
- Dependencies are pinned for release.
- No unnecessary custom implementation exists where a mature dependency was selected.
- Linux release is usable; architecture is ready for Windows port without firmware redesign.

---

# 41. Final engineering principle

The product should feel boring in the best possible way:

- Plug it in: it is immediately a two-key keyboard.
- Start osu!: the display starts showing useful information.
- Stop playing: after the cooldown it safely synchronizes state.
- Close the settings app: nothing stops working.
- Kill tosu or the daemon: the keyboard still works.
- Break the LCD: the keyboard still works.
- Reinstall the PC: the ESP can restore lifetime counters.
- Replace/reflash the ESP: the PC/JSON backup can restore lifetime counters.

The project is successful when the input path is extremely small, every nonessential subsystem can fail independently, and v1.0 can be left alone for years except for compatibility fixes.

---

# Appendix A1. Device pairing / ownership claim (amendment, 2026-09-16)

**Source:** `osupad_packaging_distribution_plan.md` §W3, owner decision of
2026-09-16. This amends §12–§14 (counter reconciliation) and §26 (flashing); it
does not change §2, §3 or anything on the input path.

## A1.1 What was added

The pad records which host install owns it. A different install must take it
over explicitly instead of silently adopting its counters.

| Piece | Where | Rule |
|---|---|---|
| Install identity | Host, UUIDv4 in `app_state`, generated on first run | No storage ⇒ no identity ⇒ claims nothing, prompts about nothing |
| `owner_id` | Pad, 16 bytes in the NVS config blob (v3) | Absent or all zero = unclaimed. v1/v2 blobs migrate and read as unclaimed |
| `HelloAck.owner_id` | Protocol field 8 | Empty from firmware predating the change, which reads as unclaimed |
| `ClaimOwnership` | `HostToDevice` tag 14 | An NVS write, therefore **IDLE only** (§12.1, P1-3). An all-zero claim is refused |

At connect, and **before anything reconciles the counters**, the daemon
compares the pad's `owner_id` with its own identity: unclaimed is claimed
silently, its own proceeds, another install's raises a prompt and blocks counter
sync until answered. The prompt offers three answers — take over keeping the
pad's counters, take over keeping this PC's, or leave it alone.

"Leave it alone" suppresses config, layouts, telemetry and all syncing. **The
pad keeps working as a keyboard throughout**, which §3 never permitted to be in
question.

## A1.2 What this is not

It is an **ownership model, not a DRM scheme**, and the distinction is
deliberate rather than an admission:

- Nothing is cryptographic. No attestation, no signed handshake, no anti-tamper.
- The firmware stays flashable over USB **by design**. Anything baked into
  firmware plus app is extractable from either, so a lock would cost the user
  their own hardware and buy nothing.
- Third-party software talking to the CDC interface is not prevented. It is
  simply not supported.

It exists for one reason: a pad that changes hands silently takes tens of
thousands of lifetime presses somewhere unexpected, and those counters are the
one piece of state in this project that cannot be regenerated.

No later task may "harden" this into enforcement. That is stated here so there
is nothing to rediscover.

## A1.3 Unbinding

The **only** unbind is a documented full reflash: `espflash erase-flash`
followed by a normal flash (`docs/recovery.md` §7). There is no unpair button in
the app and no factory-reset gesture on the pad.

That is not an oversight. `owner_id` lives in NVS beside the lifetime counters,
so anything that clears one clears the other — and an unbind that could be
performed over the wire would make the claim worthless for the only thing it is
for. Requiring physical access and an erase that visibly costs the counters
keeps the two facts aligned: **you always own your hardware, and you cannot
quietly take someone else's presses.**

## A1.4 Consequences for §12–§14

The reconciliation matrix is unchanged. What changed is that it now runs *after*
the ownership decision rather than unconditionally, and a pad awaiting a
takeover answer is not reconciled at all. A reflashed pad is unclaimed, so the
first host it meets claims it — including the host that erased it, which will
also restore the counters it remembers because its generation outranks the
pad's blank one. That is correct for repairing an install and wrong for giving
the pad away; `docs/recovery.md` §7.2 says so where a user will see it.
