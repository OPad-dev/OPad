# OPad V2 PCBs

| Board | Folder | Size | Status |
|---|---|---|---|
| **Hall Effect input module V2** | `HE/` | 71.05 × 24.0 mm | **Prototype, not built yet.** Firmware support not written yet (see Firmware) |

The carrier and the MX module stay at V1 (`hardware/pcb/V1/`); nothing on them changes. V2 replaces the V1.1 HE module, which is deprecated once V2 works.

## What changed from HE V1.1, and why

The V1.1 module sent the sensors' analog voltages through the cable to the ESP32-S3's own ADC. That made two kinds of noise (`docs/history/2026-10-22-he-phantom-presses.md`):

- **Constant fuzz** of ±30–40 counts per reading, from the ESP32-S3's ADC itself. The module ID resistor divider, which has no sensor, showed the same amount, and the key and ID noise did not move together (correlation +0.08).
- **Phantom spikes** of +60 to +380 counts lasting about 70 µs to 1 ms, from outside the module (USB cable or PC side), getting into the analog signals.

V2 measures the sensors **on the module** and sends numbers over SPI, so neither reaches the measurement:

| Change | Part | Why |
|---|---|---|
| ADC on the module | TI **ADS7038** (U4), 8-channel 12-bit SAR, SPI, WQFN-16 3 × 3 mm | Its reference is its own AVDD, which also powers the sensors, so the reading is **ratiometric**: a supply wobble moves the sensor output and the ADC's ruler together. It averages internally and returns 16-bit results, so the ESP32 does not have to read it more often. Only digital SPI crosses the cable. |
| Sensor supply filter | Ferrite bead FB1 (600 Ω at 100 MHz) + 10 µF C2, 100 nF at each sensor, 1 µF at the ADC's AVDD | Keeps spikes on 3V3 away from the sensors and the reference |
| Output filter per sensor | 220 Ω series + 10 nF shunt, at the ADC | TI requires a resistor before any capacitor on the DRV5055 output (datasheet §7.1.3); the 10 nF also feeds the ADC's 12 pF sampling capacitor (ADS7038 datasheet §7.3.1). Time constant 2.2 µs, so it adds no noticeable delay |
| Three keys | U1–U3 at 19.05 mm pitch | For players who do not use the middle finger as one of two keys |

Kept from V1.1:
- **Sensor:** DRV5055**A3**, bipolar, so a switch with its magnet flipped still reads (the reading goes down instead of up).
- **Sensor position:** bottom side, on the key centres, with a 3 × 3 mm copper keepout on both layers.
- **Switch standard:** the Lekker one (V1 README, "Sensor architecture").
- **Assembly:** every part on the bottom side, so JLCPCB assembles one side.

## Connector (JST SH 8-pin, V1 pinout, same cable and carrier)

| Pin | Carrier GPIO | V1.1 HE | **V2** |
|---|---|---|---|
| 1 | 3V3 | sensor supply | 3V3 → FB1 → AVDD; ADC DVDD |
| 2 | GND | ground | ground |
| 3 | GPIO10 | analog key 1 | **SPI CS** |
| 4 | GPIO7 | analog key 2 | spare, to probe pad TP1 |
| 5 | GPIO8 | ID 1.06 V | **ID 1.65 V** (100k/100k, the "Hall SPI" ID the V1 README reserved) |
| 6 | GPIO6 | probe pad | **SPI MISO** (ADC SDO through 33 Ω R3) |
| 7 | GPIO4 | probe pad | **SPI MOSI** (ADC SDI) |
| 8 | GPIO2 | probe pad | **SPI SCLK** |

ADC channels: key 1 = AIN5, key 2 = AIN4, key 3 = AIN3 (`KEY_CH` in `scripts/generate_he_v2.py`). AIN0–AIN2, AIN6 and AIN7 are unused.

## Firmware (not written yet)

V2 needs a new HE input driver before it does anything:
- **Detection:** ID 1.65 V. Until the driver exists, firmware that does not know this ID must treat the module as unknown and never as an analog HE module.
- **Reading:** SPI on GPIO10/6/4/2 to the ADS7038, auto-sequence of the three channels with averaging.
- **Third key:** the keypress path, the protocol and the app all handle two keys today.
- **Board test:** pins 6–8 become SPI, so its spare-line check must change.

## Mechanics

- **Outline:** 71.05 × 24.0 mm, 1.5 mm corners, 1.6 mm FR-4, 2 layers. 16.475 mm from each outer key centre to the board edge, as on V1.
- **Keys:** KiCad (128.475, 83.5), (147.525, 83.5), (166.575, 83.5). Plate-mount peg holes 1.75 mm at ±5.08 mm on each key.
- **Mounting holes:** 2.2 mm (M2) at (115.5, 83.5) and (179.55, 83.5), 3.5 mm from each end on the key axis.
- **J1:** centred on the middle key at the rear edge, at the V1 offset from the key line, so the same 100 mm cable reaches it.
- **The V1.1 case does not fit:** its key deck has two keys and a 52 mm module. A three-key case is needed.

## Files and toolchain

```
V2/
├── README.md
├── lib/                       osupad.pretty + osupad.kicad_sym: the V1 parts used, plus
│                              WQFN-16 (TI RTE0016C land pattern), L_0603, ADS7038, FerriteBead
├── HE/he_input_v2.*           KiCad 10 project, schematic, routed board
│   └── production/            exports (see below)
└── scripts/
    ├── generate_he_v2.py      library, schematic and placed board (overwrites them)
    ├── route_he_v2.py         Freerouting, then GND stitching vias, then zone refill
    └── export_v2.py           ERC + DRC + parity, then the fab files (reuses V1's export_production.py)
```

To rebuild (needs KiCad 10's `pcbnew` Python module and Java):

```
python3 hardware/pcb/V2/scripts/generate_he_v2.py   # placed, unrouted board
python3 hardware/pcb/V2/scripts/route_he_v2.py      # routes it (Freerouting 1.9.0)
python3 hardware/pcb/V2/scripts/export_v2.py        # checks, then hardware/pcb/V2/HE/production/
```

- `route_he_v2.py` routes a copy without zones, because Freerouting treats pours and rule areas as obstacles. It then adds the stitching vias that clear the routed copper and refills the zones.
- Freerouting 2.1.0 routes this board completely and then hangs without writing the result, so the script uses 1.9.0 (`FREEROUTING_JAR`, default `~/.kicad-mcp/freerouting-1.9.0.jar`).
- Re-running `generate_he_v2.py` discards the routing.

Checked on 2026-10-23: ERC 0, DRC 0 violations and 0 unconnected with zones refilled, schematic parity clean. U1–U3 and J1 have the same pad layout and the same CPL rotations (180° and 0°, bottom) as the built V1.1 board.

## Ordering (JLCPCB)

Upload from `HE/production/jlcpcb/`:
- `he_input_v2-gerbers.zip` for the bare board;
- `he_input_v2-BOM-JLCPCB.csv` and `he_input_v2-CPL-JLCPCB.csv` for assembly.

Order options:
- 2 layers, 1.6 mm, HASL or ENIG;
- min track and spacing 0.2 / 0.15 mm (JLCPCB's standard process does 0.1 / 0.1);
- vias 0.6 mm with a 0.3 mm hole;
- assembly side **Bottom**;
- for the order-number marker, "Specify a location": `JLCJLCJLCJLC` on the bottom silkscreen.

| Designator | Part | LCSC |
|---|---|---|
| U1, U2, U3 | TI DRV5055A3QDBZR linear Hall sensor, SOT-23 | C266128 |
| U4 | TI ADS7038QRTERQ1, WQFN-16 3 × 3 mm (automotive grade of the ADS7038, same die and pinout; the commercial ADS7038IRTER, C2871580, had 8 in stock on 2026-10-23) | C2876384 |
| J1 | JST SM08B-SRSS-TB | C160407 |
| FB1 | Sunlord GZ1608D601TF ferrite bead, 600 Ω at 100 MHz, 0603 | C1002 |
| C1, C6, C7, C8 | 100 nF 50 V X7R 0603 | C14663 |
| C2 | 10 µF 10 V X5R 0603 | C19702 |
| C3, C4, C5 | 1 µF 50 V X5R 0603 | C15849 |
| C9, C10, C11 | 10 nF 50 V X7R 0603 | C57112 |
| R1, R2 | 100 kΩ 1 % 0603 | C25803 |
| R3 | 33 Ω 5 % 0603 | C23140 |
| R5, R6, R7 | 220 Ω 1 % 0603 | C22962 |

Every LCSC number above was checked against its LCSC product page on 2026-10-23 (title, maker and part number), because V1 once shipped with a wrong one (V1 README, Ordering). TP1 is a bare probe pad: it is not in the BOM or the CPL.

**Before paying, check JLCPCB's placement preview:**
- **U1–U3:** the single lead (pin 3, GND) points to the board's front edge, as on V1.1. The generator asserts the same pad layout as the built V1.1 board.
- **U4:** pin 1 at the silkscreen dot.
- **J1:** contacts towards the inside of the board, opening at the rear edge.
