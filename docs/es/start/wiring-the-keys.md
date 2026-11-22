---
title: Cableado de las teclas
description: Cómo cablear interruptores mecánicos a mano al cabezal P2 de la Waveshare ESP32-S3-Touch-LCD-2 o ensamblar el carrier y los módulos de entrada OPad V3.
sidebar:
  order: 2
---

Los interruptores mecánicos en OPad se leen en lógica activa-baja utilizando las resistencias pull-up internas del ESP32-S3 (`GPIO_PULLUP_ONLY`): pulsar un interruptor conecta su pin GPIO a `GND`. No se requieren resistencias ni diodos externos en montajes cableados a mano. <!-- src: /firmware/main/input/keypad.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Opción 1: Cableado manual de interruptores mecánicos (cabezal P2)

Cuando cableas dos interruptores directamente a los cabezales de la Waveshare ESP32-S3-Touch-LCD-2: <!-- src: /README.md --> <!-- src: /firmware/main/input/keypad.c -->

```text
Switch 1 (Key 1) ------------ P2 Pin 11 (GPIO 14)
                 \
                  ----------- P2 Pin 13 or Pin 2 (GND)

Switch 2 (Key 2) ------------ P2 Pin 12 (GPIO 9)
                 \
                  ----------- P2 Pin 13 or Pin 2 (GND)
```

1. **Localiza el cabezal P2** en la parte trasera de la placa Waveshare ESP32-S3-Touch-LCD-2.
2. **Cablea Key 1**: Conecta un terminal del interruptor 1 a **P2 Pin 11 (`GPIO 14`)** y el otro terminal a **GND** (P2 Pin 13 está justo al lado, o puedes usar P2 Pin 2). <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO -->
3. **Cablea Key 2**: Conecta un terminal del interruptor 2 a **P2 Pin 12 (`GPIO 9`)** y el otro terminal a **GND**. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO -->
4. **Polaridad**: Los interruptores mecánicos no tienen polaridad; cualquiera de las dos patillas puede ir al pin GPIO y la otra a `GND`.
5. **Pines personalizados**: Si conectas los interruptores a otros GPIOs admitidos de P1/P2, reasígnalos después en `opad-gui` (**Settings** -> **⌨ Keypad** -> **KEY 1 PIN** / **KEY 2 PIN** o **Auto-Detect**). Consulta [Cambio de pines de teclas](../guides/change-key-pins.md) y [Referencia de pinout](../reference/pinout.md). <!-- src: /desktop/gui/src/pages.rs#settings -->

> [!WARNING]
> Compara siempre la etiqueta física de serigrafía de la placa con **Key Pins** en `opadctl status` si una tecla no hace nada. El desplegable de la GUI muestra posiciones del cabezal del esquemático (como `GPIO14 (P2 pin 11)`), fáciles de confundir al contar los pines del cabezal. <!-- src: /AGENTS.md -->

## Opción 2: Carrier y PCBs modulares OPad V3

Si estás montando el conjunto de PCBs OPad V3 (`hardware/pcb/V3/`): <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

- **Controller Carrier V3**: Encaja sobre los cabezales P1 y P2 de la placa Waveshare mediante dos conectores hembra 1x14.
- **Cable de módulo**: Conecta el carrier al **MX Input Module V3** (3 teclas, zócalos hot-swap Kailh) o al **HE Input Module V3** (3 teclas, sensores Hall Effect + ADC ADS7028) mediante un **FFC tipo B de 14 vías y 1.0 mm** (80 mm en la carcasa V3).
- **Cable USB**: Conecta el carrier a la **USB Board V3** mediante un **FFC tipo A de 8 vías y 1.0 mm** (80 mm en la carcasa V3, pinout simétrico).
- **Enrutado fijo del carrier V3**:
  - **MX Input Module V3** (FFC derecho): `KEY1 = GPIO 10`, `KEY2 = GPIO 7`, `KEY3 = GPIO 6`, `ID = GPIO 8` (nota: un módulo MX V3 con el FFC invertido no se lee; inserta el cable tipo B derecho). <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->
  - **HE Input Module V3**: `ID = GPIO 8`, SPI3 hacia el ADS7028 en `MISO = GPIO 6`, `SCLK = GPIO 2`, `MOSI = GPIO 4`, `CS = GPIO 10` (el firmware remapea automáticamente los pines SPI si el FFC de 14 vías está invertido, con `ID` en `GPIO 7`). <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Verificar el cableado

1. Conecta el teclado al ordenador por USB.
2. Comprueba las asignaciones de pines activas y el módulo detectado en un terminal: <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
   ```bash
   opadctl status
   ```
3. En un carrier + módulo V3, ejecuta la autoprueba de PCB de hardware con todas las teclas sueltas: <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
   ```bash
   opadctl board-test
   ```
   (Añade `--fix-pins` si un módulo MX en carrier sigue configurado en los pines predeterminados autónomos `GPIO 14` / `GPIO 9`, o `--magnet` para verificar los sensores Hall Effect V3 en vivo.)
4. En montajes cableados a mano, también puedes activar **Settings** -> **🛠 Diagnostics** -> **Enable Diagnostics Menu in Navigation** en `opad-gui` y probar pulsaciones y rebotes en la pestaña **⌨ Switch Chatter & Input**, o usar **Auto-Detect** en **Settings** -> **⌨ Keypad**. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/gui/src/diagnostics.rs -->
