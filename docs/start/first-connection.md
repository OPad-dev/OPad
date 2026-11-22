---
title: First Connection
description: Connecting OPad to a host computer, verifying 1000 Hz USB HID keyboard enumeration without software, and default key mapping tests.
---

The keypad is designed to function as an independent, driverless USB keyboard immediately upon being plugged into a computer (Product Rule 2). No background daemon, companion app, or specialized device drivers are required for basic typing and rhythm gaming. <!-- src: /AGENTS.md -->

## Plugging in the Pad

1. Connect a USB Type-C data cable to the Type-C port on the Waveshare ESP32-S3 board (or the rear Type-C port if using the OPad V3 case and carrier). <!-- src: /README.md -->
2. Connect the opposite end directly to a USB 2.0 or 3.0 port on your computer.
3. Upon connection:
   - The ESP32-S3 powers on immediately.
   - The 2.0-inch LCD display initializes and illuminates with the default OPad boot screen.
   - The device enumerates over USB within 1–2 seconds.

## USB HID Keyboard Verification

OPad enumerates on the host bus with custom USB descriptors (`firmware/main/usb/usb_descriptors.c`): <!-- src: /firmware/main/usb/usb_descriptors.c -->
- **Vendor ID (VID)**: `0x303A` (Espressif Systems)
- **Product ID (PID)**: `0x4001` (OPad Application Runtime)
- **Manufacturer String**: `"OPad"`
- **Product String**: `"OPad Keypad"`
- **Serial String**: Format `OPAD-<MAC_ADDRESS>` derived from the ESP32-S3 factory MAC address. <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

### Verifying on Linux
Open a terminal and run `lsusb`:
```bash
lsusb -d 303a:4001
```
Output confirms enumeration:
```
Bus 001 Device 024: ID 303a:4001 Espressif OPad Keypad
```
Inspect kernel logs with `dmesg`:
```bash
dmesg | grep -i opad
```
The output shows two HID keyboard interfaces and one CDC-ACM serial interface initialized at 1000 Hz polling rate ($bInterval = 1$). <!-- src: /firmware/main/usb/usb_descriptors.c -->

### Verifying on Windows
1. Open **Device Manager**.
2. Expand **Keyboards**: You will observe standard **HID Keyboard Device** entries registered by Windows.
3. Expand **Ports (COM & LPT)**: You will see a **USB Serial Device (COMx)** corresponding to the pad's CDC control interface.

## Default Key Mappings

Out of the box, OPad is pre-configured with standard rhythm game keys: <!-- src: /firmware/main/input/keypad.c -->

| Control | Default Pin | USB HID Usage | Output Character |
|---|---|---|---|
| **Key 1** | GPIO 14 (Header P2 Pin 11) | `0x1D` | `z` <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO --> |
| **Key 2** | GPIO 9 (Header P2 Pin 12) | `0x1B` | `x` <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO --> |
| **Key 3** (V3 Hall Effect) | GPIO 15 (Header P2 Pin 7) | `0x06` | `c` <!-- src: /firmware/main/usb/usb_hid.c --> |
| **Touchscreen** | CST816S Surface | `0x35` | `` ` `` / `~` (osu! Quick Retry) <!-- src: /firmware/main/usb/usb_hid.c --> |

### Quick Typing Test
Open any text editor or browser search bar:
1. Tap the physical switch wired to Key 1: character `z` appears on screen.
2. Tap the physical switch wired to Key 2: character `x` appears on screen.
3. Touch and hold the LCD touchscreen: character `` ` `` is entered.

The pad is now fully operational as an ultra-low-latency competitive keypad. To add song telemetry HUD overlays or customize actuation points, proceed to install the desktop suite. <!-- src: /README.md -->
