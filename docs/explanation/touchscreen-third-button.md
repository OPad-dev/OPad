---
title: Touchscreen as the Third Button
description: Explanation of the CST816S capacitive touch controller integration, resting finger Quick Retry, gesture classification, and gameplay isolation.
category: explanation
source_files:
  - /firmware/main/input/touch_retry.c
  - /firmware/main/input/gesture.c
  - /firmware/main/usb/usb_hid.c
  - /docs/specs/v2-hall-effect-decisions.md
---

On the Waveshare ESP32-S3-Touch-LCD-2, the capacitive touchscreen is engineered not merely as a graphical interface, but as **the third button** (Product Rule 4). It delivers clean digital presses, sustained held states, and reliable releases without burdening the CPU or degrading keyboard latency. <!-- src: /firmware/main/input/touch_retry.c -->

## Hardware & Interrupt Architecture

The touch subsystem is powered by a Hynitron CST816S capacitive touch controller communicating over I2C (`CST816_I2C_ADDR = 0x15` at 100 kHz) on GPIO 47 (SDA) and GPIO 48 (SCL), with an active-low interrupt line on GPIO 46. <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_ADDR -->

### Low-CPU Execution Model
- **Zero Polling in Standby**: When no finger touches the screen, the `touch_retry` task idles completely, sleeping on a FreeRTOS semaphore triggered by the hardware GPIO interrupt (`TOUCH_IDLE_CHECK_MS = 1000` fallback). No I2C bus traffic occurs while the pad is untouched. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_IDLE_CHECK_MS -->
- **Dynamic Polling During Contact**: Once a finger touches the surface, the CST816 pulls INT low, waking the task. The task enters a 10 ms polling loop (`TOUCH_POLL_MS = 10`) to track motion and contact duration. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_POLL_MS -->
- **Clean Release Debouncing**: To survive noisy I2C reads, release detection requires two consecutive empty reads (~20 ms) before dropping the press state (`TOUCH_RELEASE_READS = 2`). A single transient read failure will not interrupt a held touch. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_RELEASE_READS -->

## Context-Aware Touch Modes

The touch supervisor categorizes operation into three distinct modes: <!-- src: /firmware/main/input/touch_retry.c#touch_mode_t -->

| Mode | Trigger Condition | Touch Action | Swipe Action |
|---|---|---|---|
| **`TOUCH_MODE_PLAY`** | Host actively streaming beatmap (`PLAYING`). | Resting finger sends Quick Retry (`` ` `` / `~`). | Media gestures run without keyboard report alteration. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_PLAY --> |
| **`TOUCH_MODE_MENU`** | Outside beatmap or in osu! song selection. | Taps do not trigger Quick Retry. | Swipes control volume, track skip, or custom keys. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_MENU --> |
| **`TOUCH_MODE_RETRY`** | No host connected or tosu inactive. | Every touch acts as Quick Retry. | Swipes disabled. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_RETRY --> |

### The In-Game Resting Finger (Quick Retry)
In rhythm games, accidentally restarting a map due to an inadvertent swipe or graze is catastrophic. OPad implements a stillness window: <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US -->
- When a touch begins during a map, Quick Retry is not sent immediately. The pad observes finger travel for 60 ms (`PLAY_RETRY_STILL_US = 60,000`).
- If the finger moves less than 12 pixels (`PLAY_RETRY_STILL_PX = 12`) during this interval, it is classified as a deliberate resting press, and HID keycode `0x35` (`` ` ``) is asserted.
- Because osu!'s native Quick Retry action requires a sustained hold rather than an instantaneous tap, this 60 ms stillness filter introduces zero perceptible delay to the player while completely eliminating accidental retries during swipe motions.

## Swipe Classification & Media Gestures

Motion tracking is handled by the gesture state machine (`firmware/main/input/gesture.c`): <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

- **Axis Disambiguation**: A gesture begins at `(origin_x, origin_y)`. Movement must exceed `GESTURE_SWIPE_PX` (24 pixels) with one axis distinctly dominating ($2 \cdot |dy| \ge 3 \cdot |dx|$ for vertical, or $2 \cdot |dx| \ge 3 \cdot |dy|$ for horizontal). Diagonal swipes wait for clearer movement. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->
- **Default Bindings**:
  - **Swipe Up / Down**: Master volume adjustment. When osu! is focused on the desktop host, `opad-daemon` steps osu!'s in-game volume; otherwise, it steps system audio.
  - **Swipe Left / Right**: Media track navigation (Previous Track / Next Track).
- **Windows Alt Protection**: When sending volume or media key combinations involving modifier keys, Windows will interpret an Alt press-and-release without an intermediate key as a command to open the application window menu (`SC_KEYMENU`), which steals focus from the game. To prevent focus loss, the pad taps `HID_KEY_F24` before releasing Alt (`TOUCH_KEY_TAP_MS = 15`). <!-- src: /firmware/main/input/touch_retry.c#HID_KEY_F24 -->

## Gameplay Isolation Invariant

In accordance with Product Rule 4:
1. **Never Alt mid-map**: Swipes executed mid-game never insert Alt or modifier keys into the keyboard report.
2. **Clean Screen**: Nothing extra is rendered on the LCD screen during active gameplay. Swipe feedback badges and volume overlays are suppressed until the beatmap completes.
3. **Uncalibrated Hall Effect Pads**: If an HE pad is uncalibrated, it is locked to its calibration screen (`docs/specs/v2-hall-effect-decisions.md` D7). In this state, swipes are disabled, no keyboard keys are transmitted, and touch events are restricted strictly to the calibration UI buttons. <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
