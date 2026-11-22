---
title: Changing Key Pins
description: How to reconfigure mechanical switch GPIO pin assignments using opadctl or the graphical interface, pin restrictions, and validation rules.
category: how-to
source_files:
  - /README.md
  - /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h
  - /firmware/main/config/config_validate.c
  - /firmware/main/config/device_config.c
  - /protocol/opad.proto
---

By default, OPad assigns Key 1 to GPIO 14 (Header P2 Pin 11) and Key 2 to GPIO 9 (Header P2 Pin 12). If you need to avoid specific pins or adapt to custom enclosures, you can reconfigure switch GPIO assignments via `opadctl` or `opad-gui`. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

> [!NOTE]
> Key pin reassignments apply only to hand-wired mechanical switches. Modular V3 input carriers fix their pin assignments in PCB traces; on those pads, pin configuration options are hidden automatically. <!-- src: /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

## Allowed GPIO Pins

Only pins that support internal pull-up resistors and do not conflict with onboard peripherals (ST7789 display SPI, CST816S touch I2C, USB D+/D-, or debug UART) may be used: <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

| Header Position | GPIO Number | Default Function | Usable for Switch? |
|---|---|---|---|
| P2 Pin 11 | **GPIO 14** | Key 1 Default | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 12 | **GPIO 9** | Key 2 Default | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 7 | **GPIO 15** | Key 3 (HE V3) / Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 8 | **GPIO 13** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 9 | **GPIO 11** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 10 | **GPIO 12** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 1 | **GPIO 2** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 2 | **GPIO 4** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 3 | **GPIO 6** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 9 | **GPIO 7** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 10 | **GPIO 10** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 4 | **GPIO 16** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 6 | **GPIO 18** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 7 | **GPIO 21** | Spare | Yes <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |

### Disallowed Pins
- **GPIO 19 & 20**: Native USB full-speed data lines (`D-` and `D+`).
- **GPIO 43 & 44**: UART0 ROM bootloader and serial console (`TX0` and `RX0`).
- **GPIO 47 & 48**: I2C bus for touch controller and IMU.
- **GPIO 17**: Hardware camera power-down line (`CAM_PWDN`), tied to a 10 kΩ pull-down resistor to GND.
- **GPIO 8**: Module detection ADC divider line.

## Changing Pins via opadctl

To reassign pins from the terminal, use `opadctl config set`: <!-- src: /desktop/cli/src/main.rs -->
```bash
# Set Key 1 to GPIO 15 and Key 2 to GPIO 13
opadctl config set key1-gpio 15
opadctl config set key2-gpio 13
```

Validate the new assignments by querying the device:
```bash
opadctl status
```
Output will reflect the active GPIO pin assignments.

## Changing Pins via the Graphical Interface

1. Open `opad-gui`.
2. Navigate to the **Device** page.
3. Locate the **Key 1 Pin** and **Key 2 Pin** dropdown selectors.
4. Select the target schematic header position (e.g., `GPIO15 (P2 pin 7)`).
5. Click **Apply Changes**. The daemon transmits `SetConfig` (`opad.proto`) and immediately stages the new configuration in RAM without requiring a pad reboot. <!-- src: /protocol/opad.proto#SetConfig -->

## Testing Actuation with detect-pin

To ensure the physical switch is wired to the correct pin before playing:
```bash
opadctl detect-pin 15
```
Press the physical switch wired to GPIO 15. The pad will report the detected contact closure to the terminal.
