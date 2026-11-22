---
title: What You Need
description: Hardware prerequisites, board specifications, compatible switches, USB cables, and operating system requirements for building and using OPad.
sidebar:
  order: 1
---

OPad is built around an off-the-shelf microcontroller development board paired with either hand-wired switches or custom OPad V3 modular PCBs. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md -->

## Hardware Requirements

### 1. Main Controller Board
- **Waveshare ESP32-S3-Touch-LCD-2**: ESP32-S3R8 microcontroller (240 MHz dual-core Xtensa LX7, **16 MB Flash**, **8 MB Octal PSRAM**), integrated 2.0-inch ST7789 IPS LCD (`320x240`), and CST816S capacitive touchscreen. <!-- src: /README.md --> <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 2. Switches & Input Modules
You can build an OPad with hand-wired mechanical switches or the OPad V3 modular PCBs: <!-- src: /hardware/pcb/V3/README.md -->
- **Hand-Wired Mechanical (MX)**: Two standard Cherry MX-compatible mechanical switches (Cherry, Gateron, Kailh, Outemu, etc.) wired between header P2 and `GND`. No external resistors or diodes are needed. <!-- src: /README.md --> <!-- src: /firmware/main/input/keypad.c -->
- **OPad V3 Modular PCBs**:
  - **Controller Carrier V3**: Plugs directly onto headers P1 and P2 of the Waveshare board and connects to the input module via a 14-way, 1.0 mm type B FFC cable (80 mm in the V3 case) and to the USB board via an 8-way, 1.0 mm type A FFC cable. <!-- src: /hardware/pcb/V3/README.md -->
  - **MX Input Module V3**: 3-key mechanical hotswap PCB with Kailh hot-swap sockets (`CPG151101S11`). <!-- src: /hardware/pcb/V3/README.md -->
  - **HE Input Module V3**: 3-key Hall Effect magnetic switch PCB with TI DRV5055A3 Hall sensors, TI ADS7028 12-bit SPI ADC, and dedicated LP5907 3.3 V LDO. <!-- src: /hardware/pcb/V3/README.md -->
  - **USB Board V3**: Passive rear USB-C daughterboard with ESD protection, ferrite, and back-feed Schottky diode. <!-- src: /hardware/pcb/V3/README.md -->

### 3. USB Cable
- A USB Type-C **data** cable (USB-C to USB-A or USB-C to USB-C). Charge-only cables supply power but cannot enumerate USB HID or CDC interfaces.

## Host Computer Requirements

OPad supports **Linux** and **Windows 10/11**: <!-- src: /README.md -->

### Linux
- Standard kernel `usbhid` and `cdc_acm` drivers.
- Supported release package targets: Ubuntu 22.04 / 24.04 / 26.04, Debian 12 / 13, RHEL / AlmaLinux / Rocky 9 & 10, current Fedora, and Arch Linux (release packages and bundled `tosu` require glibc 2.28 or newer). <!-- src: /README.md -->
- `udev` rule `70-opad.rules` for non-root serial and input access (installed automatically by `.deb` and `.rpm` packages). <!-- src: /packaging/linux/udev/70-opad.rules -->

### Windows
- **Windows 10 or Windows 11 (64-bit / x64)**. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->
- Built-in inbox drivers (`hidclass.sys` for HID and `usbser.sys` for CDC serial); no third-party driver or Zadig setup is required. <!-- src: /README.md -->

### Optional Desktop Software
OPad works immediately as a 1000 Hz USB HID keyboard with no software installed. The optional desktop suite (`opad-daemon`, `opad-gui`, `opadctl`, and bundled `tosu`) adds live osu! telemetry, display layout editing, lifetime counters, Hall Effect tuning, and firmware updates. <!-- src: /AGENTS.md --> <!-- src: /README.md -->
