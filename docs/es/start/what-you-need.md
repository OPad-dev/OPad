---
title: Qué necesitas
description: Requisitos previos de hardware, especificaciones de la placa, interruptores compatibles, cables USB y requisitos del sistema operativo para montar y usar OPad.
---

OPad se construye en torno a una placa de desarrollo microcontroladora comercial combinada con interruptores cableados a mano o con las PCBs modulares OPad V3. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md -->

## Requisitos de hardware

### 1. Placa controladora principal
- **Waveshare ESP32-S3-Touch-LCD-2**: Microcontrolador ESP32-S3R8 (Xtensa LX7 de doble núcleo a 240 MHz, **16 MB de Flash**, **8 MB de Octal PSRAM**), pantalla IPS LCD ST7789 integrada de 2.0 pulgadas (`320x240`) y pantalla táctil capacitiva CST816S. <!-- src: /README.md --> <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### 2. Interruptores y módulos de entrada
Puedes montar un OPad con interruptores mecánicos cableados a mano o con las PCBs modulares OPad V3: <!-- src: /hardware/pcb/V3/README.md -->
- **Mecánico cableado a mano (MX)**: Dos interruptores mecánicos estándar compatibles con Cherry MX (Cherry, Gateron, Kailh, Outemu, etc.) cableados entre el cabezal P2 y `GND`. No se necesitan resistencias ni diodos externos. <!-- src: /README.md --> <!-- src: /firmware/main/input/keypad.c -->
- **PCBs modulares OPad V3**:
  - **Controller Carrier V3**: Se acopla directamente a los cabezales P1 y P2 de la placa Waveshare y se conecta al módulo de entrada mediante un cable FFC tipo B de 14 vías y 1.0 mm (80 mm en la carcasa V3) y a la placa USB mediante un cable FFC tipo A de 8 vías y 1.0 mm. <!-- src: /hardware/pcb/V3/README.md -->
  - **MX Input Module V3**: PCB hotswap mecánica de 3 teclas con zócalos hot-swap Kailh (`CPG151101S11`). <!-- src: /hardware/pcb/V3/README.md -->
  - **HE Input Module V3**: PCB de interruptores magnéticos Hall Effect de 3 teclas con sensores Hall TI DRV5055A3, ADC SPI de 12 bits TI ADS7028 y LDO LP5907 de 3.3 V dedicado. <!-- src: /hardware/pcb/V3/README.md -->
  - **USB Board V3**: Placa auxiliar USB-C trasera pasiva con protección ESD, ferrita y diodo Schottky antirretorno. <!-- src: /hardware/pcb/V3/README.md -->

### 3. Cable USB
- Un cable USB Tipo-C **de datos** (USB-C a USB-A o USB-C a USB-C). Los cables solo de carga suministran corriente pero no pueden enumerar interfaces USB HID ni CDC.

## Requisitos del ordenador anfitrión

OPad es compatible con **Linux** y **Windows 10/11**: <!-- src: /README.md -->

### Linux
- Controladores estándar del núcleo `usbhid` y `cdc_acm`.
- Objetivos de paquetes de lanzamiento soportados: Ubuntu 22.04 / 24.04 / 26.04, Debian 12 / 13, RHEL / AlmaLinux / Rocky 9 y 10, Fedora actual y Arch Linux (los paquetes de lanzamiento y el `tosu` incluido requieren glibc 2.28 o superior). <!-- src: /README.md -->
- Regla `udev` `70-opad.rules` para acceso serie y de entrada sin root (instalada automáticamente por los paquetes `.deb` y `.rpm`). <!-- src: /packaging/linux/udev/70-opad.rules -->

### Windows
- **Windows 10 o Windows 11 (64 bits / x64)**. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->
- Controladores integrados (`hidclass.sys` para HID y `usbser.sys` para serie CDC); no requiere controladores de terceros ni Zadig. <!-- src: /README.md -->

### Software de escritorio opcional
OPad funciona de inmediato como un teclado USB HID a 1000 Hz sin software instalado. La suite de escritorio opcional (`opad-daemon`, `opad-gui`, `opadctl` y `tosu` incluido) añade telemetría en vivo de osu!, edición del diseño de pantalla, contadores de por vida, ajuste Hall Effect y actualizaciones de firmware. <!-- src: /AGENTS.md --> <!-- src: /README.md -->
