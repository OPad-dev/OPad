---
title: Calibrating Hall Effect Switches
description: Guided calibration walkthrough for magnetic switches on OPad, recording resting and bottom-out levels, and tuning rapid trigger in opad-gui.
category: how-to
source_files:
  - /firmware/main/input/he_cal.c
  - /firmware/main/ui/he_cal_ui.c
  - /firmware/main/input/he_cal_runner.c
  - /desktop/gui/src/keys.rs
  - /desktop/gui/src/he_monitor.rs
  - /protocol/opad.proto
---

Hall Effect magnetic switches require calibration to establish the magnetic flux boundaries between the stem's resting position (top) and fully depressed bottom-out state. In accordance with normative decision record D7, an uncalibrated Hall Effect pad is locked to its on-screen calibration wizard until calibration is completed. <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Guided Calibration Walkthrough

Calibration can be completed directly on the keypad's touchscreen or guided through `opad-gui`. <!-- src: /firmware/main/input/he_cal_runner.c -->

### Step 1: Resting Position Capture
1. Make sure no fingers or objects are touching any switch stems.
2. Tap the **Start Calibration** button on the touchscreen (or click **Start Calibration** on the Calibration page in `opad-gui`). <!-- src: /firmware/main/ui/he_cal_ui.c -->
3. The pad collects 64 samples per key over 16 ms (`REST_SETTLE_SAMPLES = 64`) to calculate the baseline magnetic rest level. <!-- src: /firmware/main/input/he_cal_runner.c#REST_SETTLE_SAMPLES -->
4. If a switch stem is moving or noisy during this step, the calibration runner reports `HE_CAL_ERR_REST_NOT_STILL`. Ensure the pad is resting on a steady desk. <!-- src: /firmware/main/ui/he_cal_ui.c -->

### Step 2: Key 1 Bottom-Out Capture
1. The display prompts: `Press Key 1 firmly to the bottom`.
2. Push Key 1 all the way down until it physically contacts the switch housing and hold it steady for 1 second.
3. The pad registers the maximum magnetic reading and sounds a confirmation beep or advances the visual progress bar.

### Step 3: Key 2 Bottom-Out Capture
1. The display prompts: `Press Key 2 firmly to the bottom`.
2. Push Key 2 completely down and hold for 1 second.
3. If using an OPad V3 3-key module, repeat the process for Key 3.

### Step 4: Verification & Save
1. The pad validates that the magnetic travel delta between resting and bottom-out exceeds the minimum acceptable span threshold.
2. If successful, the calibration runner writes the new A/B calibration record to NVS (`s_store12` in `he_cal.c`). <!-- src: /firmware/main/input/he_cal.c -->
3. The calibration lock screen dismisses, returning the keypad to normal operational mode.

## Tuning Actuation & Rapid Trigger in opad-gui

Once calibrated, fine-tune switch responsiveness in `opad-gui`: <!-- src: /desktop/gui/src/keys.rs -->

1. Open `opad-gui` and click **Keys** in the sidebar.
2. **Understand the Units**: In accordance with decision record D3, all travel sliders represent a percentage of the calibrated magnetic span (in 0.1% increments), never fixed millimeters. <!-- src: /desktop/gui/src/keys.rs -->
3. **Actuation Point**:
   - Sets the downward travel percentage required to actuate the switch (e.g., `20.0%`).
4. **Rapid Trigger Settings**:
   - Toggle **Rapid Trigger** on.
   - **Press Sensitivity**: Distance the stem must travel downward to re-register an actuation while held (e.g., `0.2 mm` equivalent, represented as `5.0%`).
   - **Release Sensitivity**: Distance the stem must travel upward to trigger an immediate release (e.g., `5.0%`).
   - **Deadzone**: Top and bottom deadzone margins (default `5.0%`) to eliminate magnetic jitter when the key is fully released or bottomed out.
5. **Live Switch Cut-Away Canvas**:
   - As you physically press the switch, observe the real-time visualizer canvas in `opad-gui`. The visualizer tracks stem travel, actuation thresholds, and release points polled from `HeLiveSample` at 50 Hz. <!-- src: /desktop/gui/src/keys.rs -->
6. Click **Save Settings** to commit the rapid trigger configuration to pad memory.
