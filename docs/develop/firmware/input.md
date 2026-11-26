---
title: Firmware Input Subsystem
description: Implementation reference for the firmware input pipeline, eager mechanical debouncing, Key 3, Hall Effect V1/V3 sampling and Rapid Trigger, the calibration lock, module monitoring, and unified key event dispatch.
sidebar:
  order: 2
---

The input subsystem reads mechanical switches or Hall Effect sensors, decides presses (debounce or Rapid Trigger), counts them, and hands each change to the USB HID layer. Every task on this path runs on **core 0** at `configMAX_PRIORITIES - 1`, from IRAM ([overview](overview.md#the-keypress-path)). Source: [/firmware/main/input/](https://github.com/OPad-dev/OPad/tree/main/firmware/main/input).

> [!IMPORTANT]
> Anything changed under `firmware/main/input/` must say how it affects key latency and how that was checked ([/AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1), and needs a run on a real pad ([Testing Checklist](../../testing-checklist.md) §1).

## Which Key Source Runs

`app_main` picks the key source once, at boot, from the module ID (`board_detect_module()` in [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c), thresholds in [/firmware/main/input/module_id.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_id.h)):

| Module (`board_module_type_t`) | ID voltage | Keys come from |
|---|---|---|
| `BOARD_MODULE_NONE` (hand-wired) | ~0 V | `keypad.c` on the configured pins (defaults GPIO14 / GPIO9) |
| `BOARD_MODULE_MX` (MX V1) | ~0.30 V | `keypad.c`; default pins are moved to GPIO10 / GPIO7 (`BOARD_MX_KEY1_GPIO` / `BOARD_MX_KEY2_GPIO`) |
| `BOARD_MODULE_MX_V3` | ~2.24 V | `keypad.c` on GPIO10 / GPIO7, plus Key 3 on GPIO6 (`keypad_set_key3_gpio(BOARD_MX3_KEY3_GPIO)`); keys off on a turned-over cable |
| `BOARD_MODULE_HE` (HE V1) | ~1.06 V | `he_backend.c` with `HE_SOURCE_ESP_ADC`, only when built with `CONFIG_OPAD_HE_BACKEND`; otherwise keys off with an LCD notice |
| `BOARD_MODULE_HE_V3` | ~1.65 V | `he_backend.c` with `HE_SOURCE_ADS7028` (always built in); the key pins are the module's SPI lines |

For HE and unsupported modules the keypad is disabled before its ISR is armed (`keypad_set_input_enabled(false)`) and the pins parked analog or detached, so analog sensor outputs can never type.

## Mechanical Keypad (`keypad.c`, `debounce.c`)

[/firmware/main/input/keypad.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/keypad.c), [/firmware/main/input/debounce.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.c):

- **ISR (`gpio_isr_handler`, `IRAM_ATTR`)**: on any edge of a key pin, takes `esp_timer_get_time()`, reads the pin and calls `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)` under `s_keypad_spinlock`. An accepted press calls `key_event_press_accepted()` right there; any accepted change wakes `keypad_task` with `vTaskNotifyGiveFromISR`. The ISR does not submit HID reports.
- **Eager debounce (`debounce_step`, pure C, host-tested)**:
  - `DEBOUNCE_SOURCE_EDGE`: a level different from `accepted_pressed` outside the lockout is accepted at once (`DEBOUNCE_ACTION_PRESS` / `DEBOUNCE_ACTION_RELEASE`) and starts a lockout of `lockout_us`; edges inside it are ignored. Range `DEBOUNCE_MIN_US = 500` to `DEBOUNCE_MAX_US = 20000`, default `DEBOUNCE_DEFAULT_US = 5000` ([/firmware/main/input/debounce.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.h)).
  - `DEBOUNCE_SOURCE_RESAMPLE`: once the lockout ends, `keypad_task` reads the pin once; if it settled opposite to `accepted_pressed`, that transition is applied (and counted, if a press) with a fresh lockout.
- **`keypad_task` loop**: sleeps until notified or until the nearest lockout ends, then (1) resamples expired lockouts, (2) calls `key_event_report()` for every key whose state differs from what was reported, (3) applies a staged config, (4) steps pin detection, (5) runs a requested line test.
- **Staged config (`keypad_set_config`)**: keycode and pin changes wait until every key is released, so a held key never changes keycode and only `keypad_task` touches key GPIOs. A failed pin move keeps the previous pins armed and records `DIAG_EVENT_CONFIG_REJECTED`.
- **Pin detection (`detect_scan_step`)**: pull-ups on every allowed free pin (`KEY_GPIO_ALLOWED` in [/firmware/main/config/config_validate.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/config_validate.c)); a pin held low for `DETECT_CONFIRM_US = 15000` µs wins. Polled every 10 ms between key events, so keys keep working.
- **Line test (`keypad_line_test`)**: swaps each key pin's pull-up for a pull-down for `LINE_TEST_SETTLE_US = 300` µs to see an external pull-up or a short, then restarts debouncing from the pins. Used by the [board test](diag.md#carrier--input-module-pcb-self-test-board_testc).

## Hall Effect Input (`he_backend.c`)

[/firmware/main/input/he_backend.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_backend.c) owns both HE sources and feeds every frame through one step, `process_frame()`: median per key, health bookkeeping, `he_rt_step()` per key, and `key_event_press_accepted()` on a down event. Design record: [/docs/specs/v2-hall-effect-decisions.md](../../specs/v2-hall-effect-decisions.md); concepts: [Hall Effect and Rapid Trigger](../../explanation/hall-effect.md).

| | V1 (`HE_SOURCE_ESP_ADC`) | V3 (`HE_SOURCE_ADS7028`) |
|---|---|---|
| Sampling | ADC1 continuous DMA, `ADC_CHANNEL_9` (Key 1, GPIO10) and `ADC_CHANNEL_6` (Key 2, GPIO7) alternating at `HE_SAMPLE_HZ = 64000` | GPTimer wakes `he_spi` every `HE_SPI_FRAME_US = 50` µs (20 kHz); ADS7028 auto-sequence over AIN5 / AIN4 / AIN3 = Keys 1 / 2 / 3 |
| Frame | `HE_FRAME_CONVS = 16` (8 per key), one frame per 250 µs; median of the valid readings (`he_frame.c`) | 4 conversions read at 40 MHz on SPI3 (no DMA; after the first read, straight on the registers via `ads7028_read16_again`), the first dropped as stale; each CRC-checked (`ads7028.c`) and matched to its key by channel ID (`he_spi_frame.c`) |
| Who reports | IRAM ADC ISR `on_frame` steps the engine and notifies `he_task` only on a key change; `he_task` calls `publish()` | The sampler task calls `publish()` itself (`on_spi_frame`), avoiding a hand-over that cost ~100 µs of tail |
| Keys | 2 | 3 |
| Build | `CONFIG_OPAD_HE_BACKEND` | always |

- **`publish()`** (IRAM, under `s_publish_mutex`) calls `key_event_report()` once per changed key, in order, from whichever task publishes.
- **Health (`he_health.c`, pure C)**: `he_task` checks every 2 ms (`HE_CHECK_TICKS`). A key with no valid reading for `HE_STALL_US = 5000` µs is released and ignored; no frames at all for that long releases every key and restarts the source with back-off from 10 ms to 1 s (`HE_RESTART_BACKOFF_MIN_US` / `_MAX_US`). Both keys absent for `HE_ABSENT_US = 100 ms` means the module was removed. These values are marked provisional (OPEN G) in the source.
- **Rapid Trigger engine (`he_rt.c`, pure C, host-tested)**: states `HE_RT_ARMED`, `HE_RT_PRESSED`, `HE_RT_RT_RELEASED`. Plain mode presses at `actuation` and releases at `release`; Rapid Trigger releases after `rt_release` counts up from the peak and re-presses after `rt_press` down from the trough while deeper than `release` (or `top_reset` in continuous mode). `need_release` makes a key held at boot, after a parameter change or after a fault (`he_rt_fault`) come up before it can press ([/firmware/main/input/he_rt.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_rt.h)).
- **Thresholds (`he_params.c`)**: settings are 0.1 % steps of the calibrated span (`HE_PCT_FULL = 1000`), turned into raw counts off the key path and floored at a noise limit (`HE_PROVISIONAL_LIMITS`).

### Calibration and the lock (decision D7)

An uncalibrated HE pad is locked: `he_backend_reload_params()` gives every key `HE_PARAMS_NEVER` (thresholds that never press) while `he_cal_is_locked_decision()` is true or calibration runs, the screen shows the lock overlay ([UI](ui.md#hall-effect-calibration-overlay-he_cal_uic)), and touch only drives that screen's button ([touch](touch.md#calibration-gating)). No key is sent while locked.

| File | Role |
|---|---|
| [/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c), [he_cal_record.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_record.c) | A/B calibration records in NVS ([config and NVS](config-and-nvs.md#hall-effect-calibration-records)) |
| [/firmware/main/input/he_cal_runner.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_runner.c) | Stages `REST`, `KEY1`, `KEY2`, `KEY3` (V3), `SUCCESS` / `FAILED`; 5 full presses per key |
| [/firmware/main/input/he_cal_capture.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_capture.c) | Rest window (spread ≤ 36, mean 1500..2500), median of 5 peaks, `rest_pressed_margin = 72` |
| [/firmware/main/input/he_cal_button.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_button.c) | On-screen button hit test: down and up inside, excursion < `GESTURE_SWIPE_PX` |
| [/firmware/main/input/he_cal_barrier.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_barrier.h), [/firmware/main/usb/usb_report_barrier.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_report_barrier.c) | `he_backend_start_calibration()` suppresses the keys, waits (≤ 100 ms each) for the reporter to publish the releases and for TinyUSB callbacks to drain, and only then starts capture; a failure restores the old thresholds |
| [/firmware/main/input/he_cal_touch_guard.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_touch_guard.h) | Refuses a start while a touch is still in flight ("Release the touchscreen first") |

Calibration samples arrive at 4 kHz on both sources: V3 passes one frame in `HE_CAL_EVERY_V3 = 5`.

## Unified Key Events (`key_event.c`)

Every source converges on [/firmware/main/input/key_event.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/key_event.c):

- **`key_event_press_accepted()` (`IRAM_ATTR`)**: relaxed atomic increments of `s_lifetime_presses[k]` (64-bit) and `s_map_presses[k]` (32-bit, zeroed by the host per attempt), stores `s_last_press_us[k]`, and for a game key (`s_game_keys` mask, [/firmware/main/input/game_keys.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/game_keys.h)) pushes the timestamp to the press log.
- **`key_event_report()` (`IRAM_ATTR`)**: calls `usb_hid_handle_key_event()` **first** ([USB](usb.md#split-endpoint-hid-submission-usb_hidc)); then `latency_stats_record()` on success or `latency_stats_record_deferred()` when the endpoint was busy, toggles the bench debug GPIO if enabled, and on a press calls `ui_notify_activity()` (one atomic store).
- **Press log ([/firmware/main/input/press_log.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/press_log.c))**: lock-free single-producer ring of `PRESS_LOG_CAPACITY = 128` key-downs with pad timestamps, drained by the protocol task for the host's tap rate; a full log drops the newest press and counts it.
- **Latency stats ([/firmware/main/input/latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c))**: lock-free edge-to-submit histogram (p50/p99/p99.9/max, deferred count), what `opadctl latency` shows.

## Module Monitor (`module_monitor.c`, `module_watch.c`)

A module swapped while powered must not let analog outputs type. On pads that are not MX, `module_mon` (core 1, `tskIDLE_PRIORITY + 1`) checks the ID line every 5 ms (`board_module_present_fast`) and turns the keys off after `MODULE_WATCH_FAST_READS = 3` disagreeing reads in a row; they come back only after `MODULE_WATCH_RESTORE_READS = 3` full readings of the boot module. A different module needs a restart. With HE input running, ADC1 belongs to the backend, so the ID is read only by pausing it (`he_backend_probe_module`) while both keys are absent ([/firmware/main/input/module_monitor.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_monitor.c), [/firmware/main/input/module_watch.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_watch.h)).

## Debug Firmware Hooks

`CONFIG_OPAD_HE_DEBUG` builds log every HE press with the readings before it (`he_debug.c`) or, on MX, every tap and every pin change the pad did not send (`mx_debug.c`). Outside that build every hook is a no-op ([/firmware/main/input/he_debug.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_debug.h), [/firmware/main/input/mx_debug.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/mx_debug.h)).
