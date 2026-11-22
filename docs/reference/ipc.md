---
title: IPC Protocol Reference
description: Specification of the local Inter-Process Communication (IPC) transport, JSON framing, Windows/Linux security model, and request/response protocol between opad-daemon, opad-gui, and opadctl.
sidebar:
  order: 6
---

`opad-daemon` owns the USB CDC connection, SQLite database, `tosu` supervision, and update checks. `opad-gui` and `opadctl` communicate with `opad-daemon` over a local IPC channel (`IPC_PROTOCOL_VERSION = 1`). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transport & Discovery

Platform transports are abstracted behind `IpcStream`, `IpcServerStream`, and `IpcListener`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcStream -->

### Linux: Unix Domain Sockets

On Linux, IPC uses a Unix domain stream socket. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#IpcStream -->

- **Socket Resolution Order**:
  1. `$XDG_RUNTIME_DIR/opad/daemon.sock` (when `$XDG_RUNTIME_DIR` is set and non-empty). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path -->
  2. `/run/user/<uid>/opad/daemon.sock` (if `/run/user/<uid>` exists). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
  3. `/tmp/opad-<uid>/daemon.sock` (fallback). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
- **Permissions**: Socket directory mode `0700` and socket node mode `0600` (owner UID only). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->

### Windows: Named Pipes

On Windows 10/11, IPC uses per-user Windows Named Pipes. <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#IpcStream -->

- **Pipe Path**: `\\.\pipe\opad-ipc-<SID>` (where `<SID>` is the current user token's SID string, falling back to `default` if lookup fails for path formatting; listener creation refuses to start if SID lookup fails). <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- **Security (DACL)**: Every pipe instance is created with a protected SDDL DACL allowing only `SYSTEM` and the calling user: <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl -->
  ```text
  D:P(A;;GA;;;SY)(A;;GA;;;<USER_SID>)
  ```
- **Single-daemon guarantee**: The first instance uses `first_pipe_instance(true)` so a second daemon fails with `IpcError::AlreadyRunning`. <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#create_listener -->

## Wire Framing

Messages are length-delimited UTF-8 JSON frames: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->

```text
+-----------------------------------+-----------------------------------------+
| Length Prefix (4 bytes, u32-LE)   | JSON Payload (UTF-8, serde_json)        |
+-----------------------------------+-----------------------------------------+
```

- `MAX_REQUEST_FRAME_SIZE`: `1,048,576` bytes (1 MiB). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->
- `MAX_RESPONSE_FRAME_SIZE`: `8,388,608` bytes (8 MiB). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_RESPONSE_FRAME_SIZE -->

## Connection Lifecycle & Handshake

Every client opens a connection and performs a handshake (`connect_and_handshake`) before sending other requests: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake -->

1. **Client sends `IpcRequest::Handshake`**:
   ```json
   {
     "Handshake": {
       "client_version": "1.0.0",
       "client_protocol": 1
     }
   }
   ```
2. **Daemon replies**:
   - On version match (`IPC_PROTOCOL_VERSION == 1`), `IpcResponse::HandshakeAck`:
     ```json
     {
       "HandshakeAck": {
         "daemon_version": "1.0.0",
         "daemon_protocol": 1,
         "device_connected": true
       }
     }
     ```
   - On mismatch, `IpcResponse::HandshakeRejected { daemon_protocol, reason }` and closes the connection. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

## IPC Request Catalog (`IpcRequest`)

All variants defined in `IpcRequest` ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs)): <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Status, Config & Counters
- `GetStatus` -> `Status { mode, device_connected, device_info, counters, counters_source, pc_counters, esp_counters, config, last_sync_time, last_sync_error, storage_error, tosu_connected, latency, pending_replacement, pending_takeover, incompatible, last_backup, foreign_pad, he_calibration, key3 }`
- `UpdateConfig(DeviceConfig)` -> `ConfigUpdated { config, deferred_persist }` or `OperationDeferred`
- `ForceSync` -> `SyncCompleted { success, counters }`
- `ResetCounters { confirm }` -> `CountersReset { counters }`
- `ResetLatencyStats` -> clears device latency stats

### Identity, Ownership & Counter Recovery
- `ResolveTakeover { take_over, keep_device_counters }` -> answers `TakeoverPrompt` for a pad owned by another installation
- `ResolveReplacement { restore }` -> restores previous pad's counters onto a newly connected low-count pad (or starts fresh)
- `RestoreDeviceFromPc { confirm }` -> `CountersRestored { counters }`
- `ImportPcFromDevice { confirm }` -> `CountersRestored { counters }`

### Backup & Restore
- `ExportBackup` -> `BackupExported(JsonBackup)`
- `PreviewImport(JsonBackup)` -> `ImportPreview { current, incoming, device_id_matches, is_counter_rollback, warnings }`
- `ImportBackup { backup, confirm }` -> `BackupImported { success, counters, config }`

### Screen Layouts & Live UI
- `GetLayouts` -> `Layouts { idle, playing }`
- `SetLayout { screen, layout }` -> `LayoutApplied { screen, message }`
- `ResetLayout { screen }` -> reverts screen to firmware default
- `GetUiValues` -> `UiValues(Vec<(u8, SourceValue)>)`
- `PlayAnimation { animation, grade }` -> `AnimationPlayed` (result screen animation preview)

### Flashing, Firmware Updates & Reboot
- `PrepareFlash` -> `ReadyForFlash { port }` (daemon releases the CDC port)
- `FinishFlash` -> `FlashFinished { firmware_version, protocol_version, compatible }`
- `ResumeDevice` -> `DeviceResumed` (resumes polling without waiting for app re-enumeration)
- `GetFirmwareUpdate` -> `FirmwareUpdateOffer(FirmwareOffer)`
- `InstallFirmwareUpdate { confirm }` -> `FirmwareUpdateFinished { from, to, firmware_version, running_partition, protocol_version, compatible }` (refused if `confirm: false`)
- `RestartPad` -> `PadRestarting`

### Hall Effect Calibration & Live View
- `StartHeCalibration` -> `HeCalibrationStarted`
- `CancelHeCalibration` -> `HeCalibrationCancelled`
- `GetHeCalibrationProgress` -> `HeCalibrationProgress(Option<HeCalibrationProgress>)`
- `GetHeLive` -> `HeLive { supported, sample }` (keeps pad streaming; stops 2 s after last request)

### Diagnostics, Logs & Tap Stats
- `DetectPin { key_id, timeout_ms, exclude_gpio }` -> `PinDetected { key_id, gpio, success }`
- `RunBoardTest` -> `BoardTest(BoardTestReport)` (tests carrier & input module PCBs: module ID, cable direction, key lines, connector shorts, HE V3 ADS7028/sensors)
- `GetTapStats` / `SetTapHistoryPeriod { days }` -> `TapStats(TapStatsSnapshot)`
- `GetLogEntries { since_seq, limit, level, source }` -> `LogEntries { entries, latest_seq }`
- `TriggerEasterEgg` -> `EasterEggTriggered`

### Desktop App & `tosu` Updates
- `GetUpdateStatus` -> `UpdateStatus { app, tosu, last_check, last_error, restart_required, channel, checking, last_result }`
- `CheckForUpdates` -> `UpdateCheckStarted`
- `InstallUpdate { component }` -> `UpdateStarted { component }`
- `SetUpdateChannel { channel }` (`"stable"` or `"beta"`)
- `SetUpdateEnabled { component, enabled }` (per-component toggle: `App` or `Tosu`, never global)
