---
title: Wiring the Keys
description: Wire two mechanical switches to header P2 of the Waveshare ESP32-S3-Touch-LCD-2, or connect the OPad V3 carrier, module and USB board.
sidebar:
  order: 2
---

## Option 1: hand-wire two switches

Each switch goes between one GPIO pin and GND. The firmware turns on the pin's internal pull-up, so the pin reads high when the switch is open and low when you press it. You need no resistors or diodes. <!-- src: /firmware/main/input/keypad.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

```text
Switch 1 (Key 1):  one leg -> P2 pin 11 (GPIO14)   other leg -> GND (P2 pin 13 or pin 2)
Switch 2 (Key 2):  one leg -> P2 pin 12 (GPIO9)    other leg -> GND (P2 pin 13 or pin 2)
```
<!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO --> <!-- src: /AGENTS.md#6. Hardware and debugging facts -->

1. Find header **P2** on the Waveshare board.
2. Wire **Key 1**: one leg of the first switch to **P2 pin 11 (GPIO14)**, the other leg to **GND**. P2 pin 13, right next to it, is GND, and so is P2 pin 2.
3. Wire **Key 2**: one leg of the second switch to **P2 pin 12 (GPIO9)**, the other leg to GND.
4. Either leg of a switch can go to the GPIO. Switches have no polarity.

These two pins are the defaults, so the pad works with no setup. If you would rather use other header pins, wire them now and set them in the app afterwards; [Changing key pins](../guides/change-key-pins.md) lists the pins you can use.

> [!WARNING]
> If a key does nothing, first compare the GPIO label printed on the board next to the wire (the silkscreen) with **Key Pins** in `opadctl status`. The app's pin list names schematic header positions such as `GPIO14 (P2 pin 11)`, and it is easy to count header pins wrong. <!-- src: /AGENTS.md#6. Hardware and debugging facts --> <!-- src: /desktop/crates/opad-model/src/lib.rs#KEY_PINS -->

## Option 2: the OPad V3 PCBs

The V3 set gives you three keys and a rear USB-C port. Everything plugs together; there is nothing to solder to the board. <!-- src: /hardware/pcb/V3/README.md -->

1. Seat the **controller carrier V3** on headers P1 and P2 of the Waveshare board.
2. Connect the carrier to your input module, the **MX input module V3** or the **HE input module V3**, with the **14-way, 1.0 mm type B FFC**.
3. Connect the carrier to the **USB board V3** with the **8-way, 1.0 mm type A FFC**. Its pinout reads the same from either end.
4. Plug your PC into the USB board's USB-C port.

The carrier fixes which pins each key uses, so there is nothing to configure: <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

| Module | Lines on the ESP32-S3 |
|---|---|
| MX input module V3 | Key 1 GPIO10, Key 2 GPIO7, Key 3 GPIO6, module ID GPIO8 |
| HE input module V3 | ADS7028 over SPI: MISO GPIO6, SCLK GPIO2, MOSI GPIO4, CS GPIO10; module ID GPIO8 |

The firmware reads which module is fitted from the module ID line. A turned-over 14-way cable moves the ID to GPIO7: the firmware remaps an HE module's SPI lines to match, but it cannot read an MX V3 module that way, so make sure the cable is not turned over. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MODULE_ID_ALT_GPIO --> <!-- src: /hardware/pcb/V3/README.md -->

## Check your wiring

1. Plug the pad into your PC. With no app at all, open a text editor and press each key: you should see `z`, `x` (and `c` for Key 3 on a V3 module). See [First connection](first-connection.md). <!-- src: /firmware/main/usb/usb_hid.c -->
2. With the desktop app installed, run `opadctl status` and check **Key Pins** and **Input Module**. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
3. On a V3 carrier, run the board self-test with every key released: <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

   ```bash
   opadctl board-test
   ```

   It checks the module ID, cable direction, key lines and connector shorts, and on an HE V3 module the ADC and sensors. Add `--magnet` to then watch the HE sensors live while you hold a magnet over each key, or `--fix-pins` if an MX module's keys are still set to the hand-wired defaults (GPIO14 / GPIO9): it moves them to GPIO10 / GPIO7 and tests again.
4. To see each press and any switch bounce, turn on **Settings** → **🛠 Diagnostics** → **Enable Diagnostics Menu in Navigation** in `opad-gui`, then open **Diagnostics** → **⌨ Switch Chatter & Input**. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/gui/src/diagnostics.rs -->
