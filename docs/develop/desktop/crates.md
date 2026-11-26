---
title: Shared Crates Reference
description: Responsibilities, key types and constants of the nine library crates in desktop/crates/.
sidebar:
  order: 5
---

The desktop workspace ([/desktop/Cargo.toml](https://github.com/OPad-dev/OPad/blob/main/desktop/Cargo.toml)) splits shared logic into nine library crates under `desktop/crates/`. How they fit together: [Desktop Suite Overview](overview.md).

## 1. `opad-model`

[/desktop/crates/opad-model/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/lib.rs). Domain types shared by every binary, with no dependency on the other crates:

- Core structs: `DeviceInfo`, `DeviceConfig`, `CounterState`, `Key3Counters`, `LatencyStats`, `JsonBackup`, `HeKeyConfig`, `HeCalibrationStatus`, `HeLiveSample`; `DEFAULT_KEY3_HID_USAGE = 0x06`.
- Submodules: `paths` (per-OS data, config and socket paths, [/desktop/crates/opad-model/src/paths.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/paths.rs)), `board_test` (grades the pad's raw PCB measurements), `diag` (names and formats the firmware's diagnostic events), `log`, `result_anim`, `swipe` (swipe actions and `OSU_SHORTCUTS`, cross-checked against the firmware's `SWIPE_LABELS`), `tap_rate`, `ui_source`.

## 2. `opad-protocol`

[/desktop/crates/opad-protocol/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-protocol/src/lib.rs). The serial codec:

- `build.rs` compiles [/protocol/opad.proto](https://github.com/OPad-dev/OPad/blob/main/protocol/opad.proto) with `protox` + `prost`; no external `protoc`.
- Legacy, marked (`0xAA 0x55`) and checked (`0xAA 0x5A` + CRC-16/CCITT-FALSE, `crc16`) framing. Format: [Serial Wire Protocol](../../reference/serial-protocol.md).

## 3. `opad-ipc`

[/desktop/crates/opad-ipc/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/lib.rs). Local IPC between the daemon, the GUI and `opadctl`:

- Unix stream socket (directory `0700`, socket `0600`) in [/desktop/crates/opad-ipc/src/transport/unix.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/transport/unix.rs); Windows named pipe `\\.\pipe\opad-ipc-<SID>` with the protected DACL `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)` in [/desktop/crates/opad-ipc/src/transport/windows.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/transport/windows.rs).
- `u32` little-endian length prefix + UTF-8 JSON (`MAX_REQUEST_FRAME_SIZE = 1 MiB`, `MAX_RESPONSE_FRAME_SIZE = 8 MiB`, `IPC_PROTOCOL_VERSION = 1`). Message catalogue: [IPC Reference](../../reference/ipc.md).

## 4. `opad-storage`

[/desktop/crates/opad-storage/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-storage/src/lib.rs). SQLite persistence in `opad.db`:

- WAL mode and foreign keys on open; embedded migrations up to schema v19 (`schema_migrations`, `device_state`, `config`, `layouts`, `app_state`, `tap_attempts` and the other tap-stat tables in [/desktop/crates/opad-storage/src/tap_stats.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-storage/src/tap_stats.rs)).
- The gameplay write guard: while the daemon has writes disallowed (`PLAYING`, `COOLDOWN`), every write returns `StorageError::WritesBlocked`. Also `reconcile_counters`.

## 5. `opad-device`

[/desktop/crates/opad-device/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-device/src/lib.rs). The serial device manager:

- **Discovery**: a port is a pad when its VID is Espressif (`ESPRESSIF_VID = 0x303A`) or pid.codes (`PIDCODES_VID = 0x1209`) **and** its product string contains "OPad" or its serial starts with `OPAD-` (or `OSUPAD-` on old firmware). It does **not** match on the PID, so `303a:8415` and the release candidates' `303a:4001` both connect; `OPAD_APP_PID = 0x8415` is only a constant. A port with no strings gets a Hello probe. The ROM bootloader (`ESP_ROM_BOOTLOADER_PID = 0x1001`) and system serial ports are skipped. Ports are rescanned every 400 ms (`PORT_SCAN_INTERVAL`).
- **Handshake**: Hellos alternate marked and legacy framing every 400 ms (`HELLO_RETRY`) until the pad answers; a `HelloAck` with `frame_crc` switches commands to checked frames ([App and Pad Versions](../../explanation/app-and-pad-versions.md)).
- **Events**: `DeviceEvent` on a broadcast channel (`EVENT_BUFFER = 256`): `Connected`, `Ownership { owner_id }`, `Disconnected`, `Counters`, `StatusUpdate` and others. The daemon decides what ownership means.
- **Flashing** ([/desktop/crates/opad-device/src/flash.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-device/src/flash.rs)), wrapping pinned `espflash` **4.6.0**:
  - `enter_bootloader` tries `BOOT_TRIGGERS` (the `BOOTLOADER` command, then the 1200-baud touch) and waits for `303a:1001`;
  - `plan_app_update` + `otadata_image` write the new image to the slot not running (`APP_PARTITION_OFFSET = 0x20000` or `OTA_1_OFFSET = 0x220000`) and point `otadata` (`OTADATA_OFFSET = 0xf000`) at it as a new image, so the bootloader rolls back if it never confirms;
  - `write_images`, `flash`, `recover` (full image set to a pad that does not enumerate as OPad, [Recovery](../../recovery.md)), `wait_for_port`, `wait_until_openable`;
  - `reset_to_app` leaves download mode with an RTC watchdog reset through its own `WRITE_REG` (espflash's `--after watchdog-reset` is a no-op on the S3);
  - `check_esp32s3_image` / `check_esp32s3_image_file` reject an image for another chip.

## 6. `opad-layout`

[/desktop/crates/opad-layout/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-layout/src/lib.rs). Layout schema, validation (`SCREEN_W` × `SCREEN_H` = 320×240, `MAX_WIDGETS = 32`) and wire / NVS packing that matches the firmware's `ui_layout_t` ([firmware UI](../firmware/ui.md#layouts-and-deferred-persistence-ui_storec)).

## 7. `opad-ui-preview`

[/desktop/crates/opad-ui-preview/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ui-preview/src/lib.rs). Compiles the firmware's `ui/core` and LVGL for the host (`csrc/preview.c`) and renders RGBA buffers, so the GUI's Screen Designer shows what the pad will draw. When LVGL or the firmware's `sdkconfig` is missing, `build.rs` warns and builds `csrc/preview_stub.c` instead.

## 8. `opad-tosu`

[/desktop/crates/opad-tosu/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-tosu/src/lib.rs). `tosu` integration (pinned **v4.26.2**):

- `TosuManager`: WebSocket client for the v2 and precise sockets (`parse_tosu_v2_json`, `parse_precise_keys`), publishing telemetry, key counts and a connected flag.
- `spawn_tosu_supervisor` / `TosuSupervisor`: runs `tosu` as a child process, found by `find_tosu_binary()` (`$OPAD_TOSU_PATH`, `~/.local/opt/tosu/tosu`, `tosu` on `$PATH`, then the bundled copy last, so a user-installed tosu always wins), with forced `tosu.env` settings.
- `ptrace_access()` / `yama_ptrace_scope()`: explains on Linux when Yama blocks `tosu` from reading osu!'s memory.

## 9. `opad-update`

[/desktop/crates/opad-update/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/lib.rs). Signed updates for the app, `tosu` and firmware ([Updates and Automatic Rollback](../../explanation/updates-and-rollback.md)):

| Module | Role |
|---|---|
| `verify` | minisign Ed25519 signatures (`RELEASE_KEY`; `TEST_KEY` only with `OPAD_TRUST_TEST_KEY=1` or `/etc/opad/trust-test-key`) and SHA-256 hashes |
| `manifest`, `client`, `http` | Feed parsing and fetching; `client` honours `OPAD_MANIFEST_URL` |
| `origin` | `InstallOrigin` (`Windows`, `Deb`, `Rpm`, `Aur`, `AppImage`, `User`, `Source`, `Unknown`) decides how an app update is applied |
| `gate`, `schedule`, `rollout` | `may_update_now` (never during gameplay), the user's schedule, staged rollout |
| `download`, `app`, `tosu`, `firmware`, `helper`, `version` | `write_atomic` downloads, per-component apply, the privileged helper protocol, version comparison |

Binaries: `opad-feed` ([/desktop/crates/opad-update/src/bin/opad-feed.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/bin/opad-feed.rs): builds, signs and verifies feeds) and `opad-update-helper` (run as root through `pkexec` on Linux; checks a `.deb` / `.rpm` / Arch package against the signed feed and installs it, [/desktop/crates/opad-update/src/helper.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/helper.rs)).
