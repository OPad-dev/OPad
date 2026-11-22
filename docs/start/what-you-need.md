---
title: What You Need
description: Hardware prerequisites, board specifications, compatible switches, USB cables, and operating system requirements for building and using OPad.
---

OPad is built around an off-the-shelf microcontroller development board paired with either hand-wired switches or custom modular PCBs. <!-- src: /README.md -->

## Hardware Requirements

### 1. Main Controller Board
- **Waveshare ESP32-S3-Touch-LCD-2**: The standard hardware platform featuring an ESP32-S3 microcontroller (240 MHz dual-core Xtensa LX7, 8 MB Flash, 2 MB PSRAM), an integrated 2.0-inch ST7789 IPS LCD display (320x240 resolution), and a CST816S capacitive touchscreen. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 2. Switches & Input Modules
You can build an OPad using either hand-wired mechanical switches or custom PCB modules: <!-- src: /hardware/pcb/V3/README.md -->
- **Hand-Wired Mechanical (MX)**: Two standard Cherry MX-compatible mechanical keyboard switches. Any switch brand (Cherry, Gateron, Kailh, Outemu) is supported. Switches connect between header P2 and ground with standard jumper wires.
- **OPad V3 Modular PCBs (Recommended)**:
  - **Controller Carrier V3**: Carrier board that plugs directly into the Waveshare development board headers. <!-- src: /hardware/pcb/V3/README.md -->
  - **MX Input Module V3**: 3-key mechanical hotswap PCB with Kailh sockets. <!-- src: /hardware/pcb/V3/README.md -->
  - **HE Input Module V3**: 3-key magnetic Hall Effect PCB with rapid trigger sensors. Compatible with magnetic switches such as Gateron Magnetic Jade / KS-20. <!-- src: /hardware/pcb/V3/README.md -->
  - **USB Board V3**: Daughterboard routing USB Type-C signals cleanly to the case rear. <!-- src: /hardware/pcb/V3/README.md -->

### 3. USB Cable
- A high-quality USB Type-C to Type-A (or Type-C to Type-C) data cable.
- Ensure the cable supports reliable high-speed data transmission; cheap charge-only cables will supply power but cannot enumerate USB HID or serial CDC interfaces.

## Host Computer Requirements

OPad works on both Linux and Windows operating systems: <!-- src: /README.md -->

### Linux
- **Kernel**: Linux 5.15 or newer with standard `cdc_acm` and `usbhid` modules loaded.
- **Distributions**: Fully supported on Ubuntu, Debian, Fedora, Arch Linux, and derivatives.
- **Permissions**: A standard `udev` rule is required so regular users can access the CDC serial port without root (`70-opad.rules`).

### Windows
- **Operating System**: Windows 10 (64-bit) or Windows 11.
- **Drivers**: Standard USB HID and CDC ACM class drivers are built into Windows 10/11; no third-party INF files or WinUSB drivers are needed.

### Optional Desktop Software
The pad functions completely as a 1000 Hz USB keyboard without installing any desktop software (Product Rule 2). The optional desktop suite (`opad-daemon`, `opad-gui`, `opadctl`, and `tosu`) provides live in-game telemetry, screen design, and Hall Effect tuning. <!-- src: /README.md -->
