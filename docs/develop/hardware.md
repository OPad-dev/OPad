---
title: Hardware Architecture and PCB Ecosystem
description: Hardware generation history (V1, V2, V3), carrier boards, modular MX and Hall Effect input modules, custom 3D printed cases, and opadverify hardware verification suite.
---

# Hardware Architecture and PCB Ecosystem

OPad separates the host controller—a Waveshare ESP32-S3-Touch-LCD-2 board—from the physical key switches using an interchangeable input module architecture.

This document details the PCB iterations (V1 through V3), connector pinouts, cable mechanics, custom enclosure parameters, and the automated verification suite (`opadverify`).

<!-- src: /hardware/pcb/V3/README.md#OPad V3 PCBs -->
<!-- src: /hardware/pcb/V3/README.md#ADS7028 -->
<!-- src: /hardware/pcb/V3/README.md#LP5907 -->
<!-- src: /hardware/pcb/V1/README.md#OPad V1 PCBs -->
<!-- src: /hardware/pcb/V1/README.md#DRV5055A3 -->
<!-- src: /hardware/pcb/V2/README.md#OPad V2 PCBs -->
<!-- src: /hardware/pcb/V2/README.md#ADS7038 -->
<!-- src: /hardware/3d/README.md#Waveshare ESP32-S3-Touch-LCD-2 -->
<!-- src: /hardware/3d/README.md#opad_enclosure.scad -->
<!-- src: /hardware/verify/README.md#opadverify -->
<!-- src: /hardware/verify/README.md#test_system.py -->

---

## 1. Hardware Generations Overview

| Generation | Carrier Board | Cable Type | Key Modules | ADC & Sensor Architecture | Status |
|---|---|---|---|---|---|
| **V1** | Controller carrier V1 (C-shaped, headers P1/P2) | 8-pin JST SH (1.0 mm) | MX V1 (2-key)<br>HE V1.1 (2-key) | HE V1.1 routed analog voltages from TI DRV5055A3 sensors over cable to ESP32-S3 internal ADC1. | Superseded |
| **V2** | Controller carrier V1 | 8-pin JST SH (1.0 mm) | HE V2 prototype (3-key) | On-module digitization via TI ADS7038 12-bit SPI ADC. Internal averaging. | Prototype |
| **V3** | Controller carrier V3 + USB Board V3 | 14-way 1.0 mm FFC (Module)<br>8-way 1.0 mm FFC (USB) | MX V3 (3-key)<br>HE V3 (3-key) | On-module TI ADS7028 12-bit SPI ADC, LP5907 ultra-low noise LDO from 5 V VBUS, sensor ground keepouts. | **Current Production** |

---

## 2. V3 Production PCB Ecosystem

The V3 generation ([hardware/pcb/V3/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/pcb/V3/README.md)) resolves analog noise and structural strain by introducing a dedicated USB board and high-density flat flexible cables (FFC):

```mermaid
flowchart LR
    USB_C["USB-C Port\n(Rear Wall)"] -->|8-way FFC\n(Power + USB D+/D-)| USB_PCB["USB Board V3"]
    USB_PCB --> Carrier["Controller Carrier V3"]
    Carrier -->|Header Sockets| ESP["Waveshare ESP32-S3-Touch-LCD-2\n(Controller & Display)"]
    Carrier -->|14-way FFC\n(SPI / Keys / ID / Power)| Module["Input Module V3\n(MX V3 or HE V3, 3 Keys)"]
```

### Key Differences in V3

1. **Dedicated USB-C Board**: Relieves mechanical insertion stress from the Waveshare board's built-in connector. Contains ESD protection diodes, VBUS ferrite beads, filtering capacitors, and a Schottky diode preventing reverse feed.
2. **Reversible 14-way FFC Module Cable**: Replaces the fragile JST SH connector with a 1.0 mm pitch type B FFC.
3. **On-Module Digitization (HE V3)**: Features a TI **ADS7028** 12-bit SAR ADC connected via SPI. Analog sensor signals stay strictly on the module board.
4. **Isolated Sensor Power**: Hall Effect sensors and the ADS7028 reference voltage are powered by an onboard **LP5907** low-dropout regulator sourced from 5 V VBUS, completely bypassing the Waveshare switching buck converter noise.
5. **Single-Sided Assembly**: All surface-mount components reside on the bottom side of the modules and top side of the carrier, minimizing manufacturing cost at JLCPCB.

---

## 3. Cable Pinouts and Wiring

### Module Cable (14-Way FFC, 1.0 mm Pitch, Type B)

The 14-way FFC connects the Carrier V3 to either an MX V3 or HE V3 module:

| Contact | Carrier Net | HE V3 Net | MX V3 Net | Notes |
|---|---|---|---|---|
| **1** | GND | GND | GND | Ground |
| **2** | 5V | 5V | – | VBUS power to LP5907 LDO |
| **3** | 3V3 | 3V3 | 3V3 | Logic level supply |
| **4** | GPIO8 | ID Divider | ID Divider | Module ID voltage (ADC1_CH7) |
| **5** | GPIO6 | SPI MISO | KEY3 | Digital input / SPI |
| **6** | GND | GND | GND | Ground shield |
| **7** | GPIO2 | SPI SCLK | – | SPI clock (ADC1_CH1) |
| **8** | GPIO4 | SPI MOSI | – | SPI MOSI (ADC1_CH3) |
| **9** | GND | GND | GND | Ground shield |
| **10** | GPIO10 | SPI CS | KEY1 | Chip select / Key 1 (ADC1_CH9) |
| **11** | GPIO7 | – | KEY2 | Key 2 (ADC1_CH6) |
| **12** | 3V3 | 3V3 | 3V3 | Logic level supply |
| **13** | 5V | 5V | – | VBUS power |
| **14** | GND | GND | GND | Ground |

> [!NOTE]
> **Cable Reversibility**: If the 14-way cable is inserted reversed (connecting contact $k$ to $15 - k$), power and ground remain pinned to power and ground. The module ID line swaps between GPIO8 and GPIO7. Firmware samples both ADC channels at startup and remaps the GPIO matrix automatically.

#### Module ID Voltage Detection

Each input module contains a resistor divider on the ID line sampled by the controller ADC at startup:

| Module Type | Resistor Divider ($R_{top} / R_{bottom}$) | Nominal ID Voltage | Firmware Threshold Range |
|---|---|---|---|
| **MX V1** | 100 kΩ / 10 kΩ | 0.30 V | $0.00\,\text{V} - 0.68\,\text{V}$ |
| **HE V1.1** | 100 kΩ / 47 kΩ | 1.06 V | $0.68\,\text{V} - 1.35\,\text{V}$ |
| **HE V3 (and V2)**| 100 kΩ / 100 kΩ | 1.65 V | $1.35\,\text{V} - 1.95\,\text{V}$ |
| **MX V3** | 47 kΩ / 100 kΩ | 2.24 V | $1.95\,\text{V} - 2.60\,\text{V}$ |
| **Unconnected / None** | Floating (internal pull-down) | ~0.00 V | Defaults to hand-wired headers |

### USB Cable (8-Way FFC, 1.0 mm Pitch, Type A)

Connecting the rear USB board to the carrier board:

| Contact | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| **Net** | GND | 5V | D− | D+ | D+ | D− | 5V | GND |

The pinout is fully symmetric: contact $k$ maps identically to $9 - k$. USB D+ and D− lines use paired conductors for signal integrity, and the dual 5V pins safely carry up to 1.0 A combined.

---

## 4. 3D Enclosure Design

The custom enclosure is parametrically modeled in OpenSCAD ([hardware/3d/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/3d/README.md)):

- **Top Case & Bottom Deck**: `hardware/3d/custom_case/V3/opad_enclosure.scad`.
- **Display Dimensions**: Accommodates the Waveshare glass lens ($37.52 \times 59.22\,\text{mm}$ envelope) and active display viewport ($30.99 \times 41.20\,\text{mm}$).
- **Switch Plate**: Standard 1.5 mm plate thickness with $14.0 \times 14.0\,\text{mm}$ cutouts and standard $19.05\,\text{mm}$ center-to-center key pitch.
- **Assembly**: Held together with four corner locating pins and clamping posts, without requiring screws through the switch PCB.

---

## 5. Automated Hardware Verification (`opadverify`)

To prevent manufacturing errors (such as the mirrored SOT-23 footprint on HE V1.0), OPad uses a custom verification suite ([hardware/verify/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/verify/README.md)):

```bash
# Setup verification environment
python3 -m venv .tools/venv && .tools/venv/bin/pip install -r hardware/verify/requirements.txt
cd hardware/verify

# Verify golden netlist against Gerber fabrication output
../../.tools/venv/bin/python -m opadverify golden/carrier_v3.toml
../../.tools/venv/bin/python -m opadverify golden/he_v3.toml

# Run cable orientation and net continuity verification
../../.tools/venv/bin/python tests/test_system.py
```

### Verification Checks

1. **Footprint Handedness**: Rebuilds connectivity directly from Gerber copper geometries and plated drill holes, verifying pin handedness against manufacturer datasheets.
2. **JLCPCB CPL Alignment**: Fetches JLCPCB's exact EasyEDA footprint models for all BOM parts and validates that placed parts land precisely on target pads without manual rotation.
3. **Cable Reversal Simulation**: `test_system.py` executes end-to-end net tracing from module pads to ESP32 GPIOs under both standard and reversed cable orientations.
