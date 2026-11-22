---
title: Firmware Input Subsystem
description: Implementation reference for the firmware input pipeline, eager mechanical debouncing, Hall Effect V1/V3 sampling, and unified key event dispatching.
---

The input subsystem reads physical mechanical switches and Hall Effect sensors, runs debouncing or Rapid Trigger, updates monotonic press counters, and submits USB HID reports. All key-path tasks run on **Core 0** at `configMAX_PRIORITIES - 1`.

## Mechanical Keypad Pipeline (`keypad.c` & `debounce.c`)

Implemented in [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c) and [/firmware/main/input/debounce.c](/firmware/main/input/debounce.c):

- **GPIO ISR (`gpio_isr_handler`, `IRAM_ATTR`)**: Triggered on configured switch edges (defaults `GPIO 14` for Key 1, `GPIO 9` for Key 2; V3 MX carrier uses `GPIO 10`, `GPIO 7`, `GPIO 6`). Timestamps the edge with `esp_timer_get_time()`, calls `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)`, increments accepted press counters via `key_event_press_accepted`, and wakes `keypad_task` with `vTaskNotifyGiveFromISR`.
- **Eager Debounce (`debounce_step`)**:
  - `DEBOUNCE_SOURCE_EDGE`: If `level != state->accepted_pressed` and `now_us >= state->lockout_end_us`, accepts the transition immediately (`DEBOUNCE_ACTION_PRESS` or `DEBOUNCE_ACTION_RELEASE`) and sets `lockout_end_us = now_us + lockout_us` (`DEBOUNCE_DEFAULT_US = 5000`, range `500`–`20000` µs in [/firmware/main/input/debounce.h](/firmware/main/input/debounce.h)).
  - `DEBOUNCE_SOURCE_RESAMPLE`: Run by `keypad_task` when `lockout_end_us` expires; reads the GPIO once and corrects `accepted_pressed` (starting a fresh lockout) if the pin settled in the opposite state.
- **Deferred config staging**: `keypad_set_config` stages pin/keycode changes and applies them in `keypad_task` only after keys are released so keycodes never change mid-hold and only `keypad_task` touches key GPIOs.
- **Pin detection & line test**: `keypad_task` also serves interactive pin detection (`detect_scan_step`, `DETECT_CONFIRM_US = 15000` µs) and `keypad_line_test` (pull-up/pull-down check for `board_test.c`).

## Hall Effect Pipeline (`he_backend.c`, `he_spi_source.c`, `he_rt.c`)

Implemented across [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c), and [/firmware/main/input/he_rt.c](/firmware/main/input/he_rt.c):

- **V1 Internal ADC (`HE_SOURCE_INTERNAL_ADC`)**: Continuous ADC1 DMA alternating `ADC_CHANNEL_9` (Key 1) and `ADC_CHANNEL_6` (Key 2) at `HE_SAMPLE_HZ = 64000` Hz (`HE_FRAME_CONVS = 16`, 250 µs / 4 kHz frames). The IRAM ISR `on_frame` computes each channel's median, steps `he_rt_step`, and notifies `he_task` only on key state edges.
- **V3 External ADS7028 (`HE_SOURCE_SPI_ADC`)**: A hardware `gptimer` wakes Core 0 task `he_spi` every `HE_SPI_FRAME_US = 50` µs (20 kHz). `he_spi` reads 4 CRC-protected 32-bit words over 40 MHz SPI3 (`SPI_DMA_DISABLED`, direct `GPSPI3` register loop `ads7028_read16_again` in [/firmware/main/input/ads7028.c](/firmware/main/input/ads7028.c)) for `AIN3`/`AIN4`/`AIN5` (Keys 3/2/1), runs `he_rt_step`, and calls `key_event_report` directly. Calibration capture is decimated 5:1 (`SPI_CAL_DECIMATE = 5`) to match the 4 kHz runner.
- **3-State Rapid Trigger (`he_rt_step`)**: Transitions between `HE_RT_ARMED`, `HE_RT_PRESSED`, and `HE_RT_RT_RELEASED`, enforcing `need_release` after boot, parameter changes, or ADC stalls (`he_rt_fault`).

## Unified Key Event Dispatch (`key_event.c`)

Both mechanical and Hall Effect backends converge on [/firmware/main/input/key_event.c](/firmware/main/input/key_event.c):

- **`key_event_press_accepted` (`IRAM_ATTR`)**: Atomically increments `s_lifetime_presses[key_index]` and `s_map_presses[key_index]`, records `s_last_press_us`, and pushes game-key presses (filtered by `s_game_keys`) to `press_log_push`.
- **`key_event_report` (`IRAM_ATTR`)**: Calls `usb_hid_handle_key_event(key_index, pressed, edge_us)` in [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c) first; records delivery latency via `latency_stats_record` (or `latency_stats_record_deferred` when the endpoint is busy), toggles `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM` if enabled, and notifies UI activity (`ui_notify_activity`) on press.
