---
title: Shared Crates Reference
description: Technical catalog and responsibilities of the nine internal shared Rust crates in desktop/crates/.
---

The desktop Cargo workspace ([/desktop/Cargo.toml](/desktop/Cargo.toml)) splits shared logic into nine internal crates under `desktop/crates/`:

## Crate Catalog

### 1. `opad-model` ([/desktop/crates/opad-model/src/lib.rs](/desktop/crates/opad-model/src/lib.rs))
Domain models, validation, and cross-platform helpers:
- Core structs: `DeviceInfo`, `DeviceConfig`, `CounterState`, `Key3Counters`, `LatencyStats`, `JsonBackup`, `HeKeyConfig`, `HeCalibrationStatus`, `HeLiveSample`.
- Submodules: `paths` ([/desktop/crates/opad-model/src/paths.rs](/desktop/crates/opad-model/src/paths.rs)), `board_test`, `diag`, `log`, `result_anim`, `swipe`, `tap_rate`, `ui_source`.

### 2. `opad-protocol` ([/desktop/crates/opad-protocol/src/lib.rs](/desktop/crates/opad-protocol/src/lib.rs))
Pure-Rust serial protocol codec:
- Compiles [/protocol/opad.proto](/protocol/opad.proto) via `protox` + `prost` in `build.rs` (no external `protoc` required).
- Implements Legacy, Marked (`0xAA 0x55`), and Checked (`0xAA 0x5A` + CRC-16/CCITT-FALSE) framing (`crc16`).

### 3. `opad-ipc` ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs))
Local IPC between `opad-daemon`, `opad-gui`, and `opadctl`:
- Unix stream sockets (`0700`/`0600`) in [/desktop/crates/opad-ipc/src/transport/unix.rs](/desktop/crates/opad-ipc/src/transport/unix.rs) and Windows named pipes (`\\.\pipe\opad-ipc-<SID>` with protected SDDL `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)`) in [/desktop/crates/opad-ipc/src/transport/windows.rs](/desktop/crates/opad-ipc/src/transport/windows.rs).
- Length-delimited UTF-8 JSON framing (`MAX_REQUEST_FRAME_SIZE = 1 MiB`, `MAX_RESPONSE_FRAME_SIZE = 8 MiB`, `IPC_PROTOCOL_VERSION = 1`).

### 4. `opad-storage` ([/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs))
SQLite persistence (`opad.db`):
- WAL mode and foreign keys enabled on open; embedded migrations up to schema v19 (`schema_migrations`, `device_state`, `config`, `layouts`, `app_state`, tap stats).
- Gameplay write guard (`StorageError::WritesBlocked`) and `reconcile_counters`.

### 5. `opad-device` ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs))
Serial device manager and firmware flasher:
- Auto-discovers `303a:4001`, negotiates Marked/Legacy framing (`HELLO_RETRY = 400 ms`), exposes `Ownership` state.
- Implements hands-free flashing (`flash.rs`: `plan_app_update`, `flash_firmware`, `recover`, `reboot_to_bootloader`) wrapping pinned `espflash` **4.6.0** plus RTC watchdog reset out of ROM download mode.

### 6. `opad-layout` ([/desktop/crates/opad-layout/src/lib.rs](/desktop/crates/opad-layout/src/lib.rs))
Layout schema, validation (`MAX_WIDGETS = 32`, `320x240` bounds), and wire/NVS serialization matching firmware `ui_layout_t`.

### 7. `opad-ui-preview` ([/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs))
Compiles the firmware's C LVGL layout renderer on the host to produce RGBA buffers for pixel-exact Screen Designer previews.

### 8. `opad-tosu` ([/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs))
`tosu` process supervisor (`TosuSupervisor`, binary search order, forced `tosu.env` overrides), WebSocket v2 + `/precise` parser, and Linux Yama `ptrace_access()` diagnostics.

### 9. `opad-update` ([/desktop/crates/opad-update/src/lib.rs](/desktop/crates/opad-update/src/lib.rs))
Signed update verification (minisign Ed25519 + SHA-256), `InstallOrigin` policies (`windows`, `deb`, `rpm`, `aur`, `appimage`, `user`, `source`), gameplay safety gate (`may_update_now`), atomic file writes (`write_atomic`), and the `opad-feed` manifest generator.
