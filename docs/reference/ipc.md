---
title: IPC Protocol Reference
description: Specification of the local Inter-Process Communication (IPC) transport, JSON framing, security model, and request/response protocol between opad-daemon, GUI, and opadctl.
category: reference
source_files:
  - /desktop/crates/opad-ipc/src/lib.rs
  - /desktop/crates/opad-ipc/src/transport/unix.rs
  - /desktop/crates/opad-ipc/src/transport/windows.rs
  - /desktop/daemon/src/ipc_handlers.rs
---

The OPad desktop architecture delegates hardware interaction, tosu integration, keypress counter tracking, and background synchronization to a persistent background service (`opad-daemon`). Desktop user interfaces—including `opad-gui` and the command-line utility `opadctl`—communicate with `opad-daemon` over a local Inter-Process Communication (IPC) channel. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transport & Discovery

The IPC layer abstracts OS-specific communication mechanisms behind unified `IpcStream` and `IpcListener` interfaces. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcStream -->

### Linux: Unix Domain Sockets

On Linux and Unix-like operating systems, the transport uses a standard Unix domain stream socket. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#IpcStream -->

- **Socket Resolution Order**:
  1. `$XDG_RUNTIME_DIR/opad/daemon.sock` (if `$XDG_RUNTIME_DIR` is set and non-empty). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path -->
  2. `/run/user/<uid>/opad/daemon.sock` (if directory `/run/user/<uid>` exists). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
  3. `/tmp/opad-<uid>/daemon.sock` (fallback). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
- **Filesystem Permissions**: The socket directory is created with `0700` (`rwx------`) permissions and the socket node with `0600` (`rw-------`) mode, restricting access strictly to the owner's UID. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->

### Windows: Named Pipes

On Windows 10 and 11, the transport uses Windows Named Pipes. <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#IpcStream -->

- **Pipe Path**: `\\.\pipe\opad-ipc-<sid>` (where `<sid>` is the Windows Security Identifier string of the active user token). <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- **Security & Access Control (DACL)**:
  Named pipes created with default security attributes can be connected to by any process across different logon sessions. To maintain the same user isolation as Unix permissions, `opad-daemon` constructs an explicit Security Descriptor Definition Language (SDDL) string containing a Discretionary Access Control List (DACL): <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl -->
  ```sddl
  D:(A;;GA;;;<USER_SID>)(A;;GA;;;SY)
  ```
  - `(A;;GA;;;<USER_SID>)`: Grants Generic All (`GA`) access to the logged-in user.
  - `(A;;GA;;;SY)`: Grants Generic All (`GA`) access to Local System (`SYSTEM`).
  - Access is explicitly denied to all other users and lower-integrity sessions.

## Wire Framing

The IPC transport transmits discrete request and response messages using a length-delimited JSON protocol. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->

```
+-----------------------------------+-----------------------------------------+
| Length Prefix (4 bytes, u32-LE)   | JSON Payload (UTF-8, serde_json)        |
+-----------------------------------+-----------------------------------------+
```

### Framing Rules

- **Header**: 4-byte unsigned 32-bit integer encoded in little-endian order, denoting the byte length of the ensuing JSON payload. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->
- **Payload**: Serialized JSON string matching `IpcRequest` or `IpcResponse`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest --> <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->
- **Size Bounds**:
  - `MAX_REQUEST_FRAME_SIZE`: 1,048,576 bytes (1 MiB). Requests exceeding this limit return `IpcError::Protocol`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->
  - `MAX_RESPONSE_FRAME_SIZE`: 8,388,608 bytes (8 MiB). Protects clients from unbounded allocations during memory dumps or large diagnostic exports. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_RESPONSE_FRAME_SIZE -->

## Connection Lifecycle & Handshake

Every client must complete a protocol handshake immediately after establishing the transport connection before any commands will be processed. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake -->

1. **Client Handshake**: Client sends `IpcRequest::Handshake`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
   ```json
   {
     "Handshake": {
       "client_version": "1.0.0",
       "client_protocol": 1
     }
   }
   ```
2. **Daemon Evaluation**:
   - If `client_protocol == IPC_PROTOCOL_VERSION` (currently `1`), daemon replies `IpcResponse::HandshakeAck`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->
     ```json
     {
       "HandshakeAck": {
         "daemon_version": "1.0.0",
         "daemon_protocol": 1,
         "device_connected": true,
         "device_info": { ... }
       }
     }
     ```
   - If protocol versions diverge incompatibly, daemon replies `IpcResponse::HandshakeRejected` and closes the stream. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

## IPC Request Catalog

The full enumeration of client requests handled by `opad-daemon` is defined by `IpcRequest`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Device Status & Configuration
- **`GetStatus`**: Queries daemon status, hardware connection, active profile, gameplay mode, and latest counters. Returns `IpcResponse::Status`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`UpdateConfig(DeviceConfig)`**: Updates device configuration in daemon memory and sends `SetConfig` to the pad. If gameplay is active, NVS write is deferred and `IpcResponse::OperationDeferred` or `ConfigUpdated` is returned. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ForceSync`**: Forces an immediate bidirectional synchronization between daemon state and pad NVS. Returns `IpcResponse::SyncCompleted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetCounters { confirm }`**: Resets pad and desktop keypress lifetime counters. Requires `confirm: true`. Returns `IpcResponse::CountersReset`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetLatencyStats`**: Clears running USB latency jitter statistics on both host and device. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Identity & Counter Ownership
- **`ResolveTakeover { take_over, keep_device_counters }`**: Resolves an ownership conflict when a pad is plugged into a new host. `take_over: true` adopts the pad; `keep_device_counters` dictates whether pad or PC counter totals are authoritative. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResolveReplacement { restore }`**: Handles replacement of a damaged pad with new hardware sharing previous configuration backups. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RestoreDeviceFromPc { confirm }`**: Overwrites pad counters with host PC database counters. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ImportPcFromDevice { confirm }`**: Adopts pad counters into the local host database. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Backup & Restore
- **`ExportBackup`**: Generates a complete serialized `JsonBackup` representing device configuration, screen layouts, and counter history. Returns `IpcResponse::BackupExported`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`PreviewImport(JsonBackup)`**: Validates a backup JSON structure against the connected pad and inspects counter delta risks without applying changes. Returns `IpcResponse::ImportPreview`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ImportBackup { backup, confirm }`**: Applies an inspected backup to the daemon and pad. Returns `IpcResponse::BackupImported`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Screen Layouts & Live UI
- **`GetLayouts`**: Fetches active idle and in-game custom screen configurations. Returns `IpcResponse::Layouts`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetLayout { screen, layout }`**: Transmits a new widget layout to the pad display for either `Idle` or `Playing` screens. Returns `IpcResponse::LayoutApplied`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetLayout { screen }`**: Reverts a screen layout back to internal default widgets. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetUiValues`**: Polls current values of mapped display variables (accuracy, combo, UR, PPM, song position). Returns `IpcResponse::UiValues`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`PlayAnimation { grade }`**: Triggers celebration grade overlay on the pad screen. Returns `IpcResponse::AnimationPlayed`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Firmware Update & Port Control
- **`PrepareFlash`**: Instructs `opad-daemon` to release the USB CDC serial port and hold background polling so `opadctl flash` or flasher tools can access the device. Returns `IpcResponse::ReadyForFlash`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`FinishFlash`**: Signals that flashing concluded; daemon reconnects to the pad and re-verifies protocol compatibility. Returns `IpcResponse::FlashFinished`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResumeDevice`**: Resumes daemon polling without running flash completion routines. Returns `IpcResponse::DeviceResumed`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetFirmwareUpdate` / `InstallFirmwareUpdate`**: Checks for downloadable firmware releases and schedules OTA transfer. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RestartPad`**: Reboots the connected pad into normal runtime mode once idle. Returns `IpcResponse::PadRestarting`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Hall Effect Calibration & Telemetry
- **`StartHeCalibration`**: Initiates multi-step guided Hall Effect calibration. Returns `IpcResponse::HeCalibrationStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`CancelHeCalibration`**: Cancels ongoing Hall Effect calibration. Returns `IpcResponse::HeCalibrationCancelled`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetHeCalibrationProgress`**: Polls guided calibration steps and error states. Returns `IpcResponse::HeCalibrationProgress`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetHeLive`**: Requests live normalized travel distance and raw magnetic ADC readings (~50 Hz) for graphical rendering. Returns `IpcResponse::HeLive`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Diagnostics & Hardware Testing
- **`DetectPin { pin, timeout_ms }`**: Commands pad to detect pin contact actuation on header GPIOs. Returns `IpcResponse::PinDetected`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RunBoardTest`**: Executes board self-test suite (I2C, display, touch controller, ADC). Returns `IpcResponse::BoardTest(BoardTestReport)`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetTapStats` / `SetTapHistoryPeriod`**: Queries live tap rate histogram data and tap speed metrics. Returns `IpcResponse::TapStats`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetLogEntries { since_seq }`**: Drains diagnostic log events from the daemon ring buffer. Returns `IpcResponse::LogEntries`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`TriggerEasterEgg`**: Sends easter egg animation trigger. Returns `IpcResponse::EasterEggTriggered`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### App & tosu Updates
- **`GetUpdateStatus`**: Queries desktop software and tosu version statuses. Returns `IpcResponse::UpdateStatus`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`CheckForUpdates`**: Triggers immediate background release lookup. Returns `IpcResponse::UpdateCheckStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`InstallUpdate { component }`**: Triggers binary download and atomic replacement of desktop suite binaries. Returns `IpcResponse::UpdateStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetUpdateChannel { channel }`**: Switches between `stable` and `beta` software release tracks. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetUpdateEnabled { enabled }`**: Enables or disables background automatic update checking. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
