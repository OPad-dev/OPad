---
title: Cambiar los pines de las teclas
description: Indica a OPad en qué pines del cabezal están tus interruptores cableados a mano, eligiéndolos en la app o con Auto-Detect, y qué pines puedes usar.
sidebar:
  order: 5
---

Un OPad cableado a mano lee Key 1 en GPIO14 (cabezal P2, pin 11) y Key 2 en GPIO9 (P2, pin 12) de forma predeterminada. Si cableaste los interruptores a otros pines del cabezal, configúralos en la app. No necesitas recompilar el firmware, y el teclado recuerda los pines. <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO --> <!-- src: /desktop/gui/src/pages.rs#settings -->

> [!NOTE]
> Esto solo sirve para interruptores cableados a mano. En las PCBs OPad V3 el carrier fija los pines (MX V3: GPIO10, GPIO7, GPIO6; HE V3: un ADC por SPI), así que la app oculta los ajustes de pines. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Qué necesitas

- El teclado conectado, con la app de escritorio en marcha.
- Cada interruptor cableado entre uno de los pines de abajo y GND.

## Pines que puedes usar

El firmware acepta estos 14 GPIOs del cabezal. Cada uno puede llevar un interruptor a GND usando el pull-up interno del chip, y ninguno lo usa nada más en la placa: <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> <!-- src: /desktop/crates/opad-model/src/lib.rs#KEY_PINS -->

| Cabezal | GPIOs |
|---|---|
| P2 | GPIO14 (pin 11, predeterminado de Key 1), GPIO9 (pin 12, predeterminado de Key 2), GPIO15 (pin 7), GPIO13 (pin 8), GPIO11 (pin 9), GPIO12 (pin 10) |
| P1 | GPIO2 (pin 1), GPIO4 (pin 2), GPIO6 (pin 3), GPIO16 (pin 4), GPIO18 (pin 6), GPIO21 (pin 7), GPIO7 (pin 9), GPIO10 (pin 10) |

Key 1 y Key 2 tienen que estar en pines distintos. Cualquier otro pin se rechaza, porque algo de la placa lo necesita: <!-- src: /firmware/main/config/config_validate.c -->

- GPIO19 y GPIO20: USB D− y D+.
- GPIO43 y GPIO44: la consola UART0.
- GPIO47 y GPIO48: el bus I2C del táctil y la IMU.
- GPIO17: tiene un pull-down de 10 kΩ en la placa.
- GPIO8: la línea de ID de módulo V3, con un divisor resistivo a GND.

Un teclado que un firmware antiguo dejó en GPIO8 lo muestra como `GPIO8 (unsupported)`. Sigue funcionando ahí mientras cambias otros ajustes; simplemente no puedes mover una tecla *a* un pin que no esté en la lista. <!-- src: /desktop/crates/opad-model/src/lib.rs#validate_update --> <!-- src: /desktop/gui/src/pages.rs#settings -->

## Configura los pines

1. Abre la app de OPad y ve a **Settings** → **⌨ Keypad**.
2. En la tarjeta **Keys**, busca **KEY 1 PIN** y **KEY 2 PIN**. Para cada uno, haz una de estas dos cosas: <!-- src: /desktop/gui/src/pages.rs#settings -->
   - **elige el pin** de la lista, por ejemplo `GPIO15 (P2 pin 7)`; o
   - haz clic en **Auto-Detect** (cambia a **Detecting... (click to cancel)**) y pulsa el interruptor cableado para esa tecla. El teclado vigila todos los pines permitidos y libres y elige el que se mantiene a nivel bajo durante 15 ms. <!-- src: /firmware/main/input/keypad.c#DETECT_CONFIRM_US -->
3. Haz clic en **Save settings**.

El teclado cambia a los pines nuevos en cuanto se sueltan las dos teclas y los guarda en cuanto está en reposo. <!-- src: /firmware/main/input/keypad.c#keypad_task -->

> [!TIP]
> Un carrier V3 con un módulo MX cuyas teclas siguen con los valores predeterminados del cableado a mano (GPIO14 / GPIO9) se puede arreglar desde un terminal: `opadctl board-test --fix-pins` pone Key 1 = GPIO10 y Key 2 = GPIO7 y vuelve a probar. <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

## Comprueba que ha funcionado

1. Ejecuta `opadctl status` y comprueba que **Key Pins** muestra tus GPIOs nuevos. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
2. Pulsa las dos teclas en un editor de texto, u obsérvalas en **Diagnostics** → **⌨ Switch Chatter & Input** en la app (activa antes el menú Diagnostics en **Settings** → **🛠 Diagnostics**). <!-- src: /desktop/gui/src/diagnostics.rs --> <!-- src: /desktop/gui/src/pages.rs#settings -->

## Si no ha funcionado

- **Una tecla no hace nada tras el cambio:** compara la etiqueta GPIO impresa en la placa junto al cable con **Key Pins** en `opadctl status`. Es fácil contar mal en la placa posiciones del cabezal como `P2 pin 11`. <!-- src: /AGENTS.md#6. Hardware and debugging facts -->
- **No hay ajustes de pines:** el teclado comunica un módulo con pines fijos (un carrier V3 con un módulo MX o HE). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Auto-Detect no encuentra nada:** comprueba que la otra patilla del interruptor va a GND y que su pin está en la lista de arriba.
