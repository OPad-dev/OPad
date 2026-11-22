---
title: Firmware UI Subsystem
description: Implementation reference for the ST7789 SPI2 display driver, LVGL v9.5 integration, memory allocation, layout store, and Hall Effect calibration overlay.
---

The UI subsystem renders customizable `320x240` layouts, live telemetry, result animations, and the Hall Effect calibration overlay on the Waveshare 2.0-inch ST7789 IPS LCD. All rendering runs on **Core 1** under **LVGL v9.5.0** and `esp_lvgl_port` 2.9.0 ([/firmware/dependencies.lock](/firmware/dependencies.lock), [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c)).

## Display Hardware & SPI2 Driver (`board_display.c`)

Implemented in [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c) and [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h):

- **Controller**: ST7789 IPS LCD (`320x240`, RGB565, `LCD_RGB_ENDIAN_RGB`, color inversion on, `swap_xy = true`, `mirror_x = true` for landscape orientation).
- **Bus**: `SPI2_HOST` with `SPI_DMA_CH_AUTO` at `BOARD_LCD_PIXEL_CLOCK_HZ = 80 MHz` (~15 ms full-frame transfer). SPI DMA interrupts are pinned to Core 1 (`ESP_INTR_CPU_AFFINITY_1`) so LCD transfers never preempt Core 0 input.
- **Pins**: `SCLK = GPIO 39`, `MOSI = GPIO 38`, `DC = GPIO 42`, `CS = GPIO 45`, backlight `BL = GPIO 1` (LEDC PWM, `board_display_set_brightness`).
- **Fault injection**: `CONFIG_OPAD_TEST_FAIL_LCD` forces `board_display_init` to return `ESP_FAIL` to verify headless keyboard operation.

## LVGL Memory & Render Loop (`ui_port.c`)

- **Memory pool in PSRAM (`CONFIG_LV_MEM_SIZE_KILOBYTES = 160`, `CONFIG_OPAD_LVGL_PSRAM = y`)**: Dedicated 160 KB pool allocated in the 8 MB Octal PSRAM by default so LVGL never contends with internal RAM needed by the key path and DMA buffers (a Hall Effect V3 build with the pool in internal RAM ran out of DMA RAM for the draw buffers) ([/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults), [/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild)).
- **Off-key-path UI buffers in PSRAM (`CONFIG_SPIRAM_ALLOW_BSS_SEG_EXTERNAL_MEMORY = y`)**: `s_layouts` in [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c), `s_pending_layouts` in [/firmware/main/ui/ui_store.c](/firmware/main/ui/ui_store.c), and `s_subjects` / `s_str_buf` / `s_str_prev` in [/firmware/main/ui/core/ui_data.c](/firmware/main/ui/core/ui_data.c) are placed in PSRAM with `EXT_RAM_BSS_ATTR` (guarded by `#ifdef ESP_PLATFORM` in `ui_data.c` so the desktop `opad-ui-preview` crate builds without `esp_attr.h`).
- **40-line DMA draw buffers (`UI_DRAW_BUF_LINES = 40`)**: `ui_init` allocates two `320x40` RGB565 draw buffers (`25.6 KB` each) in internal DMA-capable RAM for the LCD SPI2 DMA ([/firmware/main/ui/ui.h](/firmware/main/ui/ui.h), [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c)).
- **30 fps cap & largest caches**: `CONFIG_LV_DEF_REFR_PERIOD = 33` caps redraws at 30 fps (a full frame takes ~15.4 ms over 80 MHz SPI), paired with 32 KB instruction cache (`CONFIG_ESP32S3_INSTRUCTION_CACHE_32KB=y`) and 64 KB / 64 B-line data cache (`CONFIG_ESP32S3_DATA_CACHE_64KB=y`, `CONFIG_ESP32S3_DATA_CACHE_LINE_64B=y`) to keep PSRAM/flash-backed rendering fast ([/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)).
- **20 ms UI timer (`pad_timer_cb`)**: Runs inside the Core 1 LVGL task (`PAD_TIMER_PERIOD_MS = 20`), updating pad-local sources (key lifetime/map counts, 80 ms key-flash hold, 1 s rolling KPS, clock/date, PC/tosu status) and display sleep/wake (`update_sleep`).

## Layout Engine & Deferred Persistence (`ui_store.c`, `ui_screen.c`)

- **Two screens**: `UI_SCREEN_IDLE` and `UI_SCREEN_PLAYING`, each holding up to 32 widgets with Montserrat 12/14/16/20/24/32/48 pt fonts ([/firmware/main/ui/core/ui_screen.c](/firmware/main/ui/core/ui_screen.c), [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)).
- **NVS store ([/firmware/main/ui/ui_store.c](/firmware/main/ui/ui_store.c))**: Saves layouts in `NVS_NS_UI` (`opad_ui`) under keys `"layout0"` / `"layout1"` wrapped in `layout_blob_t` (`LAYOUT_BLOB_VERSION = 1`).
- **Gameplay flash invariant**: `ui_store_save` and `ui_store_erase` defer writes into `s_pending_layouts` / `s_dirty_save_mask` / `s_dirty_erase_mask` whenever `runtime_get_state() != OPAD_STATE_IDLE`, and `ui_store_flush_dirty` commits them in `IDLE` (retrying failed writes after `FLUSH_RETRY_US = 10 s`).

## Hall Effect Calibration Overlay (`he_cal_ui.c`)

Implemented in [/firmware/main/ui/he_cal_ui.c](/firmware/main/ui/he_cal_ui.c) on `lv_layer_top()` (20 ms LVGL timer, only initialized when `he_backend_running()`):
- Shows the red **Keys not calibrated** lock screen (`HE_LOCK_SCREEN_WAIT_FOR_APP` for 5 s, then `HE_LOCK_SCREEN_NO_APP_BUTTON` with **Calibrate here** if no capable app connects, or `HE_LOCK_SCREEN_USE_APP` when connected).
- Displays rest capture, 5 numbered progress boxes (`1`–`5`) turning green across 5 full presses per key (Keys 1–2 on V1, Keys 1–3 on V3), 1 s green **Done!** confirmation, or 2 s red **Calibration failed** reason.
