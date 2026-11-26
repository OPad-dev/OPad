---
title: Parts and Boards
description: The Waveshare board, the switches or OPad V3 PCBs, the cables and the case you need to build your own OPad.
sidebar:
  order: 1
---

This section is for building your own OPad. If you have a finished OPad, you have nothing to build: start at [What you need](../start/what-you-need.md).

An OPad you build is a Waveshare development board plus either two hand-wired switches or the OPad V3 PCBs. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md --> Building one takes three steps:

1. Get the parts on this page.
2. Wire the switches or plug the V3 boards together: [Wiring the keys](wiring-the-keys.md).
3. Plug it in and check that it types: [First connection](../start/first-connection.md). From there the steps are the same as for a finished OPad.

> [!NOTE]
> The OPad that is sold is **OPad V1**; its boards and case are internally the **V3** revision. When these docs say V1, V2 or V3, they mean a board or case revision, not the product. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) --> <!-- src: /hardware/pcb/V3/README.md -->

## The board

**Waveshare ESP32-S3-Touch-LCD-2.** It has an ESP32-S3 running at 240 MHz with 16 MB of flash and octal PSRAM, a 2.0-inch 320×240 ST7789 screen and a CST816 capacitive touch controller. The firmware needs nothing else on the board. <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Switches: pick one way to build it

- **Hand-wired:** two MX-style mechanical switches, each wired between a header pin and GND. You need no resistors or diodes, because the firmware turns on the ESP32-S3's internal pull-ups. See [Wiring the keys](wiring-the-keys.md). <!-- src: /firmware/main/input/keypad.c -->
- **OPad V3 PCBs** (three keys): <!-- src: /hardware/pcb/V3/README.md -->
  - **Controller carrier V3:** plugs onto the board's P1 and P2 headers.
  - **One input module:** the **MX input module V3** (Kailh hot-swap sockets) or the **HE input module V3** (Hall Effect: TI DRV5055 sensors, a TI ADS7028 ADC and its own LP5907 regulator).
  - **USB board V3:** the case's rear USB-C port, with ESD protection, a ferrite and a Schottky diode.
  - **Cables:** a 14-way, 1.0 mm type B FFC to the module and an 8-way, 1.0 mm type A FFC to the USB board (80 mm each in the V3 and V4 cases).

The KiCad projects and full part lists are in [`hardware/pcb/V3/`](https://github.com/OPad-dev/OPad/tree/main/hardware/pcb/V3); [Hardware](../develop/hardware.md) gives the overview.

## The case

The V4 case holds the carrier under the Waveshare board, the USB board with its USB-C port through the rear wall, and the three-key MX or Hall Effect module. Its OpenSCAD files and print notes are in [`hardware/3d/custom_case/V4/`](https://github.com/OPad-dev/OPad/blob/main/hardware/3d/custom_case/V4/README.md). <!-- src: /hardware/3d/custom_case/V4/README.md -->

V4 is the V3 case's inside in a faceted outside, and it has not been printed yet. The [V3 case](https://github.com/OPad-dev/OPad/blob/main/hardware/3d/custom_case/V3/README.md) is the printed, test-fitted reference; the boards and assembly are the same in both. <!-- src: /hardware/3d/custom_case/V4/README.md -->

## A USB-C data cable

Use a cable that carries data. A charge-only cable powers the pad, but your PC never sees it as a keyboard.

## Computer

A pad you build needs the same computer as a finished one: Linux or Windows 10/11, no driver. See [What you need](../start/what-you-need.md). <!-- src: /README.md -->
