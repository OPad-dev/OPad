---
title: Firmware Touchscreen Subsystem
description: Implementation reference for the CST816S I2C touch driver, Core 1 touch_retry task, gesture classifier, and swipe feedback labels.
sidebar:
  order: 4
---

The touchscreen subsystem drives the Hynitron CST816S capacitive touch controller over I2C to implement OPad's third button ([/AGENTS.md](/AGENTS.md) §1.4). All touch code runs on **Core 1** at priority `2` in `touch_retry_task` ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).

## Hardware & I2C Configuration

- **Controller**: CST816S at `CST816_I2C_ADDR = 0x15` on `I2C_NUM_0`, `CST816_I2C_FREQ_HZ = 100000` (100 kHz) ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).
- **Pins** ([/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h)):
  - `SDA`: **`GPIO 48`** (`BOARD_I2C_SDA_GPIO`)
  - `SCL`: **`GPIO 47`** (`BOARD_I2C_SCL_GPIO`)
  - `INT`: **`GPIO 46`** (`BOARD_TOUCH_INT_GPIO`, active-low falling edge)
- **Auto-sleep disabled**: Writes `0xFF` to `CST816_REG_AUTO_SLEEP` (`0xFE`) so the controller never adds multi-millisecond wake-up delay.

## Interrupt-Idle / Poll-Active Loop

1. **Idle**: `touch_retry_task` blocks on a FreeRTOS semaphore signaled by the `GPIO 46` falling-edge ISR—zero CPU or I2C traffic while untouched.
2. **Active finger**: Polls every `TOUCH_POLL_MS = 10` ms reading `CST816_REG_TOUCH_NUM` and coordinates (`CST816_REG_TOUCH_XH`).
3. **Release debounce**: Requires `TOUCH_RELEASE_READS = 2` consecutive empty reads (~20 ms) before releasing so one noisy I2C read never drops a Quick Retry hold.
4. **Calibration reservation**: While an uncalibrated Hall Effect pad is locked (D7) or calibrating, `touch_retry.c` forwards touch samples exclusively to `he_cal_ui_touch_sample` and sends no HID keys or swipes.

## Coordinate Mapping & Gesture Classification (`gesture.c`)

The CST816S sensor is mounted in portrait (`240x320`) while the UI runs in landscape (`320x240`), so coordinates are mapped via `TOUCH_SWAP_XY = 1` and `TOUCH_MIRROR_Y = 1` ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).

In [/firmware/main/input/gesture.h](/firmware/main/input/gesture.h) and [/firmware/main/input/gesture.c](/firmware/main/input/gesture.c):
- `GESTURE_SWIPE_PX = 30` px threshold from contact origin (`gesture_begin`) before a movement classifies as a swipe (`gesture_feed`).
- **3:2 axis dominance**: vertical swipe requires `2 * |dy| >= 3 * |dx|`; horizontal swipe requires `2 * |dx| >= 3 * |dy|`; diagonals wait for further motion.
- **In-map behavior (`PLAYING`)**: Holds Quick Retry for up to `PLAY_SWIPE_WINDOW_US = 250,000` µs (250 ms) to distinguish a resting finger from an intentional swipe. If the finger releases before 250 ms without swiping, `tap_retry()` sends a short Quick Retry tap; if held past 250 ms, Quick Retry stays held until finger-up. Swipes during a map cancel Quick Retry and never inject Alt or keyboard keys.
- **Volume & media swipes**: Sent directly by the pad over HID (`usb_media.c` for consumer media/volume and mouse wheel; `usb_hid.c` for Alt+wheel osu! volume outside maps, with `release_alt()` waiting up to `TOUCH_KEY_WAIT_MS = 30` ms for the F24 press/release to clear Windows' Alt menu bar).

## Swipe Feedback Labels (`swipe_labels.c`)

Outside gameplay, [/firmware/main/input/swipe_labels.c](/firmware/main/input/swipe_labels.c) maps swipe actions and osu! shortcuts (e.g., `F1` Mods, `F2` Random, `F3` Options, volume, media controls) to short on-screen overlay labels (`SWIPE_LABELS`). During `PLAYING`, no badges are drawn.
