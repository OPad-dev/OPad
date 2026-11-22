---
title: opadctl CLI Tool Architecture
description: Implementation reference for the opadctl command-line utility, clap argument parsing, IPC client dispatch, and standalone serial flashing logic.
category: reference
source_files:
  - /desktop/cli/src/main.rs
---

The `opadctl` binary is the command-line management and diagnostic tool for OPad. It provides scriptable device management, counter reconciliation, backup operations, log monitoring, and direct firmware flashing routines. <!-- src: /desktop/cli/src/main.rs -->

## Dual Mode Execution Architecture

`opadctl` executes in two distinct operational modes depending on the subcommand invoked: <!-- src: /desktop/cli/src/main.rs -->

```
+-------------------------------------------------------------+
|                      opadctl invocation                     |
+-------------------------------------------------------------+
                               |
        +----------------------+----------------------+
        |                                             |
        v                                             v
[IPC Client Mode]                             [Direct Hardware Mode]
(Status, Sync, Reset, Config,                 (Flash, Recover, Bootloader)
 Backup, Monitor, Board-Test)                         |
        |                                             |
        v                                             v
Communicates with `opad-daemon`               Bypasses daemon, accesses raw
via Unix Socket or Named Pipe                 USB CDC / ROM bootloader directly
```

### 1. IPC Client Mode (Daemon-Mediated)
For commands that query or modify state (`status`, `sync`, `reset`, `config`, `backup`, `monitor`, `board-test`): <!-- src: /desktop/cli/src/main.rs -->
- Connects to the local daemon using `opad_ipc::connect_and_handshake`.
- Enforces user session security (0700/0600 on Linux, user-restricted SDDL DACL on Windows).
- Packages parameters into typed `IpcRequest` JSON frames and awaits `IpcResponse`.
- Renders formatted tabular terminal outputs, JSON payloads, or live follow streams.

### 2. Direct Hardware Mode (Daemon-Independent)
For firmware flashing and chip recovery commands (`flash`, `recover`, `bootloader`): <!-- src: /desktop/cli/src/main.rs -->
- If `opad-daemon` is running, `opadctl` transmits `IpcRequest::PrepareFlash` to politely instruct the daemon to release the serial CDC port.
- If `opad-daemon` is offline or unresponsive, `opadctl` proceeds autonomously, directly claiming the device serial port.
- Communicates directly with the ESP32-S3 ROM bootloader (`303a:1001`) via `opad-device::flash` routines.
- Executes partition table analysis, trial `otadata` synthesis, and RTC watchdog resets.

## Command Catalog Implementation

The CLI subcommands are parsed using `clap` derive macros (`Commands` enum in `desktop/cli/src/main.rs`): <!-- src: /desktop/cli/src/main.rs#Commands -->

- **`status`**: Queries daemon status, hardware connection, profile ID, gameplay mode, and latest counters.
- **`sync`**: Forces an immediate bidirectional synchronization between desktop SQLite storage and pad NVS.
- **`reset --yes`**: Executes a destructive counter generation bump and reset. Requires explicit `--yes` confirmation flag.
- **`config get` / `config set`**: Reads or modifies individual device settings (debounce ms, key GPIOs, screen brightness).
- **`backup export` / `backup import` / `backup preview`**: Serializes and deserializes `JsonBackup` files with counter rollback protection.
- **`monitor --follow`**: Streams real-time diagnostic log events from daemon and pad circular ring buffers.
- **`board-test`**: Executes board self-test routines (I2C, display ID, touch, ADC channels).
- **`flash`**: Flashes firmware binary to the inactive OTA partition with automatic rollback protection.
- **`recover`**: Recovers an unresponsive pad via a cable replug window without requiring physical BOOT button presses.
