---
title: Daemon Subsystem Architecture
description: Implementation reference for opad-daemon, the device actor queue, runtime state supervisor, telemetry diffing, automatic backups, and IPC handlers.
---

`opad-daemon` is the background Tokio service coordinating USB CDC traffic, `tosu` telemetry, SQLite persistence, automatic backups, software/firmware updates, and local IPC ([/desktop/daemon/src/main.rs](/desktop/daemon/src/main.rs), [/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs)).

## Device Actor (`device_actor.rs`)

All runtime host-to-pad commands serialize through a single actor task in [/desktop/daemon/src/device_actor.rs](/desktop/daemon/src/device_actor.rs):

- **Bounded FIFO channel**: Capacity `QUEUE_DEPTH = 256` carrying `DeviceCommand` (`TimeSync`, `ClaimOwnership`, `Config`, `HostStatus`, `DataUpdate`, `Layout`, `RequestStatus`, `PlayAnimation`).
- **Sequential dispatch & retries**: Awaits each command in order. On `DeviceError::Busy`, retries up to `BUSY_RETRIES = 5` times with `BUSY_BACKOFF = 20 ms`, except stale `DataUpdate` diffs which instead set `resend_data` so the next flush transmits a fresh full snapshot.

## Runtime State Supervisor (`runtime.rs`)

Defined in [/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs):

- **Polling & heartbeats**: `STATUS_POLL_INTERVAL = 1 s`, `HOST_STATUS_INTERVAL = 1 s` (reports `tosu_connected`, `is_playing`, `play_id`, `osu_active`).
- **Cooldown & write guard**: Mirrors firmware's 5 s cooldown (`COOLDOWN_DURATION = 5 s`), blocking SQLite writes (`Storage::set_writes_allowed(false)`) during `Playing` and `Cooldown`.
- **Automatic backups**: Arms `AUTO_BACKUP_DELAY = 20 s` after post-play sync settles in `Idle` and writes `opad-backup-<timestamp>.json` into `<data_dir>/backups/` (keeping the newest 10 files via [/desktop/daemon/src/backup.rs](/desktop/daemon/src/backup.rs)).
- **Tap rate tracking**: Tracks live attempts (preferring exact pad key-down timestamps over coarse `tosu` timestamps), ignores replays/other keyboards (`TOSU_ONLY_PRESSES = 8`), and buffers up to `MAX_UNSAVED_ATTEMPTS = 500` while storage writes are paused.

## Telemetry Differential Sync (`telemetry.rs`)

[/desktop/daemon/src/telemetry.rs](/desktop/daemon/src/telemetry.rs) (`DataSync`) ingests `tosu` v2 values and emits only changed `(source_id, SourceValue)` entries, throttled to `flush_interval_playing(hz)` (`1`–`30 Hz`, default `10 Hz` / `100 ms`) while playing and `FLUSH_INTERVAL_IDLE = 500 ms` while idle.

## IPC Request Dispatcher (`ipc_handlers.rs`)

[/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs) handles authenticated `IPC_PROTOCOL_VERSION = 1` requests:
- Runs `take_over_if_pad_is_ahead` on foreign-owned pads when the pad is at or ahead of the PC on both keys; otherwise enforces `pad_guard()` until the user resolves `TakeoverPrompt`.
- Coordinates `PrepareFlash` / `FinishFlash` / `ResumeDevice` port handoffs for `opadctl` and handles host-driven firmware updates ([/desktop/daemon/src/firmware_update.rs](/desktop/daemon/src/firmware_update.rs)).
