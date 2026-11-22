---
title: Files and Paths Reference
description: Specification of all filesystem locations, database paths, log files, socket paths, and OS-specific directory structures used by the OPad desktop suite.
---

The OPad desktop suite strictly resolves all persistent paths, database files, process state, and bundled binaries through central path helpers to guarantee predictable behavior across Linux and Windows 10/11 environments. Login-launched daemons frequently inherit unpredictable working directories (such as `C:\Windows\System32` on Windows), so relative paths are never used. <!-- src: /desktop/crates/opad-model/src/paths.rs#PathError -->

## Filesystem Locations by Platform

| Component | Linux Location | Windows Location | Purpose |
|---|---|---|---|
| **Data Directory** | `$XDG_DATA_HOME/opad` (or `~/.local/share/opad`) | `%APPDATA%\opad` | Persistent user data (SQLite database). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **State Directory** | `$XDG_STATE_HOME/opad` (or `~/.local/state/opad`) | `%APPDATA%\opad` | Disposable runtime logs. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir --> |
| **Database File** | `<data_dir>/opad.db` | `<data_dir>\opad.db` | Primary SQLite database. <!-- src: /desktop/crates/opad-model/src/paths.rs#database_path --> |
| **Daemon Log** | `<state_dir>/daemon.log` | `<state_dir>\daemon.log` | Standard output/error of the daemon process. <!-- src: /desktop/crates/opad-model/src/paths.rs#daemon_log_path --> |
| **tosu Log** | `<state_dir>/tosu.log` | `<state_dir>\tosu.log` | Standard output/error of supervised tosu instance. <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path --> |
| **Private Lib Directory** | `<install_prefix>/lib/opad` | `<install_dir>` | Private auxiliary tools and markers. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir --> |
| **Bundled tosu Binary** | `<install_lib_dir>/tosu/tosu` | `<install_lib_dir>\tosu\tosu.exe` | Bundled binary for osu! memory integration. <!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary --> |
| **Install Origin** | `<install_lib_dir>/install-origin` | `<install_lib_dir>\install-origin` | Packaging channel marker (`deb`, `rpm`, `tar`, `msi`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_origin_path --> |

### Linux Path Resolution

On Linux systems, path resolution adheres to the XDG Base Directory specification: <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **Data directory**: Evaluates `$XDG_DATA_HOME/opad`. If unset or empty, falls back to `$HOME/.local/share/opad`.
- **State directory**: Evaluates `$XDG_STATE_HOME/opad`. If unset or empty, falls back to `$HOME/.local/state/opad`.
- **Install Prefix**: Deduced from the directory tree containing the running binary (`opad-daemon` or `opad-gui`). For example, `/usr/bin/opad-daemon` yields prefix `/usr` and library path `/usr/lib/opad`. A local build in `~/.local/bin/opad-daemon` yields `~/.local/lib/opad`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix -->

### Windows Path Resolution

On Windows 10 and 11: <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- Windows does not have a separate XDG-style state directory. Both data and runtime logs reside under `%APPDATA%\opad` (typically `C:\Users\<User>\AppData\Roaming\opad`). This ensures the uninstaller has a single data folder to purge. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- The application binaries, bundled tosu, and origin markers all reside together in the root installation directory (e.g., `C:\Program Files\OPad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->

## SQLite Database (`opad.db`)

All persistent desktop state is managed in a single SQLite database initialized in Write-Ahead Logging mode (`PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage::open -->

### Schema Tables

The database maintains schema versioning in `schema_migrations`: <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage -->

- **`app_state`**: Key-value table storing desktop installation metadata: <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage -->
  - `install.id`: Randomly generated UUIDv4 string uniquely identifying the desktop installation. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->
  - `tap_stats.history_days`: Configured rolling retention window for tap speed statistics. <!-- src: /desktop/crates/opad-storage/src/lib.rs#HISTORY_DAYS_KEY -->
- **`device_state`**: Stores device-specific persistent state indexed by device serial number:
  - Device owner ID string.
  - Reconciled lifetime keypress counts (`lifetime_key1`, `lifetime_key2`, and optional `lifetime_key3`).
  - Counter generation sequence numbers.
- **`config`**: Cached device configuration structures (`DeviceConfig`) serialized per device serial ID.
- **`layouts`**: Custom screen layout JSON payloads for `idle` and `playing` screens.

### Write Invariants

To avoid storage I/O interfering with latency-critical timing, database transactions are blocked during active gameplay (`PLAYING`) and cooldown phases (`StorageError::WritesBlocked`). Pending updates are buffered in memory and flushed once the device returns to `IDLE`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#StorageError::WritesBlocked -->

## Install Identity (`install.id`)

The desktop daemon generates a persistent UUIDv4 identity on first execution and stores it under key `install.id` in `app_state`. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->

- **Scope**: Per-installation, not per-machine. If the user uninstalls and chooses "delete all data", the database and identity are deleted. Reinstalling generates a fresh identity. <!-- src: /desktop/daemon/src/identity.rs -->
- **Ownership**: The 16-byte binary representation is used in `ClaimOwnership` commands to claim hardware pads. Pads record this 16-byte owner ID in non-volatile flash (NVS). <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->
- **Degraded Operation**: If storage fails or `opad.db` is unopenable, the daemon runs degraded without an identity rather than crashing; it simply does not claim devices or prompt for counter reconciliation. <!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
