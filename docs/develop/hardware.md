---
title: Hardware
description: OPad's board generations (V1, V2, V3), the carrier and input modules, the cables and module ID, the case, and the opadverify checker that runs before any board is ordered.
sidebar:
  order: 5
---

OPad keeps the Waveshare ESP32-S3-Touch-LCD-2 as an unmodified controller and puts everything else on its own PCBs: a carrier that plugs onto the Waveshare's headers, and an input module with the switches. The split means a module can change (MX to Hall Effect, two keys to three) without touching the controller or the firmware's core, and the module identifies itself through a resistor divider so the firmware picks the right input path at boot. The KiCad projects, generator scripts and full part lists are in [`hardware/pcb/`](https://github.com/OPad-dev/OPad/tree/main/hardware/pcb); this page is the overview.

## Generations

The OPad that is sold is **OPad V1**; its boards and case are the **V3** revision below. V1, V2 and V3 in these docs name board and case revisions, never the product. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) --> <!-- src: /hardware/pcb/V3/README.md -->

| Generation | Carrier Board | Cable Type | Key Modules | ADC & Sensor Architecture | Status |
|---|---|---|---|---|---|
| **V1** | Controller carrier V1 (headers P1/P2) | 8-pin JST SH (1.0 mm) | MX V1 (2-key), HE V1.1 (2-key) | Analog DRV5055A3 voltages routed over cable to ESP32-S3 internal ADC1. | Superseded ([/hardware/pcb/V1/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/pcb/V1/README.md)) |
| **V2** | Controller carrier V1 | 8-pin JST SH (1.0 mm) | HE V2 (3-key) | On-module TI ADS7038 12-bit SPI ADC. | Designed, never built; superseded by V3 ([/hardware/pcb/V2/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/pcb/V2/README.md)) |
| **V3** | Controller carrier V3 + USB Board V3 | 14-way 1.0 mm type B FFC (module), 8-way 1.0 mm type A FFC (USB) | MX V3 (3-key), HE V3 (3-key) | On-module TI **ADS7028** 12-bit SPI ADC, **LP5907** 3.3 V LDO from 5 V VBUS, 3x3 mm sensor ground keepouts. | Current; built and working on the bench ([/hardware/pcb/V3/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/pcb/V3/README.md)) |

## V3 boards

Defined in [/hardware/pcb/V3/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/pcb/V3/README.md):

```mermaid
flowchart LR
    USB_PCB["USB Board V3 (USB-C at the rear wall)"] -->|"8-way FFC (5 V, D+, D-)"| Carrier["Controller Carrier V3"]
    Carrier -->|"Header Sockets (P1/P2)"| ESP["Waveshare ESP32-S3-Touch-LCD-2"]
    Carrier -->|"14-way FFC (SPI / Keys / ID / Power)"| Module["Input Module V3 (MX V3 or HE V3, 3 Keys)"]
```

- **USB Board V3** (`20.0 x 22.0 mm`): Passive rear USB-C port with ProTek SRV05-4 ESD protection, Murata ferrite, capacitors, and B5817W Schottky diode preventing back-feeding from the Waveshare port.
- **Controller Carrier V3** (`35.0 x 48.2 mm`): Dual 1x14 female header sockets seating onto Waveshare P1/P2 plus 14-way module and 8-way USB FFC connectors.
- **MX Input Module V3** (`71.05 x 24.0 mm`): 3-key Kailh hot-swap (`CPG151101S11`) mechanical switch PCB.
- **HE Input Module V3** (`71.05 x 24.0 mm`): 3-key Hall Effect PCB with three TI DRV5055A3 sensors, TI ADS7028 12-bit SPI ADC, and TI LP5907 3.3 V LDO.
- **Single-sided assembly**: Top side on carrier and USB board; bottom side on MX/HE modules.

## Cables and module ID

### Module cable (14-way, 1.0 mm, type B, 80 mm)

| Contact | Carrier Pin | HE V3 | MX V3 |
|---|---|---|---|
| 1 | `GND` | `GND` | `GND` |
| 2 | `5V` | `5V` (to LP5907 LDO) | – |
| 3 | `3V3` | `3V3` | `3V3` |
| 4 | `GPIO 8` | `ID` (1.65 V) | `ID` (2.24 V) |
| 5 | `GPIO 6` | `MISO` | `KEY3` |
| 6 | `GND` | `GND` | `GND` |
| 7 | `GPIO 2` | `SCLK` | – |
| 8 | `GPIO 4` | `MOSI` | – |
| 9 | `GND` | `GND` | `GND` |
| 10 | `GPIO 10` | `CS` | `KEY1` |
| 11 | `GPIO 7` | – (alt `ID` if reversed) | `KEY2` |
| 12 | `3V3` | `3V3` | `3V3` |
| 13 | `5V` | `5V` | – |
| 14 | `GND` | `GND` | `GND` |

If the 14-way FFC is inserted reversed (`k` -> `15 - k`), power/ground still match and `ID` moves from `GPIO 8` to `GPIO 7`; the firmware detects the turned-over cable at boot and swaps the SPI3 pins for **HE V3** (`ads7028_carrier_pins`, pin table in [the ADS7028 notes](../hardware/components/ads7028.md)).  An **MX V3** module on a reversed cable is not read ([/hardware/pcb/V3/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/pcb/V3/README.md)).

### Module ID thresholds

The firmware reads the ID divider once at boot and picks the module's input path from the band it falls in ([module_id.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_id.h)). The bands sit between the nominal voltages so resistor tolerance cannot move a module into its neighbour's band. The runtime keeps polling the ID afterwards: a module pulled out or swapped while powered turns the keys off at once (a Hall Effect module's analog outputs on digital key pins would chatter), and a different module needs a restart ([module_watch.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_watch.h)).

| Module | Divider (`R_top / R_bottom`) | Nominal ID | Cut-off Range |
|---|---|---|---|
| **MX V1** | `100 kΩ / 10 kΩ` | `0.30 V` | `60 mV` – `680 mV` (loaded >= `60 mV`) |
| **HE V1.1** | `100 kΩ / 47 kΩ` | `1.06 V` | `680 mV` – `1350 mV` |
| **HE V3 (and V2)** | `100 kΩ / 100 kΩ` | `1.65 V` | `1350 mV` – `1950 mV` |
| **MX V3** | `47 kΩ / 100 kΩ` | `2.24 V` | `1950 mV` – `2600 mV` |
| **None** | Floating or shorted to 3V3 | `~0 V` or `>= 2.6 V` | Hand-wired defaults |

### USB cable (8-way, 1.0 mm, type A, 80 mm)

| Contact | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| **Net** | `GND` | `5V` | `D-` | `D+` | `D+` | `D-` | `5V` | `GND` |

Mirror-symmetric (`k` -> `9 - k`), carrying up to 1.0 A across the paired 5 V contacts.

## Case

Parametric OpenSCAD enclosure files live in [/hardware/3d/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/3d/README.md) (`hardware/3d/custom_case/V3/`), using a 1.5 mm switch plate with `14.0 x 14.0 mm` cutouts, `19.05 mm` key pitch, and four corner locating pins.

## Checking a board before ordering (`opadverify`)

[/hardware/verify/README.md](https://github.com/OPad-dev/OPad/blob/main/hardware/verify/README.md) checks generated Gerbers, drill files, BOMs, and JLCPCB CPL models against golden netlists and runs cable-reversal net tracing (`test_system.py`).

It exists because HE V1.0 shipped with a mirrored SOT-23 footprint that ERC, DRC and the assembler all passed: the checker rebuilds connectivity from the Gerber copper instead of trusting KiCad, and compares every footprint with the datasheet drawing and with JLCPCB's own part model at the CPL position.

```bash
python3 -m venv .tools/venv && .tools/venv/bin/pip install -r hardware/verify/requirements.txt
cd hardware/verify
../../.tools/venv/bin/python -m opadverify golden/he_v3.toml     # also carrier_v3, usb_v3, mx_v3; -v lists every pass
../../.tools/venv/bin/python tests/test_mutations.py             # proves each check can fail
../../.tools/venv/bin/python tests/test_system.py                # cables both ways round, ID margins
```

Exit status 0 means no error. The board files are regenerated with `hardware/pcb/V3/scripts/generate_v3.py`, `route_v3.py` and `export_v3.py`; see the V3 README before ordering.
