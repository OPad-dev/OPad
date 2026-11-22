---
title: opadctl CLI Tool Architecture
description: Implementation reference for the opadctl command-line utility, clap parsing, IPC client dispatch, and standalone serial flashing/recovery.
---

`opadctl` ([/desktop/cli/src/main.rs](/desktop/cli/src/main.rs)) is OPad's Clap-based CLI for status queries, counter sync/reset, JSON backups, log monitoring, latency/tap-rate inspection, board tests, environment setup, and hands-free USB flashing/recovery.

## Execution Modes

```text
+-------------------------------------------------------------+
|                      opadctl invocation                     |
+-------------------------------------------------------------+
                               |
        +----------------------+----------------------+----------------------+
        |                                             |                      |
        v                                             v                      v
[IPC Client Mode]                             [Direct Hardware Mode]  [Local Setup]
(status, sync, reset, export, import,         (flash, recover,        (setup)
 monitor, firmware-update, latency,            bootloader)
 stats, board-test, animation, restart)               |
        |                                             |
        v                                             v
Connects to opad-daemon via                   Asks daemon to release port
Unix socket / Windows named pipe              if running; flashes via espflash
```

1. **IPC Client Mode**: Connects via `opad_ipc::connect_and_handshake`, sends typed `IpcRequest` frames, and formats `IpcResponse` output.
2. **Direct Hardware Mode (`flash`, `recover`, `bootloader`)**: If `opad-daemon` is running, sends `IpcRequest::PrepareFlash` so the daemon releases the CDC port, then executes `opad_device::flash` routines against the pad (`303a:4001`) or ESP32-S3 ROM bootloader (`303a:1001`) using pinned `espflash` **4.6.0**. Afterward sends `FinishFlash` (or `ResumeDevice` after `bootloader`/failure). Works standalone if the daemon is stopped.
3. **Local Setup Mode (`setup`)**: Checks Linux `70-opad.rules` and Yama `ptrace_scope` / `cap_sys_ptrace` locally without needing `opad-daemon` (or reports inbox drivers on Windows).

## Subcommands (`Commands` enum)

All 16 subcommands in [/desktop/cli/src/main.rs](/desktop/cli/src/main.rs):
- `status`: Prints daemon mode, pad info, running OTA slot, input module & HE calibration, counters, ownership, `tosu` state, latency (`p50`/`p99.9`/`max`), and config.
- `sync`: Sends `IpcRequest::ForceSync`.
- `reset [--yes]`: Destructive generation-bumping counter reset (aborts without `--yes`).
- `export <FILE>` / `import [--yes] <FILE>`: Exports or previews+imports `JsonBackup` files.
- `monitor [-l <LIMIT>] [-f] [--level <LEVEL>] [--source <SOURCE>]`: Streams daemon/pad/program/`tosu` logs (polls every 500 ms with `-f`).
- `flash [--port <PORT>] [--full] <FIRMWARE>`: Inactive-slot OTA update or `--full` recovery flash.
- `recover [--timeout <SECONDS>] <FIRMWARE>`: Catches ESP32-S3 ROM bootloader on cable replug and writes the full flash set (no BOOT or RESET buttons).
- `bootloader [--port <PORT>]`: Reboots pad into ROM download mode (`303a:1001`) and leaves it there.
- `firmware-update [--yes]`: Checks signed release manifest and installs firmware update with explicit consent.
- `latency [--reset]`: Prints or resets device-measured keypress-to-HID latency (`p50`, `p99`, `p99.9`, `max`, deferred count).
- `stats [--period <DAYS>]`: Prints attempt and historical KPM tap rates (and optionally updates history period, `0` = all time, max `3650`).
- `board-test [--fix-pins] [--magnet]`: Runs carrier/module PCB test (module ID, FFC orientation, key lines, shorts, HE V3 ADS7028/sensors).
- `animation <NAME> [GRADE]`: Previews a result animation (`pop`, `drop`, `burst`, ID, or `none`) for grade `SS`/`S`/`A`/`B`/`C`/`D`.
- `restart`: Flushes counters and restarts the pad once idle.
- `setup`: Verifies platform permissions (`udev` + `ptrace` on Linux).
