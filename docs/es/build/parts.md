---
title: Piezas y placas
description: La placa Waveshare, los interruptores o las PCBs OPad V3, los cables y la carcasa que necesitas para montar tu propio OPad.
sidebar:
  order: 1
---

Esta sección es para montar tu propio OPad. Si tienes un OPad terminado, no tienes nada que montar: empieza por [Qué necesitas](../start/what-you-need.md).

Un OPad montado por ti es una placa de desarrollo de Waveshare más dos interruptores cableados a mano o las PCBs OPad V3. <!-- src: /README.md --> <!-- src: /hardware/pcb/V3/README.md --> Montarlo lleva tres pasos:

1. Consigue las piezas de esta página.
2. Cablea los interruptores o encaja entre sí las placas V3: [Cableado de las teclas](wiring-the-keys.md).
3. Conéctalo y comprueba que escribe: [Primera conexión](../start/first-connection.md). A partir de ahí, los pasos son los mismos que con un OPad terminado.

> [!NOTE]
> El OPad que se vende es el **OPad V1**; internamente, sus placas y su carcasa son la revisión **V3**. Cuando esta documentación habla de V1, V2 o V3, se refiere a una revisión de placa o de carcasa, no al producto. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) --> <!-- src: /hardware/pcb/V3/README.md -->

## La placa

**Waveshare ESP32-S3-Touch-LCD-2.** Lleva un ESP32-S3 a 240 MHz con 16 MB de flash y PSRAM octal, una pantalla ST7789 de 2,0 pulgadas y 320×240 y un controlador táctil capacitivo CST816. El firmware no necesita nada más en la placa. <!-- src: /firmware/sdkconfig.defaults --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Interruptores: elige una forma de montarlo

- **Cableado a mano:** dos interruptores mecánicos de tipo MX, cada uno cableado entre un pin del cabezal y GND. No necesitas resistencias ni diodos, porque el firmware activa los pull-ups internos del ESP32-S3. Consulta [Cableado de las teclas](wiring-the-keys.md). <!-- src: /firmware/main/input/keypad.c -->
- **PCBs OPad V3** (tres teclas): <!-- src: /hardware/pcb/V3/README.md -->
  - **Controller carrier V3:** se acopla a los cabezales P1 y P2 de la placa.
  - **Un módulo de entrada:** el **MX input module V3** (zócalos hot-swap Kailh) o el **HE input module V3** (Hall Effect: sensores TI DRV5055, un ADC TI ADS7028 y su propio regulador LP5907).
  - **USB board V3:** el puerto USB-C trasero de la carcasa, con protección ESD, una ferrita y un diodo Schottky.
  - **Cables:** un FFC de 14 vías, 1,0 mm y tipo B hacia el módulo y un FFC de 8 vías, 1,0 mm y tipo A hacia la USB board (80 mm cada uno en las carcasas V3 y V4).

Los proyectos de KiCad y las listas de piezas completas están en [`hardware/pcb/V3/`](https://github.com/OPad-dev/OPad/tree/main/hardware/pcb/V3); [Hardware](../develop/hardware.md) da una visión general.

## La carcasa

La carcasa V4 aloja el carrier bajo la placa Waveshare, la USB board con su puerto USB-C a través de la pared trasera y el módulo MX o Hall Effect de tres teclas. Sus archivos de OpenSCAD y las notas de impresión están en [`hardware/3d/custom_case/V4/`](https://github.com/OPad-dev/OPad/blob/main/hardware/3d/custom_case/V4/README.md). <!-- src: /hardware/3d/custom_case/V4/README.md -->

La V4 es el interior de la carcasa V3 dentro de un exterior facetado, y todavía no se ha impreso. La [carcasa V3](https://github.com/OPad-dev/OPad/blob/main/hardware/3d/custom_case/V3/README.md) es la referencia ya impresa y probada; las placas y el montaje son los mismos en las dos. <!-- src: /hardware/3d/custom_case/V4/README.md -->

## Un cable USB-C de datos

Usa un cable que transmita datos. Un cable solo de carga alimenta el teclado, pero tu PC nunca lo ve como teclado.

## Ordenador

Un teclado montado por ti necesita el mismo ordenador que uno terminado: Linux o Windows 10/11, sin controladores. Consulta [Qué necesitas](../start/what-you-need.md). <!-- src: /README.md -->
