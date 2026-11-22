---
title: Wiring the Keys
description: Step-by-step wiring guide for mechanical switches on Header P2, GPIO assignments, pull-up logic, and OPad V3 carrier assembly.
---

OPad reads switches active-low: each switch pin is configured with internal ESP32-S3 pull-up resistors (`GPIO_PULLUP_ONLY`). Pressing a switch connects the GPIO pin directly to ground (`GND`). No external resistors, diodes, or capacitors are required. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Hand-Wiring Mechanical Switches (MX)

When building an OPad on a breadboard or custom enclosure using the raw Waveshare ESP32-S3-Touch-LCD-2 board, switches wire directly to Header P2: <!-- src: /docs/reference/pinout.md -->

```
Switch 1 (Key 1) ------------ P2 Pin 11 (GPIO 14)
                 \
                  ----------- P2 Pin 2 / Pin 13 (GND)

Switch 2 (Key 2) ------------ P2 Pin 12 (GPIO 9)
                 \
                  ----------- P2 Pin 2 / Pin 13 (GND)
```

### Wiring Instructions
1. **Identify Header P2**: Locate the dual 14-pin 1.27 mm male header labeled **P2** on the rear of the Waveshare development board.
2. **Key 1 Connection**: Connect one terminal of Switch 1 to **P2 Pin 11 (GPIO 14)**. Connect the other terminal to any **GND** pin (such as P2 Pin 2 or Pin 13). <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO -->
3. **Key 2 Connection**: Connect one terminal of Switch 2 to **P2 Pin 12 (GPIO 9)**. Connect the other terminal to **GND**. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO -->
4. **Third Key (Optional MX / HE V3)**: If using Key 3, wire the switch between **P2 Pin 7 (GPIO 15)** and **GND**.
5. **Polarity**: Mechanical switches are non-polarized contact closures; it does not matter which switch pin connects to GPIO and which to GND.

> [!WARNING]
> Always verify the physical silkscreen label against `opadctl status` before suspecting firmware faults. The GUI and documentation refer to schematic header positions ("GPIO14 / P2 Pin 11"), which can easily be miscounted on 1.27 mm high-density headers (AGENTS.md §6).

## Using the OPad V3 Carrier Board

If you are using the custom OPad V3 PCB carrier (`hardware/pcb/V3/Carrier/`): <!-- src: /docs/reference/pinout.md -->
- **No Hand Wiring**: The carrier board features dual female 1.27 mm receptacles that mate directly onto headers P1 and P2 of the Waveshare board.
- **Fixed Routing**: The PCB traces route GPIO 14, GPIO 9, and GPIO 15 to the input module connector with appropriate decoupling and ground planes.
- **Hotswap Sockets**: The MX Module V3 features Kailh hotswap sockets. Simply align the two metal pins of any Cherry MX-style mechanical switch with the hotswap socket holes and press gently until seated flush.

## Verifying Switch Contacts

Once wired:
1. Plug the pad into your computer.
2. Run `opadctl status` or open `opad-gui` to check detected inputs:
   ```bash
   opadctl status
   ```
3. Test physical actuation in the **Diagnostics** page of `opad-gui` or via `opadctl detect-pin`:
   ```bash
   opadctl detect-pin 14
   ```
   Press the switch: the terminal will report `Pin 14 actuated successfully!`.
