---
title: What You Need
description: The board, switches or V3 PCBs, cable and operating system you need to build and use an OPad.
sidebar:
  order: 1
---

An OPad is a Waveshare development board plus either two hand-wired switches or the OPad V3 PCBs. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md -->

## Hardware

### The board

**Waveshare ESP32-S3-Touch-LCD-2.** It has an ESP32-S3 running at 240 MHz with 16 MB of flash and octal PSRAM, a 2.0-inch 320×240 ST7789 screen and a CST816 capacitive touch controller. The firmware needs nothing else on the board. <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### Switches: pick one way to build it

- **Hand-wired:** two MX-style mechanical switches, each wired between a header pin and GND. You need no resistors or diodes, because the firmware turns on the ESP32-S3's internal pull-ups. See [Wiring the keys](wiring-the-keys.md). <!-- src: /firmware/main/input/keypad.c -->
- **OPad V3 PCBs** (three keys): <!-- src: /hardware/pcb/V3/README.md -->
  - **Controller carrier V3:** plugs onto the board's P1 and P2 headers.
  - **One input module:** the **MX input module V3** (Kailh hot-swap sockets) or the **HE input module V3** (Hall Effect: TI DRV5055 sensors, a TI ADS7028 ADC and its own LP5907 regulator).
  - **USB board V3:** the case's rear USB-C port, with ESD protection, a ferrite and a Schottky diode.
  - **Cables:** a 14-way, 1.0 mm type B FFC to the module and an 8-way, 1.0 mm type A FFC to the USB board (80 mm each in the V3 case).

### A USB-C data cable

Use a cable that carries data. A charge-only cable powers the pad, but your PC never sees it as a keyboard.

## Computer

OPad works on **Linux** and **Windows 10/11 (x64)**. The pad uses the drivers your OS already has: `usbhid` and `cdc_acm` on Linux, `hidclass.sys` and `usbser.sys` on Windows. You never install a driver. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->

The desktop app's release packages are built for x86-64 and target: <!-- src: /README.md --> <!-- src: /scripts/release/release_notes.sh -->

- Ubuntu 22.04, 24.04 and 26.04; Debian 12 and 13 (`.deb`);
- Fedora and RHEL / AlmaLinux / Rocky 9 and 10 (`.rpm`);
- Arch Linux (`.pkg.tar.zst`);
- any other Linux (AppImage);
- Windows 10 and 11 (installer).

The Linux binaries are built against glibc 2.31 and the bundled tosu against glibc 2.28, so they run on every distribution above. <!-- src: /.github/workflows/release-app.yml --> <!-- src: /Makefile#TOSU_GLIBC -->

> [!NOTE]
> The desktop app is optional. Without it the pad is still a complete 1000 Hz keyboard, and its touchscreen sends Quick Retry. See [Installing the desktop app](install-the-app.md) for what the app adds. <!-- src: /AGENTS.md#1. Product rules -->
