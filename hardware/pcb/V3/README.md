# OPad V3 PCBs

| Board | Folder | Size | Status |
|---|---|---|---|
| Controller carrier V3 | `Carrier/` | 35.0 × 62.5 mm (the V1 outline plus a USB-C tab) | Designed, routed, checked. **Not ordered, not built** |
| MX input module V3 | `MX/` | 71.05 × 24.0 mm, 3 keys | Designed, routed, checked. **Not ordered, not built** |
| HE input module V3 | `HE/` | 71.05 × 24.0 mm, 3 keys | Designed, routed, checked. **Not ordered, not built** |

These replace every V1/V2 board (decisions H10–H17 and section 4.2 in
`docs/specs/hardware-redesign-brief.md`). **No firmware supports them yet.**
The work still to do is listed under "Before ordering" below.

## What is different from V1

- **One cable:** a 14-way, 1.0 mm FFC replaces the JST SH 8-pin. It is reversible
  (a reversed cable only swaps signals between ESP32 pins, see Cable).
- **USB-C on the carrier,** on a tab past the Waveshare's USB end with the mouth on
  the rear edge. It has ESD protection, a VBUS ferrite and capacitors, and a Schottky
  so a cable in the Waveshare's own port cannot feed this one. The port points back
  and 16° up with the screen tilt. The carrier must be screwed to its posts so the
  plug does not load the header sockets. (The V3 case does this with pins on its
  bottom-plate posts, without screws.)
- **Three keys** on both modules, and **four corner holes** for the case's locating pins.
- **HE:**
  - The ADC is an **ADS7028** (C880341), the ADS7038's sibling. It has the same pins,
    registers, averaging and CRC, and it is in JLCPCB's assembly stock. The ADS7038 is
    only on LCSC.
  - An **LP5907 LDO** powers the sensors and the ADC reference from the cable's 5 V,
    not from the Waveshare's buck-converter 3V3.
  - CS has a pull-up, and the SPI lines have ESD protection.
- **Every part is on one side:** the top of the carrier, the bottom of the modules.

## Cable (14-way FFC, 1.0 mm, any type A or B, about 100 mm)

| Contact | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Carrier | GND | 5V | 3V3 | GPIO8 | GPIO6 | GND | GPIO2 | GPIO4 | GND | GPIO10 | GPIO7 | 3V3 | 5V | GND |
| HE | GND | 5V | 3V3 | ID | MISO | GND | SCLK | MOSI | GND | CS | – | 3V3 | 5V | GND |
| MX | GND | – | 3V3 | ID | KEY3 | GND | – | – | GND | KEY1 | KEY2 | 3V3 | – | GND |

A reversed cable connects contact k to 15 − k. Power and ground still meet power and
ground. The module's ID arrives on GPIO7 instead of GPIO8 (both are ADC1 pins), and the
firmware swaps the other pins in the GPIO matrix. `hardware/verify/tests/test_system.py`
follows every pin both ways round. MX keeps KEY1 = GPIO10 and KEY2 = GPIO7, as on V1.

Module IDs: MX V3 = 47k/100k = 2.24 V, HE V3 = 100k/100k = 1.65 V (the class HE V2
reserved, since the ADC and pins are the same). Firmware cut-offs: 0.68 / 1.35 / 1.95 / 2.6 V.

## Checks (2026-10-23)

| Check | Carrier | MX | HE |
|---|---|---|---|
| KiCad ERC, DRC with zones refilled, schematic parity | 0 | 0 | 0 |
| `hardware/verify` (fab files against the intended netlist, datasheet pin drawings, JLCPCB's own part models at the CPL) | 0 errors | 0 errors | 0 errors |
| `hardware/verify/tests/test_system.py` (cable both ways, ID margins) | all pass | all pass | all pass |

The only warning is that LCSC publishes no footprint for the 0603 ferrite C1002. It is a
two-terminal part, so only its position is unchecked.

- **CPL:** `export_v3.py` writes the CPL by fitting JLCPCB's own footprint of every part
  onto our pads. Every polarised bottom-side part therefore sits at 0° or 180°. That
  is the case the evidence covers (`hardware/verify/opadverify/jlc.py`). For that reason
  the Hall sensors are turned a quarter from V1.1; the generator asserts that their pads
  are V1.1's layout rotated, never mirrored.
- **Sensor keepout:** a 3 × 3 mm copper keepout sits round each Hall sensor, as on V1.1.

## Files

```
V3/
├── lib/                 footprints (new ones drawn from the makers' drawings) and symbols
├── Carrier/ MX/ HE/     KiCad projects, route.json (stitching vias, power track widths), production/
└── scripts/
    ├── v3lib.py         library: footprints, symbols
    ├── generate_v3.py   schematics and placed boards (overwrites the routing)
    ├── route_v3.py      Freerouting 1.9.0, retried until DRC has no error and nothing unconnected
    └── export_v3.py     V1's export_production.py checks and files, with the JLCPCB-model CPL
```

```
python3 hardware/pcb/V3/scripts/generate_v3.py [carrier] [mx] [he]
python3 hardware/pcb/V3/scripts/route_v3.py Carrier|MX|HE
python3 hardware/pcb/V3/scripts/export_v3.py [Carrier] [MX] [HE]
cd hardware/verify && ../../.tools/venv/bin/python -m opadverify golden/he_v3.toml   # and carrier_v3, mx_v3
```

## Before ordering

1. **Case:** designed (`hardware/3d/custom_case/V3`, fit-checked, not printed yet).
   The screen stack goes in from below against a lip. Three bottom-plate posts with pins
   in the carrier's M2 holes hold it and take the USB plug's push, so no spacers or screws.
   The module cable route is about 70 mm, so an 80 mm FFC fits best; 100 mm fits with an S-fold.
2. **Firmware:**
   - the ADS7028/7038 driver;
   - the third key;
   - cable-reversal detection;
   - the new ID classes;
   - the protocol and app for three keys.
3. **JLCPCB preview:** check every part sits on its pads before paying. The checker
   predicts no turning is needed (`-v` prints where pin 1 of each polarised part must be).
4. **Order 5, assemble 2** of each board first.

## Parts (JLCPCB, all in assembly stock on 2026-10-23)

| Board | Part | LCSC |
|---|---|---|
| all | JUSHUO AFA07-S14FCA-00 FFC connector | C262720 |
| carrier | HRO TYPE-C-31-M-12 USB-C | C165948 |
| carrier | XUNPU FH2.54-09-14PZD 1×14 socket, 8.5 mm (×2) | C7500781 |
| carrier, HE | ProTek SRV05-4-P-T7 ESD array | C85364 |
| carrier | Murata BLM18PG121SN1D ferrite, 2 A | C14709 |
| carrier | B5817W Schottky (JLCPCB's model: pad 1 anode, pad 2 cathode) | C7420328 |
| carrier, HE | Sunlord GZ1608D601TF ferrite | C1002 |
| MX | Kailh CPG151101S11 hot-swap socket (×3) | C41430893 |
| HE | TI DRV5055A3QDBZR Hall sensor (×3) | C266128 |
| HE | TI ADS7028IRTER ADC | C880341 |
| HE | TI LP5907MFX-3.3 LDO | C80670 |

The full lists, with the passives, are in each board's `production/jlcpcb/*-BOM-JLCPCB.csv`.
The cable is not assembled: any 14-way, 1.0 mm FFC, type A or B, 80 mm for the V3 case
(LCSC C54350071 type A, C54350068 type B; 100 mm, C34155 / C64627, also fits).
