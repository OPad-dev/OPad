---
title: What You Need
description: An OPad, a USB-C data cable and a Linux or Windows 10/11 computer. The desktop app is optional.
sidebar:
  order: 1
---

- **Your OPad.** It comes assembled, with three keys (mechanical or Hall Effect, depending on the version you chose; osu! plays with two, and the third is there for your play style), plus a touchscreen you can tap for Quick Retry during a map. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) --> <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /AGENTS.md#1. Product rules -->
- **A USB-C data cable.** One comes with the pad. If you use another, make sure it carries data: a charge-only cable powers the pad, but your PC never sees it as a keyboard. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) -->
- **A computer with Linux or Windows 10/11 (x64).** The pad uses the drivers your system already has (`usbhid` and `cdc_acm` on Linux, `hidclass.sys` and `usbser.sys` on Windows), so you never install a driver. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->

The desktop app is optional. Without it the pad is still a complete 1000 Hz keyboard, and its touchscreen sends Quick Retry. [Installing the desktop app](install-the-app.md) lists what the app adds and which Linux distributions its packages are built for. <!-- src: /AGENTS.md#1. Product rules -->

Building your own pad from a Waveshare board instead? Start with [Parts and boards](../build/parts.md).

## Next

Plug the pad in: [First connection](first-connection.md).
