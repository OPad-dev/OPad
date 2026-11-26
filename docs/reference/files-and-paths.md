---
title: Files and Paths Reference
description: Where the OPad desktop suite keeps its database, backups, logs, IPC endpoint and bundled tools on Linux and Windows, and the SQLite schema.
sidebar:
  order: 7
---

Every path the desktop suite reads or writes is resolved in `desktop/crates/opad-model/src/paths.rs`. A daemon started at login inherits an unpredictable working directory (often `C:\Windows\System32` on Windows), so no path is ever relative: when a directory cannot be resolved, the call fails instead of guessing.
<!-- src: /desktop/crates/opad-model/src/paths.rs -->

## Locations

| What | Linux | Windows |
|---|---|---|
| Data directory | `$XDG_DATA_HOME/opad`, else `~/.local/share/opad` | `%APPDATA%\opad` |
| State directory (logs) | `$XDG_STATE_HOME/opad`, else `~/.local/state/opad` | `%APPDATA%\opad` (no separate state directory) |
| Database | `<data>/opad.db` | `<data>\opad.db` |
| Automatic backups | `<data>/backups/`, newest 10 kept | `<data>\backups\` |
| Daemon log (a daemon the GUI started) | `<state>/daemon.log` | `<state>\daemon.log` |
| tosu log | `<state>/tosu.log` | `<state>\tosu.log` |
| IPC endpoint | `$XDG_RUNTIME_DIR/opad/daemon.sock` (fallbacks in [IPC Protocol Reference](ipc.md#transport)) | `\\.\pipe\opad-ipc-<SID>` |
<!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir, /desktop/crates/opad-model/src/paths.rs#state_dir, /desktop/daemon/src/backup.rs#backup_dir, /desktop/daemon/src/backup.rs#KEEP, /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path, /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->

## Installed files

The private install directory is found from the running executable, so one rule covers every prefix:
<!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix, /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->

| What | Linux | Windows |
|---|---|---|
| Install prefix | The executable's grandparent (`/usr/bin/opad-daemon` → `/usr`, `~/.local/bin/opad-daemon` → `~/.local`) | The executable's directory, by default `%LOCALAPPDATA%\Programs\opad` (per-user install) |
| Private directory (`install_lib_dir`) | `<prefix>/lib/opad` | `<prefix>` |
| Bundled tosu | `<lib>/tosu/tosu` | `<prefix>\tosu\tosu.exe` |
| Install-origin marker | `<lib>/install-origin` | `<prefix>\install-origin` |
| Pinned `espflash` | `<prefix>/lib/opad/bin/espflash` (or next to the executable) | `<prefix>\espflash.exe` |
<!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary, /desktop/crates/opad-model/src/paths.rs#install_origin_path, /desktop/crates/opad-device/src/flash.rs#resolve_espflash, /packaging/windows/installer.iss -->

When no bundled `espflash` is found, the flasher tries `%USERPROFILE%\.cargo\bin\espflash.exe` on Windows, then `espflash` on `PATH`.
<!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

The `install-origin` marker names how OPad was installed, which decides how it updates: `windows`, `deb`, `rpm`, `aur`, `appimage`, `user` or `source`. Anything else reads as unknown.
<!-- src: /desktop/crates/opad-update/src/origin.rs#InstallOrigin -->

## SQLite database (`opad.db`)

The database opens with `PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`. Applied migrations are recorded in `schema_migrations (version, applied_at)`; the current schema is v19.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#open, /desktop/crates/opad-storage/src/lib.rs#migrate -->

| Table | Key | Contents | Since |
|---|---|---|---|
| `device_state` | `device_id` (`OPAD-<MAC>`) | `board_profile`, `firmware_version`, `counter_generation`, `lifetime_key1`, `lifetime_key2`, `lifetime_key3` (NULL until a three-key pad synced), `last_seen_at`, `last_sync_at` | v1; key 3 v18 |
| `config` | `id` = `1` (single row) | The app's copy of the pad config: keys, debounce, brightness, display sleep, gameplay refresh rate, `tosu_endpoint`, `press_color_rgb`, key pins, swipe actions/keys/modifiers, `result_animations`, `key3_hid_usage`, `game_keys` | v1, columns added v2–v17 |
| `layouts` | `screen` (`0` = Idle, `1` = Playing) | `json`, `updated_at` | v3 |
| `app_state` | `key` | `value`, `updated_at` | v7 |
| `tap_attempts` | `id` | One row per finished attempt: times, beatmap, mods, title/artist/difficulty, per-channel (`k1`, `k2`, `combined`) press count, intervals, average and peak, song press count and length, song peak | v9; song v10, song peak v11 |
<!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1, /desktop/crates/opad-storage/src/lib.rs#apply_v3, /desktop/crates/opad-storage/src/lib.rs#apply_v7, /desktop/crates/opad-storage/src/lib.rs#apply_v18, /desktop/crates/opad-storage/src/tap_stats.rs#apply_v9, /desktop/crates/opad-layout/src/lib.rs#Screen -->

Migration v19 renamed pads stored as `OSUPAD-<MAC>` (firmware up to `1.0.0-rc`) to `OPAD-<MAC>`, keeping their counters.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->

### `app_state` keys

| Key | Holds |
|---|---|
| `install.id` | This install's identity (below) |
| `stats.history_days` | The statistics history period, `0` = all time |
| `update.app.enabled`, `update.tosu.enabled`, `update.firmware.auto` | Automatic-update switches; missing = on |
| `update.channel` | `stable` (default) or `beta` |
| `update.schedule`, `update.last_result` | The updater's check schedule and the last install's result |
<!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY, /desktop/crates/opad-storage/src/tap_stats.rs#HISTORY_DAYS_KEY, /desktop/daemon/src/updater.rs#APP_ENABLED_KEY, /desktop/daemon/src/updater.rs#read_flag -->

### Write guard

The daemon turns storage writes off for a map and its cooldown: every write then returns `StorageError::WritesBlocked`. Data that must survive, such as finished tap attempts and a new history period, is kept in memory and saved once writes are allowed again.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#check_writes_allowed, /desktop/daemon/src/runtime.rs, /desktop/crates/opad-ipc/src/lib.rs#TapStatsSnapshot, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

## Install identity

On first run the daemon generates a UUIDv4 and stores it as `install.id`. Its 16 bytes are what a pad records as its owner (`ClaimOwnership`). It is per install, not per machine: uninstalling deletes the data directory (the Windows uninstaller, and the Linux packages' `uninstall-cleanup` for XDG default paths), so a reinstall gets a new identity. It protects counter integrity, not exclusivity; nothing about it is cryptographic.
<!-- src: /desktop/daemon/src/identity.rs, /packaging/windows/installer.iss, /packaging/linux/uninstall-cleanup.sh -->

If the database cannot be opened, the daemon runs without an identity: it claims no pads and shows no ownership prompts.
<!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
