---
title: Referencia de pinout de hardware
description: Pinout de cabezales, asignaciones GPIO, mapas de periféricos y conexiones FFC del carrier V3 para OPad.
sidebar:
  order: 4
---

Esta página documenta el pinout de hardware de la placa Waveshare ESP32-S3-Touch-LCD-2, el cableado autónomo de interruptores mecánicos y las interconexiones FFC del carrier y los módulos OPad V3.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /hardware/pcb/V3/README.md -->

---

## Cabezales de Waveshare ESP32-S3-Touch-LCD-2

La Waveshare ESP32-S3-Touch-LCD-2 expone dos cabezales de 14 pines: cabezal P1 y cabezal P2.
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

| Pin | Red cabezal P1 | Función cabezal P1 | Red cabezal P2 | Función cabezal P2 |
|---|---|---|---|---|
| 1 | `GPIO 2` | GPIO libre / FFC carrier V3 `SCLK` | `3V3` | Raíl de alimentación 3.3 V |
| 2 | `GPIO 4` | GPIO libre / FFC carrier V3 `MOSI` | `GND` | Referencia de masa |
| 3 | `GPIO 6` | GPIO libre / FFC carrier V3 `MISO` / MX V3 Key 3 | `GPIO 43` | UART0 TX (Consola) — Reservado |
| 4 | `GPIO 16` | GPIO libre | `GPIO 44` | UART0 RX (Consola) — Reservado |
| 5 | `GPIO 17` | Pull-down integrado 10 kΩ (`CAM_PWDN`) — Reservado | `GPIO 47` | I2C táctil e IMU `SCL` — Reservado |
| 6 | `GPIO 18` | GPIO libre | `GPIO 48` | I2C táctil e IMU `SDA` — Reservado |
| 7 | `GPIO 21` | GPIO libre | `GPIO 15` | GPIO libre |
| 8 | `GPIO 8` | ID de módulo `ADC1_CH7` (divisor 10 kΩ) — Reservado en firmware | `GPIO 13` | GPIO libre |
| 9 | `GPIO 7` | MX V3 Key 2 / ID de módulo FFC invertido `ADC1_CH6` / GPIO libre | `GPIO 11` | GPIO libre |
| 10 | `GPIO 10` | MX V3 Key 1 / FFC carrier V3 `CS` / GPIO libre | `GPIO 12` | GPIO libre |
| 11 | `GPIO 20` | USB `D+` — Reservado | `GPIO 14` | **Key 1 por defecto** (`Z`) |
| 12 | `GPIO 19` | USB `D-` — Reservado | `GPIO 9` | **Key 2 por defecto** (`X`) |
| 13 | `GND` | Referencia de masa | `GND` | Referencia de masa |
| 14 | `5V` | USB VBUS directo (5 V) | `VBAT` | Entrada de batería |
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

---

## Cableado de interruptores autónomos (cabezal P2)

Los interruptores mecánicos estándar (MX) no tienen polaridad. Conecta cada interruptor entre su pin GPIO y `GND`:
<!-- src: /README.md -->

| Tecla | Pin del cabezal | GPIO | Uso HID por defecto | Conexión |
|---|---|---|---|---|
| **Key 1** | Cabezal P2, Pin 11 | `GPIO 14` | `0x1D` (`Z`) | Pin A interruptor -> P2 Pin 11, Pin B interruptor -> `GND` (P2 Pin 13) |
| **Key 2** | Cabezal P2, Pin 12 | `GPIO 9` | `0x1B` (`X`) | Pin A interruptor -> P2 Pin 12, Pin B interruptor -> `GND` (P2 Pin 13 o Pin 2) |
| **Masa** | Cabezal P2, Pin 13 / Pin 2 | `GND` | — | Referencia de masa |
<!-- src: /README.md, /firmware/main/input/keypad.c -->

---

## Pines GPIO válidos para interruptores

Para montajes cableados a mano, las teclas pueden reasignarse en `opad-gui` (**Settings** -> **⌨ Keypad**) a cualquiera de los 14 GPIOs aceptados por `KEY_GPIO_ALLOWED` en el firmware:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /desktop/gui/src/pages.rs#settings -->

| Cabezal | Pines y GPIOs admitidos |
|---|---|
| **P1** | Pin 1 (`GPIO 2`), Pin 2 (`GPIO 4`), Pin 3 (`GPIO 6`), Pin 4 (`GPIO 16`), Pin 6 (`GPIO 18`), Pin 7 (`GPIO 21`), Pin 9 (`GPIO 7`), Pin 10 (`GPIO 10`) |
| **P2** | Pin 7 (`GPIO 15`), Pin 8 (`GPIO 13`), Pin 9 (`GPIO 11`), Pin 10 (`GPIO 12`), Pin 11 (`GPIO 14`), Pin 12 (`GPIO 9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

### Pines reservados

La validación del firmware (`device_config_validate`) rechaza estos pines:
- **USB `D-` / `D+`**: `GPIO 19` y `GPIO 20`. <!-- src: /firmware/main/config/config_validate.c -->
- **Consola UART0**: `GPIO 43` y `GPIO 44`. <!-- src: /firmware/main/config/config_validate.c -->
- **Bus I2C táctil e IMU**: `GPIO 47` (`SCL`) y `GPIO 48` (`SDA`). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Interrupción táctil**: `GPIO 46` (interrupción activa-baja del CST816S). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **SPI de pantalla y retroiluminación**: `GPIO 38` (`MOSI`), `GPIO 39` (`SCLK`), `GPIO 42` (`DC`), `GPIO 45` (`CS`), `GPIO 1` (retroiluminación LEDC PWM). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Pull-down de placa**: `GPIO 17` (`CAM_PWDN`, pull-down integrado de 10 kΩ a `GND`). <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->
- **ADC de ID de módulo**: `GPIO 8` (conectado al divisor de ID de módulo en montajes con carrier; ten en cuenta que `opad-gui` actualmente lista `GPIO8 (P1 pin 8)` en su desplegable, pero el firmware lo rechaza). <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

---

## Periféricos integrados en placa

| Periférico | Señal | GPIO | Notas |
|---|---|---|---|
| **ST7789 2.0" LCD** | `SCLK` | `GPIO 39` | `SPI2_HOST`, 80 MHz (`BOARD_LCD_PIXEL_CLOCK_HZ`) |
| | `MOSI` | `GPIO 38` | Datos SPI |
| | `DC` | `GPIO 42` | Selección Dato / Comando |
| | `CS` | `GPIO 45` | Selección de chip |
| | `BL` | `GPIO 1` | Retroiluminación LEDC PWM |
| **Táctil CST816S** | `SDA` | `GPIO 48` | I2C0 (dirección `0x15`) |
| | `SCL` | `GPIO 47` | Reloj I2C0 |
| | `INT` | `GPIO 46` | Interrupción por flanco activa-baja |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/touch_retry.c -->

---

## Interconexiones del carrier y módulos V3

El hardware V3 divide el teclado en un Controller Carrier, una placa auxiliar USB-C trasera y módulos de entrada intercambiables de 3 teclas MX o Hall Effect conectados mediante cables FFC de 1.0 mm.
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### FFC de módulo de 14 vías (Tipo B, paso 1.0 mm)

| Contacto | Pin del carrier | MX Input Module V3 | HE Input Module V3 (ADS7028 SPI3) |
|---|---|---|---|
| 1 | `GND` | `GND` | `GND` |
| 2 | `5V` | Sin uso | `5V` (alimenta LDO LP5907 de 3.3 V) |
| 3 | `3V3` | `3V3` | `3V3` |
| 4 | `GPIO 8` | `ID` (47 kΩ / 100 kΩ = **2.24 V**) | `ID` (100 kΩ / 100 kΩ = **1.65 V**) |
| 5 | `GPIO 6` | `KEY3` | `MISO` |
| 6 | `GND` | `GND` | `GND` |
| 7 | `GPIO 2` | Sin uso | `SCLK` |
| 8 | `GPIO 4` | Sin uso | `MOSI` |
| 9 | `GND` | `GND` | `GND` |
| 10 | `GPIO 10` | `KEY1` | `CS` |
| 11 | `GPIO 7` | `KEY2` | Sin uso (o `ID` con cable invertido) |
| 12 | `3V3` | `3V3` | `3V3` |
| 13 | `5V` | Sin uso | `5V` |
| 14 | `GND` | `GND` | `GND` |
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/module_id.h -->

Clases de tensión de ID de módulo (`module_id.h`):
- **MX V1**: `0.30 V` (`100 kΩ / 10 kΩ`, umbral `< 680 mV`)
- **HE V1**: `1.06 V` (`100 kΩ / 47 kΩ`, umbral `< 1350 mV`)
- **HE V3**: `1.65 V` (`100 kΩ / 100 kΩ`, umbral `< 1950 mV`)
- **MX V3**: `2.24 V` (`47 kΩ / 100 kΩ`, umbral `< 2600 mV`)
<!-- src: /firmware/main/input/module_id.h, /firmware/main/input/module_id.c#module_id_classify -->

#### Mapeo de FFC reversible

Si el FFC de 14 vías se inserta al revés, el contacto `k` conecta con el contacto `15 - k`: alimentación y masa siguen alineadas, y el ID del módulo llega a `GPIO 7` (`ADC1_CH6`) en lugar de `GPIO 8` (`ADC1_CH7`). Para **HE V3**, el firmware remapea automáticamente los pines SPI3 (`BOARD_HE3_REV_*`: `MISO = GPIO 10`, `SCLK = GPIO 4`, `MOSI = GPIO 2`, `CS = GPIO 6`). Un módulo **MX V3** con el cable invertido no se lee.
<!-- src: /hardware/pcb/V3/README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MODULE_ID_ALT_GPIO -->

### FFC USB de 8 vías (Tipo A, paso 1.0 mm)

| Contactos | Red | Función | Destino |
|---|---|---|---|
| 1, 8 | `GND` | Referencia de masa | Masa del carrier |
| 2, 7 | `5V` | USB 5 V VBUS (1 A combinado) | Cabezal P1 Pin 14 |
| 3, 6 | `D-` | Datos USB Full-Speed negativo | Cabezal P1 Pin 12 (`GPIO 19`) |
| 4, 5 | `D+` | Datos USB Full-Speed positivo | Cabezal P1 Pin 11 (`GPIO 20`) |
<!-- src: /hardware/pcb/V3/README.md -->

El pinout del FFC USB de 8 vías es completamente simétrico en espejo, por lo que se lee igual desde cualquier extremo.
<!-- src: /hardware/pcb/V3/README.md -->
