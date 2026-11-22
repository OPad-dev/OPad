---
title: Hardware Pinout Reference
description: Header pinouts, GPIO assignments, peripheral mappings, and carrier FFC connections for OPad.
---

This page documents the complete hardware pinout for the Waveshare ESP32-S3-Touch-LCD-2 board, standalone mechanical switch wiring, and the V3 carrier board interconnects.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

---

## Waveshare ESP32-S3-Touch-LCD-2 Headers

The development board exposes two 14-pin 2.54 mm headers: Header P1 (rear) and Header P2 (front).
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

| Pin | Header P1 Net | Header P1 Function | Header P2 Net | Header P2 Function |
|---|---|---|---|---|
| 1 | `GPIO 2` | Free GPIO / Carrier FFC SCLK | `3V3` | 3.3 V Power Rail |
| 2 | `GPIO 4` | Free GPIO / Carrier FFC MOSI | `GND` | Ground Reference |
| 3 | `GPIO 6` | Free GPIO / Carrier FFC MISO | `GPIO 43` | UART0 TX (Console) — Reserved |
| 4 | `GPIO 16` | Free GPIO | `GPIO 44` | UART0 RX (Console) — Reserved |
| 5 | `GPIO 17` | On-board 10k Pull-down (CAM_PWDN) — Reserved | `GPIO 47` | Touch & IMU I2C SCL — Reserved |
| 6 | `GPIO 18` | Free GPIO | `GPIO 48` | Touch & IMU I2C SDA — Reserved |
| 7 | `GPIO 21` | Free GPIO | `GPIO 15` | Free GPIO |
| 8 | `GPIO 8` | Module ID ADC1_CH7 / Free GPIO | `GPIO 13` | Free GPIO |
| 9 | `GPIO 7` | Module ID Alt (reversed FFC) / Free GPIO | `GPIO 11` | Free GPIO |
| 10 | `GPIO 10` | Carrier FFC CS / Free GPIO | `GPIO 12` | Free GPIO |
| 11 | `GPIO 20` | USB D+ (USB-C PHY) — Reserved | `GPIO 14` | **Key 1 Default** (`Z`) |
| 12 | `GPIO 19` | USB D- (USB-C PHY) — Reserved | `GPIO 9` | **Key 2 Default** (`X`) |
| 13 | `GND` | Ground Reference | `GND` | Ground Reference |
| 14 | `5V` | Raw USB VBUS Input (5 V) | `VBAT` | Battery Input |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

---

## Standalone Switch Wiring (Header P2)

Standard mechanical (MX) switches have no polarity. When wiring switches directly without a carrier module, wire each switch between its designated GPIO pin and ground:
<!-- src: /README.md -->

| Key | Header Pin | GPIO | Default Keycode | Connection Instructions |
|---|---|---|---|---|
| **Key 1** | Header P2, Pin 11 | `GPIO 14` | `0x1D` ('Z') | Switch Pin A -> Header P2 Pin 11, Switch Pin B -> GND (P2 Pin 13) |
| **Key 2** | Header P2, Pin 12 | `GPIO 9` | `0x1B` ('X') | Switch Pin A -> Header P2 Pin 12, Switch Pin B -> GND (P2 Pin 13) |
| **Ground** | Header P2, Pin 13 | `GND` | — | Ground reference for both switches |
<!-- src: /README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

Pins 11, 12, and 13 on Header P2 sit directly adjacent on the board edge, allowing direct 3-pin Dupont jumper or breadboard connections.
<!-- src: /README.md -->

---

## Valid Switch GPIO Pins

Keys can be reconfigured to any supported header pin in the desktop app (**Settings → Keys**) or via `opadctl`. Only pins that support input with an internal pull-up to 3.3V and a switch to ground are offered:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /README.md -->

| Header | Supported Pins & GPIOs |
|---|---|
| **P1** | Pin 1 (`GPIO 2`), Pin 2 (`GPIO 4`), Pin 3 (`GPIO 6`), Pin 4 (`GPIO 16`), Pin 6 (`GPIO 18`), Pin 7 (`GPIO 21`), Pin 8 (`GPIO 8`), Pin 9 (`GPIO 7`), Pin 10 (`GPIO 10`) |
| **P2** | Pin 7 (`GPIO 15`), Pin 8 (`GPIO 13`), Pin 9 (`GPIO 11`), Pin 10 (`GPIO 12`), Pin 11 (`GPIO 14`), Pin 12 (`GPIO 9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /README.md -->

### Reserved Pins

The firmware rejects configuration attempts on these pins:
- **USB D- / D+**: `GPIO 19` and `GPIO 20` (Native USB OTG PHY).
  <!-- src: /firmware/main/config/config_validate.c -->
- **Console UART0**: `GPIO 43` and `GPIO 44` (Flashing, bootloader messages, serial logging).
  <!-- src: /firmware/main/config/config_validate.c -->
- **Touch & IMU I2C Bus**: `GPIO 47` (SCL) and `GPIO 48` (SDA).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Touch Interrupt**: `GPIO 46` (CST816 active-low touch interrupt).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Display SPI & PWM**: `GPIO 38` (MOSI), `GPIO 39` (SCLK), `GPIO 42` (DC), `GPIO 45` (CS), and `GPIO 1` (Backlight PWM).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Board Pull-down**: `GPIO 17` has an onboard 10 kΩ pull-down to ground for camera powerdown (`CAM_PWDN`).
  <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->
- **Module ID ADC**: `GPIO 8` is connected to the carrier module ID resistor divider.
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

---

## On-Board Peripherals

These pins connect to soldered components on the Waveshare display board:
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

| Peripheral | Signal | GPIO | Notes |
|---|---|---|---|
| **ST7789 2.0" LCD** | SCLK | `GPIO 39` | SPI2 host, clocked at 80 MHz |
| | MOSI | `GPIO 38` | SPI data |
| | DC | `GPIO 42` | Data / Command select |
| | CS | `GPIO 45` | Chip select |
| | Backlight | `GPIO 1` | LEDC PWM backlight control |
| **CST816 Touch** | SDA | `GPIO 48` | I2C0 (0x15 address), 100 kHz |
| | SCL | `GPIO 47` | I2C0 clock |
| | INT | `GPIO 46` | Active-low edge interrupt |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/touch_retry.c -->

---

## V3 Carrier & Module Interconnects

The V3 hardware architecture separates the keypad into a Carrier Board, a dedicated USB daughterboard, and swappable Switch Modules (MX or Hall Effect) connected via flexible flat cables (FFC).
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 14-Way Module FFC (`J_MOD`)

The carrier connects to switch modules using a 14-conductor 1.0 mm pitch horizontal FFC cable:
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

| Contact | Carrier Net | MX Module Function | Hall Effect V3 Function (ADS7028 SPI) |
|---|---|---|---|
| 1 | `GND` | Ground | Ground |
| 2 | `5V_MOD` | 5 V Power Rail (Filtered via FB1) | 5 V Power Rail (Filtered via FB1) |
| 3 | `3V3` | 3.3 V Power Rail | 3.3 V Power Rail |
| 4 | `GPIO 8` | Module ID Voltage (0.30 V) | Module ID Voltage (1.65 V) |
| 5 | `M_MISO` (`GPIO 6`) | Key 3 Switch | SPI MISO |
| 6 | `GND` | Ground | Ground |
| 7 | `M_SCLK` (`GPIO 2`) | Unused | SPI SCLK |
| 8 | `M_MOSI` (`GPIO 4`) | Unused | SPI MOSI |
| 9 | `GND` | Ground | Ground |
| 10 | `M_CS` (`GPIO 10`) | Key 1 Switch | SPI CS |
| 11 | `GPIO 7` | Key 2 Switch | Spare / Module ID Alt |
| 12 | `3V3` | 3.3 V Power Rail | 3.3 V Power Rail |
| 13 | `5V_MOD` | 5 V Power Rail | 5 V Power Rail |
| 14 | `GND` | Ground | Ground |
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

#### Reversible Cable Mapping

The 14-way FFC connector layout is symmetrical. If the cable is inserted upside down, contact $k$ connects to contact $15 - k$:
- On an inverted cable, the module ID arrives on `GPIO 7` instead of `GPIO 8`.
- The firmware detects this condition via `module_id.c`, marks `cable_reversed = true`, and dynamically adjusts SPI pin mappings (`BOARD_HE3_REV_*`).
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MODULE_ID_ALT_GPIO -->

### 8-Way USB FFC (`J_USB`)

The rear USB-C daughterboard connects to the carrier board via an 8-way 1.0 mm pitch FFC cable:
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

| Contacts | Net | Function | Destination |
|---|---|---|---|
| 1, 8 | `GND` | Ground Reference | Carrier Ground plane |
| 2, 7 | `5V` | USB 5 V VBUS | Header P1 Pin 14 |
| 3, 6 | `USB_DN` | USB Full-Speed Data Minus | Header P1 Pin 12 (`GPIO 19`) |
| 4, 5 | `USB_DP` | USB Full-Speed Data Plus | Header P1 Pin 11 (`GPIO 20`) |
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

Because contacts mirror symmetrically around the center (1 & 8, 2 & 7, 3 & 6, 4 & 5), the cable functions identically in either orientation.
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->
