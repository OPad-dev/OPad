# osuPad V1 PCBs

Two boards, joined by one 8-wire cable:

| Board | Folder | Size | What it does |
|---|---|---|---|
| **MX input module** | `MX/` | 52.0 × 24.0 mm | Two Kailh MX hot-swap sockets under the key plate |
| **Controller carrier** | `PCB Base/` | 35.0 × 48.2 mm, C-shaped | Plugs onto the Waveshare ESP32-S3-Touch-LCD-2 headers and brings the module signals to a connector |

The input module is **replaceable**: a future Hall-effect (rapid trigger) module uses the same outline, mounting holes, connector position and pinout, and plugs into the same carrier and cable. The module reports which kind it is through an ID voltage on the connector.

Status: both boards pass ERC, DRC (zero errors, zero warnings) and schematic parity in KiCad 10.0.6. Production files are in each board's `production/` folder.

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

- All six signals are ADC1 pins. On the ESP32-S3, ADC continuous (DMA) mode only supports ADC1 (see `osupad_v2_rapid_trigger_plan.md` §2.2).
- SPI3 can be routed to any of these pins through the GPIO matrix. None of them are strapping pins, USB, UART0, or the touch/IMU I2C.
- The pin order is chosen so both boards route on two layers with no crossings.
- **Firmware:** in the app, set Key 1 = GPIO10 and Key 2 = GPIO7 (both are already in the allowed pin list). The hand-wired defaults (GPIO14 / GPIO9) don't apply to this board.

### Module ID

Each module has its own divider from 3V3 to GND on ID. The carrier has no parts on this line.

| Module | Top resistor | Bottom resistor | ID voltage |
|---|---|---|---|
| MX V1 | 100 kΩ (R3) | 10 kΩ (R4) | 0.30 V |
| suggested: Hall analog | 100 kΩ | 47 kΩ | 1.06 V |
| suggested: Hall SPI | 100 kΩ | 100 kΩ | 1.65 V |

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

## Controller carrier (`PCB Base/`)

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

**Carrier**:

| Designator | Part | LCSC |
|---|---|---|
| J_MOD | JST SM08B-SRSS-TB, bottom side | C160407 |
| J_P1, J_P2 | 1×14 female header, 2.54 mm, 8.5 mm, top side (THT) | C2897377 |

J_MOD is the only SMD part, on the bottom. The two sockets are through-hole on the top: order them assembled as THT, or solder them yourself (plug them onto the Waveshare board while soldering so they stay aligned).

**Check the part rotation in JLCPCB's placement preview before paying.** KiCad and LCSC define rotation differently for some parts, most often the Kailh socket and the JST connector:
- the socket's two pads must cover the two copper pads next to the 3.05 mm holes;
- the JST's 8 contacts sit toward the inside of the board, with the plug opening at the board edge.

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
| 2 | MX switches (3- or 5-pin) + keycaps | |
| 2 | M2 × 4 mm screws + M2 heat-set inserts | MX module to the case (revised case) |
| 4 | M2 spacers, 8.5 mm (or 8 mm + washer), optional | Carrier to the Waveshare standoffs; the sockets hold a first prototype on their own |

## Before the first order

1. Print `production/<board>-pcb.pdf` at 100 % and lay the Waveshare board and two switches on it.
2. In the JLCPCB preview, check socket and connector rotation (see above).
3. Check the cable direction with a multimeter.
4. Order 5 of each and test-fit before a larger batch.

---

## Files

```
V1/
├── lib/                      project library shared by both boards
│   ├── osupad.kicad_sym      R, C, SW_Push, Conn_01x08, Conn_01x14 (from the KiCad library)
│   └── osupad.pretty/        KiCad library footprints + Kailh hot-swap footprint
├── MX/                       mx_input_v1.kicad_pro / .kicad_sch / .kicad_pcb, production/
├── PCB Base/                 controller_carrier_v1.kicad_pro / .kicad_sch / .kicad_pcb, production/
├── mechanical_reference_case_coords.dxf   case, plate cutouts, MX PCB outline/holes/J1 (case coordinates)
└── scripts/
    ├── generate_boards.py    one-shot generator for both projects (see below)
    └── export_production.py  ERC + DRC + parity check, then Gerbers, drills, BOM, CPL, PDFs, STEP, renders
```

Each `production/` folder contains `-gerbers.zip`, `-BOM-JLCPCB.csv`, `-CPL-JLCPCB.csv`, `-schematic.pdf`, `-pcb.pdf`, `.step` (for the case CAD), top/bottom renders, and the ERC/DRC reports.

- **Regenerating:** `python3 hardware/pcb/V1/scripts/generate_boards.py` rebuilds the schematics and boards from the netlist in the script and overwrites any edits made in KiCad. After editing in KiCad, only run `export_production.py`.
- **Requirements:** KiCad 10 (`kicad-cli` and the `pcbnew` Python module). The KiCad libraries don't need to be installed: everything used is in `lib/`.
