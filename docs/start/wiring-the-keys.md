---
title: Wiring the Keys
description: How to wire hand-wired mechanical switches to Header P2 on the Waveshare ESP32-S3-Touch-LCD-2, or assemble the OPad V3 carrier and input modules.
sidebar:
  order: 2
---

Mechanical switches on OPad are read active-low using the ESP32-S3's internal pull-ups (`GPIO_PULLUP_ONLY`): pressing a switch connects its GPIO pin to `GND`. No external resistors or diodes are required for hand-wired builds. <!-- src: /firmware/main/input/keypad.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Option 1: Hand-Wiring Mechanical Switches (Header P2)

When wiring two switches directly to the Waveshare ESP32-S3-Touch-LCD-2 headers: <!-- src: /README.md --> <!-- src: /firmware/main/input/keypad.c -->

```text
Switch 1 (Key 1) ------------ P2 Pin 11 (GPIO 14)
                 \
                  ----------- P2 Pin 13 or Pin 2 (GND)

Switch 2 (Key 2) ------------ P2 Pin 12 (GPIO 9)
                 \
                  ----------- P2 Pin 13 or Pin 2 (GND)
```

1. **Locate Header P2** on the back of the Waveshare ESP32-S3-Touch-LCD-2 board.
2. **Wire Key 1**: Connect one terminal of Switch 1 to **P2 Pin 11 (`GPIO 14`)** and the other terminal to **GND** (P2 Pin 13 is directly adjacent, or use P2 Pin 2). <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO -->
3. **Wire Key 2**: Connect one terminal of Switch 2 to **P2 Pin 12 (`GPIO 9`)** and the other terminal to **GND**. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO -->
4. **Polarity**: Mechanical switches are non-polarized; either switch leg can go to the GPIO pin and the other to `GND`.
5. **Custom pins**: If you wire switches to different supported P1/P2 GPIOs, reassign them later in `opad-gui` (**Settings** -> **⌨ Keypad** -> **KEY 1 PIN** / **KEY 2 PIN** or **Auto-Detect**). See [Changing Key Pins](../guides/change-key-pins.md) and [Pinout Reference](../reference/pinout.md). <!-- src: /desktop/gui/src/pages.rs#settings -->

> [!WARNING]
> Always check the board silkscreen label against **Key Pins** in `opadctl status` if a key does nothing. The GUI dropdown lists schematic header positions (such as `GPIO14 (P2 pin 11)`), which are easy to mix up when counting header pins. <!-- src: /AGENTS.md -->

## Option 2: OPad V3 Carrier & Modular PCBs

If you are assembling the OPad V3 PCB set (`hardware/pcb/V3/`): <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

- **Controller Carrier V3**: Seats onto headers P1 and P2 of the Waveshare board via two 1x14 female sockets.
- **Module cable**: Connects the carrier to either the **MX Input Module V3** (3 keys, Kailh hot-swap sockets) or **HE Input Module V3** (3 keys, Hall Effect sensors + ADS7028 ADC) using a **14-way, 1.0 mm type B FFC** (80 mm in the V3 case).
- **USB cable**: Connects the carrier to the **USB Board V3** using an **8-way, 1.0 mm type A FFC** (80 mm in the V3 case, symmetric pinout).
- **Fixed V3 carrier routing**:
  - **MX Input Module V3** (straight FFC): `KEY1 = GPIO 10`, `KEY2 = GPIO 7`, `KEY3 = GPIO 6`, `ID = GPIO 8` (note: an MX V3 module on a reversed FFC is not read; insert the type B cable straight). <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
  - **HE Input Module V3**: `ID = GPIO 8`, SPI3 to ADS7028 on `MISO = GPIO 6`, `SCLK = GPIO 2`, `MOSI = GPIO 4`, `CS = GPIO 10` (firmware automatically remaps SPI pins if the 14-way FFC is reversed, with `ID` on `GPIO 7`). <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Verifying Your Wiring

1. Plug the pad into your computer over USB.
2. Check active pin assignments and detected module in a terminal: <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
   ```bash
   opadctl status
   ```
3. On a V3 carrier + module, run the hardware PCB self-test with all keys released: <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
   ```bash
   opadctl board-test
   ```
   (Add `--fix-pins` if a carrier MX module is still configured on the standalone default pins `GPIO 14` / `GPIO 9`, or `--magnet` to verify Hall Effect V3 sensors live.)
4. On hand-wired builds, you can also enable **Settings** -> **🛠 Diagnostics** -> **Enable Diagnostics Menu in Navigation** in `opad-gui` and test switch presses and bounce on the **⌨ Switch Chatter & Input** tab, or use **Auto-Detect** under **Settings** -> **⌨ Keypad**. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/gui/src/diagnostics.rs -->
