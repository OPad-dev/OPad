---
title: Firmware Overview and Architecture
description: Internal architecture of the ESP32-S3 firmware, FreeRTOS tasks and priorities, IRAM linker placement, memory layout, and Kconfig test flags.
---

OPad firmware is written in C for ESP-IDF **v5.5.2** targeting the Waveshare ESP32-S3-Touch-LCD-2 (dual-core Xtensa LX7 at 240 MHz, 16 MB Flash, 8 MB Octal PSRAM) ([/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults), [/firmware/main/app_main.c](/firmware/main/app_main.c)).

## FreeRTOS Task Layout and Priorities

| Task | Core | Priority | Stack | Source | Purpose |
|---|---|---|---|---|---|
| `keypad_task` | Core 0 | `configMAX_PRIORITIES - 1` | 4096 B | [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c) | Mechanical debounce resample, HID reporting, pin detection, line test |
| `he_task` | Core 0 | `configMAX_PRIORITIES - 1` | 4096 B | [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c) | Hall Effect V1 edge dispatch, stall watchdog, pause-for-probe |
| `he_spi` | Core 0 | `configMAX_PRIORITIES - 1` | 3072 B | [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c) | Hall Effect V3 50 µs ADS7028 SPI3 sampler & direct key publish |
| TinyUSB (`tusb`) | Core 0 | `configMAX_PRIORITIES - 2` | TinyUSB default | [/firmware/main/app_main.c](/firmware/main/app_main.c) | 1000 Hz USB HID endpoints (`0x81`, `0x84`) and CDC-ACM |
| `cdc_proto` | Core 1 | `tskIDLE_PRIORITY + 3` | 6144 B | [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c) | Framed nanopb protocol receive/dispatch/TX |
| `touch_retry` | Core 1 | `2` | 3072 B | [/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c) | CST816S I2C polling while finger down, Quick Retry & swipe actions |
| LVGL port task | Core 1 | `esp_lvgl_port` default | `esp_lvgl_port` | [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c) | 20 ms `pad_timer_cb`, ST7789 SPI2 rendering, HE cal overlay |
| `runtime_task` | Core 1 | `tskIDLE_PRIORITY + 1` | 4096 B | [/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c) | `IDLE`/`PLAYING`/`COOLDOWN` state machine and deferred NVS flushes |

## Critical `sdkconfig.defaults` Settings

In [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults):
- `CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y`: 240 MHz CPU clock (along with IRAM key-path placement, reduces key-to-USB latency under display load from ~240 µs at 160 MHz to ~70 µs).
- `CONFIG_FREERTOS_HZ=1000`: 1 kHz OS tick matching 1 ms USB polling.
- `CONFIG_GPIO_CTRL_FUNC_IN_IRAM=y`: Places GPIO control helpers in IRAM for the key ISR and `keypad_task`.
- `CONFIG_TINYUSB_HID_COUNT=2`, `CONFIG_TINYUSB_CDC_COUNT=1`, `CONFIG_TINYUSB_CDC_TX_BUFSIZE=2048`, `CONFIG_TINYUSB_SUSPEND_CALLBACK=y`.
- `CONFIG_SPIRAM=y`, `CONFIG_SPIRAM_MODE_OCT=y`, `CONFIG_SPIRAM_SPEED_80M=y`, `CONFIG_SPIRAM_ALLOW_BSS_SEG_EXTERNAL_MEMORY=y`: Enables the board's 8 MB Octal PSRAM and allows placing off-key-path static buffers in PSRAM via `EXT_RAM_BSS_ATTR`.
- `CONFIG_LV_MEM_SIZE_KILOBYTES=160` and `CONFIG_LV_DEF_REFR_PERIOD=33`: Dedicated 160 KB LVGL pool (in PSRAM via `CONFIG_OPAD_LVGL_PSRAM=y` by default) and a 30 fps screen redraw cap (33 ms refresh period).
- `CONFIG_ESP32S3_INSTRUCTION_CACHE_32KB=y`, `CONFIG_ESP32S3_DATA_CACHE_64KB=y`, `CONFIG_ESP32S3_DATA_CACHE_LINE_64B=y`: Largest ESP32-S3 caches so flash-executed code/fonts and PSRAM-backed LVGL buffers render quickly.
- `CONFIG_PARTITION_TABLE_CUSTOM=y` (`partitions.csv`) and `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`: Two 2 MB OTA slots (`ota_0`, `ota_1`) with rollback until USB mounts.

## IRAM Placement & Memory Layout

- **IRAM linker fragment ([/firmware/main/linker.lf](/firmware/main/linker.lf))**: Places `keypad:keypad_task`, `usb_hid`, `he_frame`, `he_rt`, `he_spi_source:sampler_task`, `he_spi_frame`, and TinyUSB's HID/USBD/`dcd_dwc2`/`tu_hwfifo_write` hot path in IRAM (`noflash`) so Core 1 cache activity never stalls key delivery.
- **Internal RAM vs PSRAM split**: Only the keys, USB, and DMA buffers stay in internal RAM. By default (`CONFIG_OPAD_LVGL_PSRAM=y`), LVGL's 160 KB pool lives in PSRAM, and off-key-path static buffers (`s_layouts` in `ui_port.c`, `s_pending_layouts` in `ui_store.c`, `s_subjects`/`s_str_buf`/`s_str_prev` in `ui_data.c`, and `s_parser`/`msg`/`layout` in `protocol.c`) are placed in PSRAM via `EXT_RAM_BSS_ATTR`. The two 40-line LCD draw buffers (`UI_DRAW_BUF_LINES = 40`, `25.6 KB` each) stay in internal DMA-capable RAM ([/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild), [/firmware/main/ui/ui.h](/firmware/main/ui/ui.h), [/firmware/main/protocol/protocol.c](/firmware/main/protocol/protocol.c)). At boot, [/firmware/main/app_main.c](/firmware/main/app_main.c) logs `DIAG_EVENT_MEMORY` with free internal RAM and the largest free DMA-capable block.

## Compile-Time Kconfig Options (`Kconfig.projbuild`)

Defined in [/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild) under **OPad Testing and Benchmark**:
- `CONFIG_OPAD_TEST_FAIL_LCD` / `CONFIG_OPAD_TEST_FAIL_NVS`: Fault-injection builds verifying headless keyboard fallback and RAM-only counter operation.
- `CONFIG_OPAD_BENCH_HID_ONLY` / `CONFIG_OPAD_BENCH_NO_DISPLAY`: Stage A (HID + keypad only) and Stage B (CDC active, display disabled) latency benchmark modes.
- `CONFIG_OPAD_BENCH_DEBUG_GPIO` / `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM` (default `4`): Toggles a GPIO right after HID submit for logic-analyzer correlation.
- `CONFIG_OPAD_HE_BACKEND`: Enables V1 Hall Effect ADC1 DMA input (`select ADC_CONTINUOUS_ISR_IRAM_SAFE`).
- `CONFIG_OPAD_HALL_BENCH` / `CONFIG_OPAD_HE_TRACE` / `CONFIG_OPAD_HE_DEBUG`: Raw CDC sensor streaming, ~150 KB/s trace capture, and `+hedebug` per-press diagnostic logging.
- `CONFIG_OPAD_UI_PERF_TEST` / `CONFIG_OPAD_LVGL_PSRAM` (default `y`): Synthetic UI perf loop with FPS/memory overlays and `DIAG_EVENT_UI_PERF` frame-time reporting; PSRAM allocation for LVGL's memory pool.
- `CONFIG_OPAD_DEBUG_I2C_SCAN`: Probes all 126 touch-bus I2C addresses at boot (~1.3 s added boot time).
