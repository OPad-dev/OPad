---
title: Qué necesitas
description: Requisitos de hardware, especificaciones de la placa, interruptores compatibles, cables USB y requisitos del sistema operativo para montar y utilizar OPad.
---

OPad está construido alrededor de una placa de desarrollo con microcontrolador estándar combinada con interruptores cableados a mano o PCBs modulares personalizadas. <!-- src: /README.md -->

## Requisitos de hardware

### 1. Placa controladora principal
- **Waveshare ESP32-S3-Touch-LCD-2**: La plataforma de hardware estándar cuenta con un microcontrolador ESP32-S3 (Xtensa LX7 de doble núcleo a 240 MHz, 8 MB de Flash, 2 MB de PSRAM), una pantalla LCD IPS ST7789 integrada de 2,0 pulgadas (resolución 320x240) y una pantalla táctil capacitiva CST816S. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 2. Interruptores y módulos de entrada
Puedes construir un OPad utilizando interruptores mecánicos cableados a mano o módulos PCB personalizados: <!-- src: /hardware/pcb/V3/README.md -->
- **Mecánico cableado a mano (MX)**: Dos interruptores de teclado mecánico estándar compatibles con Cherry MX. Se admite cualquier marca de interruptores (Cherry, Gateron, Kailh, Outemu). Los interruptores se conectan entre la cabecera P2 y masa con cables de puente estándar.
- **PCBs modulares OPad V3 (Recomendado)**:
  - **Controller Carrier V3**: Placa portadora que se conecta directamente a las cabeceras de la placa de desarrollo Waveshare. <!-- src: /hardware/pcb/V3/README.md -->
  - **Módulo de entrada MX V3**: PCB hotswap mecánica de 3 teclas con zócalos Kailh. <!-- src: /hardware/pcb/V3/README.md -->
  - **Módulo de entrada HE V3**: PCB magnética Hall Effect de 3 teclas con sensores rapid trigger. Compatible con interruptores magnéticos como Gateron Magnetic Jade / KS-20. <!-- src: /hardware/pcb/V3/README.md -->
  - **Placa USB V3**: Placa secundaria que enruta limpiamente las señales USB Type-C hacia la parte trasera de la caja. <!-- src: /hardware/pcb/V3/README.md -->

### 3. Cable USB
- Un cable de datos de alta calidad USB Type-C a Type-A (o Type-C a Type-C).
- Asegúrate de que el cable admita transmisión de datos fiable a alta velocidad; los cables baratos exclusivos de carga suministran energía pero no pueden enumerar interfaces USB HID ni CDC serie.

## Requisitos del ordenador host

OPad funciona tanto en sistemas operativos Linux como en Windows: <!-- src: /README.md -->

### Linux
- **Núcleo**: Linux 5.15 o posterior con los módulos estándar `cdc_acm` y `usbhid` cargados.
- **Distribuciones**: Totalmente compatible con Ubuntu, Debian, Fedora, Arch Linux y derivados.
- **Permisos**: Se requiere una regla `udev` estándar para que los usuarios habituales puedan acceder al puerto serie CDC sin permisos de root (`70-opad.rules`).

### Windows
- **Sistema operativo**: Windows 10 (64 bits) o Windows 11.
- **Controladores**: Los controladores de clase USB HID y CDC ACM estándar están integrados en Windows 10/11; no se necesitan archivos INF ni controladores WinUSB de terceros.

### Software de escritorio opcional
El teclado funciona completamente como un teclado USB a 1000 Hz sin necesidad de instalar ningún software de escritorio (Regla de producto 2). La suite de escritorio opcional (`opad-daemon`, `opad-gui`, `opadctl` y `tosu`) ofrece telemetría en tiempo real durante la partida, diseño de pantallas y ajuste de Hall Effect. <!-- src: /README.md -->
