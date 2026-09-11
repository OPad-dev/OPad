# osu!pad Architecture

## 1. Design Overview
osu!pad is a competitive two-key osu! keypad and telemetry display based on the Waveshare ESP32-S3-Touch-LCD-2.

The system is separated into two decoupled layers:
1. **Low-Latency Keyboard Subsystem (ESP32-S3 firmware)**:
   - Targets a 1 ms (1000 Hz) USB HID polling interval.
   - Eager debounce: First edge accepted immediately, followed by lockout window (2-3 ms).
   - Interrupt-driven GPIO edge detection waking the highest-priority input task.
   - Operates fully independently from display, host software, CDC, tosu, or persistence.
2. **Auxiliary Telemetry & Management Subsystem (Host Daemon + Display)**:
   - Single-owner daemon (`osupad-daemon`) handling SQLite storage, tosu WebSocket connection, USB CDC protocol framing, and system tray.
   - Config app (`osupad-gui`) and CLI (`osupadctl`) communicating exclusively through local Unix domain socket IPC.

## 2. Invariant: Latency Always Wins
Per Section 3 of the technical specification:
> "If a feature measurably worsens keyboard latency or latency jitter, the feature is reduced, deferred, frozen during gameplay, or removed."

No code path handling key input may wait on LCD SPI, NVS, CDC output, memory allocation, or host synchronizations.

## 3. Runtime Modes
The host state machine follows four states determined strictly by tosu:
- `IDLE`: Normal standalone operation, clock display, lifetime counters shown.
- `PLAYING`: Triggered when tosu reports state 2 (playing). Zero SQLite or NVS disk writes allowed. Telemetry stream to display rate-limited to ~5 Hz.
- `COOLDOWN`: Exactly 5 seconds after map ends. Quick retry returns immediately to `PLAYING` without flushing to disk.
- `SYNC`: Runs when cooldown expires safely: reconciles lifetime press counters, commits pending changes to SQLite, checkpoints ESP NVS, and resynchronizes local clock.
