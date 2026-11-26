---
title: Qué necesitas
description: La placa, los interruptores o PCBs V3, el cable y el sistema operativo que necesitas para montar y usar un OPad.
sidebar:
  order: 1
---

Un OPad es una placa de desarrollo de Waveshare más dos interruptores cableados a mano o las PCBs OPad V3. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md -->

## Hardware

### La placa

**Waveshare ESP32-S3-Touch-LCD-2.** Lleva un ESP32-S3 a 240 MHz con 16 MB de flash y PSRAM octal, una pantalla ST7789 de 2,0 pulgadas y 320×240 y un controlador táctil capacitivo CST816. El firmware no necesita nada más en la placa. <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

### Interruptores: elige una forma de montarlo

- **Cableado a mano:** dos interruptores mecánicos de tipo MX, cada uno cableado entre un pin del cabezal y GND. No necesitas resistencias ni diodos, porque el firmware activa los pull-ups internos del ESP32-S3. Consulta [Cableado de las teclas](wiring-the-keys.md). <!-- src: /firmware/main/input/keypad.c -->
- **PCBs OPad V3** (tres teclas): <!-- src: /hardware/pcb/V3/README.md -->
  - **Controller carrier V3:** se acopla a los cabezales P1 y P2 de la placa.
  - **Un módulo de entrada:** el **MX input module V3** (zócalos hot-swap Kailh) o el **HE input module V3** (Hall Effect: sensores TI DRV5055, un ADC TI ADS7028 y su propio regulador LP5907).
  - **USB board V3:** el puerto USB-C trasero de la carcasa, con protección ESD, una ferrita y un diodo Schottky.
  - **Cables:** un FFC de 14 vías, 1,0 mm y tipo B hacia el módulo y un FFC de 8 vías, 1,0 mm y tipo A hacia la USB board (80 mm cada uno en la carcasa V3).

### Un cable USB-C de datos

Usa un cable que transmita datos. Un cable solo de carga alimenta el teclado, pero tu PC nunca lo ve como teclado.

## Ordenador

OPad funciona en **Linux** y **Windows 10/11 (x64)**. El teclado usa los controladores que tu sistema ya trae: `usbhid` y `cdc_acm` en Linux, `hidclass.sys` y `usbser.sys` en Windows. Nunca instalas un controlador. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->

Los paquetes de la app de escritorio se compilan para x86-64 y están pensados para: <!-- src: /README.md --> <!-- src: /scripts/release/release_notes.sh -->

- Ubuntu 22.04, 24.04 y 26.04; Debian 12 y 13 (`.deb`);
- Fedora y RHEL / AlmaLinux / Rocky 9 y 10 (`.rpm`);
- Arch Linux (`.pkg.tar.zst`);
- cualquier otro Linux (AppImage);
- Windows 10 y 11 (instalador).

Los binarios de Linux se compilan contra glibc 2.31 y el tosu incluido contra glibc 2.28, así que funcionan en todas las distribuciones anteriores. <!-- src: /.github/workflows/release-app.yml --> <!-- src: /Makefile#TOSU_GLIBC -->

> [!NOTE]
> La app de escritorio es opcional. Sin ella, el teclado sigue siendo un teclado completo a 1000 Hz y su pantalla táctil envía Quick Retry. Consulta [Instalar la app de escritorio](install-the-app.md) para ver lo que añade la app. <!-- src: /AGENTS.md#1. Product rules -->
