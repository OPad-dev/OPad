---
title: OPad Documentation
description: Overview of OPad, the low-latency ESP32-S3 osu! keypad and desktop suite, with starting points for users and developers.
sidebar:
  order: 1
---

OPad is a low-latency 1000 Hz USB HID osu! keypad built on the Waveshare ESP32-S3-Touch-LCD-2, paired with an optional Rust desktop suite (`opad-daemon`, `opad-gui`, `opadctl`) for Linux and Windows 10/11. <!-- src: /README.md --> <!-- src: /AGENTS.md -->

The keypad works out of the box as a driverless USB keyboard with no software installed. Adding the optional desktop suite enables live osu! gameplay telemetry via `tosu`, a visual screen layout designer, lifetime key counters, touchscreen swipe actions, Hall Effect switch calibration and rapid trigger tuning, and hands-free USB firmware updates. <!-- src: /AGENTS.md --> <!-- src: /README.md -->

## Start Here (Using OPad)

- [What You Need](start/what-you-need.md) — Waveshare board, mechanical or Hall Effect switches, OPad V3 PCBs, and OS requirements.
- [Wiring the Keys](start/wiring-the-keys.md) — hand-wiring switches to Header P2 or assembling the V3 carrier and modules.
- [First Connection](start/first-connection.md) — plugging in the pad and verifying 1000 Hz USB HID typing without software.
- [Installing the Desktop Suite](start/install-the-app.md) — Linux packages (`.deb`, `.rpm`, Arch, AppImage) and the Windows installer.

## Common Tasks

- [Updating Firmware](guides/update-firmware.md) — updating over USB from `opad-gui` or `opadctl` with automatic A/B rollback protection.
- [Touchscreen & Swipes](guides/touchscreen-and-swipes.md) — Quick Retry in maps and configurable swipe actions outside maps.
- [Customizing the Screen Layout](guides/customize-screen-layout.md) — designing, previewing, exporting, and importing custom `Idle` and `Playing` layouts in Screen Designer.
- [Calibrating Hall Effect Switches](guides/calibrate-hall-effect.md) — guided 5-press calibration and actuation / rapid trigger tuning.
- [Changing Key Pins](guides/change-key-pins.md) — rebinding hand-wired switch GPIOs and using **Auto-Detect**.
- [Using OPad with tosu](guides/use-with-tosu.md) — live osu! telemetry setup and Linux `ptrace` configuration.
- [Backup and Restore](guides/backup-and-restore.md) — exporting and importing configuration and lifetime key counters.
- [Recovery, Reflashing & Unbinding](recovery.md) — recovering an unresponsive pad without BOOT/RESET buttons, counter reconciliation, and unbinding.
- [Troubleshooting](troubleshooting.md) — non-QWERTY layouts, Linux Yama `ptrace_scope`, `udev` rules, Windows SmartScreen, and recovery.
- [Uninstalling OPad](guides/uninstall.md) — removing the desktop suite cleanly on Linux and Windows.

## Reference & Architecture

- **Reference**: [`opadctl` CLI](reference/opadctl.md), [GUI Reference](reference/gui.md), [Device Settings](reference/device-settings.md), [Pinout](reference/pinout.md), [Serial Protocol](reference/serial-protocol.md), [Daemon IPC](reference/ipc.md), [Files & Paths](reference/files-and-paths.md).
- **Explanations**: [System Architecture](explanation/architecture.md), [Latency Engineering](explanation/latency.md), [Touchscreen as Third Button](explanation/touchscreen-third-button.md), [App & Pad Versions](explanation/app-and-pad-versions.md), [Counters & Ownership](explanation/counters-and-ownership.md), [Updates & Rollback](explanation/updates-and-rollback.md), [Hall Effect & Rapid Trigger](explanation/hall-effect.md).

## Developing OPad

- [Developer Hub](develop/index.md) — contributor workflow, building firmware (ESP-IDF v5.5.2) and desktop (Rust), running tests, firmware and desktop internals, protocol changes, hardware, packaging, releases, and latency testing.
- [Testing Checklist](testing-checklist.md) — full hardware and software verification matrix.
