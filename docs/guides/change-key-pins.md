---
title: Changing Key Pins
description: Tell OPad which header pins your hand-wired switches are on, by picking them in the app or with Auto-Detect, and which pins you can use.
sidebar:
  order: 5
---

A hand-wired OPad reads Key 1 on GPIO14 (header P2, pin 11) and Key 2 on GPIO9 (P2, pin 12) by default. If you wired your switches to other header pins, set them in the app. You do not need to rebuild the firmware, and the pad remembers the pins. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO --> <!-- src: /desktop/gui/src/pages.rs#settings -->

> [!NOTE]
> This is only for hand-wired switches. On the OPad V3 PCBs the carrier fixes the pins (MX V3: GPIO10, GPIO7, GPIO6; HE V3: an ADC over SPI), so the app hides the pin settings. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## What you need

- The pad connected, with the desktop app running.
- Each switch wired between one of the pins below and GND.

## Pins you can use

The firmware accepts these 14 header GPIOs. Each can take a switch to GND using the chip's internal pull-up, and none is used by anything else on the board: <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> <!-- src: /desktop/crates/opad-model/src/lib.rs#KEY_PINS -->

| Header | GPIOs |
|---|---|
| P2 | GPIO14 (pin 11, Key 1 default), GPIO9 (pin 12, Key 2 default), GPIO15 (pin 7), GPIO13 (pin 8), GPIO11 (pin 9), GPIO12 (pin 10) |
| P1 | GPIO2 (pin 1), GPIO4 (pin 2), GPIO6 (pin 3), GPIO16 (pin 4), GPIO18 (pin 6), GPIO21 (pin 7), GPIO7 (pin 9), GPIO10 (pin 10) |

Key 1 and Key 2 must be on different pins. Every other pin is refused, because something on the board needs it: <!-- src: /firmware/main/config/config_validate.c -->

- GPIO19 and GPIO20: USB D− and D+.
- GPIO43 and GPIO44: the UART0 console.
- GPIO47 and GPIO48: the touch and IMU I2C bus.
- GPIO17: has a 10 kΩ pull-down on the board.
- GPIO8: the V3 module-ID line, with a resistor divider to GND.

A pad that older firmware left on GPIO8 shows it as `GPIO8 (unsupported)`. It keeps working there while you change other settings; you just cannot move a key *onto* a pin outside the list. <!-- src: /desktop/crates/opad-model/src/lib.rs#validate_update --> <!-- src: /desktop/gui/src/pages.rs#settings -->

## Set the pins

1. Open the OPad app and go to **Settings** → **⌨ Keypad**.
2. In the **Keys** card, find **KEY 1 PIN** and **KEY 2 PIN**. For each, either: <!-- src: /desktop/gui/src/pages.rs#settings -->
   - **pick the pin** from the list, for example `GPIO15 (P2 pin 7)`; or
   - click **Auto-Detect** (it changes to **Detecting... (click to cancel)**) and press the switch wired for that key. The pad watches every allowed, unused pin and picks the one that stays low for 15 ms. <!-- src: /firmware/main/input/keypad.c#DETECT_CONFIRM_US -->
3. Click **Save settings**.

The pad switches to the new pins as soon as both keys are released, and saves them once it is idle. <!-- src: /firmware/main/input/keypad.c#keypad_task -->

> [!TIP]
> A V3 carrier with an MX module whose keys are still set to the hand-wired defaults (GPIO14 / GPIO9) can be fixed from a terminal: `opadctl board-test --fix-pins` sets Key 1 = GPIO10 and Key 2 = GPIO7 and tests again. <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

## Check that it worked

1. Run `opadctl status` and check that **Key Pins** shows your new GPIOs. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
2. Press both keys in a text editor, or watch them on **Diagnostics** → **⌨ Switch Chatter & Input** in the app (turn on the Diagnostics menu in **Settings** → **🛠 Diagnostics** first). <!-- src: /desktop/gui/src/diagnostics.rs --> <!-- src: /desktop/gui/src/pages.rs#settings -->

## If it did not work

- **A key does nothing after the change:** compare the GPIO label printed on the board next to the wire with **Key Pins** in `opadctl status`. Header positions such as `P2 pin 11` are easy to count wrong on the board. <!-- src: /AGENTS.md#6. Hardware and debugging facts -->
- **There are no pin settings:** the pad reports a module with fixed pins (a V3 carrier with an MX or HE module). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Auto-Detect finds nothing:** check that the switch's other leg goes to GND and that its pin is in the list above.
