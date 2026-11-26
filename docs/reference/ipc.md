---
title: IPC Protocol Reference
description: Transport, framing, handshake and request catalogue of the local IPC channel between opad-daemon, opad-gui and opadctl on Linux and Windows.
sidebar:
  order: 6
---

`opad-daemon` owns the pad's serial port, the SQLite database, tosu and the updaters. `opad-gui` and `opadctl` reach it over a per-user local channel defined in `desktop/crates/opad-ipc`. The protocol version is `IPC_PROTOCOL_VERSION = 1`.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transport

| | Linux | Windows 10/11 |
|---|---|---|
| Channel | Unix domain stream socket | Named pipe |
| Path | `$XDG_RUNTIME_DIR/opad/daemon.sock`; if that variable is unset or empty, `/run/user/<uid>/opad/daemon.sock` when `/run/user/<uid>` exists, else `/tmp/opad-<uid>/daemon.sock` | `\\.\pipe\opad-ipc-<SID>`, with the current user's SID (`default` if the SID cannot be read) |
| Access | Socket directory `0700` and must belong to the user; socket `0600` | Protected DACL `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)`: only `SYSTEM` and the user. The daemon refuses to create the pipe when the SID cannot be read. |
| Second daemon | A daemon that finds a socket that still answers fails with `opad-daemon is already running`; a stale socket is removed | The first pipe instance is created with `first_pipe_instance(true)`, so a second daemon fails with the same error |
<!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from, /desktop/crates/opad-ipc/src/transport/unix.rs#create_listener, /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path, /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl, /desktop/crates/opad-ipc/src/transport/windows.rs#create_instance -->

## Framing

Each message is a 4-byte little-endian `u32` length followed by that many bytes of UTF-8 JSON (`serde_json`, externally tagged enums). A client sends one request and reads one response.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request, /desktop/crates/opad-ipc/src/lib.rs#read_request -->

| Limit | Value |
|---|---|
| `MAX_REQUEST_FRAME_SIZE` | 1 MiB (1,048,576 bytes) |
| `MAX_RESPONSE_FRAME_SIZE` | 8 MiB (8,388,608 bytes) |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->

## Handshake

Every connection starts with `Handshake` (`connect_and_handshake`). The daemon serves nothing else until one succeeds: any other first request is answered with `Error("Handshake required: ...")` and the connection is closed.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake, /desktop/daemon/src/ipc_handlers.rs#refuse_before_handshake, /desktop/daemon/src/ipc_handlers.rs#serve_connection -->

```json
{ "Handshake": { "client_version": "1.0.0-rc.6", "client_protocol": 1 } }
```

`client_version` is the client's crate version (`CARGO_PKG_VERSION`). The daemon answers:

```json
{ "HandshakeAck": { "daemon_version": "1.0.0-rc.6", "daemon_protocol": 1, "device_connected": true } }
```

It answers `HandshakeRejected { daemon_protocol, reason }` and closes the connection when:

- `client_protocol` is not `1`; or
- `client_version` differs from the daemon's version. The GUI, daemon and CLI ship together, so this means the app was updated on disk while the old daemon still runs; the reason tells the user to restart `opad-daemon`.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Requests added after IPC v1 shipped (`ResumeDevice`, `CheckForUpdates`, `SetUpdateChannel`, `GetTapStats`, `RunBoardTest`, `PlayAnimation`, `RestartPad`) came without a version bump. An older daemon cannot parse them and closes the connection, which clients treat as "not available".
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

## Requests

Every request can also be answered with `Error(String)`. `OperationRejected { reason }` means refused; `OperationDeferred { reason }` means refused for now, usually because a map or its cooldown is running.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Status, config and counters

| Request | Success response | Notes |
|---|---|---|
| `GetStatus` | `Status { mode, device_connected, device_info, counters, counters_source, pc_counters, esp_counters, config, last_sync_time, last_sync_error, storage_error, tosu_connected, latency, pending_replacement, pending_takeover, incompatible, last_backup, foreign_pad, he_calibration, key3 }` | |
| `UpdateConfig(DeviceConfig)` | `ConfigUpdated { config, deferred_persist }` | Can be `OperationDeferred` or `OperationRejected`. |
| `ForceSync` | `SyncCompleted { success, counters }` | |
| `ResetCounters { confirm }` | `CountersReset { counters }` | Refused without `confirm: true`. |
| `ResetLatencyStats` | `HandshakeAck { .. }` | The daemon answers with a `HandshakeAck` rather than a dedicated variant. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Ownership and counter recovery

| Request | Success response | Notes |
|---|---|---|
| `ResolveTakeover { take_over, keep_device_counters }` | `CountersRestored { counters }` | Answers the prompt for a pad owned by another install. `take_over: false` leaves the pad alone and is answered `OperationRejected`. `keep_device_counters` picks the pad's counters over this PC's. Deferred during a map. |
| `ResolveReplacement { restore }` | `CountersReset { counters }` | A new pad replaced a known one: restore the old pad's counters onto it, or start fresh. |
| `RestoreDeviceFromPc { confirm }` | `CountersRestored { counters }` | Writes this PC's counters to the pad. Refused without `confirm: true`. |
| `ImportPcFromDevice { confirm }` | `CountersRestored { counters }` | Takes the pad's counters into this PC's database. Refused without `confirm: true`. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Counter resets, restores and replacements are refused during a map or its cooldown. Requests that write to the pad are answered with `Error` while the pad belongs to another install or a takeover or replacement prompt is waiting for an answer.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#refuse_if_pad_guarded, /desktop/daemon/src/runtime.rs#pad_guard -->

### Backup and restore

| Request | Success response |
|---|---|
| `ExportBackup` | `BackupExported(JsonBackup)` |
| `PreviewImport(JsonBackup)` | `ImportPreview { current, incoming, device_id_matches, is_counter_rollback, warnings }` |
| `ImportBackup { backup, confirm }` | `BackupImported { success, counters, config }` |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Screen layouts and animations

| Request | Success response | Notes |
|---|---|---|
| `GetLayouts` | `Layouts { idle, playing }` | Custom layouts saved in the daemon; `None` = the pad's built-in one. |
| `SetLayout { screen, layout }` | `LayoutApplied { screen, message }` | Validates, saves and pushes the layout. |
| `ResetLayout { screen }` | `LayoutApplied { screen, message }` | Forgets the custom layout; the pad returns to its default. |
| `GetUiValues` | `UiValues(Vec<(u8, SourceValue)>)` | The latest data source values, for the designer's live preview. |
| `PlayAnimation { animation, grade }` | `AnimationPlayed` | `Error` without a pad, with firmware without animations, or during a map. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Flashing, firmware and restart

| Request | Success response | Notes |
|---|---|---|
| `PrepareFlash` | `ReadyForFlash { port }` | The daemon releases the serial port. Refused during a map or cooldown. |
| `FinishFlash` | `FlashFinished { firmware_version, protocol_version, compatible }` | The daemon looks for the pad again and waits up to 15 s for it. |
| `ResumeDevice` | `DeviceResumed` | Hands the port back without waiting, after `opadctl bootloader` or a failed flash. |
| `GetFirmwareUpdate` | `FirmwareUpdateOffer(FirmwareOffer)` | Reads only; safe to poll. |
| `InstallFirmwareUpdate { confirm }` | `FirmwareUpdateFinished { from, to, firmware_version, running_partition, protocol_version, compatible }` | Refused with `confirm: false`. Syncs the counters to this PC before writing. |
| `RestartPad` | `PadRestarting` | `Error` without a pad, with firmware without restart, or during a map. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#FirmwareOffer, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

If a connection that paused the pad for a flash closes without handing it back, the daemon resumes the pad itself.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#serve_connection -->

### Hall Effect

| Request | Success response | Notes |
|---|---|---|
| `StartHeCalibration` | `HeCalibrationStarted` | |
| `CancelHeCalibration` | `HeCalibrationCancelled` | |
| `GetHeCalibrationProgress` | `HeCalibrationProgress(Option<HeCalibrationProgress>)` | |
| `GetHeLive` | `HeLive { supported, sample }` | Keeps the pad streaming; it stops 2 s after the last request. `supported` is false on MX pads and older firmware; `sample` is `None` during a map. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

### Diagnostics, logs and statistics

| Request | Success response | Notes |
|---|---|---|
| `DetectPin { key_id, timeout_ms, exclude_gpio }` | `PinDetected { key_id, gpio, success }` | |
| `RunBoardTest` | `BoardTest(BoardTestReport)` | `Error` without a pad or with firmware without the test. |
| `GetTapStats` | `TapStats(TapStatsSnapshot)` | `current`, `last`, `history`, `period_days`, `unsaved_attempts`. |
| `SetTapHistoryPeriod { days }` | `TapStats(TapStatsSnapshot)` | `0` = all time. Saved once storage writes are allowed again. |
| `GetLogEntries { since_seq, limit, level, source }` | `LogEntries { entries, latest_seq }` | `level` and `source` are optional and filtered before `limit`. |
| `TriggerEasterEgg` | `EasterEggTriggered` | |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#TapStatsSnapshot -->

### App and tosu updates

| Request | Success response | Notes |
|---|---|---|
| `GetUpdateStatus` | `UpdateStatus { app, tosu, last_check, last_error, restart_required, pending_version, channel, checking, last_result }` | `app` and `tosu` are `ComponentUpdate`. |
| `CheckForUpdates` | `UpdateCheckStarted` | The check runs in the background; poll `GetUpdateStatus`. |
| `SetUpdateChannel { channel }` | `UpdateCheckStarted` | `stable` or `beta`; also starts a check of the new channel. |
| `SetUpdateEnabled { component, enabled }` | `ConfigUpdated { .. }` | `component` is `app`, `tosu` or `firmware` (automatic firmware updates while the pad is unused). Each switch is separate; there is no global one. |
| `InstallUpdate { component }` | `UpdateStarted { component }` | Only sent because a person pressed Install. Deferred unless the daemon is `Idle`. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#UpdateComponent, /desktop/crates/opad-ipc/src/lib.rs#ComponentUpdate, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
