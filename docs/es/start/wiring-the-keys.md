---
title: Cableado de las teclas
description: Guía de cableado paso a paso para interruptores mecánicos en la cabecera P2, asignación de pines GPIO, lógica pull-up y montaje de la placa portadora OPad V3.
category: tutorial
source_files:
  - /README.md
  - /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h
  - /firmware/main/input/keypad.c
  - /docs/reference/pinout.md
---

OPad lee los interruptores en lógica activa baja: cada pin de interruptor está configurado con resistencias pull-up internas del ESP32-S3 (`GPIO_PULLUP_ONLY`). Al pulsar un interruptor, el pin GPIO se conecta directamente a masa (`GND`). No se requieren resistencias, diodos ni condensadores externos. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Cableado manual de interruptores mecánicos (MX)

Al montar un OPad en una placa de prototipos o en una caja personalizada utilizando directamente la placa Waveshare ESP32-S3-Touch-LCD-2, los interruptores se conectan a la cabecera P2: <!-- src: /docs/reference/pinout.md -->

```
Interruptor 1 (Tecla 1) ------------ P2 Pin 11 (GPIO 14)
                        \
                         ----------- P2 Pin 2 / Pin 13 (GND)

Interruptor 2 (Tecla 2) ------------ P2 Pin 12 (GPIO 9)
                        \
                         ----------- P2 Pin 2 / Pin 13 (GND)
```

### Instrucciones de cableado
1. **Identificar la cabecera P2**: Localiza la cabecera macho dual de 14 pines y 1,27 mm etiquetada como **P2** en la parte trasera de la placa de desarrollo Waveshare.
2. **Conexión de la Tecla 1**: Conecta un terminal del Interruptor 1 a **P2 Pin 11 (GPIO 14)**. Conecta el otro terminal a cualquier pin **GND** (como P2 Pin 2 o Pin 13). <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO -->
3. **Conexión de la Tecla 2**: Conecta un terminal del Interruptor 2 a **P2 Pin 12 (GPIO 9)**. Conecta el otro terminal a **GND**. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO -->
4. **Tercera tecla (Opcional MX / HE V3)**: Si utilizas la Tecla 3, conecta el interruptor entre **P2 Pin 7 (GPIO 15)** y **GND**.
5. **Polaridad**: Los interruptores mecánicos son cierres de contacto sin polaridad; no importa qué pin del interruptor se conecte a GPIO y cuál a GND.

> [!WARNING]
> Comprueba siempre la serigrafía física frente a `opadctl status` antes de sospechar de fallos en el firmware. La GUI y la documentación hacen referencia a posiciones de cabecera en el esquema ("GPIO14 / P2 Pin 11"), lo cual es fácil de confundir en cabeceras densas de 1,27 mm (AGENTS.md §6).

## Uso de la placa portadora OPad V3

Si estás utilizando la placa portadora personalizada OPad V3 (`hardware/pcb/V3/Carrier/`): <!-- src: /docs/reference/pinout.md -->
- **Sin cables manuales**: La placa portadora incluye conectores hembra duales de 1,27 mm que encajan directamente en las cabeceras P1 y P2 de la placa Waveshare.
- **Rutas fijas**: Las pistas de la PCB enrutan GPIO 14, GPIO 9 y GPIO 15 hacia el conector del módulo de entrada con el desacoplo y planos de masa adecuados.
- **Zócalos hotswap**: El Módulo MX V3 cuenta con zócalos hotswap Kailh. Basta con alinear las dos patillas metálicas de cualquier interruptor mecánico estilo Cherry MX con los orificios del zócalo hotswap y presionar suavemente hasta que quede enrasado.

## Verificación de los contactos de los interruptores

Una vez cableado:
1. Conecta el teclado al ordenador.
2. Ejecuta `opadctl status` o abre `opad-gui` para comprobar las entradas detectadas:
   ```bash
   opadctl status
   ```
3. Prueba la actuación física en la página de **Diagnósticos** de `opad-gui` o mediante `opadctl detect-pin`:
   ```bash
   opadctl detect-pin 14
   ```
   Pulsa el interruptor: el terminal mostrará `Pin 14 actuated successfully!`.
