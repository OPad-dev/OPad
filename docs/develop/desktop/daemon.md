---
title: Daemon Subsystem Architecture
description: Implementation reference for opad-daemon, the device actor queue, telemetry poller, IPC dispatching, and SQLite storage synchronization.
---

The `opad-daemon` binary is the central background supervisor of the OPad desktop suite. It manages serial CDC communication, enforces ordered message delivery through a device actor, polls live beatmap statistics from tosu, reconciles keypress counters in SQLite, and serves IPC requests from `opad-gui` and `opadctl`. <!-- src: /desktop/daemon/src/runtime.rs -->

## Device Actor Pattern (`device_actor.rs`)

To eliminate race conditions between concurrent asynchronous tasks sending serial packets to the pad, all host-to-device requests route through the device actor: <!-- src: /desktop/daemon/src/device_actor.rs -->

- **Bounded Command Channel**: Commands (`DeviceCommand`) are placed onto an asynchronous Tokio channel with capacity 256 (`QUEUE_DEPTH = 256`). <!-- src: /desktop/daemon/src/device_actor.rs#QUEUE_DEPTH -->
- **Single Dispatcher**: The actor loop pops one command at a time, writes the frame to the serial port, awaits device acknowledgment, and handles transient port busy states (`BUSY_RETRIES = 5`, `BUSY_BACKOFF = 20ms`). <!-- src: /desktop/daemon/src/device_actor.rs#BUSY_RETRIES -->
- **Strict Ordering**: Screen layouts, configuration changes, and gameplay telemetry diffs arrive at the keypad in deterministic FIFO order.

## Runtime State Supervisor (`runtime.rs`)

The daemon runtime tracks physical keypad connection, tosu health, and active gameplay status: <!-- src: /desktop/daemon/src/runtime.rs -->

- **Device Lifecycle Polling**: Scans system devices periodically (`STATUS_POLL_INTERVAL = 1s`). When a pad attaches, it initiates the `Hello` handshake, checks protocol version compatibility, and inspects ownership. <!-- src: /desktop/daemon/src/runtime.rs#STATUS_POLL_INTERVAL -->
- **Heartbeat Transmission**: Emits `HostStatus` packets every 1 second (`HOST_STATUS_INTERVAL = 1s`), informing the pad whether osu! is running and focused. <!-- src: /desktop/daemon/src/runtime.rs#HOST_STATUS_INTERVAL -->
- **Cooldown Enforcement**: Mirrors the firmware's 5-second post-play cooldown (`COOLDOWN_DURATION = 5s`). SQLite database commits and backup exports are locked until cooldown concludes and the system enters `IDLE`. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->

## Telemetry Differential Synchronization (`telemetry.rs`)

During active gameplay attempts, `tosu` produces WebSocket JSON packets at high rates. The telemetry synchronizer filters updates to avoid saturating the USB serial bus: <!-- src: /desktop/daemon/src/telemetry.rs -->

- **Rate Clamping**: Telemetry flushes are clamped to a user-configured frequency between 1 Hz and 30 Hz (`flush_interval_playing`). <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing -->
- **Value Caching (`DataSync`)**: Caches previously transmitted values for each data source index (combo, accuracy, UR, PPM, song progress). If a property has not changed, it is omitted from the subsequent `DataUpdate` frame, minimizing wire payload size. <!-- src: /desktop/daemon/src/telemetry.rs -->

## IPC Request Dispatcher (`ipc_handlers.rs`)

Clients (`opad-gui` and `opadctl`) connect to the daemon over local IPC: <!-- src: /desktop/daemon/src/ipc_handlers.rs -->
- Authenticates protocol versions (`IPC_PROTOCOL_VERSION = 1`).
- Handles configuration updates (`UpdateConfig`), counter syncs (`ForceSync`), screen layout staging (`SetLayout`), and diagnostic self-tests (`RunBoardTest`).
- Executes the `take_over_if_pad_is_ahead` ownership heuristic when a pad with equal or greater counters is connected. <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead -->
- Manages port release during firmware flashing (`PrepareFlash` and `FinishFlash`).
