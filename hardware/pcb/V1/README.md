# osuPad V1 PCBs

Modular hardware ecosystem joined by one 8-wire JST-SH cable:

| Board | Folder | Size | What it does |
|---|---|---|---|
| **Controller carrier** | `Carrier/` | 35.0 × 48.2 mm, C-shaped | Plugs onto the Waveshare ESP32-S3-Touch-LCD-2 headers and brings the module signals to a connector |
| **MX input module** | `MX/` | 52.0 × 24.0 mm | Two Kailh MX mechanical hot-swap sockets under the key plate |
| **Hall Effect input module** | `HE/` | 52.0 × 24.0 mm | Two magnetic Rapid Trigger Hall sensors (TI DRV5055) under the key plate |

The input module is **swappable**: both the MX mechanical module and the Hall-effect (rapid trigger) module use the same outline, mounting holes, connector position and pinout, and plug into the same carrier and cable. The module reports which kind it is through an ID voltage on the connector (0.30 V for MX, 1.06 V for Hall Effect).

Status: the MX module and the carrier pass ERC, DRC (zero errors, zero warnings) and schematic parity in KiCad 10.0.6. The Hall Effect module is generated and checked by its own toolchain (`scripts/he_board.py`, see below), which needs no KiCad install; it reports zero rule violations and its Gerbers verify clean against the netlist. Production files are in each board's `production/` folder.

---

## Module connector (JST SH 8-pin, same on every input module)

| Pin | Net | Waveshare pin | GPIO | MX V1 use | Hall module use |
|---|---|---|---|---|---|
| 1 | 3V3 | P2-1 | – | pull-ups, ID divider | sensor supply |
| 2 | GND | P2-2 | – | switch common | ground |
| 3 | IN1 | P1-10 | **GPIO10** (ADC1_CH9) | KEY1 (left key) | analog 1 / SPI CS1 |
| 4 | IN2 | P1-9 | **GPIO7** (ADC1_CH6) | KEY2 (right key) | analog 2 / SPI CS2 |
| 5 | ID | P1-8 | GPIO8 (ADC1_CH7) | ID divider = 0.30 V | own ID voltage |
| 6 | IO6 | P1-3 | GPIO6 (ADC1_CH5) | not used | e.g. SPI MISO |
| 7 | IO4 | P1-2 | GPIO4 (ADC1_CH3) | not used | e.g. SPI MOSI |
| 8 | IO2 | P1-1 | GPIO2 (ADC1_CH1) | not used | e.g. SPI SCK |

- All six signals are ADC1 pins. On the ESP32-S3, ADC continuous (DMA) mode only supports ADC1 (see `docs/specs/v2-rapid-trigger.md` §2.2).
- SPI3 can be routed to any of these pins through the GPIO matrix. None of them are strapping pins, USB, UART0, or the touch/IMU I2C.
- The pin order is chosen so both boards route on two layers with no crossings.
- **Firmware:** in the app, set Key 1 = GPIO10 and Key 2 = GPIO7 (both are already in the allowed pin list). The hand-wired defaults (GPIO14 / GPIO9) don't apply to this board.

### Module ID

Each module has its own divider from 3V3 to GND on ID. The carrier has no parts on this line.

| Module | Top resistor | Bottom resistor | ID voltage |
|---|---|---|---|
| MX V1 | 100 kΩ (R3) | 10 kΩ (R4) | 0.30 V |
| **Hall Effect V1** | **100 kΩ (R1)** | **47 kΩ (R2)** | **1.06 V** |
| reserved: Hall SPI | 100 kΩ | 100 kΩ | 1.65 V |

The divider's source impedance is 100k ∥ 47k ≈ 32 kΩ, above the ~10 kΩ the ESP32-S3 ADC likes. ID is read once at start-up, so average a handful of samples instead of trusting a single conversion.

With no module plugged in, GPIO8 floats. The firmware can enable the internal pull-down to read about 0 V in that case.

---

## MX input module (`MX/`)

- **Case coordinates** below are those of `hardware/3d/custom_case/V1/osupad_enclosure.scad`: origin at the front-left outer corner, X to the right, Y toward the screen. KiCad uses X = 100 + case X and Y = 100 − case Y, so the top of the KiCad editor is the rear of the case and the layout is not mirrored.
- **Key centres:** case (28.475, 16.5) and (47.525, 16.5), 19.05 mm pitch, the same as the plate cutouts.
- **Outline:** case X 12…64, Y 7…31, 1.5 mm corner radius.
- **Mounting holes:** 2× M2 (2.2 mm NPTH) at case (15.5, 16.5) and (60.5, 16.5), on the key axis and clear of the switch footprints. The case needs M2 bosses (heat-set inserts) there. Hot-swap sockets must be held by screws, or the PCB moves when a switch is pulled.
- **Height:** the PCB top is **5.0 mm below the top of the plate** (plate 1.5 mm, case z = 10.0). About 3.5 mm of free space is needed under the PCB bottom for the sockets (1.85 mm) and J1 (2.9 mm).
- **Everything is on the bottom side** (sockets, J1, R1–R4, C1), so JLCPCB assembles one side only. The switches go in from the top through the plate.
- **J1** sits at the rear edge, centred on the keys, with the opening facing the screen. The cable leaves toward the controller.
- **Footprint:** `lib/osupad.pretty/Kailh_CPG151101S11_MX_Hotswap_Bottom` is the Kailh land pattern (3.05 mm pin holes, 4.0 mm centre, 1.75 mm pegs, 2.55 × 2.5 mm pads), drawn from the socket side and placed flipped. The switch pins point to the rear (standard orientation).

## Hall Effect input module (`HE/`)

Same outline, mounting holes, connector position and pinout as the MX module, so the two are interchangeable on one cable. Instead of switching a pin to ground, each key has a **TI DRV5055A3 ratiometric linear Hall sensor** whose analog output tracks the magnet in the switch stem, which is what rapid trigger needs.

### Sensor architecture (the part that matters)

This is **Architecture A: bottom-side SMT on solid FR-4.**

- There is **no 4.0 mm stem hole** at the key centres. `SW_MX_Hall_PlateMount` (used for a through-hole stem) is *not* the footprint here; `SW_MX_Hall_PlateMount_SolidCentre` is, and it contributes only the two alignment peg holes.
- U1 and U2 are centred **exactly** on the key centres (16.475, 9.5) and (35.525, 9.5), on the bottom copper, directly under the magnet.
- A **3.0 × 3.0 mm copper keepout on both layers** around each sensor keeps the ground pour out of the magnetic path, so the stem magnet's flux is not damped by eddy currents in the plane. In KiCad these are rule areas with `copperpour not_allowed`; in the Gerbers the pour is genuinely absent there.
### Why there is no centre hole, and what the board is actually compatible with

This is not a compromise forced by single-side assembly — it is what the switch standard requires. In a Lekker / Gateron KS-20 class magnetic switch:

- **the magnet sits at the centre of the switch and reaches the bottom face of the housing.** There is no MX-style centre pole to clear, and equally no room for a component on the *top* side of the PCB either;
- **two plastic alignment pins stick out of the bottom and pass straight through the PCB**, protruding about 1 mm out the back. Those are the ±5.08 mm pins, and they are why the 1.75 mm peg holes are needed.

Wooting describe exactly this in [Lekker update #5, part 2](https://wooting.io/post/we-overcame-the-challenges-lekker-update-5-part-22) and moved their own sensors to the PCB bottom for the same reason. So peg holes **and** a solid centre **and** a bottom-side sensor is the consistent set: a board that had the peg holes *and* a 4 mm centre hole would be an MX PCB-mount pattern, which is a different switch.

| Switch | Works on this board? |
|---|---|
| Gateron KS-20 / Wooting Lekker, KS-37 / Magnetic Jade, Geon Raw HE and other Lekker-standard magnetic switches | Yes — flush magnet centre, two ±5.08 mm plastic pins |
| Switches sold as "**TMR switches**" | Yes — see below |
| MX mechanical (Cherry, Gateron, Kailh…) | **No.** A real MX switch has a centre pole and metal pins. That is what the `MX/` module is for — same outline, same connector, swap the module |

**On "TMR switches":** TMR (tunnel magnetoresistance) is a *sensor* technology that lives on the keyboard's PCB, not a different kind of switch. A switch marketed as a "TMR switch" is the same magnetic switch — a magnet in an MX-form stem with a flush bottom and two pins — and the industry uses them interchangeably: Gateron Jade, Jade Pro and Lekker V2 all go into Hall boards like the Wooting 60HE and TMR boards like the Womier SK75 alike. This module reads the magnet with a Hall sensor, and the magnet does not know or care what sensor is underneath it, so TMR-branded switches drop straight in.

Two caveats, neither of which is a fit problem:

- **Magnet strength varies by switch model.** TMR sensors are far more sensitive than Hall, so a vendor *can* pair a TMR-targeted switch line with a weaker magnet. If you use such a switch here, A3 is the wrong variant and you want A1 or A2. As Wooting put it, what matters is matching the sensitivity range to the magnet, not chasing sensitivity for its own sake.
- **This board cannot be rebuilt with a TMR sensor as a drop-in.** The common analog TMR parts (MultiDimension TMR2001, TMR2083) are SOT-23-**5**, not the 3-pin SOT-23 used here, so they would need a new footprint and a re-route. Most TMR designs also mount the sensor *off-axis*; U1/U2 here are deliberately on-axis at the key centre, which is the right placement for the DRV5055.

These magnetic switches are deliberately not MX-compatible; vendors say so themselves. The claim this board makes is "every switch in the Lekker standard", not "every keyboard switch ever made". If a magnetic switch turns up with a protruding MX-style centre pole, it will foul the sensor and is out of scope for V1 — I found no current HE switch that does this, since the whole class puts the magnet where that pole would be.

### Sensor sensitivity

The magnet reads through 1.6 mm of FR-4, so the field at the die is weaker than a top-side design's. Wooting hit the same geometry and compensated with a stronger magnet *and* a more sensitive sensor. All three variants share the SOT-23 pinout and this footprint, so **the board never changes** — only which reel JLCPCB loads. Set `SENSOR_VARIANT` in `scripts/he_board.py` and regenerate.

| Variant | LCSC | Sensitivity @3.3 V | Linear range | Notes |
|---|---|---|---|---|
| A1 | `C962987` | ≈66 mV/mT | ≈±20 mT | best resolution, clips soonest at full press |
| A2 | `C266131` | ≈33 mV/mT | ≈±40 mT | **recommended middle ground** |
| A3 | `C266128` | ≈16 mV/mT | ≈±80 mT | current default; cannot clip, coarsest |

A3 is shipped as the default because it is the one variant that physically cannot saturate, which makes first bring-up unambiguous. Given Wooting's experience, expect to move to A2.

### Layout

- **Everything is on the bottom side**, so JLCPCB assembles one side.
- `U1`/`U2` are at 180°, so both analog outputs face the connector and both ground pads face the board's front edge, where a short stub and a via drop them into the pour outside the keepout.
- `C2`/`C3` sit just outside each keepout, feeding VCC from the side; `C1` is the bulk decoupler next to J1 pin 1.
- **3V3 is distributed on F.Cu**, the otherwise empty top layer, so every analog trace stays on B.Cu with no crossings. Four vias tie it to the bottom-side stubs.
- `R1`/`R2` are the 100 k / 47 k ID divider.
- J1-6/7/8 (GPIO6/4/2) are reserved for an SPI variant, so instead of being left floating they run to probe pads **TP1/TP2/TP3** below the connector — the three bare pads visible under J1 in the render. They are board features, not parts: nothing is assembled on them, they take no solder paste, and they appear in neither the BOM nor the centroid file. Delete `_probe_pad(...)` from `PARTS` in `he_board.py` and regenerate if you would rather leave those pins unconnected.
- Ground: pours on both layers, thermal relief on every ground pad, and 21 plated vias stitching the two planes.

### Pinout

| Pin | Net | Goes to |
|---|---|---|
| 1 | 3V3 | U1-1, U2-1 (VCC), C1/C2/C3, R1 |
| 2 | GND | ground pours, U1-3, U2-3, C1/C2/C3, R2 |
| 3 | IN1 | U1-2 (OUT), key 1 analog → GPIO10 (ADC1_CH9) |
| 4 | IN2 | U2-2 (OUT), key 2 analog → GPIO7 (ADC1_CH6) |
| 5 | ID | R1/R2 divider, 1.06 V → GPIO8 (ADC1_CH7) |
| 6, 7, 8 | IO6 / IO4 / IO2 | TP1 / TP2 / TP3 probe pads, reserved for SPI |

**DRV5055 SOT-23 pinout: 1 = VCC, 2 = OUT, 3 = GND** (TI DBZ package). Worth repeating because swapping 2 and 3 would put the output straight on ground.

### Toolchain

The HE board does not depend on a KiCad install. `scripts/he_board.py` holds the geometry, netlist and placement, and checks its own design rules; everything else is derived from it:

```
python3 scripts/he_board.py            # placement self-test + DRC, exit 1 on any violation
python3 scripts/generate_he_gerbers.py # Gerbers, drills, job file, BOM, CPL (JLCPCB + PCBWay)
python3 scripts/generate_he_kicad.py   # .kicad_sch, .kicad_pcb, .kicad_pro, lib tables
python3 scripts/verify_he_gerbers.py   # rasterise the Gerbers, check nets for opens/shorts
python3 scripts/render_he_preview.py   # production/*-top.png and *-bottom.png
```

`generate_he_gerbers.py` and `generate_he_kicad.py` both refuse to write anything if the DRC fails. `verify_he_gerbers.py` does not re-read the geometry and declare it fine: it replays the Gerber files, including every clear-polarity knockout, labels the copper regions, joins the layers through the vias, and then asserts that each net is one region and that no region carries two nets. It also checks that the sensor keepouts hold no pour copper and that nothing sits over a non-plated hole. Both it and the preview renderer need Pillow.

Copper pours in the Gerbers are real: the pour is filled, then every foreign pad, track, via and NPTH is knocked out with `%LPC*%` using apertures grown by the 0.3 mm pour gap, then the actual copper is flashed back. Isolation gaps show up properly in any fab viewer.

**File format:** the board is written in KiCad 8 syntax (`version 20240108`), which KiCad 8, 9 and 10 all open. The schematic is KiCad 9 syntax, because it embeds the shared symbol library and that library is saved by KiCad 10. This differs from `MX/` and `Carrier/`, which are native KiCad 10 files. Zones are written as outlines with their clearance and thermal settings; KiCad fills them on open (or any DRC run) — the shipped Gerbers already contain the filled result.

Running `scripts/generate_boards.py` under KiCad's Python regenerates MX and the carrier with `pcbnew` as before; its `build_he()` just calls the generator above, so there is only one definition of this board.

## Controller carrier (`Carrier/`)

- **Outline:** the Waveshare PCB (35.0 × 48.2 mm) with a **22.8 × 34.7 mm window**, open at the USB-C end. The TF card slot, the MX1.25 battery connector and the camera FPC stay reachable.
- **Headers** (from the Waveshare STEP model and drawing):
  - The rows are 30.48 mm apart, 2.26 mm from each long edge.
  - Pin 1 is 7.59 mm from the non-USB edge, 2.54 mm pitch.
- **Seen from the carrier's top** (the side marked `WAVESHARE ON THIS SIDE`): P1 (GPIO2 … 5V) is on the right and P2 (3V3 … VBAT) on the left, both with pin 1 at the non-USB end. This is the mirror image of Waveshare's back-side pinout photo, as it must be.
- **Holes:** 4× M2 at the Waveshare hole positions (29.0 × 42.2 mm, 3.0 mm from the edges).
- **Sockets:** 2× 1×14 female, 8.5 mm tall, soldered on the top side. J_MOD (JST SH) is on the bottom at the non-USB end, opening outward.
- **Free for later:** P1-11/12/13/14 (USB D+, D−, GND, 5V) are on the rail at the USB-C end, unused, ready for an internal USB-C board (still under discussion).
- **Antenna:** there is no copper pour under the bridge, which sits below the Waveshare's ceramic antenna.

### Stack height

| From | To | Distance |
|---|---|---|
| Waveshare PCB back | carrier top | 11.0 mm (2.5 mm male-header plastic + 8.5 mm socket) |
| carrier top | carrier bottom | 1.6 mm |
| carrier bottom | J_MOD bottom (non-USB end only) | 2.9 mm |
| carrier bottom | socket pin tails | about 3 mm; clip them flush after soldering |
| glass front | carrier bottom | about 19.8 mm |

**The V1 case does not fit this stack.** With the 20.3° deck, the carrier's lower long edge reaches the case floor, and the four Waveshare support pillars in the bottom plate are in the way. The case revision needs:
- the screen deck raised by about 3 mm, or a steeper deck;
- the pillars removed, or replaced by M2 standoffs through the carrier holes;
- bosses for the MX module holes;
- the cable routed from the left end of the screen to the rear of the key module (a 100 mm cable is enough).

---

## Ordering

### Bare PCBs (JLCPCB or PCBWay)

Upload `production/<board>-gerbers.zip`. Both boards use standard options:

| Option | Value |
|---|---|
| Layers | 2 |
| Thickness | 1.6 mm (keep it: the Kailh socket and 5.0 mm plate-to-PCB distance assume 1.6 mm) |
| Copper | 1 oz |
| Min track / clearance | 0.25 / 0.2 mm |
| Min drill | 0.3 mm (vias) |
| Finish | HASL lead-free or ENIG |
| Order number | "Specify a location" (JLCPCB): each board has a `JLCJLCJLCJLC` silkscreen mark on the bottom |

The Gerbers include the board outline on its own layer (the carrier's window and corner radii are in Edge.Cuts) plus separate PTH and NPTH drill files.

### Assembly (JLCPCB)

Upload `production/<board>-BOM-JLCPCB.csv` and `production/<board>-CPL-JLCPCB.csv`.

**MX module**: assembly side **Bottom**.

| Designator | Part | LCSC |
|---|---|---|
| SW1, SW2 | Kailh CPG151101S11 MX hot-swap socket | C41430893 |
| J1 | JST SM08B-SRSS-TB | C160407 |
| R1, R2, R4 | 10 kΩ 1 % 0603 | C25804 |
| R3 | 100 kΩ 1 % 0603 | C25803 |
| C1 | 100 nF 50 V X7R 0603 | C14663 |

**Hall Effect module**: assembly side **Bottom**.

| Designator | Part | LCSC |
|---|---|---|
| U1, U2 | TI DRV5055A3QDBZR linear Hall sensor, SOT-23 | C266128 |
| J1 | JST SM08B-SRSS-TB | C160407 |
| C1, C2, C3 | 100 nF 50 V X7R 0603 | C14663 |
| R1 | 100 kΩ 1 % 0603 | C25803 |
| R2 | 47 kΩ 1 % 0603 | C25819 |

TP1–TP3 are bare probe pads; they are board features, not parts, and are absent from the BOM and centroid file. U1/U2 are an **Extended** LCSC part, so JLCPCB adds the one-off feeder charge.

> Two part numbers that circulate for this board are wrong and were corrected here: **C2843516 is a Schottky diode** (Jingdao SSL310F), not the DRV5055 — the sensor is **C266128**. And **C25816 is a 383 kΩ resistor**, not 47 kΩ — the 47 kΩ `0603WAF4702T5E` is **C25819**. Building from the older numbers would populate a diode where the Hall sensor goes and put the ID line at 3.3 V × 383/483 = 2.62 V, which the firmware would not recognise as this module.

**Carrier**:

| Designator | Part | LCSC |
|---|---|---|
| J_MOD | JST SM08B-SRSS-TB, bottom side | C160407 |
| J_P1, J_P2 | 1×14 female header, 2.54 mm, 8.5 mm, top side (THT) | C2897377 |

J_MOD is the only SMD part, on the bottom. The two sockets are through-hole on the top: order them assembled as THT, or solder them yourself (plug them onto the Waveshare board while soldering so they stay aligned).

**Check the part rotation in JLCPCB's placement preview before paying.** KiCad and LCSC define rotation differently for some parts, most often the Kailh socket, the JST connector and SOT-23 parts:
- the socket's two pads must cover the two copper pads next to the 3.05 mm holes;
- the JST's 8 contacts sit toward the inside of the board, with the plug opening at the board edge;
- on the HE module U1 and U2 are at **180°**: the single lead (pin 3, GND) must point toward the board's front edge, the two-lead side (pins 1 and 2) toward the connector. Both sensors face the same way, so if one looks wrong in the preview they both are.

### PCBWay

Use the files in `production/pcbway/`, not the JLCPCB ones:
- `<board>-gerbers-PCBWay.zip`: the order-number marker is `WayWayWay` instead of `JLCJLCJLCJLC`.
- `<board>-BOM-PCBWay.csv`: PCBWay's BOM template columns, with manufacturer, part number, type (SMD/THT) and placement notes.
- `<board>-centroid-PCBWay.csv`: placement file.

Order each board as a separate item.

### Loose parts (not on the PCBs)

| Qty | Part | Note |
|---|---|---|
| 1 | JST SH 1.0 mm 8-pin cable, 100 mm, **same-direction** (pin 1 ↔ pin 1) | Many cheap "reverse" cables swap pin 1 ↔ 8: check with a multimeter before plugging in |
| 2 | MX switches (3- or 5-pin) + keycaps | MX module |
| 2 | MX magnetic switches (Gateron KS-20, Magnetic Jade, Wooting Lekker, Geon Raw HE) + keycaps | HE module |
| 1 | 14 × 14 mm switch plate, 1.5 mm, held 5.0 mm above the PCB | both modules; the HE module needs it, since the switches are plate-mounted and nothing holds them to the PCB |
| 2 | M2 × 4 mm screws + M2 heat-set inserts | MX module to the case (revised case) |
| 4 | M2 spacers, 8.5 mm (or 8 mm + washer), optional | Carrier to the Waveshare standoffs; the sockets hold a first prototype on their own |

## Before the first order

1. Print `production/<board>-pcb.pdf` at 100 % and lay the Waveshare board and two switches on it. The HE module has no PDF; use `production/he_input_v1-bottom.png` (2240 px wide = 52.0 mm, so print it at 1094 dpi for 1:1) or open the `.kicad_pcb`.
2. In the JLCPCB preview, check socket, connector and SOT-23 rotation (see above).
3. Check the cable direction with a multimeter.
4. Order 5 of each and test-fit before a larger batch.
5. For the HE module, measure before committing to a batch: probe IN1/IN2 with the switch at rest and fully pressed. If the swing is much under ~0.5 V, move to a DRV5055A2 or A1 (drop-in, same footprint) rather than respinning the board.

---

## Files

```
V1/
├── lib/                      project library shared by all boards
│   ├── osupad.kicad_sym      R, C, SW_Push, DRV5055, Conn_01x08, Conn_01x14, TestPoint
│   └── osupad.pretty/        0603 R/C, SOT-23, JST SH, pin socket, M2 hole, Kailh hot-swap, Hall switch positions, test pad
├── Carrier/                  controller_carrier_v1.kicad_pro / .kicad_sch / .kicad_pcb, production/
├── MX/                       mx_input_v1.kicad_pro / .kicad_sch / .kicad_pcb, production/
├── HE/                       he_input_v1.kicad_pro / .kicad_sch / .kicad_pcb, production/
├── mechanical_reference_case_coords.dxf   case, plate cutouts, input PCB outline/holes/J1 (case coordinates)
└── scripts/
    ├── gen_library.py           writes the standard parts in lib/ (symbols, 0603 R/C, SOT-23, JST SH, pin socket, M2 hole)
    ├── generate_boards.py       generator for projects (see below)
    ├── export_production.py     ERC + DRC + parity check, then Gerbers, drills, BOM, CPL, PDFs, STEP, renders
    ├── he_board.py              HE module: geometry, netlist, placement and its own DRC (no KiCad needed)
    ├── generate_he_gerbers.py   HE module: Gerbers, drills, job file, BOM, CPL
    ├── generate_he_kicad.py     HE module: .kicad_sch / .kicad_pcb / .kicad_pro
    ├── verify_he_gerbers.py     HE module: rasterise the Gerbers, check the netlist for opens/shorts
    ├── render_he_preview.py     HE module: top/bottom artwork previews
    └── strokefont.py            single-stroke vector font for generated silkscreen
```

The shared library gained two footprints and one symbol for this module: `SW_MX_Hall_PlateMount_SolidCentre` (peg holes only, no stem hole), `TestPoint_Pad_1.0x1.0mm`, and the `TestPoint` symbol.

Each `production/` folder contains `-gerbers.zip`, `-BOM-JLCPCB.csv`, `-CPL-JLCPCB.csv`, `-schematic.pdf`, `-pcb.pdf`, `.step` (for the case CAD), top/bottom renders, and the ERC/DRC reports.

- **Regenerating:** `python3 hardware/pcb/V1/scripts/generate_boards.py` rebuilds the schematics and boards from the netlist in the script and overwrites any edits made in KiCad. After editing in KiCad, only run `export_production.py`.
- **Requirements:** KiCad 10 (`kicad-cli` and the `pcbnew` Python module) for MX and the carrier. The KiCad libraries don't need to be installed: everything used is in `lib/`.
- **Library:** every part in `lib/` is OPad's own drawing under the repository's MIT license. The standard parts are written by `scripts/gen_library.py` (pads and pins from the component datasheets; outlines, silkscreen, courtyards and symbol graphics computed by the script); edit the tables there and re-run it rather than editing those files in KiCad. The Kailh hot-swap, Hall switch and test-pad footprints are hand-drawn. Footprints reference KiCad's installed 3D models by path (`${KICAD10_3DMODEL_DIR}`); the models themselves are not in this repository.
- **The HE module is the exception:** it is generated and verified with plain Python (3.9+), no KiCad and no `pcbnew`; Pillow is needed only for `verify_he_gerbers.py` and `render_he_preview.py`. Its `production/` folder has no `drc.json`/`erc.json`/PDF/STEP, because those come from `kicad-cli`; run `export_production.py` under a KiCad install if you want them.
