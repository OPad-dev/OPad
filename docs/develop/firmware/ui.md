---
title: Firmware UI Subsystem
description: Implementation reference for the ST7789 SPI2 display driver, LVGL 9.5 integration on core 1, memory placement, the 20 ms pad timer, the portable ui/core layout engine, deferred layout storage, and the Hall Effect calibration overlay.
sidebar:
  order: 5
---

The UI draws the customizable 320×240 Idle and Playing layouts, result animations, swipe feedback and the Hall Effect calibration overlay on the Waveshare 2.0" ST7789 IPS LCD. It runs entirely on **core 1** under LVGL **9.5.0** with `esp_lvgl_port` **2.9.0** ([/firmware/dependencies.lock](https://github.com/OPad-dev/OPad/blob/main/firmware/dependencies.lock)). The keypad path only ever touches it through `ui_notify_activity()`, one atomic timestamp store ([/firmware/main/ui/ui.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui.h)).

The screen is optional: if `ui_init()` fails the pad logs `DIAG_EVENT_LCD_INIT_FAILED` and keeps working as a headless keyboard (`CONFIG_OPAD_TEST_FAIL_LCD` proves it).

## Files

| File | Role |
|---|---|
| [/firmware/main/ui/ui_port.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_port.c) | `ui_init()`, LVGL port setup, screen switching, `pad_timer_cb`, display sleep, the public `ui_*` API used by the protocol task |
| [/firmware/main/ui/ui_store.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_store.c) | Layout persistence in NVS, deferred outside `IDLE` |
| [/firmware/main/ui/he_cal_ui.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/he_cal_ui.c) | HE lock screen and calibration flow overlay |
| [/firmware/main/ui/easter_egg.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/easter_egg.c) | The "67" GIF (`FREAKY67`, or a map ending on a PP ending in 67) |
| [/firmware/main/ui/flash_anim.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/flash_anim.c) | The `updating` animation before the restart into the ROM bootloader |
| [/firmware/main/assets/asset_pack.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/assets/asset_pack.c) | Reads the asset packs in the `assets` and `user` partitions |
| [/firmware/main/ui/core/](https://github.com/OPad-dev/OPad/tree/main/firmware/main/ui/core) | Portable engine: layout model (`ui_core.h`), data subjects (`ui_data.c`), screen builder (`ui_screen.c`), result animations (`ui_anim.c`), default layouts and palette (`ui_defaults.c`), wire IDs (`ui_ids.h`). Depends only on LVGL, so `opad-ui-preview` compiles the same code for the desktop Screen Designer ([crates](../desktop/crates.md#7-opad-ui-preview)) |
| [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c) | ST7789 panel bring-up, sleep/wake, brightness |

`ui_ids.h` values are wire format shared with `protocol/opad.proto` and the desktop: append only, never renumber. The data sources a layout can bind are listed in [Customizing the Screen Layout](../../guides/customize-screen-layout.md).

## Display Hardware (`board_display.c`)

- **Panel**: ST7789, 320×240 RGB565, `LCD_RGB_ENDIAN_RGB`, colour inversion on; landscape via `swap_xy` and `mirror_x` on the panel, mirrored by `.rotation` in the LVGL display config. `.swap_bytes = true` gives the big-endian RGB565 the panel wants.
- **Bus**: `SPI2_HOST`, DMA (`buff_dma`), `BOARD_LCD_PIXEL_CLOCK_HZ = 80 MHz`: a full frame in ~15 ms instead of ~31 ms. That is above the ST7789's rated 62.5 MHz write clock; it was checked on a pad with no artifacts ([/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h)).
- **Interrupt affinity**: the panel IO's SPI interrupt is allocated on core 1 (`isr_cpu_id = ESP_INTR_CPU_AFFINITY_1`), so LCD transfers never interrupt core 0.
- **Pins**: SCLK GPIO39, MOSI GPIO38, DC GPIO42, CS GPIO45, no reset line, backlight GPIO1 on LEDC PWM (`board_backlight_init`, `board_display_set_brightness`).

## LVGL Port and Render Loop (`ui_port.c`)

- **Port task**: `lvgl_port_init()` with `task_affinity = 1`, `task_priority = 2`, `task_max_sleep_ms = 500`. `esp_lvgl_port`'s tick timer lives on core 0, so `ui_init()` stops it (`lvgl_port_stop()`) and sets `lv_tick_set_cb(ui_tick_ms)`: LVGL reads `esp_timer` directly.
- **Draw buffers**: two (`double_buffer`) of `UI_SCREEN_W × UI_DRAW_BUF_LINES` = 320×40 pixels, 25.6 KB each, in internal DMA RAM. 20 lines saved 25.6 KB but drew the result animation a third slower.
- **Refresh**: `CONFIG_LV_DEF_REFR_PERIOD = 33` caps redraws at 30 fps; values change at most 10 times a second and 60 fps looked no different ([/firmware/sdkconfig.defaults](https://github.com/OPad-dev/OPad/blob/main/firmware/sdkconfig.defaults)).
- **`pad_timer_cb`** (`PAD_TIMER_PERIOD_MS = 20`, in the LVGL task):
  - loads `UI_SCREEN_PLAYING` while `runtime_get_state() == OPAD_STATE_PLAYING`, otherwise `UI_SCREEN_IDLE`; forces Idle while the HE pad is locked or calibrating; stops a result animation when a map starts;
  - triggers the easter egg when a map ends with PP ending in 67;
  - `update_sleep()`: after `sleep_s` without activity (default 600 s) disables invalidation and sleeps the panel; activity wakes it and restores brightness;
  - while awake, `update_pad_sources()`: lifetime and per-map counts of the two game-key cards (`game_keys_slots`), an 80 ms key-flash hold so a tap is visible, totals, 1 s rolling KPS, PC/tosu status (clears daemon-sourced values when the port closes), uptime, and clock/date once a minute after the host has set the time.
- **Protocol-side API**: `ui_set_number()`, `ui_set_string()`, `ui_set_layout()`, `ui_play_animation()` (ignored during a map) take the LVGL lock from the protocol task; `ui_lock()` / `ui_unlock()` batch several updates and return false when headless.

## Memory Placement

- **LVGL pool**: `CONFIG_LV_MEM_SIZE_KILOBYTES = 160`, allocated from PSRAM at boot when `CONFIG_OPAD_LVGL_PSRAM = y` (default) through `LV_MEM_POOL_ALLOC` ([/firmware/main/ui/lvgl_psram_pool.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/lvgl_psram_pool.h)). With the pool in internal RAM an HE V3 pad had too little DMA RAM left for the draw buffers and booted blank. `easter_egg_init()` adds a further 512 KB PSRAM pool for the GIF.
- **Static buffers in PSRAM** (`EXT_RAM_BSS_ATTR`, `CONFIG_SPIRAM_ALLOW_BSS_SEG_EXTERNAL_MEMORY`): `s_layouts` (`ui_port.c`), `s_pending_layouts` (`ui_store.c`), `s_subjects` / `s_str_buf` / `s_str_prev` (`ui_data.c`, behind `#ifdef ESP_PLATFORM` so the desktop preview builds without `esp_attr.h`).
- **Caches**: 32 KB instruction and 64 KB / 64 B-line data cache halve frame time for flash-resident code and fonts and the PSRAM pool (gameplay screen 29 → 14.5 ms per frame, per the comment in `sdkconfig.defaults`).

## Layouts and Deferred Persistence (`ui_store.c`)

- Two screens, `UI_SCREEN_IDLE` and `UI_SCREEN_PLAYING`, each up to `UI_MAX_WIDGETS = 32` widgets ([/firmware/main/ui/core/ui_core.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/core/ui_core.h)). At boot `ui_store_load()` reads each, falling back to `ui_default_layout()`.
- Stored in `NVS_NS_UI` (`opad_ui`) under `layout0` / `layout1`, wrapped in `layout_blob_t` (`LAYOUT_BLOB_VERSION = 1`, size-checked against `ui_layout_t`).
- **No flash writes outside `IDLE`**: `ui_store_save()` and `ui_store_erase()` only set `s_pending_layouts`, `s_dirty_save_mask` and `s_dirty_erase_mask` while `runtime_get_state() != OPAD_STATE_IDLE`; `ui_store_flush_dirty()` (called by the runtime task) commits them in `IDLE` and retries a failed write after `FLUSH_RETRY_US = 10 s`.

## Hall Effect Calibration Overlay (`he_cal_ui.c`)

Built on `lv_layer_top()` with its own 20 ms LVGL timer, and only when `he_backend_running()` (MX pads have no calibration timer). It implements decision D7 of [/docs/specs/v2-hall-effect-decisions.md](../../specs/v2-hall-effect-decisions.md); the user flow is in [Calibrating Hall Effect](../../guides/calibrate-hall-effect.md).

| Screen | Shown when | Text |
|---|---|---|
| `HE_LOCK_SCREEN_WAIT_FOR_APP` | Locked, first 5 s after boot (`BOOT_APP_WAIT_US`) | **Keys not calibrated** / "Looking for the OPad app..." |
| `HE_LOCK_SCREEN_USE_APP` | Locked, an app that can calibrate is connected | "Calibrate in the OPad app" / "on your PC"; no button |
| `HE_LOCK_SCREEN_NO_APP_BUTTON` | Locked, no capable app | "For the best setup, get the app:" `https://opad.dev/download` (`OPAD_DOWNLOAD_URL`) and a **Calibrate here** button |
| Rest | Calibration started | **Calibrating** / "Take your fingers off both keys" (all keys on V3) |
| Key *n* | Each key in turn | **Key *n*** / "Press key *n* all the way, 5 times", five progress boxes |
| Success | All keys captured and saved | **Done!** / "Your keys are ready", green, 1 s |
| Failure | Capture error, save error, or the app disconnected | **Calibration failed** with the reason, red, 2 s; a **Try again** button only on a locked pad with no app |

The button uses `he_cal_button.c` (down and up inside it, excursion under 30 px), so a swipe across it does nothing. The lock lifts as soon as the new calibration is saved; no reboot.

## Asset Packs and the Flash Animation (`asset_pack.c`, `flash_anim.c`)

The `assets` (OPad's, 5 MB) and `user` (the owner's, 6.8 MB) partitions each hold one asset pack: a 64-byte header with a CRC32 of the rest, a table of 64-byte entries (name, GIF or MJPEG, size, frame count, time per frame) and their data. `scripts/build_asset_pack.py` writes a pack and checks each asset on the PC first: an MJPEG must be baseline JPEG (SOF0), which TJpgDec decodes; a 4:4:4 one it refuses (`JDR_FMT3`) shows black. `opadctl flash --full` writes `opad-assets.bin` and `opad-user.bin` when they are next to the app image. <!-- src: /firmware/main/assets/asset_pack.h, /scripts/build_asset_pack.py, /desktop/cli/src/main.rs#resolve_flash_set -->

`asset_pack_open()` maps a pack with `esp_partition_mmap()` and reads it through the flash cache, never with `esp_flash_read()`, which turns the cache off on both cores and would stall the keys. A missing, erased or unreadable pack is reported as absent, and everything that uses one carries on without it. <!-- src: /firmware/main/assets/asset_pack.c -->

When the host asks for the ROM bootloader, `reboot_to_rom_download()` first calls `flash_anim_play(FLASH_ANIM_MS)` (1.2 s): the MJPEG named `updating` plays straight to the panel, one row of JPEG blocks at a time from a 10 KB DMA buffer, without LVGL, whose lock it takes and never gives back. The backlight is then latched on (`board_backlight_hold_on()`, full brightness: GPIO1's hold survives the restart), and the panel keeps the last frame in its own memory (its reset pin is not connected) while the ROM writes the flash. The RTC reset at the end of the flash clears the latch; `board_backlight_init()` releases it too. The host waits 3 s for the bootloader before its next trigger, so the animation stays well under that. <!-- src: /firmware/main/ui/flash_anim.c, /firmware/main/usb/usb_cdc.c#reboot_to_rom_download, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c#board_backlight_hold_on, /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->

An `lv_image` redrawn from a decoded frame at 12 fps hung the pad within 4–30 s in a test build (interrupt watchdog, no coredump), while the grade animations redrawn as often for 90 s did not; the cause was not found, which is why the flash animation bypasses LVGL. <!-- src: docs/history/2026-11-28-asset-demo-hang.md -->
