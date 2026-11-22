---
title: System Architecture
description: Comprehensive architecture of OPad, detailing dual-core ESP32-S3 hardware isolation, firmware tasks, desktop daemon state machines, and ordered device communication.
---

OPad is designed with a fundamental guiding principle: **latency always wins** (Product Rule 1). Every layer of hardware, firmware, and desktop architecture is engineered to guarantee that mechanical switch closures and Hall Effect actuations reach the USB host within sub-millisecond intervals without jitter. Nothing joins the keypress path: the LCD screen, flash storage (NVS), touchscreen gestures, and tosu gameplay integration remain strictly isolated. <!-- src: /README.md -->

## Dual-Core Hardware Isolation (ESP32-S3)

The ESP32-S3 contains two Xtensa LX7 cores running at 240 MHz. OPad uses asymmetric task pinning in FreeRTOS to prevent background operations from preempting key scanning or USB HID reporting. <!-- src: /firmware/main/app_main.c -->

```
+-------------------------------------------------+-------------------------------------------------+
| CORE 0 (Input & USB HID)                        | CORE 1 (UI, CDC Serial, Runtime & Storage)      |
+-------------------------------------------------+-------------------------------------------------+
| • Keypad Task / HE Task (Priority MAX - 1)      | • TinyUSB CDC Task (Priority IDLE + 3)          |
| • TinyUSB Core Task (Priority MAX - 1)          | • CST816S Touchscreen Task (Priority 2)         |
| • GPIO Direct Key ISRs / ADS7028 DMA Sampler    | • LVGL Display Engine & Backlight PWM           |
| • Zero flash writes, zero logging, zero malloc  | • Runtime State Supervisor (Priority IDLE + 1)  |
|                                                 | • Deferred NVS Storage Flushes & Diagnostics    |
+-------------------------------------------------+-------------------------------------------------+
```

### Core 0: The Keypress Path
Core 0 owns the time-critical input pipeline: <!-- src: /firmware/main/app_main.c -->
- **`keypad_task` / `he_task`**: Pinned to Core 0 with priority `configMAX_PRIORITIES - 1`. For mechanical switches, it responds directly to edge-triggered GPIO interrupts and executes immediate eager debouncing. For Hall Effect modules, it samples high-speed ADC conversions and computes rapid trigger state transitions. <!-- src: /firmware/main/input/keypad.c#keypad_task -->
- **`tinyusb_task`**: Pinned to Core 0 with high priority (`tusb_cfg.task.xCoreID = 0`). It manages 1000 Hz USB HID endpoint polling, immediately sending 8-byte standard keyboard reports whenever a switch state changes. <!-- src: /firmware/main/app_main.c -->
- **Execution Invariants**: Core 0 code never performs heap allocations (`malloc`), never takes blocking mutexes held by Core 1, and never interacts with SPI flash (NVS).

### Core 1: System Services & UI
Core 1 handles all non-critical auxiliary features: <!-- src: /firmware/main/runtime/runtime.c -->
- **`cdc_proto` (`usb_cdc_task`)**: Pinned to Core 1 (`tskIDLE_PRIORITY + 3`). Encodes and decodes Protocol Buffers messages over the CDC-ACM serial interface. <!-- src: /firmware/main/usb/usb_cdc.c#usb_cdc_task -->
- **`touch_retry` (`touch_retry_task`)**: Pinned to Core 1 (`Priority 2`). Polls the CST816S I2C touch controller during active touch contacts and detects swipe gestures. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->
- **Display Engine**: Drives the ST7789 320x240 LCD display using LVGL. Screen flushes and SPI DMA transactions run on Core 1, completely off the input path.
- **`runtime_task` (`runtime_supervisor_task`)**: Pinned to Core 1 (`tskIDLE_PRIORITY + 1`). Supervises device states, checks for map timeouts, and executes deferred NVS writes when returning to `IDLE`. <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## Firmware Runtime State Machine

The firmware runtime supervisor (`firmware/main/runtime/runtime.c`) tracks active player status to enforce strict flash protection rules: <!-- src: /firmware/main/runtime/runtime.c#s_state -->

```
        Host sends GameplayDisplayState (>= 1 Hz)
     +---------------------------------------------+
     |                                             |
     v                                             |
+----------+      First Map Packet       +---------------+      Map Ends / Stop
|   IDLE   | --------------------------> |    PLAYING    | -------------------------+
+----------+                             +---------------+                          |
     ^                                                                              |
     |                       5-second Cooldown Window                               |
     +------------------------------------------------------------------------------+
                                         COOLDOWN
```

### State Definitions
1. **`OPAD_STATE_IDLE`**: Default standby state. The pad displays the idle screen layout. Configuration updates received over serial are saved to NVS flash immediately. <!-- src: /firmware/main/runtime/runtime.c#OPAD_STATE_IDLE -->
2. **`OPAD_STATE_PLAYING`**: Active osu! beatmap session in progress. The pad enters `PLAYING` upon receiving the first `GameplayDisplayState` packet from the host daemon. All flash writes (NVS) are strictly blocked to prevent CPU cache invalidation stalls. Configuration updates are applied to RAM only and flagged dirty. <!-- src: /firmware/main/runtime/runtime.c#OPAD_STATE_PLAYING -->
3. **`OPAD_STATE_COOLDOWN`**: 5-second grace period following map completion (`COOLDOWN_DURATION_US = 5,000,000`). Prevents immediate retry attempts or map restarts from triggering write latency spikes. Once the 5-second cooldown elapses without new gameplay packets, the pad transitions back to `IDLE` and flushes deferred dirty settings to NVS. <!-- src: /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

### Host Watchdog
If the host daemon crashes or `tosu` terminates unexpectedly during a map, the firmware watchdog (`GAMEPLAY_TIMEOUT_US = 3,000,000`) detects the loss of the 1 Hz gameplay heartbeat after 3 seconds and automatically returns the pad to `COOLDOWN` and `IDLE`. <!-- src: /firmware/main/runtime/runtime.c#GAMEPLAY_TIMEOUT_US -->

## Desktop Daemon Architecture

The desktop daemon (`opad-daemon`) acts as a central coordinator between physical hardware, the osu! process (via `tosu`), and user interfaces (`opad-gui` and `opadctl`). <!-- src: /desktop/daemon/src/runtime.rs -->

### The Device Actor Pattern
In earlier implementations, multiple asynchronous tasks concurrently wrote to the serial port, causing race conditions and out-of-order frame delivery. In the current architecture, all host-to-device communication is serialized through a dedicated actor (`desktop/daemon/src/device_actor.rs`): <!-- src: /desktop/daemon/src/device_actor.rs -->

- **Ordered Command Queue**: Commands (`DeviceCommand`) are placed onto a 256-item bounded channel (`QUEUE_DEPTH = 256`). <!-- src: /desktop/daemon/src/device_actor.rs#QUEUE_DEPTH -->
- **Single-Threaded Dispatch**: The device actor task pops commands sequentially, awaits serial transmission and acknowledgment, and handles transient busy states with exponential backoff (`BUSY_RETRIES = 5`, `BUSY_BACKOFF = 20ms`). <!-- src: /desktop/daemon/src/device_actor.rs#BUSY_RETRIES -->
- **Elimination of Races**: Telemetry diffs, layout changes, and configuration writes always arrive at the pad in predictable order.

### Supervision of tosu
`opad-daemon` supervises `tosu` as a child process: <!-- src: /desktop/daemon/src/runtime.rs -->
- It launches `tosu` from the private library directory (`bundled_tosu_binary`).
- It polls `tosu`'s WebSocket interface for live gameplay metrics (song metadata, accuracy, combo, UR, KPM).
- It translates gameplay events into `GameplayDisplayState` and `DataUpdate` protobuf envelopes, forwarding them through the device actor to the pad display engine.
- When beatmaps start or end, the daemon mirrors the firmware's state machine, enforcing `COOLDOWN_DURATION` before committing SQLite database records. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->
