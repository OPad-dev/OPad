---
title: First Connection
description: Plug OPad in and check that it works as a USB keyboard, with no software or driver installed.
sidebar:
  order: 3
---

OPad is a standard USB keyboard. It needs no driver, daemon or app to type, so you can test it on any Linux or Windows 10/11 PC straight away. <!-- src: /AGENTS.md#1. Product rules -->

## Plug it in

1. Connect a USB-C **data** cable to the Waveshare board, or to the rear USB-C port of a V3 case.
2. Plug the other end into your PC, preferably straight into the PC rather than through a hub.
3. The screen turns on and the pad appears on your PC as a keyboard.

> [!NOTE]
> A Hall Effect pad that has never been calibrated shows its calibration screen instead and sends no keys until you calibrate it. See [Calibrating Hall Effect switches](../guides/calibrate-hall-effect.md). <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Try the keys

Open any text editor:

1. Press **Key 1**: `z` appears.
2. Press **Key 2**: `x` appears.
3. On a V3 module, press **Key 3**: `c` appears.
4. Touch the screen and keep your finger down: `` ` `` (backtick) is held until you lift it. Without the desktop app every touch is osu!'s Quick Retry key. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->

These are the defaults: <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/config/device_config.h -->

| Control | Default key (US QWERTY) | HID usage |
|---|---|---|
| Key 1 | `z` | `0x1D` |
| Key 2 | `x` | `0x1B` |
| Key 3 (V3 modules) | `c` | `0x06` |
| Touchscreen | `` ` `` (Quick Retry) | `0x35` |

The pad sends key positions, not letters, so on another keyboard layout you may see different characters (`w` instead of `z` on AZERTY, for example). That is expected; see [Troubleshooting §1](../troubleshooting.md#1-keyboard-layouts-qwerty-azerty-qwertz).

## Check how your PC sees it

You do not need this to play. It helps when something does not work.

OPad shows up as one USB device with two HID interfaces (the keyboard, and media controls) and one serial (CDC-ACM) port that the desktop app uses. Both HID interfaces are polled every 1 ms (1000 Hz). <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_config_desc -->

| Field | Value |
|---|---|
| USB ID | `303a:8415` (`303a:1001` is the ESP32-S3's built-in bootloader, which you only see while flashing) |
| Manufacturer | `GFerreiroS` |
| Product | `OPad ESP32-S3` |
| Serial number | `OPAD-` followed by 12 hex digits from the board's MAC address |
<!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_device_desc --> <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_string_desc --> <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

**Linux:**

```bash
lsusb -d 303a:8415
```

The pad's serial port appears as `/dev/ttyACM*`.

**Windows:** open **Device Manager**. The pad's keys appear under **Keyboards**, and its serial port under **Ports (COM & LPT)** as **OPad (COMx)**; the pad's own USB descriptor gives it that name. <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_ms_os_20_desc -->

## Next

The pad is ready to play with. For live map data on the screen, the screen designer, swipes and firmware updates, continue with [Installing the desktop app](install-the-app.md).
