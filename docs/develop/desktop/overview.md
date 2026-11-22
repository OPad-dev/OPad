---
title: Desktop Suite Overview
description: High-level architectural overview of the Rust desktop workspace, process model, cross-platform IPC transport, and internal crate boundaries.
---

The OPad desktop suite is implemented in Rust (edition 2021) as a Cargo workspace containing three top-level application binaries and nine internal modular library crates. It provides cross-platform support across Linux (X11 / Wayland) and Windows 10/11. <!-- src: /desktop/Cargo.toml -->

## Process Model & Separation of Concerns

The desktop suite delegates responsibilities across independent processes: <!-- src: /desktop/daemon/src/main.rs -->

```
+--------------------+            IPC             +--------------------+
|     opad-gui       | <------------------------> |    opad-daemon     |
| (iced / GUI / Tray)|   (Unix Socket / Pipe)     | (Tokio Runtime)    |
+--------------------+                            +--------------------+
                                                             |
+--------------------+            IPC                        | Serial CDC ACM
|      opadctl       | <-------------------------------------+
| (Clap CLI Utility) |                                       |
+--------------------+                                       v
                                                  +--------------------+
                                                  |    Physical Pad    |
                                                  |  (ESP32-S3 Device) |
                                                  +--------------------+
```

### 1. The Daemon (`opad-daemon`)
A headless background service running on Tokio. It holds exclusive access to the keypad's CDC-ACM serial port, runs the device actor queue, supervises `tosu`, persists counters in SQLite, and serves IPC requests. <!-- src: /desktop/daemon/src/main.rs -->

### 2. The Graphical Interface (`opad-gui`)
An iced-based desktop application. It renders live dashboard analytics, provides the visual screen designer, tunes Hall Effect rapid trigger thresholds, and displays real-time diagnostics. If launched while `opad-daemon` is offline, it spawns the daemon automatically. <!-- src: /desktop/gui/src/main.rs -->

### 3. The CLI (`opadctl`)
A command-line tool built with Clap. It provides status queries, counter synchronization, backup import/export, and direct serial flashing routines (`opadctl flash` and `opadctl recover`). <!-- src: /desktop/cli/src/main.rs -->

## Workspace Crates Topology

Internal libraries are decoupled into focused crates (`desktop/crates/`): <!-- src: /desktop/Cargo.toml -->

- **`opad-model`**: Core data models, device settings catalogs, keypin mappings, tap rate models, and path resolution helpers.
- **`opad-protocol`**: Pure-Rust protobuf codecs (`prost`/`protox`), framing parsers, magic markers, and CRC-16 computation.
- **`opad-ipc`**: Inter-Process Communication layer, Unix domain sockets, Windows named pipes with DACL security, and JSON message framing.
- **`opad-storage`**: SQLite persistence layer, schema migrations, write lockouts during gameplay, and counter reconciliation.
- **`opad-device`**: USB serial communication manager, device discovery loops, and serial flashing routines.
- **`opad-tosu`**: Supervisor managing the bundled `tosu` memory reader and WebSocket client.
- **`opad-layout`**: LCD widget layout models, font definitions, and serial pack/unpack logic.
- **`opad-ui-preview`**: Firmware LVGL display engine compiled for the desktop to provide pixel-exact designer previews.
- **`opad-update`**: Self-update engine managing GitHub release checks, signatures, and binary unpacking.
