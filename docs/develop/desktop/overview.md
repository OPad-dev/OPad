---
title: Desktop Suite Overview
description: High-level architectural overview of the Rust desktop workspace, process model, cross-platform IPC transport, and internal crate boundaries.
---

The OPad desktop suite is a Rust (edition 2021) Cargo workspace ([/desktop/Cargo.toml](/desktop/Cargo.toml)) containing three binaries (`opad-daemon`, `opad-gui`, `opadctl`) and nine internal library crates supporting Linux (X11/Wayland) and Windows 10/11.

## Process Model & Separation of Concerns

```text
+--------------------+            IPC             +--------------------+
|     opad-gui       | <------------------------> |    opad-daemon     |
| (iced / GUI / Tray)|   (Unix Socket / Pipe)     | (Tokio Runtime)    |
+--------------------+                            +--------------------+
                                                             |
+--------------------+            IPC                        | USB CDC-ACM
|      opadctl       | <-------------------------------------+
| (Clap CLI Utility) |                                       |
+--------------------+                                       v
                                                  +--------------------+
                                                  |    Physical Pad    |
                                                  |  (ESP32-S3 Device) |
                                                  +--------------------+
```

1. **`opad-daemon`** ([/desktop/daemon/src/main.rs](/desktop/daemon/src/main.rs)): Headless Tokio background service holding the pad's CDC-ACM serial port, running the ordered device actor, supervising `tosu`, persisting state in SQLite (`opad.db`), and serving local IPC.
2. **`opad-gui`** ([/desktop/gui/src/main.rs](/desktop/gui/src/main.rs)): `iced`-based graphical app and system tray applet (`--tray`, `--page <name>`). Spawns `opad-daemon` automatically if offline.
3. **`opadctl`** ([/desktop/cli/src/main.rs](/desktop/cli/src/main.rs)): Clap-based CLI communicating over IPC for management/diagnostics and directly over USB serial for `flash`, `recover`, and `bootloader`.

## Workspace Crates (`desktop/crates/`)

- **`opad-model`** ([/desktop/crates/opad-model/src/lib.rs](/desktop/crates/opad-model/src/lib.rs)): Domain types (`DeviceInfo`, `DeviceConfig`, `CounterState`, `JsonBackup`), paths, swipe actions, tap-rate math, board-test grading, result animations.
- **`opad-protocol`** ([/desktop/crates/opad-protocol/src/lib.rs](/desktop/crates/opad-protocol/src/lib.rs)): Pure-Rust protobuf (`protox`/`prost`), Marked/Checked framing, CRC-16/CCITT-FALSE.
- **`opad-ipc`** ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs)): Length-prefixed JSON over Unix domain sockets (`0700`/`0600`) or Windows named pipes (protected user-only SDDL DACL).
- **`opad-storage`** ([/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs)): SQLite (WAL, schema v19), gameplay write guard (`StorageError::WritesBlocked`), counter reconciliation.
- **`opad-device`** ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs)): USB CDC discovery, handshake/framing negotiation, and `espflash`-based flashing/recovery.
- **`opad-tosu`** ([/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs)): `tosu` child-process supervisor, v2/precise WebSocket client, Linux Yama `ptrace` checks.
- **`opad-layout`** ([/desktop/crates/opad-layout/src/lib.rs](/desktop/crates/opad-layout/src/lib.rs)): `320x240` layout validation and binary packing matching firmware `ui_layout_t`.
- **`opad-ui-preview`** ([/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs)): Host-compiled firmware LVGL preview renderer for the GUI Screen Designer.
- **`opad-update`** ([/desktop/crates/opad-update/src/lib.rs](/desktop/crates/opad-update/src/lib.rs)): Signed update-feed verification (minisign Ed25519 + SHA-256), install-origin policies, atomic binary swaps, and `opad-feed`.
