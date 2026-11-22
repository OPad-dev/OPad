---
title: Hardware Pinout Reference
description: Header pinouts, GPIO assignments, peripheral mappings, and V3 carrier FFC connections for OPad.
---

This page documents the hardware pinout for the Waveshare ESP32-S3-Touch-LCD-2 board, standalone mechanical switch wiring, and the OPad V3 carrier and module FFC interconnects.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /hardware/pcb/V3/README.md -->

---

## Waveshare ESP32-S3-Touch-LCD-2 Headers

The Waveshare ESP32-S3-Touch-LCD-2 exposes two 14-pin headers: Header P1 and Header P2.
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

| Pin | Header P1 Net | Header P1 Function | Header P2 Net | Header P2 Function |
|---|---|---|---|---|
| 1 | `GPIO 2` | Free GPIO / V3 Carrier FFC `SCLK` | `3V3` | 3.3 V Power Rail |
| 2 | `GPIO 4` | Free GPIO / V3 Carrier FFC `MOSI` | `GND` | Ground Reference |
| 3 | `GPIO 6` | Free GPIO / V3 Carrier FFC `MISO` / MX V3 Key 3 | `GPIO 43` | UART0 TX (Console) — Reserved |
| 4 | `GPIO 16` | Free GPIO | `GPIO 44` | UART0 RX (Console) — Reserved |
| 5 | `GPIO 17` | Onboard 10 kΩ Pull-down (`CAM_PWDN`) — Reserved | `GPIO 47` | Touch & IMU I2C `SCL` — Reserved |
| 6 | `GPIO 18` | Free GPIO | `GPIO 48` | Touch & IMU I2C `SDA` — Reserved |
| 7 | `GPIO 21` | Free GPIO | `GPIO 15` | Free GPIO |
| 8 | `GPIO 8` | Module ID `ADC1_CH7` (10 kΩ divider) — Reserved in firmware | `GPIO 13` | Free GPIO |
| 9 | `GPIO 7` | MX V3 Key 2 / Reversed-FFC Module ID `ADC1_CH6` / Free GPIO | `GPIO 11` | Free GPIO |
| 10 | `GPIO 10` | MX V3 Key 1 / V3 Carrier FFC `CS` / Free GPIO | `GPIO 12` | Free GPIO |
| 11 | `GPIO 20` | USB `D+` — Reserved | `GPIO 14` | **Key 1 Default** (`Z`) |
| 12 | `GPIO 19` | USB `D-` — Reserved | `GPIO 9` | **Key 2 Default** (`X`) |
| 13 | `GND` | Ground Reference | `GND` | Ground Reference |
| 14 | `5V` | Raw USB VBUS (5 V) | `VBAT` | Battery Input |
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

---

## Standalone Switch Wiring (Header P2)

Standard mechanical (MX) switches are non-polarized. Wire each switch between its GPIO pin and `GND`:
<!-- src: /README.md -->

| Key | Header Pin | GPIO | Default HID Usage | Connection |
|---|---|---|---|---|
| **Key 1** | Header P2, Pin 11 | `GPIO 14` | `0x1D` (`Z`) | Switch Pin A -> P2 Pin 11, Switch Pin B -> `GND` (P2 Pin 13) |
| **Key 2** | Header P2, Pin 12 | `GPIO 9` | `0x1B` (`X`) | Switch Pin A -> P2 Pin 12, Switch Pin B -> `GND` (P2 Pin 13 or Pin 2) |
| **Ground** | Header P2, Pin 13 / Pin 2 | `GND` | — | Ground reference |
<!-- src: /README.md, /firmware/main/input/keypad.c -->

---

## Valid Switch GPIO Pins

For hand-wired builds, keys can be reassigned in `opad-gui` (**Settings** -> **⌨ Keypad**) to any of the 14 GPIOs accepted by firmware `KEY_GPIO_ALLOWED`:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /desktop/gui/src/pages.rs#settings -->

| Header | Supported Pins & GPIOs |
|---|---|
| **P1** | Pin 1 (`GPIO 2`), Pin 2 (`GPIO 4`), Pin 3 (`GPIO 6`), Pin 4 (`GPIO 16`), Pin 6 (`GPIO 18`), Pin 7 (`GPIO 21`), Pin 9 (`GPIO 7`), Pin 10 (`GPIO 10`) |
| **P2** | Pin 7 (`GPIO 15`), Pin 8 (`GPIO 13`), Pin 9 (`GPIO 11`), Pin 10 (`GPIO 12`), Pin 11 (`GPIO 14`), Pin 12 (`GPIO 9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

### Reserved Pins

Firmware validation (`device_config_validate`) rejects these pins:
- **USB `D-` / `D+`**: `GPIO 19` and `GPIO 20`. <!-- src: /firmware/main/config/config_validate.c -->
- **Console UART0**: `GPIO 43` and `GPIO 44`. <!-- src: /firmware/main/config/config_validate.c -->
- **Touch & IMU I2C Bus**: `GPIO 47` (`SCL`) and `GPIO 48` (`SDA`). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Touch Interrupt**: `GPIO 46` (CST816S active-low interrupt). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Display SPI & Backlight**: `GPIO 38` (`MOSI`), `GPIO 39` (`SCLK`), `GPIO 42` (`DC`), `GPIO 45` (`CS`), `GPIO 1` (LEDC PWM backlight). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Board Pull-down**: `GPIO 17` (`CAM_PWDN`, onboard 10 kΩ pull-down to `GND`). <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->
- **Module ID ADC**: `GPIO 8` (connected to the module ID divider on carrier builds; note that `opad-gui` currently lists `GPIO8 (P1 pin 8)` in its dropdown, but firmware rejects it). <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

---

## On-Board Peripherals

| Peripheral | Signal | GPIO | Notes |
|---|---|---|---|
| **ST7789 2.0" LCD** | `SCLK` | `GPIO 39` | `SPI2_HOST`, 80 MHz (`BOARD_LCD_PIXEL_CLOCK_HZ`) |
| | `MOSI` | `GPIO 38` | SPI data |
| | `DC` | `GPIO 42` | Data / Command select |
| | `CS` | `GPIO 45` | Chip select |
| | `BL` | `GPIO 1` | LEDC PWM backlight |
| **CST816S Touch** | `SDA` | `GPIO 48` | I2C0 (address `0x15`) |
| | `SCL` | `GPIO 47` | I2C0 clock |
| | `INT` | `GPIO 46` | Active-low edge interrupt |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/touch_retry.c -->

---

## V3 Carrier & Module Interconnects

The V3 hardware splits the pad into a Controller Carrier, a rear USB-C daughterboard, and swappable 3-key MX or Hall Effect input modules connected via 1.0 mm FFC cables.
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 14-Way Module FFC (Type B, 1.0 mm pitch)

| Contact | Carrier Pin | MX Input Module V3 | HE Input Module V3 (ADS7028 SPI3) |
|---|---|---|---|
| 1 | `GND` | `GND` | `GND` |
| 2 | `5V` | Unused | `5V` (feeds LP5907 3.3 V LDO) |
| 3 | `3V3` | `3V3` | `3V3` |
| 4 | `GPIO 8` | `ID` (47 kΩ / 100 kΩ = **2.24 V**) | `ID` (100 kΩ / 100 kΩ = **1.65 V**) |
| 5 | `GPIO 6` | `KEY3` | `MISO` |
| 6 | `GND` | `GND` | `GND` |
| 7 | `GPIO 2` | Unused | `SCLK` |
| 8 | `GPIO 4` | Unused | `MOSI` |
| 9 | `GND` | `GND` | `GND` |
| 10 | `GPIO 10` | `KEY1` | `CS` |
| 11 | `GPIO 7` | `KEY2` | Unused (or `ID` on reversed cable) |
| 12 | `3V3` | `3V3` | `3V3` |
| 13 | `5V` | Unused | `5V` |
| 14 | `GND` | `GND` | `GND` |
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/module_id.h -->

Module ID voltage classes (`module_id.h`):
- **MX V1**: `0.30 V` (`100 kΩ / 10 kΩ`, cut-off `< 680 mV`)
- **HE V1**: `1.06 V` (`100 kΩ / 47 kΩ`, cut-off `< 1350 mV`)
- **HE V3**: `1.65 V` (`100 kΩ / 100 kΩ`, cut-off `< 1950 mV`)
- **MX V3**: `2.24 V` (`47 kΩ / 100 kΩ`, cut-off `< 2600 mV`)
<!-- src: /firmware/main/input/module_id.h, /firmware/main/input/module_id.c#module_id_classify -->

#### Reversible FFC Mapping

If the 14-way FFC is inserted upside down, contact `k` meets contact `15 - k`: power and ground still align, and the module ID lands on `GPIO 7` (`ADC1_CH6`) instead of `GPIO 8` (`ADC1_CH7`). For **HE V3**, firmware automatically remaps SPI3 pins (`BOARD_HE3_REV_*`: `MISO = GPIO 10`, `SCLK = GPIO 4`, `MOSI = GPIO 2`, `CS = GPIO 6`). An **MX V3** module on a reversed cable is not read.
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MODULE_ID_ALT_GPIO -->

### 8-Way USB FFC (Type A, 1.0 mm pitch)

| Contacts | Net | Function | Destination |
|---|---|---|---|
| 1, 8 | `GND` | Ground Reference | Carrier Ground |
| 2, 7 | `5V` | USB 5 V VBUS (1 A combined) | Header P1 Pin 14 |
| 3, 6 | `D-` | USB Full-Speed Data Minus | Header P1 Pin 12 (`GPIO 19`) |
| 4, 5 | `D+` | USB Full-Speed Data Plus | Header P1 Pin 11 (`GPIO 20`) |
<!-- src: /hardware/pcb/V3/README.md -->

The 8-way USB FFC pinout is completely mirror-symmetric, so it reads identically from either end.
<!-- src: /hardware/pcb/V3/README.md -->
