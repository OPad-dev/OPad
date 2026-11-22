---
title: Hall Effect and Rapid Trigger
description: Explanation of Hall Effect magnetic sensing, continuous ADC sampling, median filtering, rapid trigger state machines, and calibration storage.
---

Hall Effect (HE) magnetic switches replace physical metallic contact leaves with a permanent magnet embedded in the switch stem and a linear Hall sensor on the keypad PCB. Unlike mechanical switches that actuate at a fixed physical leaf contact point, Hall Effect switches permit dynamic actuation points and **Rapid Trigger (RT)**: resetting the key the instant upward finger travel begins, and reactivating the moment downward motion resumes. <!-- src: /firmware/main/input/he_backend.c -->

## Continuous ADC Sampling & Median Filtering

On the ESP32-S3, Hall Effect sensing is implemented with extreme care to maintain latency guarantees: <!-- src: /firmware/main/input/he_backend.c -->

- **Continuous DMA Sampling**: ADC1 operates in continuous hardware DMA mode, alternating between Key 1 (CH9) and Key 2 (CH6) at high sampling frequency (`HE_SAMPLE_HZ`). Each DMA frame completes in 250 µs (`HE_FRAME_CONVS`). <!-- src: /firmware/main/input/he_backend.c -->
- **IRAM-Locked Frame Interrupt**: The frame completion interrupt (`on_frame`) runs on Core 0 entirely in fast internal RAM (`IRAM_ATTR`). It remains active and deterministic even during flash write operations (`ADC_CONTINUOUS_ISR_IRAM_SAFE`). <!-- src: /firmware/main/input/he_backend.c -->
- **Median Filtering**: To eliminate electrical noise spikes without introducing phase delay, the frame interrupt computes the median of the valid conversions within each 250 µs DMA frame. <!-- src: /firmware/main/input/he_backend.c -->
- **Zero Polling Overhead**: The decision engine steps within the interrupt. The FreeRTOS task (`he_task`) is only awakened when a key state transition actually occurs, eliminating unnecessary context switches. <!-- src: /firmware/main/input/he_backend.c -->

## Rapid Trigger Decision Engine (`he_rt.c`)

The rapid trigger state machine tracks stem motion across four distinct operational states: <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->

```
                raw >= actuation
       +--------------------------------+
       |                                |
       v                                |
+-------------+   raw <= extreme - rt_release    +------------------+
|   PRESSED   | -------------------------------> |  RT_RELEASED     |
+-------------+                                  +------------------+
       |                                                |
       | raw <= region_exit                             | raw >= extreme + rt_press
       v                                                v
+-------------+ <--------------------------------+------+
|    ARMED    |       raw <= region_exit         |
+-------------+                                  v
                                           (Re-press Down)
```

### State Transitions
1. **`HE_RT_ARMED`**: Key is released in its resting zone. When downward movement crosses the configured actuation threshold (`raw >= p->actuation`), the key transitions to `PRESSED` and emits `HE_RT_DOWN`. The instantaneous travel depth is recorded in `k->extreme`. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
2. **`HE_RT_PRESSED`**: Key is actively held.
   - If the finger pushes deeper, `k->extreme` tracks the maximum depth (`cap_peak`).
   - If the finger pulls upward by the reset sensitivity distance (`raw <= k->extreme - p->rt_release`), the key immediately transitions to `HE_RT_RT_RELEASED` and emits `HE_RT_UP`—without requiring the key to return to the top resting point. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
   - If travel returns all the way past the top boundary (`region_exit`), it returns to `ARMED`.
3. **`HE_RT_RT_RELEASED`**: Key was released mid-travel via rapid trigger.
   - If downward motion resumes by press sensitivity (`raw >= k->extreme + p->rt_press`), the key immediately actuates (`HE_RT_DOWN`) and returns to `PRESSED`.
   - If upward travel continues past `region_exit`, the key returns to `ARMED`.

## Travel Units & Calibration (Decision Record D3)

In accordance with normative decision record D3 (`docs/specs/v2-hall-effect-decisions.md`): <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
- **Percentage of Magnetic Span**: Hall switch travel is never measured in millimeters. Manufacturing tolerances, magnet strengths, and case dimensions cause magnetic field curves to vary between switches. All actuation points and rapid trigger distances are configured strictly as a percentage of the calibrated magnetic span (in 0.1% units).
- **Two-Point Guided Calibration**:
  1. **Resting Level**: Measures magnetic flux when the switch stem is unpressed at its resting rest position.
  2. **Bottom-Out Level**: Measures magnetic flux when the switch is fully bottomed against the switch housing.
- **Independent Calibration Stores**: Because V1 (internal ADC) and V3 (SPI ADC) hardware modules operate with different signal gains and voltage baselines, each module type maintains an independent calibration namespace in NVS (`s_store12`). A V1 calibration is never applied to a V3 module. <!-- src: /firmware/main/input/he_cal.c -->

## Safety & Stall Recovery

To ensure mechanical switch reliability and prevent stuck inputs: <!-- src: /firmware/main/input/he_backend.c -->
- **ADC Health Watchdog**: If a key receives no valid ADC conversion within `HE_STALL_US`, the key is immediately released to prevent stuck inputs.
- **Fault Recovery (`he_rt_fault`)**: When recovering from an ADC stall, the key must be observed returning to the top resting state (`need_release = true`) before any new press can be registered. Sensor recovery will never emit an accidental keypress. <!-- src: /firmware/main/input/he_rt.c#he_rt_init -->
- **Uncalibrated Lockout (Decision D7)**: An uncalibrated Hall Effect pad is locked to its on-screen calibration wizard. Rapid trigger and keyboard reporting remain disabled until resting and bottom thresholds are successfully recorded. <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
