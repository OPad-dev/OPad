---
title: Firmware Overview and Architecture
description: Internal architecture of the ESP32-S3 firmware, FreeRTOS task topologies, memory partitioning, sdkconfig settings, and compile-time test flags.
---

The OPad firmware is written in C99 using ESP-IDF **v5.5.2** on the ESP32-S3 dual-core microcontroller (Xtensa LX7 at 240 MHz). It is structured around strict hardware isolation: time-critical input scanning and USB HID reporting run on Core 0, while display rendering, CDC serial protocol framing, touch gestures, and NVS storage management run on Core 1. <!-- src: /firmware/main/app_main.c -->

## FreeRTOS Task Layout and Priorities

Tasks are statically assigned to CPU cores and prioritized to ensure that keypress processing is never preempted by auxiliary background tasks: <!-- src: /firmware/sdkconfig.defaults -->

| Task Name | Core ID | Priority | Stack Size | Source Module | Purpose |
|---|---|---|---|---|---|
| **`keypad_task`** | Core 0 | `MAX_PRIORITIES - 1` | 4096 B | `input/keypad.c` | Mechanical edge debouncing and key events. <!-- src: /firmware/main/input/keypad.c --> |
| **`he_task`** | Core 0 | `MAX_PRIORITIES - 1` | 4096 B | `input/he_backend.c` | Hall Effect ADC conversion and Rapid Trigger state steps. <!-- src: /firmware/main/input/he_backend.c --> |
| **`tusb`** | Core 0 | `MAX_PRIORITIES - 1` | Default | TinyUSB Stack | USB HID endpoint report submission and 1000 Hz SOF polling. <!-- src: /firmware/main/app_main.c --> |
| **`cdc_proto`** | Core 1 | `IDLE + 3` | 6144 B | `usb/usb_cdc.c` | CDC-ACM serial framing, Nanopb proto decoding. <!-- src: /firmware/main/usb/usb_cdc.c --> |
| **`touch_retry`** | Core 1 | `2` (Low) | 3072 B | `input/touch_retry.c` | CST816S I2C touch reading and gesture tracking. <!-- src: /firmware/main/input/touch_retry.c --> |
| **`runtime_task`**| Core 1 | `IDLE + 1` | 4096 B | `runtime/runtime.c` | Map timeout watchdog, deferred NVS flushes. <!-- src: /firmware/main/runtime/runtime.c --> |
| **`gui` / `lvgl`**| Core 1 | `IDLE + 2` | Dynamic | `ui/ui_store.c` | LVGL display pipeline, ST7789 SPI DMA flushes. <!-- src: /firmware/main/ui/ui_store.c --> |

## Critical Configuration Options (`sdkconfig.defaults`)

Firmware performance invariants are enforced via Kconfig settings (`firmware/sdkconfig.defaults`): <!-- src: /firmware/sdkconfig.defaults -->

- **`CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y`**: Maximum CPU clock speed (240 MHz). Benchmarks demonstrate input-to-USB report submission latency drops from ~240 µs (at 160 MHz with display traffic) to under 70 µs at 240 MHz.
- **`CONFIG_FREERTOS_HZ=1000`**: 1000 Hz OS tick rate enabling 1 ms FreeRTOS scheduling resolution, matching the 1000 Hz USB polling interval. <!-- src: /firmware/sdkconfig.defaults -->
- **`CONFIG_GPIO_CTRL_FUNC_IN_IRAM=y`**: Locks GPIO register access functions into fast internal IRAM, eliminating flash cache misses during switch interrupt handling. <!-- src: /firmware/sdkconfig.defaults -->
- **`CONFIG_TINYUSB=y`**: Uses native ESP32-S3 USB full-speed hardware with TinyUSB composite HID + CDC drivers.

## Memory Partitioning & Linker Rules

The memory map separates internal fast RAM (IRAM/DRAM) from external SPI PSRAM: <!-- src: /firmware/main/app_main.c -->
- **IRAM Placement (`linker.lf`)**: GPIO ISR handlers, eager debounce functions (`debounce_step`), Hall Effect frame interrupts (`on_frame`), and latency statistics collectors (`latency_stats_record`) are pinned to IRAM.
- **PSRAM Allocation (`CONFIG_OPAD_LVGL_PSRAM`)**: LVGL memory pools and 320x240 RGB565 display framebuffers are allocated from external 2 MB PSRAM (`opad_lvgl_pool_alloc`), preserving internal SRAM for the FreeRTOS heap and task stacks. <!-- src: /firmware/CMakeLists.txt -->

## Compile-Time Benchmarking & Diagnostics Flags

`firmware/main/Kconfig.projbuild` provides test switches to isolate subsystems during latency validation: <!-- src: /firmware/main/Kconfig.projbuild -->

- **`CONFIG_OPAD_BENCH_HID_ONLY`**: Disables LVGL, CDC serial tasks, and runtime supervisors to measure pure baseline input hardware latency (Stage A testing). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_BENCH_NO_DISPLAY`**: Runs CDC communications and telemetry sync while disabling LCD SPI DMA transactions (Stage B testing). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_TEST_FAIL_LCD`**: Forces display initialization to fail, verifying headless keypad fallback behavior (Product Rule 2). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_TEST_FAIL_NVS`**: Simulates SPI flash failure to verify volatile RAM-only counter tracking. <!-- src: /firmware/main/Kconfig.projbuild -->
