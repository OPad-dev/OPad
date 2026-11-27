---
title: Calibrating Hall Effect Switches
description: How to calibrate magnetic switches on OPad (on the pad or in opad-gui) and tune actuation and rapid trigger settings.
sidebar:
  order: 4
---

A Hall Effect switch has a magnet in its stem, and a sensor under it measures how far the key is pressed. Calibration records each key's reading at rest and at a full press, so the pad knows each key's travel. Until a Hall Effect pad is calibrated it is locked to its calibration screen: it sends no keys, swipes are off, and a touch only presses that screen's button ([decision D7](../specs/v2-hall-effect-decisions.md)). <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## What you need

- An OPad with a Hall Effect module installed (V1 2-key or V3 3-key). <!-- src: /firmware/main/input/he_backend.c -->
- A flat, steady desk so the switches do not move during rest capture. <!-- src: /firmware/main/input/he_cal_runner.c -->
- Optionally, the OPad desktop app, for guided calibration and for tuning actuation and rapid trigger. Calibration also works on the pad alone, without the app. <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Calibrating with the OPad app (recommended)

When an uncalibrated Hall Effect pad connects to a PC running `opad-daemon`, the pad screen shows **Keys not calibrated** / **Calibrate in the OPad app** / **on your PC**, and the daemon can open `opad-gui` on **Keys**, with the calibration (**Calibrate the keys**) open. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen --> <!-- src: /desktop/gui/src/pages.rs#calibration -->

To recalibrate an already-calibrated pad, open `opad-gui` and click **Keys** -> **Recalibrate**. Calibration only starts while no map is playing and no finger is touching the pad screen. <!-- src: /desktop/gui/src/keys.rs#key_feel --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_start_calibration -->

1. On **Calibrate the keys** in `opad-gui`, take your fingers off all keys and click **Start**. <!-- src: /desktop/gui/src/pages.rs#calibration -->
2. **Step 1: keys at rest**: Keep your fingers off every key and do not touch the pad. The pad lets the readings settle for 16 ms, then averages 16 ms of readings (64 samples at 4 kHz) to record each key's resting level and check that it is steady and in range. <!-- src: /firmware/main/input/he_cal_runner.c#REST_SETTLE_SAMPLES --> <!-- src: /firmware/main/input/he_cal_capture.h#he_cal_capture_cfg_t -->
3. **Step 2: press Key 1 5 times**: Press Key 1 firmly all the way to the bottom and let it come fully back up, 5 times slowly. Each completed press fills one of the 5 numbered boxes on both `opad-gui` and the pad display. There is no time limit: take your time. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
4. **Step 3: press Key 2 5 times**: Repeat 5 full, consistent presses and full releases on Key 2. <!-- src: /desktop/gui/src/pages.rs#calibration -->
5. **Step 4: press Key 3 5 times** (V3 3-key Hall Effect module only): Repeat 5 full presses and releases on Key 3. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
6. Once the last key's 5th press completes, the pad validates the span and peak consistency (using the median of the 5 peaks as the full-press reading) and writes the calibration record to NVS (`opad_cal` on V1, `opad_cal3` on V3). The pad shows **Done!** / **Your keys are ready** for 1 second and the keys work at once. <!-- src: /firmware/main/input/he_cal_capture.h --> <!-- src: /firmware/main/input/he_cal.c --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_timer_cb -->

Click **Cancel** in `opad-gui` at any time during capture to abort; a recalibration restores the previous saved calibration, while an uncalibrated pad stays locked. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_cancel_calibration -->

## Calibrating directly on the pad (no app)

If no capable OPad app is connected, the pad waits 5 seconds after boot (**Looking for the OPad app...**), then displays **For the best setup, get the app:** along with a **Calibrate here** button on the touchscreen. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->

1. Tap **Calibrate here** on the touchscreen and immediately take your hands off the keys. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->
2. When **Calibrating** / **Take your fingers off both keys** (or **Take your fingers off all keys** on V3) finishes, follow the on-screen prompts: **Press key 1 all the way, 5 times** (**Let it come fully up each time**), then Key 2 (and Key 3 on V3). Five boxes (`1`–`5`) turn green as each press is accepted. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
3. When all keys pass validation, the pad shows **Done!** / **Your keys are ready** for 1 second and switches to normal operation. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->

## Tuning actuation and rapid trigger in opad-gui

After calibration, open `opad-gui` and click **Keys** in the sidebar. Every depth and sensitivity slider is a percentage of that key's calibrated travel, not millimetres, and moves in `0.5 %` steps. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#setting -->

1. **Link or split keys**: By default, edits apply to all keys together (**Both keys** or **All keys**). Toggle **Set keys separately** at the top to configure Key 1, Key 2 (and Key 3 on V3) independently. Switching it off copies Key 1's settings to the other keys. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#edit_settings -->
2. **Basic controls**:
   - **ACTUATION POINT**: How deep the key must move before pressing (up to `95.0 %`; the lower bound is raised automatically to clear the sensor noise floor unless overridden in **Advanced**). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Rapid trigger**: Toggle on to re-press as soon as the key moves back down and release as soon as it moves up. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **SENSITIVITY** (shown when **Rapid trigger** is on and **Advanced** is hidden): Sets both press and release movement distance together (`1.0 %`–`50.0 %`). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Continuous** (shown when **Rapid trigger** is on): Keeps rapid trigger active all the way up until the key rises above the top reset point. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
3. **Advanced controls**: Click **Advanced** (toggles to **Hide advanced**) to expose: <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **PRESS SENSITIVITY** (`1.0 %`–`50.0 %`) and **RELEASE SENSITIVITY** (`1.0 %`–`50.0 %`) separately when **Rapid trigger** is on.
   - **RELEASE POINT** (`1.0 %`–`50.0 %`): Hysteresis above the actuation point used when **Rapid trigger** is off.
   - **TOP RESET** (`1.0 %`–`50.0 %`, shown when **Continuous** is on): Depth above which continuous rapid trigger resets.
   - **BOTTOM DEAD ZONE** (`0.0 %`–`30.0 %`, `0 = off`): Ignores extra movement close to full bottom-out so pressing harder into the housing does not shift the peak.
   - **Allow below the noise floor** (when supported by firmware): Uses settings down to `1.0 %` without raising them to the sensor noise floor; sensor noise may then trigger accidental presses.
4. **Live switch view**: Press each key and watch the live cut-away switch and travel gauge (`rest`, `press`, `release`/`reset`, `dead zone`, `full`). `opad-gui` polls `GetHeLive` at ~50 Hz and smooths the drawn stem for readability, snapping directly to the real depth on press/release; the pad itself decides presses on raw readings. The live view pauses while playing a map. <!-- src: /desktop/gui/src/keys.rs#live_view --> <!-- src: /desktop/gui/src/keys.rs#take_sample -->
5. Click **Save to pad** to write the settings to the pad (or **Revert** to discard unsaved edits). <!-- src: /desktop/gui/src/keys.rs#view -->

## How to check

- On the pad, the red **Keys not calibrated** lock screen is gone and normal key presses type your configured keys (`Z`, `X`, and `C` on 3-key V3). <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /firmware/main/usb/usb_hid.c -->
- In `opad-gui` **Keys**, pressing each switch smoothly moves the cut-away stem from `0 %` (`rest`) to `100 %` (`full`) and toggles the status label between **Released** and **Pressed**. <!-- src: /desktop/gui/src/keys.rs#live_view -->
- From a terminal, `opadctl status` shows `Input Module:     Hall Effect` and `HE Calibrated:    K1: yes, K2: yes` (with `K3: yes` on a V3 module). <!-- src: /desktop/cli/src/main.rs#run -->

## What to do if it did not work

If a check fails, the pad shows **Calibration failed** and the reason. Without the app it offers a **Try again** button; with the app it says **Try again in the OPad app**: <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen --> <!-- src: /firmware/main/input/he_cal_runner.c#he_cal_runner_error_string -->

- **Keep both keys still while resting** (`HE_CAL_ERR_REST_NOT_STILL`): A key moved or vibrated during rest capture. Place the pad on a steady surface, keep fingers off all keys, and retry.
- **A key was pressed while resting** (`HE_CAL_ERR_REST_PRESSED`): a key was held down during Step 1. Start again with every key fully released.
- **Sensor reading looks wrong** (`HE_CAL_ERR_REST_OUT_OF_RANGE`) or **The key didn't register** (`HE_CAL_ERR_SIGNAL_NOT_RISING`): Check that magnetic (Hall Effect) switches are seated properly and, on V3, run `opadctl pad test --magnet` or **Troubleshooting** -> **Board test** to check the cable and sensors. <!-- src: /desktop/cli/src/main.rs#run -->
- **Press each key all the way down** (`HE_CAL_ERR_SPAN_TOO_SMALL`) or **Press the same way every time** (`HE_CAL_ERR_PEAKS_INCONSISTENT`): Bottom out each switch fully and consistently on all 5 presses, letting the stem return completely to rest between presses.
- **Press exactly 5 times** (`HE_CAL_ERR_TOO_FEW_PEAKS` / `HE_CAL_ERR_TOO_MANY_PEAKS`): Make 5 clean, distinct full presses without double-bouncing mid-press.
- **The OPad app disconnected** or **Could not save calibration**: check the USB connection and try again while no map is running.
