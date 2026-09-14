# osu!pad v2 — Rapid Trigger (Hall-Effect Keys) Plan

**Companion to:** `osupad_technical_spec_v1.md` and `osupad_remaining_work_v1.md`
**Audience:** the project owner, Antigravity and other coding agents
**Starts after:** the Linux v1.0 release (v1 hardware and firmware must keep working)

---

## 0. Scope and decisions

### The goal: rapid trigger, nothing else

Rapid trigger lets you tap faster in osu!. A normal switch only registers a press after the key passes a fixed point (about 2 mm), and only releases after it rises back above a fixed point. With rapid trigger, the key:
- **releases** as soon as it starts moving up by a small distance (e.g. 0.15 mm), wherever it is, and
- **presses again** as soon as it starts moving down by that distance.

Fingers barely have to lift between taps. That needs analog key travel, which means **Hall-effect (magnetic) switches** and a sensor under each one.

### Decided

| Decision | Choice |
|---|---|
| Feature | **Rapid trigger only**, plus the two settings it needs: first actuation point and rapid-trigger sensitivity |
| Keys | Still exactly two keys, still standard HID keyboard keys (Z/X by default) |
| Polling | Stays **1000 Hz**. The ESP32-S3 has Full-Speed USB only; 8 kHz would need High-Speed USB and is not a goal |
| v1 hardware | Keeps working. Firmware supports both input backends: **digital** (today's MX switches on GPIO) and **Hall** (new) |
| Case | The current case (V1) has no space reserved under the keys for a key PCB. A new case revision adds that space and the mounting bosses once the Hall sensor PCB exists |

### Explicitly out of scope for v2

- Analog/gamepad output, Dynamic Keystroke, Mod-Tap, per-key RGB, macros, layers, profiles
- 8 kHz polling
- More than two keys
- Wireless (see `docs/roadmap.md`)

### Spec amendments this plan introduces

These override `osupad_technical_spec_v1.md` for v2 only:

1. **§2.2 non-goals:** "Hall-effect support" and "rapid trigger" are removed from the non-goals. Everything else in §2.2 stays.
2. **§9 key input critical path:** with the Hall backend there is no GPIO edge. Keys are sampled at a fixed rate, and the critical path becomes *sample → position → rapid-trigger decision → HID submit*. The same rules apply: highest priority, core 0, no blocking, no allocation, no logging.
3. **§9.2 debounce:** does not apply to the Hall backend (no contacts to bounce). Noise is handled with filtering and hysteresis instead (see §3).
4. **§33 latency gate:** extended with sample-to-HID latency and a rapid-trigger distance test (see V2-8).

The latency invariant (§3, "latency always wins") is unchanged.

---

## 1. How it fits the existing architecture

```text
            v1 (digital)                         v2 (Hall backend)
MX switch edge -> GPIO ISR            Hall sensor -> ADC sample (fixed rate, core 0)
  -> keypad task (debounce)             -> position (calibrated, 0.01 mm units)
  -> usb_hid_handle_key_event()         -> rapid-trigger engine
                                        -> usb_hid_handle_key_event()   (unchanged)
```

Unchanged:
- HID reports and the 1000 Hz endpoint
- Lifetime and map counters (a rapid-trigger press counts as a press)
- Latency statistics, now measured from the sample that caused the change
- CDC protocol framing, daemon, sync, display, persistence rules

New:
- Input backend selection
- Sampling pipeline
- Rapid-trigger engine
- Calibration
- New settings in the protocol, daemon and GUI
- Sensor PCB and case mounting

---

## 2. Hardware choices and open questions

Everything in this section is a **candidate**. Phase V2-0 picks the final parts by measurement.

### 2.1 Switches

Any MX-footprint magnetic switch, e.g. **Wooting Lekker**, **Gateron KS-20 magnetic** (Jade/White), or **Geon Raw HE**.
- Total travel is about 3.4–4.0 mm depending on the model.
- **Magnet polarity and strength differ between brands.** Use a sensor that reads both poles (bipolar), or verify the polarity per switch model.
- They fit the existing 14 mm plate cutout. Check each model's plate thickness recommendation (the case uses 1.5 mm).

### 2.2 Sensing: three options

| Option | Parts (candidates) | Pros | Cons |
|---|---|---|---|
| **A. Analog Hall → ESP32-S3 internal ADC** | 2× ratiometric linear Hall sensor (e.g. TI DRV5055, bipolar, 3.3 V) | Fewest parts, cheapest | ESP32-S3 ADC noise and non-linearity. Needs **two free ADC1 pins** on the Waveshare headers |
| **B. Analog Hall → external SPI ADC** | Same sensors + 2-channel 12-bit SPI ADC (e.g. MCP3202) | Cleaner readings, any free GPIOs | One more chip, SPI wiring |
| **C. Digital Hall sensor** | 2× SPI linear Hall sensor with built-in ADC (e.g. TI TMAG5170) | Cleanest signal, temperature compensated, digital all the way | Most expensive, SPI driver work |

Constraints confirmed from ESP-IDF documentation:
- On the ESP32-S3, ADC **continuous (DMA) mode only supports ADC1** (GPIO1–GPIO10); ADC2 DMA is disabled because of a hardware erratum. GPIO14 (today's Key 1) is ADC2, so option A needs different pins.
- In this project, GPIO1 is the LCD backlight.
- The LCD uses SPI2. Options B and C must use **SPI3**, a separate bus, so LCD DMA transfers can never delay a sample.

**Open question (resolve in V2-0):** which GPIOs on the Waveshare P1/P2 headers are free, and which of those are ADC1. Take this from the Waveshare schematic, not guesswork.

**Sensitivity range:** pick the sensor's sensitivity variant (for example the DRV5055 A1–A4 range) from the field measured at rest and at bottom-out with the chosen switch. Aim for the full stroke to use most of the ADC range without saturating.

### 2.3 Wiring to the Waveshare board

| Option | Wires |
|---|---|
| A | 3V3, GND, 2× analog |
| B / C | 3V3, GND, SCK, MOSI, MISO, 1–2× CS |

Use a keyed connector on the sensor PCB (JST-SH or JST-PH). On the board end, solder to the header pins or use a low-profile right-angle housing; a straight Dupont housing does not fit under the screen.

---

## 3. Rapid-trigger engine specification

Pure C, no ESP-IDF dependencies, unit-tested on the host. Suggested files: `firmware/main/input/rapid_trigger.c/.h`.

### 3.1 Units and inputs

- **Position** `p`: key travel in **0.01 mm**. 0 is fully released and `travel` is bottomed out (e.g. 400 = 4.00 mm). It comes from calibration (§4), clamped to `[0, travel]`.
- One call per sample per key: `rt_step(state*, p, now_us) -> {NONE, PRESS, RELEASE}`.

### 3.2 Settings (per key, persisted)

| Setting | Meaning | Starting default | Range |
|---|---|---|---|
| `actuation_point` | Depth for the first press from a fully released key | 1.20 mm | 0.10 – travel−0.10 |
| `rt_press_sens` | Downward movement from the highest point that re-presses | 0.15 mm | 0.05 – 1.00 |
| `rt_release_sens` | Upward movement from the lowest point that releases | 0.15 mm | 0.05 – 1.00 |
| `top_deadzone` | Above this, the key counts as fully released and rapid trigger resets | 0.30 mm | 0.05 – actuation_point |
| `bottom_deadzone` | Below `travel − bottom_deadzone`, noise at bottom-out is ignored | 0.20 mm | 0 – 0.50 |
| `continuous_rt` | Rapid trigger stays active until the key rises above `top_deadzone` (on), or only below `actuation_point` (off) | on | bool |

The defaults are starting points. Tune them in V2-0 and V2-8 against the measured noise; each sensitivity must stay above the post-filter noise band.

### 3.3 State machine

```text
state per key: pressed (bool), rt_active (bool), extreme (int, 0.01 mm)

on sample p:
  p = min(p, travel - bottom_deadzone)            // ignore bottom-out noise

  if p <= top_deadzone:                           // fully released zone
      rt_active = false
      if pressed: pressed = false; return RELEASE
      extreme = p; return NONE

  if not pressed:
      extreme = min(extreme, p)                   // track the highest point since release
      if (not rt_active and p >= actuation_point) or
         (rt_active and p >= extreme + rt_press_sens):
          pressed = true; rt_active = true; extreme = p
          return PRESS
  else:
      extreme = max(extreme, p)                   // track the lowest point since press
      if p <= extreme - rt_release_sens or
         (not continuous_rt and p < actuation_point):
          pressed = false; extreme = p
          if not continuous_rt and p < actuation_point: rt_active = false
          return RELEASE
  return NONE
```

Required unit tests (host, recorded or synthetic traces):
- Clean press/release through the actuation point
- Fast alternating taps that never reach `top_deadzone` (must produce every press and release)
- Noise at rest (no presses), noise at bottom-out (no releases)
- A slow press at exactly the actuation point (no chatter)
- Continuous on/off behaviour
- Sensitivity boundary values
- Both keys interleaved

### 3.4 Filtering

- Filtering adds delay, and delay is exactly what rapid trigger removes. Keep the group delay to **≤ 1 sample period**.
- Prefer hardware oversampling (average several ADC conversions within one sample period) over a slow IIR filter. If an IIR filter is needed, use a fixed-point single-pole filter and measure its delay.
- Hysteresis is built in: the sensitivities are the hysteresis.

---

## 4. Calibration

- **Why:** magnet strength, sensor placement and PCB distance vary per unit, and the field is not linear with distance (it falls off steeply).
- **Stored per key:** raw value at rest, raw value at bottom-out, and a normalised curve mapping raw to position.
- **Curve:** start with a per-switch-model lookup table (for example 16 points, from V2-0 measurements) scaled between the rest and bottom-out values. Linear interpolation between points, integer math.
- **Guided calibration (GUI):** "leave both keys untouched" (captures rest), then "press each key fully 5 times" (captures bottom-out). Validate that the span is large enough and monotonic. Save to NVS **only in IDLE**.
- **Auto rest tracking:** while a key is released and stable (below `top_deadzone`, low variance for about 2 s), slowly follow drift in the rest value so temperature changes don't shift the actuation point. Never track while pressed or in PLAYING.
- **Fallback:** with no valid calibration, use conservative defaults and report `calibrated=false` in `DeviceStatus`. The GUI prompts to calibrate.

---

## 5. Latency

**Budget with the Hall backend:**
- Wait for the next sample: ≤ one sample period (0.25 ms at 4 kHz per key)
- Filter delay: ≤ one sample period
- Rapid-trigger decision and HID submit: tens of µs
- USB poll: ≤ 1 ms (unchanged)

The electronic latency is similar to v1. The gain is in **key travel**: a normal switch registers after ~2 mm, rapid trigger after ~0.15 mm of movement in the new direction. At a fast tapping speed of around 200 mm/s, roughly 1.8 mm less travel is about **9 ms earlier** per press and release. That is an illustrative estimate; measure it in V2-8.

**Rules:**
- Sampling task on **core 0** at the priority today's keypad task uses (`configMAX_PRIORITIES - 1`, one above the TinyUSB task).
- The ADC DMA callback or SPI read only moves raw samples into a lock-free buffer and notifies the task. No rapid-trigger work in ISR context if it can't be kept to a few instructions.
- Sample rate target: **≥ 2 kHz per key, ideally 4 kHz**. Frames must be small, so a DMA frame of 64 conversions at 8 kHz would add 8 ms and is not acceptable.

---

## 6. Phases

Each phase ends with its acceptance checks. Don't start V2-6 (PCB) before V2-0 has picked the parts.

### V2-0. Bench prototype and part selection

**Do:**
1. Hand-wire two magnetic switches with sensors for options A, B and C (breakout boards are fine) to the Waveshare board.
2. Confirm the free header GPIOs from the Waveshare schematic, and which are ADC1.
3. Add a **debug firmware build** (Kconfig `OSUPAD_HALL_BENCH`) that samples both keys and streams raw values to the host. Only in this bench build is streaming during key presses allowed.
4. Write a host script (`scripts/hall_bench.py`) that records traces to CSV and reports the numbers below.
5. For each option and switch model, measure:
   - Noise peak-to-peak at rest and at bottom-out (in raw units and in mm after scaling)
   - Effective resolution in mm
   - Stroke curve (raw vs travel, using a feeler gauge or calipers at several depths)
   - Achievable sample rate and CPU load on core 0
   - Drift over 30 minutes
6. Record everything in `docs/v2-hall-bench.md` and choose the option and parts.

**Acceptance:**
- An option achieves **post-filter noise ≤ 0.05 mm peak-to-peak** at **≥ 2 kHz per key**, with a monotonic stroke curve.
- Parts, pins and sample rate are written down.

### V2-1. Input backend abstraction (firmware)

**Do:**
- Introduce an `input_backend` interface with `init`, `start`, `get_pressed`, `get_position` (Hall only) and `get_latency_stats`.
- Move today's code into the **digital backend** unchanged.
- Select the backend with Kconfig (`OSUPAD_INPUT_DIGITAL` / `OSUPAD_INPUT_HALL`). `HelloAck` reports `input_type`.

**Acceptance:** the digital build behaves exactly like v1. Latency stats are unchanged and all existing tests pass.

### V2-2. Sampling pipeline (Hall backend)

**Do:**
- Implement the option chosen in V2-0 (ADC1 continuous DMA with small frames, or SPI3 polling in a core-0 task).
- Convert raw values to position with the calibration from V2-4 (stub defaults until then).
- Feed the rapid-trigger engine and call `usb_hid_handle_key_event()`.

**Acceptance:**
- Measured sample rate within 5% of target.
- No missed samples over 10 minutes (overflow counter = 0).
- Core 0 stays within budget; USB and HID are unaffected (the digital-build latency gate still passes on the same hardware).

### V2-3. Rapid-trigger engine

**Do:** implement §3 as pure C with the unit tests listed there, plus replay tests on the CSV traces from V2-0.

**Acceptance:**
- All unit and replay tests pass in CI (`firmware/test/host`).
- On the device: 2 minutes of fast alternating taps without reaching the top deadzone produce no missed or extra presses (compare the HID event count with a slow-motion video or a logic analyzer on a debug pin).

### V2-4. Calibration

**Do:** implement §4: storage, guided calibration messages, auto rest tracking, the `calibrated` flag.

**Acceptance:**
- After calibration, the actuation point measured with a feeler gauge is within ±0.10 mm of the setting on both keys.
- After 30 minutes of warm-up, the actuation point drifts ≤ 0.05 mm.
- No NVS writes happen outside IDLE.

### V2-5. Protocol, daemon and GUI

**Do:**
- **Protocol:** additive fields only; field numbers never reused.
  - `ConfigPayload`: the §3.2 settings per key.
  - `HelloAck.input_type`.
  - `DeviceStatus.calibrated`.
  - New messages `CalibrationStart` / `CalibrationStep` / `CalibrationResult`.
  - `LiveTravel` stream (both key positions at ≤ 60 Hz), sent **only when the GUI asks, and never during PLAYING or COOLDOWN**.
- **Daemon:** validates ranges, applies the existing defer-during-gameplay rules, stores the settings in SQLite and includes them in JSON backups (format_version bump with migration).
- **GUI:** new "Rapid Trigger" section shown only for Hall hardware:
  - actuation point, press/release sensitivity, continuous toggle
  - a calibration wizard
  - live travel bars with actuation and sensitivity markers
- **Display (optional, later):** a layout data source for live key travel on the LCD.

**Acceptance:**
- Settings round-trip GUI → daemon → device → NVS → reboot.
- Changing settings during PLAYING is deferred exactly like key mapping in v1.
- Live travel never streams during gameplay.

### V2-6. Sensor PCB

**Do:** KiCad project in `hardware/pcb/rt_keys/`:
- 2 switch positions at 19.05 mm pitch, centred on the case plate cutouts.
- One sensor centred under each switch's magnet, at the distance chosen in V2-0.
- Decoupling per the sensor datasheet, plus the ADC (option B) and connector.
- Compact board outline under the two keys (about 38 × 19 mm), 1.6 mm FR4.
- 2–4 M2 mounting holes placed clear of the switch footprints.
- Keep ground pours and magnetic or ferrous parts away from the sensor area.

**Acceptance:**
- DRC clean.
- The 3D model (with switches) fits under the key deck of the new case (V2-7) in the OpenSCAD fit check.
- Prototype boards read within the V2-0 noise budget.

### V2-7. Case revision

**Do:** copy `hardware/3d/custom_case/V1` to a new version folder and:
- Reserve space under the key deck for the PCB, the sockets and the cable.
- Add M2 heat-set insert bosses under the key deck, matching the PCB holes.
- Set the plate thickness for the chosen switches.
- Route the cable to the Waveshare header.
- Rerun the fit check with the PCB, switches and board.

**Acceptance:** fit check clean; switches can be pulled and reinserted without the PCB moving.

### V2-8. Validation and latency gate

**Do:**
1. **Electronic latency:** sample-to-HID p50/p99/p99.9/max on the Hall build, same procedure as `docs/latency-testing.md`, stages A–D.
2. **Rapid-trigger distance test:** with a slow, controlled press/release (hand or rig), log position and HID events. Verify that presses and releases happen within `sensitivity + 0.05 mm` of the turning point.
3. **Tapping test:** in osu!, compare v1 digital and v2 Hall on the same maps (stream sections). Record accuracy and any missed or ghost inputs.

**Acceptance:**
- Sample-to-HID p99.9 < 1.5 ms (sample period + filter + processing).
- No missed or extra presses in the tapping test.
- Display and CDC activity add no measurable jitter (same gate as v1).

### V2-9. Documentation

- Update `docs/architecture.md` (input backends, sampling), `docs/protocol.md` (new fields), `docs/latency-testing.md` (Hall stages) and the hardware README (PCB, new case).
- Record the spec amendments from §0 in `osupad_technical_spec_v1.md` as a "v2 amendments" appendix.

---

## 7. Risks

| Risk | Mitigation |
|---|---|
| ESP32-S3 ADC too noisy for fine sensitivities | V2-0 compares options A/B/C before any PCB work |
| Not enough free ADC1 pins on the headers | Option B or C (SPI3, any free GPIO) |
| Magnet polarity or strength differs between switch brands | Bipolar sensor, per-model curve table, calibration |
| Filter delay eats the rapid-trigger gain | ≤ 1 sample of group delay, oversampling instead of heavy filtering, measured in V2-8 |
| Temperature drift moves the actuation point | Auto rest tracking; option C is temperature compensated |
| Scope creep toward "a full Wooting clone" | §0 out-of-scope list; any addition needs an explicit owner decision |

---

## 8. References

- [minipad firmware](https://github.com/minipadKB/minipad-firmware): open-source RP2040 Hall-effect osu! keypad with rapid trigger (0.01 mm resolution, hysteresis and rapid-trigger sensitivity settings)
- [fluxpad](https://github.com/sssata/fluxpad): analog osu! keypad with Wooting Lekker switches and rapid trigger
- [ESP-IDF ADC continuous mode (ESP32-S3)](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-reference/peripherals/adc/adc_continuous.html): DMA sampling, ADC1-only limitation
