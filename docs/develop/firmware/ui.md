---
title: Firmware UI Subsystem
description: Implementation reference for the ST7789 display driver, LVGL integration, PSRAM framebuffers, and layout store architecture.
category: reference
source_files:
  - /firmware/main/ui/ui_store.c
  - /firmware/main/ui/core/ui_screen.c
  - /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c
  - /firmware/main/ui/he_cal_ui.c
---

The UI subsystem renders dynamic telemetry HUD widgets, song metadata, celebration grade animations, and guided calibration wizards on the Waveshare 2.0-inch ST7789 LCD display (320x240 resolution). All display rendering runs on Core 1 under LVGL v8, using PSRAM memory pools to protect internal SRAM and keypress latency. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c -->

## Display Hardware & SPI Driver (`board_display.c`)

- **Controller**: Sitronix ST7789 IPS LCD controller. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c -->
- **Interface**: SPI2 host (`BOARD_LCD_SPI_HOST`) operating at high clock speed with hardware DMA transfers.
- **Pin Assignments**:
  - `SCLK`: GPIO 39
  - `MOSI`: GPIO 38
  - `DC`: GPIO 42
  - `CS`: GPIO 45
  - `Backlight (BL)`: GPIO 1 (driven by LEDC PWM timer for granular 0–100% brightness control). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c#board_display_set_brightness -->

## LVGL Integration & PSRAM Memory Pool

- **PSRAM Allocation (`CONFIG_OPAD_LVGL_PSRAM`)**: LVGL's internal memory pool (`lv_mem_init`) and double-buffered RGB565 draw buffers are allocated out of external 2 MB PSRAM using `opad_lvgl_pool_alloc` (`ui/lvgl_psram_pool.h`). This keeps the internal 512 KB SRAM completely free for FreeRTOS tasks and high-priority ISR stacks. <!-- src: /firmware/CMakeLists.txt -->
- **Display Rotation**: The ST7789 hardware panel is mounted in portrait (240x320); `ui_port.c` rotates the display context via `swap_xy` and `mirror_x` to present a 320x240 landscape interface with zero software rotation penalty.

## Layout Engine & UI Store (`ui_store.c` & `ui_screen.c`)

Screen layouts are structured as collections of up to 32 dynamic widgets (`ui_widget_t`): <!-- src: /firmware/main/ui/ui_store.c -->

- **Profiles Supported**:
  - `UI_SCREEN_IDLE`: Rendered during menu navigation or desktop usage.
  - `UI_SCREEN_PLAYING`: Rendered during active osu! gameplay attempts.
- **Widget Properties (`ui_screen.c`)**:
  - Bound data sources (combo, accuracy, UR, PPM, song artist/title).
  - Configurable typography (`s_fonts` table: Montserrat 12, 14, 16, 20, 24, 32, 48 pt). <!-- src: /firmware/main/ui/core/ui_screen.c#s_fonts -->
  - Text alignments (`LV_TEXT_ALIGN_LEFT`, `CENTER`, `RIGHT`). <!-- src: /firmware/main/ui/core/ui_screen.c#s_text_align -->
  - Visual flags (`FLAG_BORDER`, `FLAG_BG_FILL`, `FLAG_HIDE_WHEN_EMPTY`).
- **Deferred NVS Persistence**: When the host sends `SetLayout` during a map, the layout is staged in RAM (`s_pending_layout`). Layouts are written to NVS only when the device transitions back to `IDLE` (`ui_store_flush_dirty`), ensuring flash erase cycles never coincide with gameplay. <!-- src: /firmware/main/ui/ui_store.c -->

## Guided Calibration UI (`he_cal_ui.c`)

When Hall Effect calibration is active: <!-- src: /firmware/main/ui/he_cal_ui.c -->
- Normal HUD screens are suppressed, and the calibration wizard screen (`he_cal_ui`) is created.
- Renders step-by-step instructions (rest position settling, bottom-out capture for Key 1 and Key 2).
- Shows live feedback bars reflecting raw ADC readings and errors (`HE_CAL_ERR_REST_NOT_STILL`, `HE_CAL_ERR_SPAN_TOO_SMALL`).
- Touch input on this screen is restricted strictly to calibration button hits, completely suppressing normal touchscreen Quick Retry and gesture swipes. <!-- src: /firmware/main/ui/he_cal_ui.c -->
