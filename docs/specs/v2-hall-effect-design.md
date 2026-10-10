# Hall Effect module: how it works (DRAFT for discussion)

**Status:** first draft, 2026-10-10. Nothing here is decided until we have
talked it through. It turns `docs/specs/v2-rapid-trigger.md` (the plan
written before the board existed) into a design for the board that now
exists, and lists the questions that need an answer before the code.

**What exists today**

- The HE module: two **TI DRV5055A3** ratiometric linear Hall sensors, one
  under each key's magnet, on the PCB bottom, reading through 1.6 mm of FR-4.
  Outputs on IN1/IN2 = **GPIO10 (ADC1_CH9)** and **GPIO7 (ADC1_CH6)**, ID
  divider 100k/47k = 1.06 V on GPIO8 (ADC1_CH7). Spare lines GPIO6/4/2 go to
  probe pads (`hardware/pcb/V1/README.md`, "Hall Effect input module").
- The pad recognises it: on this pad the ID reads 1047 mV open and 382 mV with
  the pull-down (`opadctl board-test`, 2026-10-10), which
  `board_module_from_id` maps to `BOARD_MODULE_HE`
  (`firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c`).
- v1 then turns key input off and shows a notice (`firmware/main/app_main.c`,
  step 1b and step 7). **But the key pins are still armed as digital inputs
  with pull-ups and any-edge interrupts** (`board_keys_set_gpio` in
  `board.c`). The ISR returns early while input is off
  (`gpio_isr_handler` in `firmware/main/input/keypad.c`), yet a sensor output
  near mid-rail can fire it continuously on core 0, the key core. To fix
  first, whatever else we decide (§9, step 1).
- A measurement build: `CONFIG_OSUPAD_HALL_BENCH` streams both sensors as 1 ms
  averages plus per-second spread (`firmware/main/diag/hall_bench.c`,
  `scripts/hall_bench.py`). **No measurements yet**: the first run was cut
  short (the daemon holds the port, and the GUI restarts the daemon once).

---

## 1. The rules it has to keep (AGENTS.md §1)

| Rule | What it means for HE |
|---|---|
| Latency always wins | Sample → position → decision → HID submit on core 0, at the keypad task's priority, from IRAM, no logging or allocation. Filtering may add at most one sample period. |
| A keyboard with no software | **An HE pad must type out of the box, with no calibration step in an app.** Calibration has to be automatic (§4). The app only refines it. |
| App and pad update independently | Old app + new firmware: HE types with its stored or default settings. New app + old firmware: no Rapid Trigger UI (capability flag absent). |
| Touchscreen is the third button | Untouched: the HE path changes K1/K2 only. |
| Never BOOT/RESET | Unchanged; the A/B rollback covers HE builds too. |

## 2. Firmware architecture

One firmware for both modules: the module is detected at boot and picks the
input backend. The spec's Kconfig switch (`OSUPAD_INPUT_DIGITAL/HALL`,
v2-rapid-trigger §V2-1) does not fit swappable modules on one cable.

```text
boot: board_detect_module()
  MX / none  -> digital backend  (today: GPIO edge ISR -> keypad task -> debounce)
  HE         -> hall backend     (ADC DMA -> hall task -> rapid trigger)
                    \_____________ both _____________/
                    key event sink: state, counters, press log, latency stats,
                    usb_hid_handle_key_event(key, pressed, t_us), UI wake
```

- **Key event sink.** Today the counting, press log and HID call live inside
  the ISR and `keypad_task` (`keypad.c`). They move behind one function both
  backends call, so a rapid-trigger press counts, logs and reports exactly
  like a switch press. `t_us` is the edge time for MX and the sample time for
  HE, so the latency stats keep meaning "physical event → USB submit".
- **Swapping modules** needs a power cycle (replug); detection is boot-only,
  as today. Worth showing on the LCD if the ID changes at runtime? (Q8)
- **IRAM**: the hall task and the engine join `firmware/main/linker.lf`, like
  the key path did in `1a3c038`.

## 3. Sampling

- **ADC1 continuous (DMA) mode.** On the S3 only ADC1 has working DMA
  (v2-rapid-trigger §2.2), and all module lines are ADC1.
- **Proposal: three channels.** K1 (CH9), K2 (CH6) and **the ID line
  (CH7)** as a supply reference. The DRV5055's output is ratiometric to its
  3.3 V supply, while the ADC measures against its internal reference, so
  supply ripple and drift look like key travel. The ID divider sits on the
  same 3.3 V, so `key / id` cancels it. Caveat: the divider's ~32 kΩ source
  impedance is high for a scanned ADC (README, "Module ID"); it only needs a
  slow average, but the bench has to show it settles.
- **Rate and frames.** Target ≥ 4 kHz per key. Sketch: 30 kHz total over the
  three channels, an 8-conversion frame per DMA interrupt (~0.27 ms), each
  frame averaged per channel (oversampling instead of a slow filter). The DMA
  callback only notifies the hall task. Wake rate ~3.7 kHz on core 0, to
  check against the USB task's budget.
- **Attenuation** 12 dB (about 0–3.1 V). DRV5055 at 3.3 V sits at 1.65 V with
  no field and swings by its sensitivity times the field, either way
  depending on magnet polarity.

## 4. From raw reading to travel, and calibration without an app

- **Per key:** `rest` raw value (key up), `bottom` raw value (bottomed out),
  and the sign (which way the reading moves on a press: magnet polarity
  differs by switch brand).
- **Travel** in 0.01 mm, from a curve: the field falls off steeply with
  distance, so equal raw steps are not equal travel. Start with a per-switch
  lookup table (16 points, from bench strokes), scaled between `rest` and
  `bottom` (v2-rapid-trigger §4).
- **Automatic calibration (rule 2):**
  - `rest`: averaged at boot while both readings are steady, then tracked
    slowly while a key is up and still (temperature drift). Never tracked
    while pressed or in PLAYING.
  - `bottom` and sign: learned from the deepest presses seen. The first full
    press after first boot sets them; until then a conservative default span
    from the bench measurements applies. Saved to NVS in IDLE only.
  - The app's guided calibration ("leave the keys up", "press each fully 5
    times") just runs the same capture deliberately.
- **Resolution worry.** A3 is the least sensitive variant (≈16 mV/mT, README
  "Sensor sensitivity"), and the sensor reads through the board. Because the
  field rises steeply only near the bottom, resolution near the top of the
  travel may be too coarse for 0.15 mm sensitivity there. The bench decides
  whether A3 is enough or the next batch should be A2 (Q2).

## 5. Rapid trigger

The state machine in v2-rapid-trigger §3.3 as written (actuation point,
press and release sensitivity, top and bottom dead zones, continuous mode),
in pure C with host tests in `firmware/test/host`, plus replays of bench
traces. Per key settings, defaults from §3.2 there until the bench says
otherwise.

## 6. Protocol (additive, both directions compatible)

| Change | Old app, new pad | New app, old pad |
|---|---|---|
| `HelloAck.input_module` (the existing `InputModule` enum, `protocol/osupad.proto`) and `HelloAck.rapid_trigger` capability | ignored | absent → no RT UI |
| `ConfigPayload`: per key actuation, press/release sensitivity, dead zones, continuous (0 = keep current, like the swipe fields) | pad keeps its own | not sent |
| `DeviceStatus.calibrated` | ignored | absent |
| Calibration start / step / result messages | never sent | not offered |
| `LiveTravel` stream: both positions ≤ 60 Hz, **only when the app asks and never in PLAYING/COOLDOWN** | never sent | not offered |

The daemon stores the settings like the other config (SQLite, JSON backups,
deferred during gameplay).

## 7. App

- **Rapid Trigger page**, shown only when `input_module` is HE: actuation
  point, press/release sensitivity (linked by default?), continuous toggle,
  live travel bars with the markers, calibration wizard.
- **Board test for HE** (extends `opad-model/src/board_test.rs`): each sensor
  at rest within the expected band around 1.65 V (absent sensor or cut
  trace reads at a rail), reversed cable, spare lines.

## 8. Latency budget

Wait for the sample (≤ 0.27 ms) + averaging (inside the frame) + decision and
submit (tens of µs, measured at ~70 µs edge→submit for MX after `1a3c038`) +
USB poll (≤ 1 ms, unchanged). Electronically like MX; the gain is travel
(rapid trigger re-presses after ~0.15 mm instead of crossing a fixed ~2 mm
point). Validation as v2-rapid-trigger §V2-8.

## 9. Order of work

| Step | What | Needs you at the pad |
|---|---|---|
| 1 | HE pins left analog in today's firmware (no pull-ups, no key ISR) | no |
| 2 | Bench: rest noise, slow full strokes per key, fast taps, ID-line ratio | **yes, ~5 min** |
| 3 | Rapid-trigger engine + host tests | no |
| 4 | Key event sink refactor (MX behaves byte-for-byte as today, latency rechecked) | quick MX check |
| 5 | Hall backend: sampling, automatic calibration → **HE pad types with defaults** | yes |
| 6 | Protocol + daemon + app page + HE board test | yes |
| 7 | Latency and tapping validation, docs | yes |

## 10. Questions for you

1. **Switches:** which magnetic switches are in the module (Gateron KS-20,
   Lekker, Geon Raw HE...)? Their magnet and travel set the curve.
2. **Sensor variant:** were A3 (the default) fitted? If the bench shows A3 is
   too coarse, do we plan an A2 batch?
3. **Calibration:** automatic as in §4, with the app's wizard only refining
   it. OK, or do you want calibration to be an explicit step?
4. **Defaults:** actuation 1.20 mm, sensitivity 0.15 mm, continuous on (spec
   §3.2), or your own?
5. **Settings UI:** one sensitivity for press and release, or separate?
   Per-key settings or both keys together?
6. **Plain mode:** should HE also offer "no rapid trigger, just an adjustable
   actuation point"?
7. **LCD:** show live key travel on the screen outside maps (spec V2-5 says
   optional, later)?
8. **Hot swap:** is "swap the module, replug" acceptable, or should the pad
   notice a swap while running?
9. **The supply reference (§3):** fine to sample the ID line continuously?
