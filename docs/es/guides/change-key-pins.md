---
title: Cambio de pines de teclas
description: Cómo reasignar los pines GPIO de interruptores mecánicos cableados a mano en opad-gui, pines permitidos y reservados, y notas sobre pines del carrier V3.
sidebar:
  order: 5
---

Por defecto, los montajes autónomos cableados a mano asignan Key 1 a `GPIO 14` (cabezal P2, pin 11) y Key 2 a `GPIO 9` (cabezal P2, pin 12). Si tus interruptores están cableados a otros pines admitidos del cabezal, puedes reasignarlos en `opad-gui` sin recompilar el firmware. <!-- src: /firmware/main/input/keypad.c --> <!-- src: /desktop/gui/src/pages.rs#settings -->

> [!NOTE]
> La reasignación de pines solo aplica a interruptores mecánicos cableados a mano. En las PCBs modulares OPad V3, el enrutado de los interruptores es fijo por el carrier (MX V3: `GPIO 10` / `GPIO 7` / `GPIO 6`; HE V3: ADS7028 por SPI3), y `opad-gui` oculta los selectores de pines automáticamente cuando un módulo informa pines fijos. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

## Qué necesitas

- Tu OPad conectado por USB con `opad-daemon` y `opad-gui` en ejecución.
- Cada interruptor mecánico cableado entre un pin GPIO admitido de P1/P2 y `GND`.

## Pines GPIO permitidos

El firmware acepta únicamente los 14 GPIOs de cabezal de `KEY_GPIO_ALLOWED` que admiten pull-up interno a 3.3 V sin entrar en conflicto con periféricos integrados: <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

| Posición en cabezal | GPIO | Notas |
|---|---|---|
| P2 pin 11 | `GPIO 14` | Key 1 por defecto |
| P2 pin 12 | `GPIO 9` | Key 2 por defecto |
| P2 pin 7 | `GPIO 15` | Permitido |
| P2 pin 8 | `GPIO 13` | Permitido |
| P2 pin 9 | `GPIO 11` | Permitido |
| P2 pin 10 | `GPIO 12` | Permitido |
| P1 pin 1 | `GPIO 2` | Permitido |
| P1 pin 2 | `GPIO 4` | Permitido |
| P1 pin 3 | `GPIO 6` | Permitido |
| P1 pin 4 | `GPIO 16` | Permitido |
| P1 pin 6 | `GPIO 18` | Permitido |
| P1 pin 7 | `GPIO 21` | Permitido |
| P1 pin 9 | `GPIO 7` | Permitido |
| P1 pin 10 | `GPIO 10` | Permitido |

Key 1 y Key 2 no pueden compartir el mismo GPIO. Los siguientes pines están reservados y son rechazados por la validación: <!-- src: /firmware/main/config/config_validate.c#device_config_validate -->
- `GPIO 19` / `GPIO 20`: USB nativo `D-` / `D+`.
- `GPIO 43` / `GPIO 44`: Consola UART0.
- `GPIO 47` / `GPIO 48`: Bus I2C táctil e IMU (`SCL` / `SDA`).
- `GPIO 17`: Pull-down integrado de 10 kΩ (`CAM_PWDN`).
- `GPIO 8`: Divisor resistivo ADC de identificación de módulo.

## Reasignar pines en `opad-gui`

1. Abre `opad-gui` y haz clic en **Settings** en la barra lateral.
2. Quédate en la pestaña **⌨ Keypad**. <!-- src: /desktop/gui/src/pages.rs#settings -->
3. En **Keys**, localiza **KEY 1 PIN** y **KEY 2 PIN**. Tienes dos formas de ajustar cada pin: <!-- src: /desktop/gui/src/pages.rs#settings -->
   - **Selección manual**: Elige el pin en el desplegable (por ejemplo, `GPIO15 (P2 pin 7)`).
   - **Auto-Detect**: Haz clic en **Auto-Detect** junto a **KEY 1 PIN** o **KEY 2 PIN** (el botón cambia a **Detecting... (click to cancel)**), y pulsa el interruptor físico cableado para esa tecla. El teclado escanea todos los GPIOs permitidos sin usar y rellena el pin que permanezca en nivel BAJO durante al menos 15 ms (`DETECT_CONFIRM_US = 15000`). <!-- src: /firmware/main/input/keypad.c#detect_scan_step -->
4. Haz clic en **Save settings**. El teclado prepara los nuevos pines y los aplica en cuanto ambas teclas están sueltas, guardándolos luego en NVS al quedar en `IDLE`. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/input/keypad.c#keypad_task -->

Para un módulo MX en carrier V3 cuyos pines guardados sigan en los valores autónomos predeterminados (`GPIO 14` / `GPIO 9`), también puedes cambiarlos a los pines del carrier (`GPIO 10` / `GPIO 7`) desde un terminal con `opadctl board-test --fix-pins`. <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

## Cómo comprobarlo

1. Ejecuta `opadctl status` y verifica que **Key Pins** coincide con tus nuevos GPIOs. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
2. Pulsa Key 1 y Key 2 en un editor de texto (o en la pestaña **Diagnostics** -> **⌨ Switch Chatter & Input** de `opad-gui`) y confirma que ambas teclas se registran limpiamente. <!-- src: /desktop/gui/src/diagnostics.rs -->

## Qué hacer si no funcionó

- **Una tecla no hace nada después de cambiar su pin**: Comprueba la etiqueta física de serigrafía del GPIO al que está conectado el cable y compárala con **Key Pins** en `opadctl status`. Las posiciones del cabezal en el esquemático (como `P2 pin 11`) son fáciles de contar mal en la placa. <!-- src: /AGENTS.md -->
- **No aparecen los selectores de pines**: Tu teclado informa un módulo de entrada de pines fijos (como un carrier V3 + módulo MX/HE), donde los pines van trazados en la PCB. <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Auto-Detect agota el tiempo**: Asegúrate de que la otra patilla del interruptor esté conectada a `GND` y de que el pin figure en la lista de permitidos anterior.
