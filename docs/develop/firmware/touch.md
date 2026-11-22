---
title: Firmware Touchscreen Subsystem
description: Implementation reference for the CST816S I2C touch driver, touch task execution, gesture axis tracking, and swipe label rendering.
---

The touchscreen subsystem manages the Hynitron CST816S capacitive touch controller over I2C, implementing the pad's "third button" concept (Product Rule 4). It executes on Core 1 at low priority (`touch_retry_task`), keeping all touch processing off the keypress critical path. <!-- src: /firmware/main/input/touch_retry.c -->

## Hardware & Bus Configuration

- **Controller**: CST816S capacitive touch chip (`CST816_I2C_ADDR = 0x15`). <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_ADDR -->
- **I2C Bus**: Port `I2C_NUM_0` operating at 100 kHz clock frequency (`CST816_I2C_FREQ_HZ = 100000`). <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_FREQ_HZ -->
- **Pins**: SDA on GPIO 47, SCL on GPIO 48, active-low INT line on GPIO 46.
- **Auto-Sleep Disabled**: The driver disables chip auto-sleep (`CST816_REG_AUTO_SLEEP = 0xFE`, value `0xFF`), locking the controller in dynamic active mode to prevent multi-millisecond wake-up lag. <!-- src: /firmware/main/input/touch_retry.c#CST816_REG_AUTO_SLEEP -->

## Execution & Debounce Loop

The task switches dynamically between interrupt sleeping and polling: <!-- src: /firmware/main/input/touch_retry.c -->

1. **Idle State**: The task sleeps waiting for a FreeRTOS semaphore signaled by the GPIO 46 falling-edge interrupt. Zero CPU cycles or I2C bus transactions occur while the screen is untouched. <!-- src: /firmware/main/input/touch_retry.c -->
2. **Active Touch**: When INT fires, the task enters a 10 ms polling loop (`TOUCH_POLL_MS = 10`), reading touch point coordinates (`CST816_REG_TOUCH_NUM` and `CST816_REG_TOUCH_XH`). <!-- src: /firmware/main/input/touch_retry.c#TOUCH_POLL_MS -->
3. **Release Confirmation**: Requires two consecutive empty reads (~20 ms) before dropping the touch hold state (`TOUCH_RELEASE_READS = 2`). This protects against momentary bus glitches dropping a sustained Quick Retry hold. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_RELEASE_READS -->

## Coordinate Translation & Gesture Engine (`gesture.c`)

The touch sensor is physically mounted in portrait orientation (240x320), while the display UI runs in landscape orientation (320x240): <!-- src: /firmware/main/input/touch_retry.c -->
- Coordinates are translated with `TOUCH_SWAP_XY = 1` and `TOUCH_MIRROR_Y = 1` so physical screen coordinates match GUI render orientation. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_SWAP_XY -->
- **Gesture Tracker (`gesture_tracker_t`)**:
  - `gesture_begin()`: Anchors the initial contact origin point `(origin_x, origin_y)`. <!-- src: /firmware/main/input/gesture.c#gesture_begin -->
  - `gesture_feed()`: Analyzes movement. Requires displacement $> 24$ pixels (`GESTURE_SWIPE_PX = 24`). Disambiguates axis: vertical swipes require $2 \cdot |dy| \ge 3 \cdot |dx|$; horizontal swipes require $2 \cdot |dx| \ge 3 \cdot |dy|$. Ambiguous diagonals wait for further motion. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

## Swipe Labels & UI Feedback (`swipe_labels.c`)

When a swipe action executes outside of active gameplay, the UI displays a visual badge overlay showing the corresponding action icon and text label (`SWIPE_LABELS` table in `firmware/main/input/swipe_labels.c`): <!-- src: /firmware/main/input/swipe_labels.c#SWIPE_LABELS -->

- Shortcuts supported include Random Map (`F2`), Mods (`F1`), Options (`F3`), and Media Previous/Next track.
- During active beatmaps (`PLAYING`), all on-screen badges are suppressed to ensure zero display overhead and zero visual distractions.
