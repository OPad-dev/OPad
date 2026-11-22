---
title: Hall Effect and Rapid Trigger
description: How OPad senses Hall Effect magnetic switches across V1 and V3 hardware, runs the 3-state Rapid Trigger engine, enforces noise floors, and stores calibration.
---

Hall Effect (HE) magnetic switches replace physical metallic contact leaves with a magnet in the switch stem and a linear Hall sensor on the PCB. Instead of a fixed contact point, firmware reads continuous magnetic strength for each key, allowing configurable actuation depth and **Rapid Trigger (RT)**—releasing as soon as the stem moves upward and re-pressing as soon as downward motion resumes.

## Sensing Pipelines: V1 Internal ADC vs V3 External SPI ADC

OPad supports two Hall Effect hardware generations in [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c):

| Aspect | Hall Effect V1 (2 keys) | Hall Effect V3 (3 keys) |
|---|---|---|
| Sensor readout | ESP32-S3 internal ADC1 continuous DMA (`HE_SOURCE_INTERNAL_ADC`) | External TI ADS7028 12-bit ADC over SPI3 (`HE_SOURCE_SPI_ADC`) |
| Channels | Key 1 on `ADC_CHANNEL_9`, Key 2 on `ADC_CHANNEL_6` | Key 1 on `AIN5`, Key 2 on `AIN4`, Key 3 on `AIN3` (auto-sequence) |
| Sampling cadence | `HE_SAMPLE_HZ = 64000` Hz, `HE_FRAME_CONVS = 16` -> **250 µs** frames (**4 kHz**) | Hardware `gptimer` every `HE_SPI_FRAME_US = 50` µs -> **20 kHz** frames |
| Bus / filtering | Median of valid conversions per 250 µs DMA frame inside IRAM ISR `on_frame` | 40 MHz SPI3 with per-frame CRC (`ads7028_read16_again` direct register loop); 4 reads per frame (1 stale + 3 keys), bad-CRC words dropped |
| Decision & wakeup | `on_frame` ISR runs `he_rt_step` in IRAM on Core 0 and notifies `he_task` only on key edge | Core 0 `he_spi` task (`configMAX_PRIORITIES - 1`) runs `he_rt_step` and publishes key events directly via `key_event_report` |
| Calibration capture | Every 250 µs frame (4 kHz) | Decimated 5:1 (`SPI_CAL_DECIMATE = 5`) to match the 4 kHz calibration capture |
| NVS store | `NVS_NS_CAL` (`opad_cal`), keys `cal_a`/`cal_b` | `NVS_NS_CAL3` (`opad_cal3`), keys `cal_a`/`cal_b` (Keys 1–2) + `cal3_a`/`cal3_b` (Key 3) |

Details are in [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c), [/firmware/main/input/ads7028.c](/firmware/main/input/ads7028.c), and [/firmware/main/input/he_cal.c](/firmware/main/input/he_cal.c).

On V1, the ADC continuous driver is configured with `ADC_CONTINUOUS_ISR_IRAM_SAFE` so `on_frame` keeps running in internal RAM without cache stalls. On V3, `he_spi_source.c` acquires SPI3 exclusively, runs the first transfer via the driver, and performs subsequent 32-bit CRC-protected reads directly on `GPSPI3` hardware registers (`ads7028_read16_again`), avoiding ~10 µs of driver overhead per word. If an FFC cable is inserted upside down, `ads7028_carrier_pins(cable_reversed, ...)` swaps the SPI pin mapping automatically.

## Rapid Trigger Decision Engine (`he_rt.c`)

Each key's state machine in [/firmware/main/input/he_rt.c](/firmware/main/input/he_rt.c) (`he_rt_step`) has **three** operational states (`he_rt_state_t`):

```text
                        raw >= actuation
            +----------------------------------------+
            |                                        v
   +---------------+   raw <= extreme - rt_release   +---------------+
   |  HE_RT_ARMED  | <-----------------------------  | HE_RT_PRESSED |
   +---------------+    (plain mode: raw <= release) +---------------+
            ^                                          |           ^
            |                                          | RT on:    | RT on:
            | raw <= region_exit                       | raw <=    | raw >=
            |                                          | extreme - | extreme +
            |                                          | rt_release| rt_press
            |                                          v           |
            +----------------------------------- +--------------------+
                                                 | HE_RT_RT_RELEASED  |
                                                 +--------------------+
```

### State Transitions

1. **`HE_RT_ARMED`**: The key is released and waiting for an initial press.
   - If `k->need_release` is set (after boot, parameter change, or a sensor fault), the key ignores presses until `raw <= p->release` (or `raw <= p->region_exit` when rapid trigger is enabled).
   - Once armed and `raw >= p->actuation`, the key transitions to `HE_RT_PRESSED`, emits `HE_RT_DOWN`, and initializes `k->extreme` to `cap_peak(raw, p->peak_cap)`.
2. **`HE_RT_PRESSED`**: The key is held down.
   - Without rapid trigger (`!p->rapid_trigger`): the key stays pressed until `raw <= p->release`, then returns to `HE_RT_ARMED` and emits `HE_RT_UP`.
   - With rapid trigger:
     - `k->extreme` tracks the deepest raw reading reached, capped at `p->peak_cap` (which implements the optional **BOTTOM DEAD ZONE** so pushing harder into the housing damper does not shift the release point).
     - If `raw <= p->region_exit`, the key leaves the rapid trigger zone, returns to `HE_RT_ARMED`, and emits `HE_RT_UP`.
     - Otherwise, if `raw <= k->extreme - p->rt_release`, the key transitions to `HE_RT_RT_RELEASED`, emits `HE_RT_UP`, and sets `k->extreme = raw` to track the shallowest point reached.
3. **`HE_RT_RT_RELEASED`**: The key was released mid-stroke by rapid trigger and is ready to re-press immediately.
   - `k->extreme` tracks the lowest (most released) reading seen while rising.
   - If `raw <= p->region_exit`, rapid trigger ends and the key returns to `HE_RT_ARMED` (requiring a full crossing of `p->actuation` next time).
   - If `raw >= k->extreme + p->rt_press`, the key immediately re-actuates (`HE_RT_DOWN`), transitions back to `HE_RT_PRESSED`, and resets `k->extreme = cap_peak(raw, p->peak_cap)`.

The difference between **Normal** and **Continuous** rapid trigger is simply `p->region_exit`: in Normal mode it sits at the actuation threshold (rapid trigger ends once the key rises back above actuation), whereas in Continuous mode it sits near the top at `top_reset` so rapid trigger stays active almost all the way back to rest.

## Travel Units, Noise Floors, and Calibration

Per [/docs/specs/v2-hall-effect-decisions.md](/docs/specs/v2-hall-effect-decisions.md) (D3 and D7) and [/firmware/main/input/he_params.h](/firmware/main/input/he_params.h):

- **Percentage of magnetic span (`HE_PCT_FULL = 1000`)**: Settings are stored in `0.1 %` increments of `full - rest` (magnetic counts, not millimeters), because magnet strength and sensor response vary nonlinearly across switches.
- **Noise floors (`he_params_build`)**: Movement thresholds (`rt_press`, `rt_release`, `release_hyst`) and top-reset clearance are clamped above configured raw-count noise floors (`he_limits_t`), and `he_params_build` returns explicit `HE_PARAMS_*_RAISED` flags whenever a setting is raised—never silently. Users can opt out per key via **Allow below the noise floor** (`no_floor`, dropping floors to 1 count).
- **Five-press calibration (`he_cal_capture.h`)**: Rest is averaged over 64 frames (16 ms at 4 kHz after a 64-frame settle) and checked for spread and absolute bounds. Full press records 5 full strokes per key and takes the **median** peak so a single extra-hard bottom-out does not skew the span; it also checks whether the key released below the measured rest (`HE_CAL_ERR_REST_PRESSED`).
- **A/B NVS persistence (`he_cal.c`)**: Calibration alternates between `cal_a`/`cal_b` (and `cal3_a`/`cal3_b` for Key 3 on V3) with sequence numbers and CRC32 validation. V1 (`opad_cal`) and V3 (`opad_cal3`) use separate NVS namespaces so calibrations from one hardware generation never drive the other. NVS writes are deferred while playing (`PLAYING` / `COOLDOWN`) and flushed in `IDLE`.

## Safety, Uncalibrated Lockout, and Stall Recovery

- **Uncalibrated lockout (D7)**: Until every active key has a valid calibration (`he_cal_is_locked_decision`), `he_cal_suppress_input` suppresses all key output, touchscreen swipes are disabled, and the display locks to the calibration screen ([/firmware/main/ui/he_cal_ui.c](/firmware/main/ui/he_cal_ui.c), [/firmware/main/input/he_cal_runner.c](/firmware/main/input/he_cal_runner.c)).
- **ADC stall watchdog (`he_health`)**: If a key stops producing valid conversions for `HE_STALL_US`, `he_backend.c` forces the key released (`he_rt_fault`) so a sensor or cable fault never leaves a key stuck down.
- **Safe fault recovery**: `he_rt_fault` sets `need_release = true`, so when valid readings return the key must be observed back at rest before any new press is emitted.
- **Module-probe pause**: When the V3 module monitor probes the module ID voltage, `he_backend_run_paused` cleanly pauses the ADC source, reads the ID voltage, releases the oneshot unit, and resumes streaming without leaving keys stuck.
