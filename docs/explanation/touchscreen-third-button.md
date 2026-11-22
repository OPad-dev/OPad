---
title: Touchscreen as the Third Button
description: Explanation of the CST816S capacitive touch controller integration, resting finger Quick Retry, gesture classification, and gameplay isolation.
sidebar:
  order: 3
---

On the Waveshare ESP32-S3-Touch-LCD-2, the capacitive touchscreen operates as **the third button** (AGENTS.md §1.4). It delivers a clean press, a sustained held state while the finger stays down, and a clean release, while keeping CPU and I2C overhead off the keypress path ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).

## Hardware & Interrupt Architecture

The touch subsystem uses a CST816 capacitive touch controller over I2C (`CST816_I2C_ADDR = 0x15` at 100 kHz on `I2C_NUM_0`) wired to **GPIO 48 (SDA)** and **GPIO 47 (SCL)**, with an active-low interrupt line on **GPIO 46** ([`board_pins.h`](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h), [`touch_retry.c`](/firmware/main/input/touch_retry.c)). The task runs on Core 1 so it never contends with Core 0 key input or TinyUSB.

Panel coordinates (portrait `240x320`) are transformed with `TOUCH_SWAP_XY = 1` and `TOUCH_MIRROR_Y = 1` into landscape `320x240` screen coordinates so directions match how the pad is held ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).

### Low-CPU Execution Model

- **Idle on Interrupt**: When no finger is down, `touch_retry_task` blocks on `ulTaskNotifyTake()` waiting for the GPIO 46 falling-edge ISR (`touch_int_isr`), with a `TOUCH_IDLE_CHECK_MS = 1000` ms safety fallback (falling back to 100 Hz polling only if ISR setup fails). No I2C reads happen while idle ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).
- **100 Hz Polling While Down**: Once a touch starts, the task polls every `TOUCH_POLL_MS = 10` ms (100 Hz) to track finger movement and detect release, because the CST816 INT pin can pulse per report rather than stay low.
- **Release Debouncing**: Releasing requires `TOUCH_RELEASE_READS = 2` consecutive empty reads (~20 ms) before dropping the held state, so one noisy read does not break a hold.

## Context-Aware Touch Modes

At touch-down, [`touch_mode_now()`](/firmware/main/input/touch_retry.c) selects one of three modes:

| Mode | Condition | Touch Action | Swipe Action |
|---|---|---|---|
| **`TOUCH_MODE_PLAY`** | Host sent `HostStatus` with `tosu_connected = true` within `HOST_STATUS_TIMEOUT_US` (3 s) and device state is `PLAYING`. | Resting finger (or short tap without swiping) sends Quick Retry (`` ` ``). | Media/volume swipes run without adding Alt or other keyboard keys; keyboard-key swipes are suppressed. |
| **`TOUCH_MODE_MENU`** | Host sent `HostStatus` with `tosu_connected = true` within 3 s and state is not `PLAYING`. | Taps and holds send nothing. | Swipes run configured volume, media, osu! shortcut, or free-key actions. |
| **`TOUCH_MODE_RETRY`** | No host connected, `tosu` disconnected, or no `HostStatus` for > 3 s. | Every touch immediately presses and holds Quick Retry (`` ` ``). | Swipes disabled. |

### The In-Game Resting Finger (Quick Retry)

In `TOUCH_MODE_PLAY`, Quick Retry is not asserted immediately on touch-down so swipes never accidentally restart a map ([`touch_retry_task()`](/firmware/main/input/touch_retry.c)):

- If the finger moves less than `PLAY_RETRY_STILL_PX = 12` pixels (Chebyshev distance, [`gesture_distance()`](/firmware/main/input/gesture.c)) for `PLAY_RETRY_STILL_US = 60,000` µs (60 ms), it is classified as a resting press and `usb_hid_set_touch_retry(true)` asserts Quick Retry (`` ` ``).
- If the finger stays down for `PLAY_SWIPE_WINDOW_US = 250,000` µs (250 ms) without becoming a swipe, it also asserts Quick Retry.
- If the finger lifts before 60 ms without swiping, [`tap_retry()`](/firmware/main/input/touch_retry.c) sends a press-and-release of Quick Retry.
- Once a swipe fires, `retry_pending` is cleared so that touch never sends Quick Retry.

## Swipe Classification & Media Gestures

Motion tracking lives in [`firmware/main/input/gesture.c`](/firmware/main/input/gesture.c) and [`gesture.h`](/firmware/main/input/gesture.h):

- **Threshold & Axis Locking**: Starting from `(origin_x, origin_y)`, a finger must move at least `GESTURE_SWIPE_PX = 30` pixels along one axis with a 1.5:1 ratio (`2 * |dy| >= 3 * |dx|` for vertical, `2 * |dx| >= 3 * |dy|` for horizontal) before locking onto that axis. Diagonal motion waits for clearer movement.
- **Vertical Repeat vs. Horizontal Single-Fire**: Vertical drags step once per `30` pixels (`origin_y` advances by `±30`), allowing long drags (and direction reversals) to step volume continuously; horizontal swipes fire once per touch (`horizontal_fired`). Non-volume actions only fire on the first step (`!repeat`).
- **Default Bindings** ([`touch_retry.c`](/firmware/main/input/touch_retry.c), [`opad-model/src/lib.rs`](/desktop/crates/opad-model/src/lib.rs)):
  - **Up / Down**: `Volume up` / `Volume down`. When the host sets `HostStatus.osu_active` (osu! is the foreground window on Windows, or osu!lazer is running on Linux), the pad itself steps osu!'s volume via HID mouse wheel (`usb_media_wheel`): plain wheel during a map (`playing == true`), and Left Alt + wheel in menus (`playing == false`). When `osu_active` is false, it sends USB HID Consumer Control Volume Up / Down (`usb_media_consumer_tap`).
  - **Left / Right**: `Previous track` / `Next track` via USB HID Consumer Control.
- **Windows Alt Menu Protection**: In menus, [`step_volume()`](/firmware/main/input/touch_retry.c) holds Left Alt (`HID_MOD_LEFT_ALT = 0x04`) across all wheel notches of a drag. Before releasing Alt on finger lift, [`release_alt()`](/firmware/main/input/touch_retry.c) taps `HID_KEY_F24` (`0x73`) and waits for HID delivery (`TOUCH_KEY_WAIT_MS = 30` ms) so Windows does not interpret a bare Alt press-and-release as opening the window menu (`SC_KEYMENU`).

## Gameplay Isolation Invariant

In accordance with AGENTS.md §1.4:

1. **Never Alt or Extra Keys Mid-Map**: During `TOUCH_MODE_PLAY`, [`step_volume()`](/firmware/main/input/touch_retry.c) uses the plain mouse wheel without Left Alt, and [`run_swipe_action()`](/firmware/main/input/touch_retry.c) ignores `SWIPE_ACTION_KEY` so the keyboard report carries only the switch keys and Quick Retry.
2. **Clean Screen During Play**: [`handle_swipe()`](/firmware/main/input/touch_retry.c) only calls `ui_show_swipe()` when `!playing`; nothing extra is drawn on the LCD during a map.
3. **Uncalibrated Hall Effect Pads**: When `he_cal_suppress_input(he_backend_is_locked(), he_backend_is_calibrating())` is true (decision record D7 in [`v2-hall-effect-decisions.md`](../specs/v2-hall-effect-decisions.md)), [`touch_retry_task()`](/firmware/main/input/touch_retry.c) routes touches exclusively to the calibration UI (`he_cal_ui_touch_sample`), disables swipes, and sends no keyboard or media reports.
