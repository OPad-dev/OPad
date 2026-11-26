---
title: Referencia de pines del hardware
description: Pines de los cabezales P1 y P2 de la Waveshare ESP32-S3-Touch-LCD-2, pines de tecla, periféricos de la placa y los cables FFC e ID de módulo de la placa portadora V3.
sidebar:
  order: 4
---

OPad funciona sobre la Waveshare ESP32-S3-Touch-LCD-2. Los datos de pines que siguen proceden del [esquema](https://files.waveshare.com/wiki/ESP32-S3-Touch-LCD-2/ESP32-S3-Touch-LCD-2-SchDoc.pdf) de la placa, de `firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h` y de las notas del hardware V3.
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /hardware/pcb/V3/README.md -->

> [!NOTE]
> Las posiciones en el cabezal («P2 pin 11») y los números de GPIO («GPIO14») son cosas distintas. El selector de pines de la GUI nombra ambos; la serigrafía de la placa muestra el GPIO.

## Cabezales P1 y P2

| Pin | P1 | Uso en P1 | P2 | Uso en P2 |
|---|---|---|---|---|
| 1 | `GPIO2` | Libre; `SCLK` del módulo HE V3 | `3V3` | 3,3 V |
| 2 | `GPIO4` | Libre; `MOSI` del módulo HE V3 | `GND` | Masa |
| 3 | `GPIO6` | Libre; `MISO` del módulo HE V3, Key 3 del módulo MX V3 | `GPIO43` | UART0 TX (consola), reservado |
| 4 | `GPIO16` | Libre | `GPIO44` | UART0 RX (consola), reservado |
| 5 | `GPIO17` | `CAM_PWDN`, pull-down de 10 kΩ, reservado | `GPIO47` | `SCL` del I2C de la pantalla táctil y la IMU, reservado |
| 6 | `GPIO18` | Libre | `GPIO48` | `SDA` del I2C de la pantalla táctil y la IMU, reservado |
| 7 | `GPIO21` | Libre | `GPIO15` | Libre |
| 8 | `GPIO8` | ID de módulo (`ADC1_CH7`), reservado | `GPIO13` | Libre |
| 9 | `GPIO7` | Libre; Key 2 del módulo MX; ID de módulo con un cable V3 dado la vuelta (`ADC1_CH6`) | `GPIO11` | Libre |
| 10 | `GPIO10` | Libre; Key 1 del módulo MX; `CS` del módulo HE V3 | `GPIO12` | Libre |
| 11 | `GPIO20` | USB D+, reservado | `GPIO14` | **Key 1 por defecto** |
| 12 | `GPIO19` | USB D−, reservado | `GPIO9` | **Key 2 por defecto** |
| 13 | `GND` | Masa | `GND` | Masa |
| 14 | `5V` | VBUS USB directo | `VBAT` | Batería |
<!-- src: /AGENTS.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

## Pines de tecla

### Interruptores cableados a mano

Conecta cada interruptor entre su GPIO y cualquier pin `GND`. El firmware activa el pull-up interno, así que no hace falta ninguna resistencia.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

| Tecla | GPIO predeterminado | Cabezal | Tecla predeterminada |
|---|---|---|---|
| Key 1 | `GPIO14` | P2 pin 11 | `Z` (`0x1D`) |
| Key 2 | `GPIO9` | P2 pin 12 | `X` (`0x1B`) |
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_KEY1_GPIO -->

Cualquiera de las dos teclas puede moverse a uno de estos 14 GPIO (`KEY_GPIO_ALLOWED`), pero no ambas al mismo:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/main/config/config_validate.c#device_config_validate -->

| Cabezal | Pines |
|---|---|
| P1 | 1 (`GPIO2`), 2 (`GPIO4`), 3 (`GPIO6`), 4 (`GPIO16`), 6 (`GPIO18`), 7 (`GPIO21`), 9 (`GPIO7`), 10 (`GPIO10`) |
| P2 | 7 (`GPIO15`), 8 (`GPIO13`), 9 (`GPIO11`), 10 (`GPIO12`), 11 (`GPIO14`), 12 (`GPIO9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Consulta [Cambiar los pines de las teclas](../guides/change-key-pins.md) y la [Referencia de ajustes del dispositivo](device-settings.md#pines-de-tecla-permitidos).

### Pines que nunca son pines de tecla

| GPIO | Motivo |
|---|---|
| `19`, `20` | USB D− / D+ |
| `43`, `44` | Consola UART0 |
| `47`, `48` | I2C de la pantalla táctil y la IMU |
| `17` | `CAM_PWDN`, pull-down de 10 kΩ a GND |
| `8` | ADC de ID de módulo |
| `1`, `38`, `39`, `42`, `45`, `46` | Pantalla, retroiluminación e interrupción táctil (no están en los cabezales) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### Módulos de entrada

Una placa portadora lleva las teclas de un módulo MX a `GPIO10` (Key 1) y `GPIO7` (Key 2), y la Key 3 de un módulo MX V3 a `GPIO6`. Cuando el teclado arranca con un módulo MX y ambos pines de tecla siguen en los valores predeterminados del cableado a mano, los cambia a `GPIO10` / `GPIO7` y lo guarda; los pines que alguien eligió no se tocan. La GUI solo ofrece ajustes de pines de tecla en teclados sin módulo.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MX_KEY1_GPIO, /firmware/main/app_main.c, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

## Periféricos de la placa

| Periférico | Señal | GPIO | Notas |
|---|---|---|---|
| LCD ST7789 de 2,0", 320 × 240 | `SCLK` | `GPIO39` | `SPI2_HOST`, 80 MHz (`BOARD_LCD_PIXEL_CLOCK_HZ`) |
| | `MOSI` | `GPIO38` | |
| | `DC` | `GPIO42` | |
| | `CS` | `GPIO45` | |
| | `RST` | — | Sin conectar; se reinicia por alimentación o por software |
| | Retroiluminación | `GPIO1` | PWM LEDC |
| Pantalla táctil CST816 e IMU QMI8658 | `SDA` | `GPIO48` | Bus I2C compartido |
| | `SCL` | `GPIO47` | |
| | `INT` táctil | `GPIO46` | |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Cables de la placa portadora V3

V3 divide el teclado en una placa portadora, una placa USB-C en la pared trasera y módulos intercambiables de tres teclas MX o Hall Effect, unidos por FFC de 1,0 mm.
<!-- src: /hardware/pcb/V3/README.md -->

### Cable de módulo (14 vías, tipo B)

| Contacto | Placa portadora | Módulo MX | Módulo HE (ADS7028) |
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

El módulo HE alimenta sus sensores y la referencia del ADC a partir de los 5 V del cable mediante un LDO LP5907.
<!-- src: /hardware/pcb/V3/README.md -->

Un cable dado la vuelta une el contacto *k* con el contacto 15 − *k*. La alimentación y la masa siguen coincidiendo, y el ID del módulo llega por `GPIO7` en lugar de `GPIO8`. El firmware solo lee `GPIO7` cuando `GPIO8` no muestra nada, y ahí solo cuentan las clases V3. En ese caso:
<!-- src: /firmware/main/input/module_id.h, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c -->

| Módulo | Cable dado la vuelta |
|---|---|
| HE V3 | Funciona: las líneas SPI pasan a `MISO` = `GPIO10`, `SCLK` = `GPIO4`, `MOSI` = `GPIO2`, `CS` = `GPIO6` (`BOARD_HE3_REV_*`). |
| MX V3 | Se detecta, pero sus teclas no se leen. |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_HE3_REV_MISO_GPIO, /firmware/main/app_main.c -->

### ID de módulo

Cada módulo tiene un divisor de 3V3 a GND en su contacto de ID. El firmware clasifica la tensión:
<!-- src: /firmware/main/input/module_id.h -->

| Módulo | Divisor | Tensión de ID | Se interpreta como | Llega a |
|---|---|---|---|---|
| Ninguno | — | — | por debajo de 60 mV | — |
| MX V1 | 100 kΩ / 10 kΩ | 0,30 V | por debajo de 680 mV | `GPIO8` (cable JST, en un solo sentido) |
| HE V1 | 100 kΩ / 47 kΩ | 1,06 V | por debajo de 1350 mV | `GPIO8` |
| HE V3 | 100 kΩ / 100 kΩ | 1,65 V | por debajo de 1950 mV | `GPIO8`, o `GPIO7` dado la vuelta |
| MX V3 | 47 kΩ / 100 kΩ | 2,24 V | por debajo de 2600 mV | `GPIO8`, o `GPIO7` dado la vuelta |
<!-- src: /firmware/main/input/module_id.h#MODULE_ID_PRESENT_MIN_MV, /firmware/main/input/module_id.h#MODULE_ID_MX_MAX_MV, /protocol/opad.proto#InputModule -->

### Cable USB (8 vías, tipo A)

| Contacto | Red | Destino en la placa portadora |
|---|---|---|
| 1, 8 | `GND` | Masa |
| 2, 7 | `5V` | P1 pin 14 |
| 3, 6 | `D−` | P1 pin 12 (`GPIO19`) |
| 4, 5 | `D+` | P1 pin 11 (`GPIO20`) |
<!-- src: /hardware/pcb/V3/README.md -->

El pinout es simétrico, así que el cable se lee igual en cualquier sentido. Los dos contactos de 5V llevan 1 A entre ambos. El camino es pasivo, así que el flasheo y la recuperación funcionan a través de él igual que por el puerto USB propio de la Waveshare.
<!-- src: /hardware/pcb/V3/README.md -->
