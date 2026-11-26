---
title: Hardware Pinout Reference
description: Header P1 and P2 pinout of the Waveshare ESP32-S3-Touch-LCD-2, key pins, on-board peripherals, and the V3 carrier's FFC cables and module IDs.
sidebar:
  order: 4
---

OPad runs on the Waveshare ESP32-S3-Touch-LCD-2. The pin facts below come from the board's [schematic](https://files.waveshare.com/wiki/ESP32-S3-Touch-LCD-2/ESP32-S3-Touch-LCD-2-SchDoc.pdf), `firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h` and the V3 hardware notes.
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /hardware/pcb/V3/README.md -->

> [!NOTE]
> Header positions ("P2 pin 11") and GPIO numbers ("GPIO14") are different things. The GUI's pin picker names both; the board's silkscreen shows the GPIO.

## Headers P1 and P2

| Pin | P1 | P1 use | P2 | P2 use |
|---|---|---|---|---|
| 1 | `GPIO2` | Free; V3 HE module `SCLK` | `3V3` | 3.3 V |
| 2 | `GPIO4` | Free; V3 HE module `MOSI` | `GND` | Ground |
| 3 | `GPIO6` | Free; V3 HE module `MISO`, V3 MX module Key 3 | `GPIO43` | UART0 TX (console), reserved |
| 4 | `GPIO16` | Free | `GPIO44` | UART0 RX (console), reserved |
| 5 | `GPIO17` | `CAM_PWDN`, 10 kΩ pull-down, reserved | `GPIO47` | Touch and IMU I2C `SCL`, reserved |
| 6 | `GPIO18` | Free | `GPIO48` | Touch and IMU I2C `SDA`, reserved |
| 7 | `GPIO21` | Free | `GPIO15` | Free |
| 8 | `GPIO8` | Module ID (`ADC1_CH7`), reserved | `GPIO13` | Free |
| 9 | `GPIO7` | Free; MX module Key 2; module ID on a turned-over V3 cable (`ADC1_CH6`) | `GPIO11` | Free |
| 10 | `GPIO10` | Free; MX module Key 1; V3 HE module `CS` | `GPIO12` | Free |
| 11 | `GPIO20` | USB D+, reserved | `GPIO14` | **Key 1 default** |
| 12 | `GPIO19` | USB D−, reserved | `GPIO9` | **Key 2 default** |
| 13 | `GND` | Ground | `GND` | Ground |
| 14 | `5V` | Raw USB VBUS | `VBAT` | Battery |
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

## Key pins

### Hand-wired switches

Wire each switch between its GPIO and any `GND` pin. The firmware enables the internal pull-up, so no resistor is needed.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

| Key | Default GPIO | Header | Default key |
|---|---|---|---|
| Key 1 | `GPIO14` | P2 pin 11 | `Z` (`0x1D`) |
| Key 2 | `GPIO9` | P2 pin 12 | `X` (`0x1B`) |
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_KEY1_GPIO -->

Either key can be moved to any of these 14 GPIOs (`KEY_GPIO_ALLOWED`), but not both to the same one:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/main/config/config_validate.c#device_config_validate -->

| Header | Pins |
|---|---|
| P1 | 1 (`GPIO2`), 2 (`GPIO4`), 3 (`GPIO6`), 4 (`GPIO16`), 6 (`GPIO18`), 7 (`GPIO21`), 9 (`GPIO7`), 10 (`GPIO10`) |
| P2 | 7 (`GPIO15`), 8 (`GPIO13`), 9 (`GPIO11`), 10 (`GPIO12`), 11 (`GPIO14`), 12 (`GPIO9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

See [Changing Key Pins](../guides/change-key-pins.md) and [Device Settings Reference](device-settings.md#allowed-key-pins).

### Pins that are never key pins

| GPIO | Why |
|---|---|
| `19`, `20` | USB D− / D+ |
| `43`, `44` | UART0 console |
| `47`, `48` | Touch and IMU I2C |
| `17` | `CAM_PWDN`, 10 kΩ pull-down to GND |
| `8` | Module ID ADC |
| `1`, `38`, `39`, `42`, `45`, `46` | Display, backlight and touch interrupt (not on the headers) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### Input modules

A carrier routes an MX module's keys to `GPIO10` (Key 1) and `GPIO7` (Key 2), and a V3 MX module's Key 3 to `GPIO6`. When the pad boots with an MX module and both key pins still at the hand-wired defaults, it moves them to `GPIO10` / `GPIO7` and saves that; pins someone chose are left alone. The GUI offers key pin settings only on pads without a module.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MX_KEY1_GPIO, /firmware/main/app_main.c, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

## On-board peripherals

| Peripheral | Signal | GPIO | Notes |
|---|---|---|---|
| ST7789 2.0" LCD, 320 × 240 | `SCLK` | `GPIO39` | `SPI2_HOST`, 80 MHz (`BOARD_LCD_PIXEL_CLOCK_HZ`) |
| | `MOSI` | `GPIO38` | |
| | `DC` | `GPIO42` | |
| | `CS` | `GPIO45` | |
| | `RST` | — | Not connected; reset by power or software |
| | Backlight | `GPIO1` | LEDC PWM |
| CST816 touch and QMI8658 IMU | `SDA` | `GPIO48` | Shared I2C bus |
| | `SCL` | `GPIO47` | |
| | Touch `INT` | `GPIO46` | |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## V3 carrier cables

V3 splits the pad into a carrier, a USB-C board at the rear wall, and swappable three-key MX or Hall Effect modules, joined by 1.0 mm FFCs.
<!-- src: /hardware/pcb/V3/README.md -->

### Module cable (14-way, type B)

| Contact | Carrier | MX module | HE module (ADS7028) |
|---|---|---|---|
| 1 | `GND` | `GND` | `GND` |
| 2 | `5V` | — | `5V` |
| 3 | `3V3` | `3V3` | `3V3` |
| 4 | `GPIO8` | `ID` | `ID` |
| 5 | `GPIO6` | `KEY3` | `MISO` |
| 6 | `GND` | `GND` | `GND` |
| 7 | `GPIO2` | — | `SCLK` |
| 8 | `GPIO4` | — | `MOSI` |
| 9 | `GND` | `GND` | `GND` |
| 10 | `GPIO10` | `KEY1` | `CS` |
| 11 | `GPIO7` | `KEY2` | — |
| 12 | `3V3` | `3V3` | `3V3` |
| 13 | `5V` | — | `5V` |
| 14 | `GND` | `GND` | `GND` |
<!-- src: /hardware/pcb/V3/README.md -->

The HE module powers its sensors and the ADC reference from the cable's 5 V through an LP5907 LDO.
<!-- src: /hardware/pcb/V3/README.md -->

A turned-over cable joins contact *k* to contact 15 − *k*. Power and ground still meet, and the module ID arrives on `GPIO7` instead of `GPIO8`. The firmware reads `GPIO7` only when `GPIO8` shows nothing, and only V3 classes count there. Then:
<!-- src: /firmware/main/input/module_id.h, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c -->

| Module | Turned-over cable |
|---|---|
| HE V3 | Works: the SPI lines move to `MISO` = `GPIO10`, `SCLK` = `GPIO4`, `MOSI` = `GPIO2`, `CS` = `GPIO6` (`BOARD_HE3_REV_*`). |
| MX V3 | Detected, but its keys are not read. |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_HE3_REV_MISO_GPIO, /firmware/main/app_main.c -->

### Module IDs

Each module has a divider from 3V3 to GND on its ID contact. The firmware classifies the voltage:
<!-- src: /firmware/main/input/module_id.h -->

| Module | Divider | ID voltage | Read as | Reaches |
|---|---|---|---|---|
| None | — | — | below 60 mV | — |
| MX V1 | 100 kΩ / 10 kΩ | 0.30 V | below 680 mV | `GPIO8` (JST cable, one way only) |
| HE V1 | 100 kΩ / 47 kΩ | 1.06 V | below 1350 mV | `GPIO8` |
| HE V3 | 100 kΩ / 100 kΩ | 1.65 V | below 1950 mV | `GPIO8`, or `GPIO7` turned over |
| MX V3 | 47 kΩ / 100 kΩ | 2.24 V | below 2600 mV | `GPIO8`, or `GPIO7` turned over |
<!-- src: /firmware/main/input/module_id.h#MODULE_ID_PRESENT_MIN_MV, /firmware/main/input/module_id.h#MODULE_ID_MX_MAX_MV, /protocol/opad.proto#InputModule -->

### USB cable (8-way, type A)

| Contact | Net | Carrier destination |
|---|---|---|
| 1, 8 | `GND` | Ground |
| 2, 7 | `5V` | P1 pin 14 |
| 3, 6 | `D−` | P1 pin 12 (`GPIO19`) |
| 4, 5 | `D+` | P1 pin 11 (`GPIO20`) |
<!-- src: /hardware/pcb/V3/README.md -->

The pinout is symmetric, so the cable reads the same either way round. The two 5V contacts carry 1 A together. The path is passive, so flashing and recovery work over it as over the Waveshare's own USB port.
<!-- src: /hardware/pcb/V3/README.md -->
