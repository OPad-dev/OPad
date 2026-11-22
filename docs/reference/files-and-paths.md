---
title: Files and Paths Reference
description: Filesystem locations, SQLite database schema, log files, private tool paths, and install identity used by the OPad desktop suite on Linux and Windows.
sidebar:
  order: 7
---

The OPad desktop suite resolves all persistent paths, logs, and private bundled binaries through `desktop/crates/opad-model/src/paths.rs`. Because login-launched daemons inherit unpredictable working directories (such as `C:\Windows\System32` on Windows), OPad never falls back to relative paths. <!-- src: /desktop/crates/opad-model/src/paths.rs#PathError -->

## Filesystem Locations by Platform

| Component | Linux Location | Windows Location | Purpose |
|---|---|---|---|
| **Data Directory** | `$XDG_DATA_HOME/opad` (fallback `~/.local/share/opad`) | `%APPDATA%\opad` | Persistent user data (`opad.db`, automatic backups). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **State Directory** | `$XDG_STATE_HOME/opad` (fallback `~/.local/state/opad`) | `%APPDATA%\opad` | Disposable runtime logs. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir --> |
| **Database File** | `<data_dir>/opad.db` | `<data_dir>\opad.db` | Primary SQLite database (WAL mode). <!-- src: /desktop/crates/opad-model/src/paths.rs#database_path --> |
| **Automatic Backups** | `<data_dir>/backups/` | `<data_dir>\backups\` | Rolling automatic `JsonBackup` files (newest 10 kept). <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> |
| **Daemon Log** | `<state_dir>/daemon.log` | `<state_dir>\daemon.log` | Output of a daemon launched by the GUI. <!-- src: /desktop/crates/opad-model/src/paths.rs#daemon_log_path --> |
| **tosu Log** | `<state_dir>/tosu.log` | `<state_dir>\tosu.log` | Output of the supervised `tosu` process. <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path --> |
| **IPC Endpoint** | `$XDG_RUNTIME_DIR/opad/daemon.sock` (fallbacks `/run/user/<uid>/opad/daemon.sock`, `/tmp/opad-<uid>/daemon.sock`) | `\\.\pipe\opad-ipc-<SID>` | Local daemon IPC socket / named pipe. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path --> <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path --> |
| **Private Lib Directory** | `<install_prefix>/lib/opad` | `<install_dir>` (`%LOCALAPPDATA%\Programs\opad`) | Bundled `tosu`, pinned `espflash`, and `install-origin`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir --> |
| **Bundled tosu Binary** | `<install_lib_dir>/tosu/tosu` | `<install_lib_dir>\tosu\tosu.exe` | Bundled osu! memory reader. <!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary --> |
| **Install Origin** | `<install_lib_dir>/install-origin` | `<install_lib_dir>\install-origin` | Packaging channel marker (`windows`, `deb`, `rpm`, `aur`, `appimage`, `user`, `source`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_origin_path --> <!-- src: /desktop/crates/opad-update/src/origin.rs#InstallOrigin --> |

### Linux Path Resolution

- **Data & state directories** follow XDG (`$XDG_DATA_HOME/opad` / `$XDG_STATE_HOME/opad`, defaulting to `~/.local/share/opad` and `~/.local/state/opad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **Install prefix** is derived from the running executable's grandparent directory (for example, `/usr/bin/opad-daemon` -> prefix `/usr` -> `/usr/lib/opad`; `~/.local/bin/opad-daemon` -> `~/.local/lib/opad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix -->

### Windows Path Resolution

- Windows has no separate XDG state directory, so both persistent data (`opad.db`, `backups\`) and logs (`daemon.log`, `tosu.log`) live under `%APPDATA%\opad`. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- The Inno Setup installer installs per-user (`PrivilegesRequired=lowest`) into `%LOCALAPPDATA%\Programs\opad`, placing binaries, `espflash.exe`, `install-origin`, and `tosu\tosu.exe` together in that directory. <!-- src: /packaging/windows/installer.iss -->

## SQLite Database (`opad.db`)

`opad.db` opens in Write-Ahead Logging mode (`PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`) and tracks migrations in `schema_migrations` (currently schema v19). <!-- src: /desktop/crates/opad-storage/src/lib.rs#open -->

### Core Tables

- **`app_state`**: Key-value host metadata (`key`, `value`, `updated_at`), storing `install.id`, `tap_stats.history_days`, and updater settings/schedules. <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v7 -->
- **`device_state`**: Per-pad rows keyed by canonical `device_id` (`OPAD-<MAC>`), storing `board_profile`, `firmware_version`, `counter_generation`, `lifetime_key1`, `lifetime_key2`, `lifetime_key3`, `last_seen_at`, and `last_sync_at`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1 --> <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v18 -->
- **`config`**: Singleton row (`id = 1`) caching `DeviceConfig` (keycodes, GPIO pins, debounce, brightness, sleep, gameplay refresh Hz, `tosu_endpoint`, swipe actions/keys/modifiers, `result_animations`, `key3_hid_usage`, `game_keys`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1 -->
- **`layouts`**: Custom screen layout JSON keyed by `screen` (`Idle` / `Playing`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v3 -->
- **Tap statistics tables** (`apply_v10` / `apply_v11`): Recorded gameplay attempt tap rates and history. <!-- src: /desktop/crates/opad-storage/src/lib.rs#migrate -->

### Write Guard

All SQLite mutations check `writes_allowed` and return `StorageError::WritesBlocked` during `Playing` and `Cooldown`; pending writes flush once the daemon returns to `Idle`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#WritesBlocked -->

## Install Identity (`install.id`)

On first run, `opad-daemon` generates a UUIDv4 stored under `install.id` in `app_state`: <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->

- **Per-installation scope**: Uninstalling OPad deletes the desktop data directory (`%APPDATA%\opad` on Windows, `~/.local/share/opad` on Linux package uninstall), so reinstalling creates a new `install.id`. <!-- src: /desktop/daemon/src/identity.rs -->
- **Pad ownership**: Its 16-byte binary form (`OWNER_ID_LEN = 16`) is written to the pad via `ClaimOwnership`. <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->
- **Degraded mode**: If `opad.db` cannot be opened, the daemon runs without an identity and skips claiming pads or updating PC counters instead of crashing. <!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
