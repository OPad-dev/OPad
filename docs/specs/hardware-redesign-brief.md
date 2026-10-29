# Hardware redesign brief: every board, with what V1 taught us

**Written:** 2026-10-23, at the end of the HE phantom-press investigation
and the HE V2 design. **Purpose:** the starting point for a full redesign of
all OPad boards (carrier, MX module, HE module, case), so a new session can
start from this file alone. Read it top to bottom before touching a board.

Facts carry their source (a file in this repo, a datasheet, or "owner,
in chat" for what the owner reported and is not recorded anywhere else).
Anything not yet decided is marked **Open**. Proposals are marked as such:
none of them has been agreed unless it says **Decided**.

---

## 1. What exists today

### 1.1 Boards

| Board | Folder | Size | State |
|---|---|---|---|
| Controller carrier V1 | `hardware/pcb/V1/Carrier/` | 35.0 × 48.2 mm, C-shaped | Built, working |
| MX input module V1 | `hardware/pcb/V1/MX/` | 52.0 × 24.0 mm, 2 keys | Built, working |
| HE input module V1.0 | (superseded) | 52.0 × 24.0 mm | **Erratum:** mirrored SOT-23 footprint, sensors powered backwards |
| HE input module V1.1 | `hardware/pcb/V1/HE/` | 52.0 × 24.0 mm, 2 keys | Built (the owner has 10), works, **noisy** (§3) |
| HE input module V2 | `hardware/pcb/V2/HE/` | 71.05 × 24.0 mm, **3 keys**, ADC on module | Designed, routed, exported 2026-10-23. **Not ordered, not built, no firmware** |
| Case V1 | `hardware/3d/custom_case/V1/` | 20.3° screen | Does not fit the V1 PCBs |
| Case V1.1 | `hardware/3d/custom_case/V1.1/` | 76 × 82.5 mm | Fits carrier + 2-key module. **Does not fit HE V2** |

Full details of V1: `hardware/pcb/V1/README.md`. Of HE V2:
`hardware/pcb/V2/README.md`. Of the cases: `hardware/3d/README.md`,
`hardware/3d/custom_case/V1.1/README.md`.

### 1.2 The system as wired today

```
PC ──USB-C──> Waveshare ESP32-S3-Touch-LCD-2 (dev board: ESP32-S3, 2" LCD + backlight, touch, IMU, PSRAM)
                 │ two 1×14 male headers (P1, P2)
                 ▼
              Carrier (two 1×14 8.5 mm sockets, J_MOD JST-SH 8-pin, no other parts)
                 │ 100 mm JST-SH 8-pin cable, same-direction (pin 1 ↔ pin 1)
                 ▼
              Input module (MX or HE), J1 JST-SH 8-pin, under a 1.5 mm switch plate
```

- The carrier has **no components on any signal**; it only routes header pins
  to J_MOD (`hardware/pcb/V1/README.md`, Controller carrier).
- The input module is swappable on one cable and identifies itself by an ID
  voltage divider (§1.4).
- The firmware picks the input backend at boot from the module ID; a module
  change while powered asks for a restart
  (`firmware/main/input/module_monitor.c`, `docs/specs/v2-hall-effect-decisions.md` D1).

### 1.3 Module connector (JST SH 8-pin, every module)

| Pin | Net | Waveshare pin | GPIO | MX V1 | HE V1.1 | HE V2 |
|---|---|---|---|---|---|---|
| 1 | 3V3 | P2-1 | – | pull-ups, ID divider | sensor supply | 3V3 → ferrite → AVDD; ADC DVDD |
| 2 | GND | P2-2 | – | switch common | ground | ground |
| 3 | IN1 | P1-10 | GPIO10 (ADC1_CH9) | KEY1 | analog key 1 | SPI CS |
| 4 | IN2 | P1-9 | GPIO7 (ADC1_CH6) | KEY2 | analog key 2 | spare (TP1) |
| 5 | ID | P1-8 | GPIO8 (ADC1_CH7) | 0.30 V | 1.06 V | 1.65 V |
| 6 | IO6 | P1-3 | GPIO6 (ADC1_CH5) | unused | probe pad | SPI MISO (via 33 Ω) |
| 7 | IO4 | P1-2 | GPIO4 (ADC1_CH3) | unused | probe pad | SPI MOSI |
| 8 | IO2 | P1-1 | GPIO2 (ADC1_CH1) | unused | probe pad | SPI SCLK |

Sources: `hardware/pcb/V1/README.md` (Module connector), `hardware/pcb/V2/README.md`.

- All six signals are ADC1 pins: ESP32-S3 continuous (DMA) mode only works on ADC1.
- None are strapping, USB, UART0 or touch/IMU I2C pins.
- **Only one GND wire** in the cable. Every pin is now used by HE V2 except pin 4.
- Many cheap JST "reverse" cables swap pin 1 ↔ 8; the board test catches it (§1.7).

### 1.4 Module ID

| Module | Divider (top / bottom) | ID voltage |
|---|---|---|
| MX V1 | 100 kΩ / 10 kΩ | 0.30 V |
| HE V1.x | 100 kΩ / 47 kΩ | 1.06 V |
| HE V2 (was "reserved: Hall SPI") | 100 kΩ / 100 kΩ | 1.65 V |

Firmware classification today (`firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c`,
`board_module_from_id`): loaded < 120 mV → none; open < 650 mV → MX;
open < 1600 mV → HE (analog); anything higher → none.

**Known issue:** 1.65 V is only 50 mV above the 1600 mV HE cut-off. A V2
module reading slightly low would be treated as an analog V1.1 module (wrong
pins read as analog; nothing is damaged). The V2 driver must move the cut-off
to about halfway between 1.06 and 1.65 V (~1.35 V) and add a V2 class.

### 1.5 Firmware facts that constrain hardware

- **HE input is off in a normal build:** `CONFIG_OSUPAD_HE_BACKEND`, "experimental",
  `default n` (`firmware/main/Kconfig.projbuild:35`). Only `sdkconfig.hedebug`
  turns it on (with the debug log). **Open:** make one firmware that serves
  every module by ID.
- **ADC sampling (HE V1.1):** ADC1 continuous DMA, 64 000 conversions/s,
  keys alternate, 16 conversions per 250 µs frame (8 per key), 12-bit, 12 dB
  (D1 in `docs/specs/v2-hall-effect-decisions.md`). The S3 allows up to
  83 333/s.
- **Median filter (since 2026-10-22, merged to main as `f146fb0`):** each
  key uses the median of its valid conversions in the frame
  (`firmware/main/input/he_frame.c`), ~0.11 ms average added delay. This
  departs from D2 ("no filtering on the key path") at the owner's request.
- **Decision engine:** `he_rt.c` (ARMED / PRESSED / RT_RELEASED), thresholds in
  0.1 % of each key's calibrated span, integer-only on the key path (D3, D4).
  Bottom dead zone caps the peak (`peak_cap`), which stops spikes while held
  from causing double presses.
- **Keypress path:** frame interrupt on core 0 in IRAM steps the engine; it wakes
  `he_task` only on a change; USB HID latest-state delivery (D1, D6).
- **RAM is tight:** static DIRAM was 91.9 % with HE (`docs/history/2026-10-20-he-night-report.md`)
  and 93.4 % later (D1). On 2026-10-22, ~800 B more in the debug build made LCD
  init fail (static lines on screen); halving a ring buffer fixed it (owner, in chat).
  A V2 driver must not grow internal RAM carelessly.
- **The pad and the key path are 2-key everywhere today:** keypad, HID
  report, protocol, app, board test. A 3-key module needs all of them changed
  (§6).

### 1.6 Product rules that bind the hardware (AGENTS.md §1)

1. **Latency always wins.** Nothing joins the keypress path. Any filter or
   bus on the key path must state its latency cost.
2. **A keyboard with no software:** 1000 Hz HID, no driver. An HE pad must type
   out of the box.
3. **App and pad update independently** (protocol both ways).
4. **The touchscreen is the third button** (press, hold, release).
5. **Never the BOOT or RESET buttons:** flashing and recovery over USB only.
   A redesign must not create a board that needs them.
6. **Every key comes from a finger:** no macros or host-driven presses.

### 1.7 Board test

`opadctl board-test` / Diagnostics → PCB Test (`firmware/main/diag/board_test.c`,
judged in `desktop/crates/opad-model/src/board_test.rs`) checks: module ID,
cable direction (pulls GPIO2 up), key pins = GPIO10/7, each key line held
high by the module's 10 kΩ pull-up, and that connector pins 6–8 follow the pad's pulls
(no solder bridges). It is written for the MX module. HE V2 turns
pins 6–8 into SPI, so the spare-line check must change.

---

## 2. Requirements for the redesign

From the owner, 2026-10-22/23, in chat:

- **"This needs to be a finished product."** The highest priority is
  **solving the noise**. Better components are welcome, "a mix of best
  components but not super pricey".
- **Everything goes in the design** (no external ferrites or add-ons a user has to fit).
- **Changes must not break the MX board.**
- **Hot-swap that fits any switch** (any Lekker-standard magnetic switch,
  including magnets with either pole down).
- **A 3-key variant** for players who do not use the middle finger
  (V2 HE is already 3 keys). **Open:** does the MX module get a 3-key
  version too, and are both 2- and 3-key versions kept?
- Prototyping: when a new revision works, the old one is **deprecated
  immediately**; no long-term support for V1.1 hardware.
- Order through **JLCPCB** with assembly (BOM + CPL), as V1.

---

## 3. What we learned: the HE noise problem

Full record: `docs/history/2026-10-22-he-phantom-presses.md` (log in
`docs/history/2026-10-22-he-phantom-presses/he-debug.log.gz`). Summary plus
the parts that happened after that document was last edited.

### 3.1 Two different kinds of noise

**Kind 1: constant fuzz.** ±30–40 counts over a second at rest on every
channel (12-bit ADC counts, 12 dB). The module ID line, a plain 100k/47k
divider with no sensor on it, showed the same amount. The key fuzz and the
ID fuzz **do not move together** (correlation +0.08, measured from the debug
firmware's ID-channel log, owner's pad, 2026-10-22 evening). So the fuzz is
mostly **the ESP32-S3 ADC's own noise**, independent per channel, not
supply or ground noise. Sensor gain or supply filtering cannot fix it;
only a better measurement can. It sets the app's lowest usable actuation
(~20 % today) and rapid-trigger sensitivity.

**Kind 2: phantom spikes.** One reading jumps +61 to +379 counts above
rest, lasting 1 frame (250 µs, most of them) up to ~2.75 ms, always upwards,
often on both keys in the same frame, in bursts of seconds to minutes. Not
mechanical (a finger moves ≤ ~50–60 counts per frame). They caused presses
at rest (1 ms presses) and, while held, double presses (a spike becomes the
"deepest point", the fall back looks like a lift; fixed by the bottom dead
zone). ~2900 phantoms over 7 boots on 2026-10-22 (history doc).

### 3.2 What was ruled out

From the history doc: actuation setting alone, result animation, LCD clock,
GUI redraws, the live-view stream, the front-panel USB port, the touch
controller, backlight PWM (now solid-on at 100 %, kept), the module monitor,
heat, and host→pad serial traffic (`scripts/he_phantom_probe.py`).

### 3.3 What was observed but not proven

- **154.0–154.1 ms beat** in every log: a crystal-grade ~1 kHz source about
  −20 ppm off the pad's clock, which is neither the pad's clock (would beat
  at 156.0 ms) nor the USB frame clock (154.67 ms). Hypothesis (not tested):
  the PC's 1 kHz kernel timer making CPU/GPU current pulses that reach the pad
  over USB.
- **osu! frame limiter → VSync** at 22:42 was followed by ~150 phantoms in
  under 2 minutes with the pad idle, mostly both keys, +200 to +290 counts.
  Seen once, never repeated.
- **No phantoms in 76 minutes untouched**; bursts came with playing.
- **Another HE V1.1 board + a USB hub** (22:45): clean afterwards.
- **Owner's 4-step test** (new/old board × hub/direct, after 22:45; owner, in
  chat): **no phantoms in any step**, and the frame-limiter switch did not trigger
  any. Wiggling the module cable produced nothing either.
- **23:40 boot** (debug log): both keys sat −116 counts for ~40 s while the
  ID line stayed normal. Suspected a magnet or magnetic object near the pad;
  the owner was asked and has not answered.

**Conclusion used for the redesign:** the root cause of the spikes was never
pinned down. It came and went with the PC's state, the board, the hub and
time. The redesign therefore must **not depend on knowing the source**: the
measurement has to be immune to anything on the cable and the 3V3 rail.

### 3.4 Numbers that matter for settings

- **Phantoms from rest vs actuation depth** (6 751 phantoms over two days'
  logs, counts above rest): +150 blocks 67 %, +200 blocks 91 %, +250 98 %,
  +300 99 % (owner's logs, analysed in chat 2026-10-22). Only spikes large
  enough to press at the old settings were recorded.
- **Switch spans:** the original switches went ~0 → +450–510 counts at full
  press; the switches the owner fitted on 2026-10-22 reach ~+650 (owner's log).
  Magnet strength varies per switch, up to ~100 mV between pairs (design
  doc §10b). Recalibrate after every switch swap.
- **V1.1 bench:** rest ~1.80 V, +568 mV at hard bottom-out (≈ 2.37 V), A3,
  through 1.6 mm FR-4 (`docs/specs/v2-hall-effect-design.md` §10b).

### 3.5 What commercial boards allow (researched 2026-10-22)

Typical magnetic switch travel ~4 mm (1 % ≈ 0.04 mm).

| Brand | Actuation | RT sensitivity | Source |
|---|---|---|---|
| Wooting | down to 0.1 mm | down to 0.1 mm | [help.wooting.io](https://help.wooting.io/article/102-how-to-set-up-rapid-trigger) |
| Wooting (Valorant guide) | 0.2–0.5 mm for movement keys | – | [wooting.io](https://wooting.io/post/wooting-valorant-guide-in-game-benefits-settings-and-pro-profiles) |
| Logitech G515 Rapid | down to 0.1 mm | – | [support.logi.com](https://support.logi.com/hc/en-ph/articles/34257266624919-What-are-Analog-magnetic-key-switches-and-how-are-they-used-on-the-G515-RAPID-TKL-Keyboard) |
| MonsGeek (blog, rhythm games) | ~0.2 mm | ~0.1 mm reset | [monsgeek.com](https://monsgeek.com/blog/what-is-rapid-trigger-keyboard) |

The owner ran 20 % actuation (~0.8 mm) and 50 % sensitivity (~2 mm) on
V1.1. **Target for the redesign:** clean enough that 0.1–0.3 mm (2.5–7 %)
works without phantoms, like the boards above.

### 3.6 Why Wooting / Sayo are clean (reasoning; their schematics are not public)

| | Commercial HE board | OPad V1 |
|---|---|---|
| Layout | Sensors, ADC and MCU on **one PCB**, solid ground plane, short traces | Module → cable (1 GND wire) → carrier → dev board |
| Power | Board built only to be a keyboard | Dev board sharing 3V3 with a 2" LCD, backlight, PSRAM |
| USB input | Ferrites, capacitors, ESD on VBUS | Whatever the Waveshare dev board has |
| ADC | Cleaner MCU ADC or a separate ADC chip | ESP32-S3 ADC (Espressif's docs ask for capacitors and multisampling) |
| Firmware | Heavy averaging, per-key calibration | Median of 8 per 250 µs, per-key calibration |

### 3.7 The Waveshare's 3V3 is a switching regulator (found 2026-10-23)

From the Waveshare schematic (`ESP32-S3-Touch-LCD-2-SchDoc.pdf`, AGENTS.md §6):

- USB-C H1 VBUS **is** the `5V` net, with no diode or filter; header P1-14 is
  that same net.
- `5V` → Schottky D1 (MBR230LSFT1G) → `VCC`; the battery reaches `VCC`
  through PMOS Q1 (AO3401), which turns off while `5V` is present.
- `VCC` → U7 **TMI3112H buck converter** (2.2 µH, 22 µF out) → `3V3`.
- USB D+/D− reach GPIO20/19 through 22 Ω (R23, R26); the header pins
  P1-11/12 are GPIO20/19 on the MCU side of those resistors.

So the 3V3 that powered the V1.1 sensors, and that HE V2 still uses for
AVDD, is the output of a switching converter shared with the LCD,
backlight and PSRAM. Buck converters at light load usually switch in
bursts whose rate follows the load (not checked for the TMI3112H). This
is a plausible source of the load-dependent spikes in §3.1, **not proven**.
It is why the redesign powers the measurement from 5 V through its own
LDO (H11).

---

## 4. Decisions already made (owner, 2026-10-22/23)

| # | Decision | Why |
|---|---|---|
| H1 | **Option 1: the ADC goes on the HE module**; only digital SPI crosses the cable. **Decided.** | Removes both noise kinds without knowing the spike source. Option 2 (better analog module) keeps the ESP32 ADC fuzz and still sends analog over the cable. |
| H2 | **Sensor: keep the DRV5055A3** (bipolar). **Decided.** | Reads either magnet pole (a flipped magnet reads downwards, calibration can detect it); ±88 mT range never saturates with strong magnets; proven on V1.1; same SOT-23. |
| H3 | **No DRV5056** (unipolar). | Reads only one pole: a switch with its magnet flipped reads nothing. Its 2× signal does not matter once the ADC is good, because the sensor's own magnetic noise (0.2 mT p-p for every variant) is then the limit. |
| H4 | **No TMR sensors.** | Saturate with strong magnets (Jade/Lekker ~70–80 mT at bottom-out per Gateron, less through the PCB); usually off-axis placement tuned to one switch; 5-pin packages (TMR2001, TMR2083 are SOT-23-5); fewer, pricier parts at JLCPCB. "TMR switches" are ordinary magnetic switches and work on Hall boards. |
| H5 | **No LDO for the sensors.** | DRV5055 needs VCC 3.0–3.63 V (TI SBAS640C); the module gets 3.3 V, so an LDO would push the sensors out of spec. A ratiometric ADC + ferrite filter does the job instead. (A 5 V-fed LDO would need 5 V on the cable: a carrier change.) |
| H6 | **No filtering on the carrier.** | The MX module's keys use 10 kΩ pull-ups on the module; an RC (e.g. 100 nF) on the carrier would slow MX edges by ~1 ms. Filtering belongs on the HE module. |
| H7 | **ADC: TI ADS7038** (8-ch, 12-bit SAR, 1 MSPS, SPI, on-chip averaging to 16-bit, AVDD is the reference → ratiometric). | One 3 × 3 mm chip for all keys, averaging off the ESP32, ~$3.70–5.30. Cheaper fallback considered: Microchip MCP3202 (2-ch, no averaging; ESP32 would average; LCSC not checked). Rejected: 24-bit precision ADCs (sensor noise is the limit); Allegro A1324–6 (5 V only); TMAG5273 digital 3D Hall (too slow, shared bus). |
| H8 | **Carrier and MX module unchanged by HE V2.** | Constraint from the owner. HE V2 uses the reserved pins and ID 1.65 V. |
| H9 | **Optional ESD diodes left out of HE V2** for simplicity. **Open for the redesign.** | Modules are plugged by hand. |

### 4.1 Second round (owner, 2026-10-23, after the review of HE V2)

| # | Decision | Why |
|---|---|---|
| H10 | **Module cable: FFC (flat flex) with ZIF connectors** replaces the JST-SH 8-pin. **Decided.** | The 8-pin JST-SH cable is hard to find; FFC cables and connectors are cheap and everywhere and fit the ~3.5 mm under the module. |
| H11 | **USB: the best option** (owner: "whatever is the best"): the measurement is powered from 5 V through an LDO on the HE module (not from the Waveshare's buck 3V3, §3.7), and the carrier gets its own filtered USB-C input (ESD, VBUS filter, reverse-feed protection). No galvanic isolator (cost, its own DC-DC noise, risk to the USB detach used for flashing and recovery). **Decided.** | Keeps USB and buck noise off the sensors and the ADC reference without depending on knowing the spike source. |
| H12 | **USB-C moves to the back** of the pad (the Waveshare's own port is on its short side today). **Decided**; exact position follows the new case. | Owner. |
| H13 | **3 keys only, for both MX and HE.** No 2-key variants. **Decided.** | Owner: the 2-key layout was a mistake. |
| H14 | **Nothing is ordered until the design has been examined exhaustively and simulated** (static checks against the datasheets, SPICE of the power, analog and SPI paths, a magnet-to-key digital twin, an ADS7038 emulator for the firmware). The checks must catch the V1.0 mirrored-sensor error when run on V1.0. **Decided.** | The ADC is the expensive part; V1.0 shipped with an error no check caught. |
| H15 | HE V2 (`hardware/pcb/V2/HE/`) is **not ordered**; it is superseded by the redesign and used to validate the checks. | Follows from H10, H11, H14. |
| H16 | **Four** locating holes on each input module, not two, for the case's pins. **Decided.** | Owner. |
| H17 | "Emulation" means proving **every trace** right: a checker that rebuilds connectivity from the fab files and compares it with a hand-written intended netlist and the datasheet pin drawings (`hardware/verify/`). No magnet model. **Decided.** | Owner: V1 failed on the traces, not on magnetics. |
| H18 | **The USB-C is on its own board at the rear wall,** linked to the carrier by an 8-way FFC; the module keeps its 14-way FFC. This replaces the 2026-10-23 choice of one FFC and the USB-C on a carrier tab. **Decided** (owner, 2026-10-28). | The case wall takes the plug's push, not the header sockets; the port can sit anywhere on the rear wall; the carrier loses the 14.3 mm tab. |

### 4.2 Architecture of the new board set (2026-10-23, building on 4.1)

| Board | What it does |
|---|---|
| **USB board** (H18) | Lies flat at the case's rear wall, **USB-C** (HRO TYPE-C-31-M-12, C165948) mouth on its rear edge; two M2 holes for the case's pins, which take the plug's push. CC1/CC2 5.1 kΩ; SRV05-4 ESD on D+/D− with REF on VBUS; 2 A ferrite (BLM18PG121SN1D, C14709) between 100 nF and 4.7 µF on VBUS; B5817W Schottky (C7420328; JLCPCB's model: pad 1 anode, pad 2 cathode) so a host on the Waveshare's own port cannot feed this one; shield to GND through 1 MΩ ∥ 4.7 nF. 8-pin FFC to the carrier. 20 × 22 mm. |
| **Carrier** | V1's outline and headers, plus a lug in the window's USB-end rear corner for the 8-pin USB FFC (mouth on the USB-end edge; P1's sockets fill the rear edge). USB FFC: 5V → P1-14, D+ → P1-11, D− → P1-12. 14-pin FFC to the module at the non-USB end. 33 Ω series on the four SPI-capable lines, SRV05-4 ESD on the six module signals, ferrite + capacitors on the 5 V to the module, bulk on 3V3, all three Waveshare GND pins tied. |
| **MX module** | 3 Kailh sockets, 10 kΩ pull-ups, ID 47k/100k = 2.24 V, 4 corner holes, 14-pin FFC. |
| **HE module** | HE V2 plus: LP5907-3.3 LDO (C80670) from the cable's 5 V for the sensors and ADC AVDD (1 µF in, 1 µF out; AVDD total kept under the LDO's 10 µF limit); DVDD on 3V3; 100 kΩ CS pull-up; SRV05-4 ESD on the SPI lines; ID 100k/100k = 1.65 V (the V2 class: same ADC, same pins); 4 corner holes; 14-pin FFC. |

**FFC links, both reversible:**

- Module, 14-pin, 1.0 mm (JUSHUO AFA07-S14FCA-00, C262720, 2.5 mm tall, bottom contact):
  `1 GND | 2 5V | 3 3V3 | 4 ID (GPIO8) | 5 MISO (GPIO6) | 6 GND | 7 SCLK (GPIO2) | 8 MOSI (GPIO4) | 9 GND | 10 CS (GPIO10) | 11 SPARE (GPIO7) | 12 3V3 | 13 5V | 14 GND`.
  A reversed (type B) cable maps pin k to 15 − k: power and ground land on power and ground;
  ID ↔ GPIO7 (also ADC1), MISO ↔ CS, SCLK ↔ MOSI. The firmware finds the ID on GPIO8 or GPIO7
  and swaps the SPI pins in the GPIO matrix, so either cable type works.
  MX uses CS = KEY1 (GPIO10, as V1), SPARE = KEY2 (GPIO7, as V1), MISO = KEY3 (GPIO6).
- USB, 8-pin, 1.0 mm (JUSHUO AFA07-S08FCA-00, C262714):
  `1 GND | 2 5V | 3 D− | 4 D+ | 5 D+ | 6 D− | 7 5V | 8 GND`, the same read from either end;
  D+ and D− each use two conductors in parallel, joined on both boards. The two 5V conductors
  carry 1 A together (0.5 A per contact, connector datasheet). Different widths on purpose:
  the module cable cannot go into the USB connector.

**Module IDs:** MX V1 0.30 V, HE V1 1.06 V, HE SPI (V2 and the new HE) 1.65 V, new MX 2.24 V.
Firmware cut-offs halfway between: 0.68, 1.35, 1.95, 2.6 V.

**JLCPCB placement:** the CPL is computed so that JLCPCB's own footprint of each part
lands on our pads (`hardware/verify/opadverify/jlc.py`), with polarised bottom-side
parts only at 0° or 180°.

Findings from the review of HE V2 that the redesign must fix: CS has no
pull-up (the ADC sees a floating CS during boot and plugging); SCLK/MOSI/CS
have no series resistor at the driver and only one GND wire; the ADS7038's
CRC must be used by the driver so a corrupted SPI frame is dropped, not
read as a key; the ID (1.65 V) is 50 mV from a cut-off; no ESD protection;
AVDD comes from the Waveshare's buck 3V3 (§3.7).

---

## 5. HE module V2 as designed (2026-10-23)

Everything here is in `hardware/pcb/V2/README.md` and
`hardware/pcb/V2/scripts/generate_he_v2.py`; repeated so the redesign can
start from it.

### 5.1 Circuit

- **U1–U3** DRV5055A3QDBZR (C266128), SOT-23, bottom side, on the key centres,
  180°, 3 × 3 mm copper keepout on both layers around each.
- **U4** ADS7038, WQFN-16 3 × 3 mm (RTE0016C: pads 0.6 × 0.24 mm, 0.5 mm pitch,
  pad centres ±1.4 mm, exposed pad 1.68 mm square, paste 1.55 mm).
  Pinout: AIN2–5 = pins 1–4, AIN6/7 = 5/6, AVDD 7, DECAP 8, GND 9, DVDD 10,
  CS 11, SDO 12, SCLK 13, SDI 14, AIN0 15, AIN1 16, EP 17 (GND).
  Needs 1 µF on each of AVDD, DVDD, DECAP. Sampling cap CSH 12 pF, RSW 150 Ω.
  BOM uses **ADS7038QRTERQ1** (C2876384, automotive grade, same die and
  pinout, 1 869 in stock and cheaper than the IRTER on 2026-10-23;
  ADS7038IRTER C2871580 had 8 in stock).
- **Channels:** key 1 = AIN5, key 2 = AIN4, key 3 = AIN3. AIN0–2, 6, 7 unused.
- **Supply:** 3V3 → FB1 (Sunlord GZ1608D601TF, 600 Ω @ 100 MHz, C1002) →
  AVDD rail with C2 10 µF (C19702) → sensors + ADC AVDD (C3 1 µF). DVDD from
  3V3 direct (C5 1 µF), DECAP C4 1 µF (C15849). C1 100 nF at J1. 100 nF at
  each sensor (C6–C8, C14663; TI asks ≥ 10 nF).
- **Per-sensor output filter:** 220 Ω series (R5–R7, C22962) then 10 nF to GND
  (C9–C11, C57112) at the ADC. **TI forbids a capacitor directly on the
  DRV5055 output; the resistor must come first** (datasheet §7.1.3). τ = 2.2 µs.
  The 10 nF also feeds the ADC's 12 pF sampling capacitor.
- **SPI:** R3 33 Ω (C23140) in series on SDO → MISO, for the 100 mm cable.
- **ID:** R1/R2 100 kΩ (C25803) → 1.65 V.
- **J1:** JST SM08B-SRSS-TB (C160407). TP1: bare probe pad on pin 4 (not in BOM/CPL).

### 5.2 Mechanics

- 71.05 × 24.0 mm, 1.5 mm corners, 1.6 mm FR-4, 2 layers, everything on the
  bottom (one-side assembly). Keys at 19.05 mm pitch; KiCad coordinates
  (128.475, 83.5), (147.525, 83.5), (166.575, 83.5); 16.475 mm from each outer
  key centre to the board edge (as V1).
- Peg holes 1.75 mm at ±5.08 mm per key (`SW_MX_Hall_PlateMount_SolidCentre`,
  no centre hole; the Lekker standard).
- M2 holes (2.2 mm NPTH) at (115.5, 83.5) and (179.55, 83.5), on the key axis.
- J1 centred on the middle key at the rear edge, at the V1 offset, so the
  100 mm cable still reaches.
- Silkscreen "OPad HE module V2.0", KEY1–KEY3, `JLCJLCJLCJLC` on the bottom.

### 5.3 Layout and rules

- Rules: 0.2 mm track, 0.15 mm clearance (0.25/0.2 could not reach the QFN
  pads). Vias 0.6 mm / 0.3 mm drill.
- GND pours on both layers; 4 GND vias in the exposed pad; 26 stitching vias
  (the top pour was removed as an island without them).
- ERC 0, DRC 0 violations and 0 unconnected with zones refilled, schematic
  parity clean (2026-10-23).
- `check_orientation()` asserts the SOT-23 and J1 pads match the built V1.1
  (guard against another V1.0-style mirrored sensor).

### 5.4 JLCPCB placement lesson

JLCPCB's preview showed U4's pin-1 marker (magenta dot) in the corner
diagonally opposite the silkscreen dot. Fixed by adding 180° for the WQFN
footprint in the CPL (`JLC_CPL_FIXES` in `hardware/pcb/V2/scripts/export_v2.py`);
the CPL now says `U4 … Bottom, 180.0`. The SOT-23s were correct (magenta and
white dots on the same corner, as on the working V1.1). **Always compare the
magenta dot with the silkscreen pin-1 dot in the preview.** V1's carrier
needed a similar fix for the pin sockets (90°).

### 5.5 Toolchain (and its traps)

```
python3 hardware/pcb/V2/scripts/generate_he_v2.py   # library, schematic, placed board (overwrites)
python3 hardware/pcb/V2/scripts/route_he_v2.py      # Freerouting 1.9.0, stitching vias, zone refill
python3 hardware/pcb/V2/scripts/export_v2.py        # ERC+DRC+parity, then fab files (reuses V1 export_production.py)
```

- KiCad 10.0.6, `pcbnew` Python module (Python 3.14).
- **pcbnew under Python 3.14:** iterating `Tracks()` / `GraphicalItems()`
  fails (`'SwigPyIterator' object has no attribute 'next'`); parse the
  `.kicad_pcb` text instead. Loading a **second board in one process** fails
  (`SwigPyObject has no attribute Zones/Save`): one subprocess per step.
- **Freerouting:** route a copy **without zones and rule areas** (it treats
  them as obstacles). Version **2.1.0 routes completely, then hangs without
  writing**: use **1.9.0** (`FREEROUTING_JAR`, default
  `~/.kicad-mcp/freerouting-1.9.0.jar`), single-threaded (`-mt 1`).
- The KiCad MCP `autoroute` tool crashed on the same iterator bug.
- V1's HE board used a separate pure-Python generator (`he_board.py`, no
  KiCad); V2 uses pcbnew like MX and the carrier.

### 5.6 What V2 still needs before it does anything

1. Firmware SPI driver for the ADS7038 (auto-sequence of AIN5/4/3 with
   on-chip averaging), on the key path, with its latency measured.
2. Module ID class for 1.65 V, and the 1600 mV cut-off moved (§1.4).
3. A third key through keypad, HID report, protocol, app and calibration.
4. Board test: pins 6–8 are SPI.
5. Calibration that accepts a key whose reading goes **down** (flipped magnet).
6. A three-key case.

---

## 6. What a full redesign should consider

These came up in the investigation. Nothing here is decided unless §4 says
so.

### 6.1 HE module

- Start from V2 (§5). It already fixes the noise path by design.
- **ESD protection** on the connector lines (low-capacitance TVS array), since
  modules are hot-plugged by hand (H9).
- **More signal per mm** is optional once the ADC is on the module: A2 doubles
  the gain but can clip near bottom-out with strong magnets (A2 reaches its
  ~3.1 V limit at ~48 mT); A4 (~7.5 mV/mT, ~±170 mT) for very strong magnets.
  All share the footprint. H2 keeps A3.
- Variants: 2-key and 3-key, or only 3-key. **Open.**

### 6.2 Cable and connector

- Only **one GND** wire today. More ground wires lower ground impedance. With
  the ADC on the module this matters less (digital SPI), but it still carries
  return current for the SPI edges.
- Options: a 10-pin JST-SH with 2–3 GND, or a ground between the SPI lines.
  **This changes the carrier and every module.** **Open.**
- Keep the cable same-direction and keep the board test's reverse-cable check.

### 6.3 Carrier

- Today: no parts. H6 forbids RC filtering on it while MX uses 10 kΩ
  pull-ups.
- Candidate additions that do not slow MX: **bulk capacitance** on 3V3 (10–47 µF
  across P2-1/P2-2 was the proposed hand fix), a ferrite bead on the module's
  3V3 feed, ESD diodes on J_MOD.
- **Free pins:** P1-11/12/13/14 (USB D+, D−, GND, 5V) are routed to a rail at
  the USB-C end for a possible **internal USB-C board** (`hardware/pcb/V1/README.md`).
  An internal USB-C board could add **USB-side filtering** (ferrite on VBUS,
  capacitors, ESD), which the Waveshare dev board decides today.
- No copper pour under the bridge below the Waveshare's ceramic antenna.
- Stack: Waveshare back → carrier top 11.0 mm (2.5 mm header plastic + 8.5 mm
  socket); carrier 1.6 mm; J_MOD 2.9 mm below; glass to carrier bottom ~19.8 mm.

### 6.4 MX module

- Works; must keep working (owner).
- Kailh CPG151101S11 hot-swap (C41430893), 10 kΩ pull-ups R1/R2 (C25804),
  ID 100k/10k (R3 C25803, R4 C25804), C1 100 nF, J1 C160407, everything bottom.
- A 3-key MX module would need the same firmware 3-key work as HE V2. **Open.**
- If the cable/connector changes (§6.2), MX changes with it.

### 6.5 The big question: keep the Waveshare dev board?

The cleanest HE boards put MCU, ADC and sensors on **one PCB**. OPad
depends on the Waveshare ESP32-S3-Touch-LCD-2 for the screen and touch, which
also shares 3V3 with the LCD, backlight and PSRAM, and has its own USB input
circuit. With the ADC on the module (H1) the measurement no longer depends on
that, so keeping the Waveshare is reasonable. A custom main board would be a
much larger project (USB-C, ESP32-S3 module, LCD/touch connector). **Open.**

### 6.6 Case

- V1.1 fits the 52 mm two-key module only: key deck for 2 keys, screwless
  clamp with pins through the module holes at case (15.5, 16.5) and (60.5, 16.5).
- HE V2 (71.05 mm, holes 64.05 mm apart, J1 on the middle key) needs a new
  deck and new clamp positions.
- Keep: plate 1.5 mm, PCB top 5.0 mm below the plate top, ~3.5 mm free under
  the PCB for parts and J1, 16° screen tilt, silicone feet.
- Coordinates: case X = KiCad X − 100, case Y = 100 − KiCad Y for V1 (V2 board
  coordinates differ; recompute).

---

## 7. Rules for every new board (lessons)

1. **Check SOT-23 pin order against the datasheet's top view and KiCad's flip.**
   V1.0 shipped mirrored (VCC on OUT); DRC and the assembler could not catch it.
   DRV5055 DBZ: 1 = VCC, 2 = OUT, 3 = GND.
2. **Verify every LCSC number on its product page.** V1 had two wrong ones
   (C2843516 is a diode, C25816 is 383 kΩ).
3. **Check JLCPCB's placement preview** for every polarised part (magenta dot
   vs silkscreen dot); fix with `JLC_CPL_FIXES`, not by hand each order.
4. **Never put a capacitor directly on a DRV5055 output**; resistor first.
5. **Keep the 3 × 3 mm copper keepout** around each Hall sensor, both layers.
6. **ERC, DRC with zones refilled, and schematic parity must be 0** before export
   (`export_production.py` stops otherwise).
7. **Stitch both ground pours**; a pour with no vias gets removed as an island.
8. **Prefer parts in LCSC stock** with margin for 5–10 boards plus spares;
   check stock the day you order.
9. **Order 5, test, then batch.** Measure each sensor's swing before a batch.
10. **New files need REUSE licensing** (`REUSE.toml` covers `hardware/**`
    as CERN-OHL-S-2.0 and `hardware/**/*.py` as GPL-3.0-or-later).
11. **Any change to `firmware/main/input/` or `usb/`** states its latency effect
    and how it was checked (AGENTS.md §1).

---

## 8. Open questions to settle first

Settled on 2026-10-23 (§4.1): keep the Waveshare (1, kept by H11), cable
(2: FFC, H10), keys (3: three only, H13), USB-C board (4: yes, at the back,
H11/H12; its own board on an 8-way FFC since 2026-10-28, H18), ESD (5: yes, H11 and the HE V2 findings), ordering HE V2 (8: no,
H15).

Still open:

1. One firmware for all modules (HE on by default) (§1.5)?
2. Was a magnet near the pad at 23:40 on 2026-10-22 (§3.3)?
3. FFC pitch, pin count and pinout (proposal in progress).
4. ~~Case~~: designed 2026-10-24 (`hardware/3d/custom_case/V3`): 84 x 83 mm, USB-C out of
   the rear wall, stack inserted from below against a lip, four module pins. Not printed yet.
