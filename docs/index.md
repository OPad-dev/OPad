---
title: OPad Documentation
description: OPad is a low-latency osu! keypad on the Waveshare ESP32-S3-Touch-LCD-2, with optional desktop software for Linux and Windows 10/11.
sidebar:
  order: 1
---

OPad is a three-key osu! keypad built on the Waveshare ESP32-S3-Touch-LCD-2. It is a 1000 Hz USB keyboard: osu! plays with two keys, the third is there for your play style, and the touchscreen is a fourth button. <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_config_desc -->

You do not need any software to play: plug the pad in and it types like any keyboard, on Linux and Windows 10/11, with no driver. The optional desktop app (`opad-daemon`, `opad-gui` and `opadctl`) adds the extras: <!-- src: /AGENTS.md#1. Product rules -->

- live osu! map and play data on the pad's screen, through the bundled tosu;
- a designer for the pad's idle and in-map screens;
- lifetime key counters kept on the pad and on your PC;
- touchscreen swipe actions;
- Hall Effect calibration and rapid trigger;
- firmware updates over USB.

## Get started

1. [What you need](start/what-you-need.md): your OPad, a USB-C data cable and a supported OS.
2. [First connection](start/first-connection.md): plug the pad in and check that it types.
3. [Installing the desktop app](start/install-the-app.md): Linux packages, AppImage and the Windows installer.

## Build your own

- [Parts and boards](build/parts.md): the Waveshare board, switches or V3 PCBs, cables and case.
- [Wiring the keys](build/wiring-the-keys.md): hand-wire two switches, or assemble the V3 carrier and module.

## Common tasks

- [Update the firmware](guides/update-firmware.md)
- [Use the touchscreen and swipes](guides/touchscreen-and-swipes.md)
- [Customize the screen layout](guides/customize-screen-layout.md)
- [Calibrate Hall Effect switches](guides/calibrate-hall-effect.md)
- [Change the key pins](guides/change-key-pins.md)
- [Use OPad with tosu](guides/use-with-tosu.md)
- [Back up and restore](guides/backup-and-restore.md)
- [Uninstall the software](guides/uninstall.md)

## When something goes wrong

- [Troubleshooting](troubleshooting.md): keyboard layouts, tosu on Linux, USB permissions, Windows warnings.
- [Counters, recovery and unbinding](recovery.md): how counters are reconciled, reflashing, and bringing back a pad that does not show up. No BOOT or RESET button is ever needed.

## Reference and background

- **Reference:** [`opadctl`](reference/opadctl.md), [GUI](reference/gui.md), [device settings](reference/device-settings.md), [pinout](reference/pinout.md), [serial protocol](reference/serial-protocol.md), [daemon IPC](reference/ipc.md), [files and paths](reference/files-and-paths.md).
- **How it works:** [architecture](explanation/architecture.md), [latency](explanation/latency.md), [the touchscreen as a fourth button](explanation/touchscreen-button.md), [app and pad versions](explanation/app-and-pad-versions.md), [counters and ownership](explanation/counters-and-ownership.md), [updates and rollback](explanation/updates-and-rollback.md), [Hall Effect and rapid trigger](explanation/hall-effect.md).
- **Contributing:** the [developer hub](develop/index.md) covers building, testing, internals, packaging and releases. The [testing checklist](testing-checklist.md) lists every hardware and software check.
