# Hall Effect module: how it works (DRAFT for discussion)

**Status:** draft, 2026-10-10, updated after the first discussion. Nothing here is decided until we have
talked it through. It turns `docs/specs/v2-rapid-trigger.md` (the plan
written before the board existed) into a design for the board that now
exists, and lists the questions that need an answer before the code.

**Decided so far (2026-10-10, first discussion)**

| # | Topic | Decision |
|---|---|---|
| — | Priorities | **1. latency, 2. accuracy.** Where they conflict, latency wins; accuracy wins over convenience (UI simplicity, CPU, flash wear). |
| 1 | Switches | **Gateron Jade Silent** (magnetic, silent dampers). Curve and default span come from bench strokes of this switch. |
| 3 | Calibration | **Explicit first, automatic later** (§4). |
| 4 | Default | **Behaves like MX out of the box:** plain mode, a fixed actuation point at about mid-travel (an MX switch actuates at ~2.0 mm), release just above it, rapid trigger off. Exact point from the bench. |
| 5 | Settings | The most accurate option: **per key, separate press and release sensitivity** (§5). |
| 6 | Plain mode | **Yes**, and it is the default (decision 4). |
| 7 | LCD travel | **Only while testing or calibrating**, never in normal use. |
| 8 | Module swap | Checked at boot and, cheaply, while idle (never in a map); a change shows "replug the USB cable" on the LCD. In this enclosure the cable usually has to come out to swap anyway. |
| — | Switch swap | Each switch's magnet gives its own key-up reading. A steady, clearly different one at boot means new switches: the LCD asks for a recalibration and the defaults apply until then. |
| 9 | Supply reference | Adaptive: the pad watches its own supply noise and uses the ID-line correction only when it is needed. No user setting. |
| — | IRAM | The HE key path runs from IRAM like MX's; only the fitted module's path runs (§2). |

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
| A keyboard with no software | **An HE pad must type out of the box, before any calibration:** rest is measured at boot and a bench-measured default span applies until the explicit calibration runs (§4). |
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
  as today (open question, §10.3).
- **IRAM.** The hall task, the travel conversion and the engine join
  `firmware/main/linker.lf`, like the key path did in `1a3c038`.
  - *Can the MX path be unloaded when HE is fitted?* Not usefully. IRAM
    placement is fixed when the firmware is linked, not at runtime. Copying
    code into executable heap at boot would mean turning off the S3's
    memory protection (`CONFIG_ESP_SYSTEM_MEMPROT_FEATURE`) and fighting
    Xtensa literal pools, all to free about **2.5 KB**: what the MX-only
    code takes in IRAM today (`keypad.c` 2488 B + `board.c` key reads 64 B,
    from `build/opad-firmware.map`; the USB part, about 6.5 KB, serves both
    modules). IRAM in use is about 86 KB of the S3's 512 KB internal SRAM.
  - What matters for latency is that the **unused backend does not run**:
    with HE fitted, no key GPIO interrupt, no keypad task wake-ups, and the
    pins left analog. With MX fitted, the ADC is never started and the hall
    task never created. Both paths stay resident in IRAM, so whichever runs
    never waits on a flash cache miss.

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
- **Rate and frames.** Target ≥ 4 kHz per key. The ID line changes only as
  fast as the supply, so it does not need an equal share: the S3's pattern
  table holds 24 entries (`SOC_ADC_PATT_LEN_MAX`), so a pattern of
  K1,K2 × 11 then ID × 2 gives the keys ~92 % of the conversions. Sketch:
  ~30 kHz total, a frame per DMA interrupt of ~0.25 ms, each frame averaged
  per channel (oversampling instead of a slow filter, so no added delay
  beyond the frame). The DMA callback only notifies the hall task. Wake rate
  ~4 kHz on core 0, to check against the USB task's budget.
- **If the bench shows the supply is quiet enough,** the ID channel is
  dropped and the keys get every conversion (latency first).
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
- **Phase 1: explicit calibration (decision 3).** Rule 2 still holds: an
  uncalibrated HE pad types.
  - `rest`: averaged at every boot while both readings are steady. It is
    accepted only if it is close to the stored one, so a key held down
    while plugging in cannot poison it; otherwise the stored value is used.
    This part is automatic from the start: it is trivial and safe.
  - `bottom` and sign: set by an **explicit calibration** ("leave both keys
    up", then "press each key fully 5 times"), started from the app or
    `opadctl calibrate`, with live travel on the LCD while it runs
    (decision 7). Saved to NVS, in IDLE only.
  - Before the first calibration: the default span for Gateron Jade Silent
    measured on the bench, so a new pad types at once.
- **Phase 2: automatic calibration,** once phase 1 is proven on the pad:
  `rest` tracked slowly while a key is up and still (temperature drift),
  `bottom` refined from the deepest presses seen. Never while pressed or in
  PLAYING; NVS writes only in IDLE.
- **Silent switches.** The Jade Silent's damper compresses, so "bottomed
  out" is not one reading: it depends on how hard the key is hit. The
  calibration takes the typical deepest reading of the 5 presses, not the
  single deepest, and the bench measures how much it moves between a soft
  and a hard press.
- **Resolution worry.** A3 is the least sensitive variant (≈16 mV/mT, README
  "Sensor sensitivity"), and the sensor reads through the board. Because the
  field rises steeply only near the bottom, resolution near the top of the
  travel may be too coarse for 0.15 mm sensitivity there. The bench decides
  whether A3 is enough or the next batch should be A2 (Q2).

## 5. Rapid trigger

The state machine in v2-rapid-trigger §3.3 as written (actuation point,
press and release sensitivity, top and bottom dead zones, continuous mode),
in pure C with host tests in `firmware/test/host`, plus replays of bench
traces.

- **Two modes per key:** plain (fixed actuation point, release a little above
  it as hysteresis) and rapid trigger.
- **Default: plain, at about mid-travel, like an MX switch (decision 4).**
  Rapid trigger is off until turned on in the app. (A threshold at the very
  bottom would not work with the silent damper (§4): a soft press would
  never reach it.)
- **Settings per key, press and release sensitivity separate (decision 5).**
  The two sensors and magnets never match exactly, so per-key values are the
  accurate choice. The app may offer a "link" toggle as a convenience; the
  firmware always stores four values.

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

- **Rapid Trigger page**, shown only when `input_module` is HE: mode
  (plain / rapid trigger) per key, actuation point, press and release
  sensitivity, continuous toggle, live travel bars with the markers,
  calibration wizard.
- **LCD:** live travel only during calibration and the HE board test
  (decision 7).
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
| 3 | Rapid-trigger engine (plain + RT) + host tests | no |
| 4 | Key event sink refactor (MX behaves byte-for-byte as today, latency rechecked) | quick MX check |
| 5 | Hall backend in IRAM: sampling, rest at boot, default span → **HE pad types, plain at bottom-out** | yes |
| 6 | Explicit calibration + protocol + daemon + app page + HE board test | yes |
| 7 | Latency and tapping validation, docs | yes |
| 8 | Automatic calibration (phase 2) | yes |

## 10. Bench result: the fitted sensors do not sense (2026-10-10)

Bench build on this pad, readings in raw 12-bit counts at 12 dB:

| Line | Reading | Meaning |
|---|---|---|
| ID (GPIO8) | 1211 | 1.06 V: the ADC and the module's 3V3/GND are fine |
| GPIO6 probe pad, internal pull-up | 4095 | 3.3 V is above full scale |
| IN1 / IN2, keys up | ~3535 both | ≈ 3.06 V = the DRV5055's top limit (VL max = VCC − 0.2 V) |
| IN1 / IN2, keys pressed fully | unchanged | no response to the magnet |
| IN2 with the **switch removed** | unchanged | not the magnet: no field, still at the top |
| IN1/IN2 with internal pull-down / pull-up | −12 / +2 counts | the line is actively driven, not open |

A working DRV5055 outputs VCC/2 ≈ 1.65 V with no field. The datasheet's
fault table (§7.1.4) gives "GND disconnects → output close to VCC", which
fits. The design is not the cause: pinout matches the datasheet (SOT-23:
1 VCC, 2 OUT, 3 GND), KiCad DRC finds no unconnected pad, the switch
footprint has a solid centre, and the firmware never drives these lines
(board test uses pulls only). Suspects, both sensors alike: sensor GND
pin (pin 3) not soldered, wrong part on the reel, or wrong placement.
Next: multimeter on U1/U2 (pin 1 = 3.3 V, pin 3 = 0 V and continuous to
module GND, pin 2 = ~1.65 V with no magnet), and a look at the parts.

Follow-up the same day, with a multimeter on the chip legs: VCC 3.36 V,
GND 0.0 V (and continuous to the connector's ground tabs), OUT 2.90 V,
on both sensors. Ruled out since: soldering, a second HE board (reads the
same), the carrier (connectors only), the Waveshare side (GPIO10/GPIO7
also go to the camera connector as CAM_D5/CAM_D6, with no parts on them,
and no camera is fitted), a latch-up (a 10 s USB unplug changes nothing,
no chip is warm), and a fake part (marking reads "55A3", TI's code for
DRV5055A3). A switch magnet held directly on a chip moves its output by
only ~20–30 mV, upwards only. Datasheet VQ at 3.3 V is 1.59–1.71 V. All
four chips from the one JLCPCB order sit saturated high: the remaining
suspect is the batch (damaged in assembly or defective). Next: a JLCPCB
claim, and two or three DRV5055A3 from an authorised distributor fitted
by hand to confirm.

Also found: the daemon's config push on every connect re-arms the key pins
and stopped the ADC conversions until restarted. The HE backend must keep
host config away from the HE pins (step 1).

## 11. Still open

1. Why both sensors sit at the top limit (§10). Nothing else on the HE
   side can be measured until a working sensor reads ~1.65 V at rest.
2. Sensor variant (A3 or other): decided by the bench once sensors work.
