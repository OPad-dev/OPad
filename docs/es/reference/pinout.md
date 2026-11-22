---
title: Referencia de Distribución de Pines (Pinout)
description: Distribución de pines de cabecera, asignaciones GPIO, mapeos de periféricos y conexiones FFC del carrier para OPad.
---

Esta página documenta la distribución completa de pines de hardware para la placa Waveshare ESP32-S3-Touch-LCD-2, el cableado de interruptores mecánicos independientes y las interconexiones de la placa carrier V3.
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

---

## Waveshare ESP32-S3-Touch-LCD-2 Headers

La placa de desarrollo expone dos cabeceras de 14 pines con espaciado de 2.54 mm: Cabecera P1 (posterior) y Cabecera P2 (frontal).
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

| Pin | Red Cabecera P1 | Función Cabecera P1 | Red Cabecera P2 | Función Cabecera P2 |
|---|---|---|---|---|
| 1 | `GPIO 2` | GPIO Libre / FFC Carrier SCLK | `3V3` | Línea de Alimentación 3.3 V |
| 2 | `GPIO 4` | GPIO Libre / FFC Carrier MOSI | `GND` | Referencia de Tierra |
| 3 | `GPIO 6` | GPIO Libre / FFC Carrier MISO | `GPIO 43` | UART0 TX (Consola) — Reservado |
| 4 | `GPIO 16` | GPIO Libre | `GPIO 44` | UART0 RX (Consola) — Reservado |
| 5 | `GPIO 17` | Pull-down 10k en placa (CAM_PWDN) — Reservado | `GPIO 47` | I2C SCL Táctil e IMU — Reservado |
| 6 | `GPIO 18` | GPIO Libre | `GPIO 48` | I2C SDA Táctil e IMU — Reservado |
| 7 | `GPIO 21` | GPIO Libre | `GPIO 15` | GPIO Libre |
| 8 | `GPIO 8` | ADC1_CH7 ID de Módulo / GPIO Libre | `GPIO 13` | GPIO Libre |
| 9 | `GPIO 7` | ID de Módulo Alt (FFC invertido) / GPIO Libre | `GPIO 11` | GPIO Libre |
| 10 | `GPIO 10` | FFC Carrier CS / GPIO Libre | `GPIO 12` | GPIO Libre |
| 11 | `GPIO 20` | USB D+ (PHY USB-C) — Reservado | `GPIO 14` | **Key 1 por Defecto** (`Z`) |
| 12 | `GPIO 19` | USB D- (PHY USB-C) — Reservado | `GPIO 9` | **Key 2 por Defecto** (`X`) |
| 13 | `GND` | Referencia de Tierra | `GND` | Referencia de Tierra |
| 14 | `5V` | Entrada USB VBUS sin regular (5 V) | `VBAT` | Entrada de Batería |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /README.md -->

---

## Standalone Switch Wiring (Header P2)

Los interruptores mecánicos convencionales (MX) no tienen polaridad. Al cablear interruptores directamente sin un módulo carrier, conecta cada interruptor entre su pin GPIO designado y tierra:
<!-- src: /README.md -->

| Tecla | Pin de Cabecera | GPIO | Código de Tecla por Defecto | Instrucciones de Conexión |
|---|---|---|---|---|
| **Key 1** | Cabecera P2, Pin 11 | `GPIO 14` | `0x1D` ('Z') | Pin A del interruptor -> Cabecera P2 Pin 11, Pin B -> GND (P2 Pin 13) |
| **Key 2** | Cabecera P2, Pin 12 | `GPIO 9` | `0x1B` ('X') | Pin A del interruptor -> Cabecera P2 Pin 12, Pin B -> GND (P2 Pin 13) |
| **Tierra** | Cabecera P2, Pin 13 | `GND` | — | Referencia de tierra para ambos interruptores |
<!-- src: /README.md, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

Los pines 11, 12 y 13 en la Cabecera P2 se ubican contiguos en el borde de la placa, permitiendo conexiones directas con cables Dupont de 3 pines o en placa de prototipos.
<!-- src: /README.md -->

---

## Valid Switch GPIO Pins

Las teclas pueden reconfigurarse a cualquier pin de cabecera compatible en la aplicación de escritorio (**Ajustes → Teclas**) o mediante `opadctl`. Solo se ofrecen pines compatibles con entrada mediante pull-up interno a 3.3V e interruptor a masa:
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /README.md -->

| Cabecera | Pines y GPIOs Soportados |
|---|---|
| **P1** | Pin 1 (`GPIO 2`), Pin 2 (`GPIO 4`), Pin 3 (`GPIO 6`), Pin 4 (`GPIO 16`), Pin 6 (`GPIO 18`), Pin 7 (`GPIO 21`), Pin 8 (`GPIO 8`), Pin 9 (`GPIO 7`), Pin 10 (`GPIO 10`) |
| **P2** | Pin 7 (`GPIO 15`), Pin 8 (`GPIO 13`), Pin 9 (`GPIO 11`), Pin 10 (`GPIO 12`), Pin 11 (`GPIO 14`), Pin 12 (`GPIO 9`) |
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /README.md -->

### Reserved Pins

El firmware rechaza los intentos de configuración en los siguientes pines:
- **USB D- / D+**: `GPIO 19` y `GPIO 20` (PHY nativo USB OTG).
  <!-- src: /firmware/main/config/config_validate.c -->
- **Consola UART0**: `GPIO 43` y `GPIO 44` (Flasheo, mensajes de bootloader, registro por serie).
  <!-- src: /firmware/main/config/config_validate.c -->
- **Bus I2C Táctil e IMU**: `GPIO 47` (SCL) y `GPIO 48` (SDA).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Interrupción Táctil**: `GPIO 46` (Interrupción táctil por flanco bajo activo del CST816).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **SPI y PWM de Pantalla**: `GPIO 38` (MOSI), `GPIO 39` (SCLK), `GPIO 42` (DC), `GPIO 45` (CS) y `GPIO 1` (PWM de retroiluminación).
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
- **Pull-down en Placa**: `GPIO 17` cuenta con un pull-down integrado de 10 kΩ a masa para el apagado de la cámara (`CAM_PWDN`).
  <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->
- **ADC de ID de Módulo**: `GPIO 8` está conectado al divisor de resistencias para la identificación del módulo carrier.
  <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

---

## On-Board Peripherals

Estos pines se conectan a componentes soldados en la placa con pantalla de Waveshare:
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

| Periférico | Señal | GPIO | Notas |
|---|---|---|---|
| **LCD ST7789 2.0"** | SCLK | `GPIO 39` | Host SPI2, reloj a 80 MHz |
| | MOSI | `GPIO 38` | Datos SPI |
| | DC | `GPIO 42` | Selección de datos / comandos |
| | CS | `GPIO 45` | Chip select |
| | Retroiluminación | `GPIO 1` | Control de retroiluminación por PWM LEDC |
| **Táctil CST816** | SDA | `GPIO 48` | I2C0 (dirección 0x15), 100 kHz |
| | SCL | `GPIO 47` | Reloj I2C0 |
| | INT | `GPIO 46` | Interrupción activa en bajo |
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h, /firmware/main/input/touch_retry.c -->

---

## V3 Carrier & Module Interconnects

La arquitectura de hardware V3 desacopla el teclado en una Placa Carrier, una placa secundaria USB dedicada y Módulos de Interruptores intercambiables (MX o Hall Effect) conectados mediante cables planos flexibles (FFC).
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 14-Way Module FFC (J_MOD)

El carrier se conecta a los módulos de interruptores mediante un cable FFC horizontal de 14 conductores y paso de 1.0 mm:
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

| Contacto | Red Carrier | Función Módulo MX | Función Hall Effect V3 (SPI ADS7028) |
|---|---|---|---|
| 1 | `GND` | Tierra | Tierra |
| 2 | `5V_MOD` | Línea de Alimentación 5 V (Filtrada vía FB1) | Línea de Alimentación 5 V (Filtrada vía FB1) |
| 3 | `3V3` | Línea de Alimentación 3.3 V | Línea de Alimentación 3.3 V |
| 4 | `GPIO 8` | Voltaje de ID de Módulo (0.30 V) | Voltaje de ID de Módulo (1.65 V) |
| 5 | `M_MISO` (`GPIO 6`) | Interruptor Key 3 | SPI MISO |
| 6 | `GND` | Tierra | Tierra |
| 7 | `M_SCLK` (`GPIO 2`) | Sin usar | SPI SCLK |
| 8 | `M_MOSI` (`GPIO 4`) | Sin usar | SPI MOSI |
| 9 | `GND` | Tierra | Tierra |
| 10 | `M_CS` (`GPIO 10`) | Interruptor Key 1 | SPI CS |
| 11 | `GPIO 7` | Interruptor Key 2 | Reserva / ID de Módulo Alt |
| 12 | `3V3` | Línea de Alimentación 3.3 V | Línea de Alimentación 3.3 V |
| 13 | `5V_MOD` | Línea de Alimentación 5 V | Línea de Alimentación 5 V |
| 14 | `GND` | Tierra | Tierra |
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

#### Reversible Cable Mapping

La distribución del conector FFC de 14 vías es simétrica. Si el cable se inserta invertido, el contacto $k$ se conecta al contacto $15 - k$:
- En un cable invertido, el ID del módulo llega por `GPIO 7` en lugar de `GPIO 8`.
- El firmware detecta esta condición a través de `module_id.c`, establece `cable_reversed = true` y ajusta dinámicamente los mapeos de pines SPI (`BOARD_HE3_REV_*`).
<!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_MODULE_ID_ALT_GPIO -->

### 8-Way USB FFC (J_USB)

La placa secundaria trasera USB-C se conecta a la placa carrier mediante un cable FFC de 8 vías con paso de 1.0 mm:
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

| Contactos | Red | Función | Destino |
|---|---|---|---|
| 1, 8 | `GND` | Referencia de Tierra | Plano de tierra del carrier |
| 2, 7 | `5V` | USB 5 V VBUS | Cabecera P1 Pin 14 |
| 3, 6 | `USB_DN` | Datos Negativos USB Full-Speed | Cabecera P1 Pin 12 (`GPIO 19`) |
| 4, 5 | `USB_DP` | Datos Positivos USB Full-Speed | Cabecera P1 Pin 11 (`GPIO 20`) |
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->

Dado que los contactos se reflejan simétricamente alrededor del centro (1 y 8, 2 y 7, 3 y 6, 4 y 5), el cable funciona idénticamente en cualquier orientación.
<!-- src: /hardware/pcb/V3/Carrier/carrier_v3.kicad_sch -->
