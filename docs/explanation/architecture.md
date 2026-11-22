---
title: System Architecture
description: Overall architecture of OPad, covering ESP32-S3 dual-core task isolation, firmware runtime states, and the desktop daemon actor model.
sidebar:
  order: 1
---

OPad is organized around **latency always wins** ([/AGENTS.md](/AGENTS.md) §1.1): key scanning and 1000 Hz USB HID reporting run on Core 0 with zero heap allocation, zero blocking locks shared with Core 1, and zero flash writes during play. The LCD screen, NVS persistence, touchscreen gestures, and `tosu` telemetry stay off the keypress path.

## Dual-Core Hardware Isolation (ESP32-S3)

The ESP32-S3 runs dual Xtensa LX7 cores at 240 MHz (`CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y` in [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)). FreeRTOS tasks are pinned asymmetrically in [/firmware/main/app_main.c](/firmware/main/app_main.c):

```text
+-------------------------------------------------+-------------------------------------------------+
| CORE 0 (Input & USB HID)                        | CORE 1 (UI, CDC Serial, Runtime & Storage)      |
+-------------------------------------------------+-------------------------------------------------+
| • keypad_task / he_task / he_spi (MAX - 1)      | • cdc_proto / usb_cdc_task (IDLE + 3)           |
| • TinyUSB device task (MAX - 2)                 | • touch_retry_task (Priority 2)                 |
| • GPIO key ISRs / V1 ADC1 DMA / V3 SPI3 polling | • LVGL display task & ST7789 SPI2 flushes       |
| • IRAM key path, zero flash writes, zero malloc | • runtime_supervisor_task (IDLE + 1)            |
|                                                 | • Deferred NVS checkpoints & diagnostics        |
+-------------------------------------------------+-------------------------------------------------+
```

### Core 0: The Keypress Path
- **Input tasks** (`keypad_task` in [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c), `he_task` in [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), `he_spi` in [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c)): Pinned to Core 0 at `configMAX_PRIORITIES - 1`. Mechanical switches use an IRAM GPIO ISR with eager debouncing; Hall Effect V1 uses internal ADC1 continuous DMA (`4 kHz` frames), and Hall Effect V3 uses a 50 µs hardware `gptimer` polling the external ADS7028 over 40 MHz SPI3 (`SPI_DMA_DISABLED`, direct register reads).
- **TinyUSB task**: Pinned to Core 0 at `configMAX_PRIORITIES - 2` ([/firmware/main/app_main.c](/firmware/main/app_main.c)) so only the input tasks outrank it. Drives the dual 1000 Hz HID endpoints (`EP 0x81` and `EP 0x84`) and CDC-ACM.
- **IRAM linker pinning**: [/firmware/main/linker.lf](/firmware/main/linker.lf) locks `keypad_task`, `usb_hid`, `he_frame`, `he_rt`, `he_spi_source:sampler_task`, `he_spi_frame`, and TinyUSB's HID/DWC2 hot path into IRAM (`noflash`) so Core 1 display/flash traffic cannot cause instruction cache stalls.

### Core 1: System Services & UI
- **`cdc_proto` (`usb_cdc_task`)**: Pinned to Core 1 (`tskIDLE_PRIORITY + 3`) in [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c); decodes framed nanopb protocol messages and dispatches replies.
- **`touch_retry` (`touch_retry_task`)**: Pinned to Core 1 (priority 2) in [/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c); sleeps on the CST816S interrupt and polls I2C only while a finger is down.
- **Display & LVGL**: Renders `320x240` layouts to the ST7789 over 80 MHz SPI2 at up to 30 fps (`CONFIG_LV_DEF_REFR_PERIOD=33`), with LVGL's 160 KB pool and off-key-path UI/protocol buffers in 8 MB Octal PSRAM (`CONFIG_OPAD_LVGL_PSRAM=y`, `EXT_RAM_BSS_ATTR`) so internal RAM stays reserved for Core 0 input, USB, and the two 40-line DMA draw buffers (`UI_DRAW_BUF_LINES = 40`), backed by 32 KB instruction and 64 KB data caches ([/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c), [/firmware/main/ui/ui.h](/firmware/main/ui/ui.h), [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)).
- **`runtime_task` (`runtime_supervisor_task`)**: Pinned to Core 1 (`tskIDLE_PRIORITY + 1`) in [/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c); manages gameplay state transitions, timeouts, and deferred NVS flushes.

## Firmware Runtime State Machine

[/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c) enforces flash-write isolation via three runtime states:

```text
         Host sends HostStatus / gameplay heartbeat
      +---------------------------------------------+
      |                                             |
      v                                             |
+----------+      Map Starts               +---------------+     Map Ends / 3 s Timeout
|   IDLE   | ----------------------------> |    PLAYING    | ----------------------------+
+----------+                               +---------------+                             |
      ^                                                                                  |
      |                          5 s Cooldown (COOLDOWN_DURATION_US)                     |
      +----------------------------------------------------------------------------------+
                                            COOLDOWN
```

1. **`OPAD_STATE_IDLE`**: Default state. Shows the idle layout; config, calibration, and counter checkpoints may write to NVS.
2. **`OPAD_STATE_PLAYING`**: Entered when the host reports active gameplay. All NVS writes are blocked; config and counter updates stay in RAM and are flagged dirty.
3. **`OPAD_STATE_COOLDOWN`**: 5-second grace period (`COOLDOWN_DURATION_US = 5,000,000` µs) after a map ends so quick retries do not trigger NVS writes. Returning to `IDLE` flushes deferred changes.
4. **Host watchdog**: If gameplay heartbeats stop for 3 seconds (`GAMEPLAY_TIMEOUT_US = 3,000,000` µs), firmware exits `PLAYING` into `COOLDOWN` and `IDLE`.

## Desktop Daemon Architecture

`opad-daemon` ([/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs)) coordinates USB CDC communication, SQLite persistence (`opad.db`), `tosu` supervision, updates, and local IPC for `opad-gui` and `opadctl`.

### Single-Writer Device Actor
All host-to-pad commands from the runtime loop flow through a single ordered task in [/desktop/daemon/src/device_actor.rs](/desktop/daemon/src/device_actor.rs):
- Bounded channel depth `QUEUE_DEPTH = 256` carrying `DeviceCommand` (`TimeSync`, `ClaimOwnership`, `Config`, `HostStatus`, `DataUpdate`, `Layout`, `RequestStatus`, `PlayAnimation`).
- Each command is sent and awaited sequentially so config, layout, and telemetry messages never overtake each other.
- Transient `DeviceError::Busy` errors retry up to `BUSY_RETRIES = 5` times with `BUSY_BACKOFF = 20 ms`, except stale `DataUpdate` diffs which instead set `resend_data` so the next flush sends a fresh snapshot.

### `tosu` Supervision and Storage Guard
`opad-daemon` supervises `tosu` via [/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs), converts WebSocket v2 telemetry diffs into rate-limited `DataUpdate` and `HostStatus` messages ([/desktop/daemon/src/telemetry.rs](/desktop/daemon/src/telemetry.rs)), and mirrors the pad's `Playing` / `Cooldown` window by disabling SQLite writes (`Storage::set_writes_allowed(false)`) until the post-play cooldown ends.
