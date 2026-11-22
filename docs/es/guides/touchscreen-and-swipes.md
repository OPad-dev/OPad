---
title: Uso de la pantalla táctil y deslizamientos
description: Cómo usar la pantalla táctil como tercer botón para Quick Retry en partida y configurar acciones por deslizamiento direccional para volumen, multimedia y atajos.
sidebar:
  order: 2
---

La pantalla táctil capacitiva de 2.0 pulgadas de OPad actúa como **el tercer botón** (AGENTS.md §1.4). Durante un mapa de osu!, un dedo apoyado envía Quick Retry (`` ` ``) mientras que los deslizamientos nunca disparan Quick Retry; fuera de un mapa, los toques no hacen nada y los deslizamientos direccionales ejecutan acciones configurables. Sin anfitrión o sin `tosu`, cualquier toque es Quick Retry. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->

## Qué necesitas

- Tu OPad conectado al PC. Para los deslizamientos sensibles al contexto (detección dentro/fuera de mapa y control del volumen de osu!), `opad-daemon` y `tosu` deben estar en ejecución; sin ellos, cualquier toque envía Quick Retry y los deslizamientos quedan desactivados. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->
- `opad-gui` instalado si deseas personalizar las acciones de deslizamiento. <!-- src: /desktop/gui/src/pages.rs#settings -->

## Comportamiento en partida: Quick Retry

Mientras juegas un mapa de osu! (`TOUCH_MODE_PLAY`): <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_PLAY -->

- **Dedo apoyado**: Apoya y mantén el dedo sobre la pantalla táctil. El teclado observa el movimiento durante 60 ms (`PLAY_RETRY_STILL_US`); si el dedo permanece dentro de 12 píxeles (`PLAY_RETRY_STILL_PX`), o permanece apoyado 250 ms (`PLAY_SWIPE_WINDOW_US`) sin convertirse en deslizamiento, el teclado mantiene pulsado Quick Retry (`` ` ``) hasta que levantes el dedo. Un toque breve que se levanta sin deslizar también envía una pulsación y liberación de Quick Retry. <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US, /firmware/main/input/touch_retry.c#tap_retry -->
- **Protección frente a deslizamientos**: Iniciar un deslizamiento cancela Quick Retry para ese toque, de modo que deslizar para ajustar el volumen o pasar de pista nunca reinicia tu mapa. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->
- **Sin modificadores de teclado ni superposiciones en partida**: Durante un mapa, no se envían deslizamientos de teclas (`osu!` shortcuts ni `Other key`), los deslizamientos de volumen nunca pulsan Alt y no se dibuja ninguna superposición de deslizamiento en la LCD del teclado. <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action, /firmware/main/input/touch_retry.c#handle_swipe -->

## Comportamiento fuera de partida: deslizamientos direccionales

Fuera de un mapa (`TOUCH_MODE_MENU`), los toques no envían nada y los deslizamientos direccionales disparan las acciones configuradas: <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_MENU, /desktop/crates/opad-model/src/lib.rs#DEFAULT_SWIPE_UP -->

| Dirección del deslizamiento | Acción predeterminada | Comportamiento |
|---|---|---|
| **Deslizar arriba** (`↑ SWIPE UP`) | `Volume up` | Sube el volumen de osu! cuando osu! está activo (ventana en primer plano en Windows, u osu!lazer en ejecución en Linux); sube el volumen del sistema en caso contrario. Se repite cada 30 píxeles en un arrastre largo. <!-- src: /firmware/main/input/touch_retry.c#step_volume, /firmware/main/input/gesture.h#GESTURE_SWIPE_PX --> |
| **Deslizar abajo** (`↓ SWIPE DOWN`) | `Volume down` | Baja el volumen de osu! cuando osu! está activo; baja el volumen del sistema en caso contrario. Se repite cada 30 píxeles en un arrastre largo. <!-- src: /firmware/main/input/touch_retry.c#step_volume, /firmware/main/input/gesture.h#GESTURE_SWIPE_PX --> |
| **Deslizar izquierda** (`← SWIPE LEFT`) | `Previous track` | Envía USB HID Consumer Control Previous Track una vez por deslizamiento. <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action --> |
| **Deslizar derecha** (`SWIPE RIGHT →`) | `Next track` | Envía USB HID Consumer Control Next Track una vez por deslizamiento. <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action --> |

Un deslizamiento requiere desplazarse al menos **30 píxeles** (`GESTURE_SWIPE_PX = 30`) con un eje dominante en proporción mínima de 1.5:1 (`2 * |dy| >= 3 * |dx|` para vertical, `2 * |dx| >= 3 * |dy|` para horizontal). <!-- src: /firmware/main/input/gesture.h#GESTURE_SWIPE_PX, /firmware/main/input/gesture.c#gesture_feed -->

> [!NOTE]
> En un teclado Hall Effect sin calibrar, la pantalla táctil está bloqueada al botón de calibración en pantalla, los deslizamientos están desactivados y no se envía ninguna tecla hasta completar la calibración. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->

## Configurar acciones de deslizamiento personalizadas

Las acciones de deslizamiento se configuran en `opad-gui`: <!-- src: /desktop/gui/src/pages.rs#settings -->

1. Conecta tu OPad, abre `opad-gui`, selecciona **Settings** en la barra lateral y asegúrate de estar en la pestaña **⌨ Keypad**. <!-- src: /desktop/gui/src/pages.rs#settings -->
2. Desplázate hasta la tarjeta **Touchscreen swipes**. Cada dirección tiene su propio desplegable: **↑ SWIPE UP**, **↓ SWIPE DOWN**, **← SWIPE LEFT** y **SWIPE RIGHT →**. <!-- src: /desktop/gui/src/pages.rs#settings -->
3. Elige una acción del desplegable: <!-- src: /desktop/crates/opad-model/src/swipe.rs#swipe_choices, /desktop/crates/opad-model/src/lib.rs#SwipeAction -->
   - **Multimedia / Volumen / Desactivado**: `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`.
   - **Atajos nombrados de osu!**: Atajos integrados de osu! y osu!lazer (como `osu!: Random beatmap (F2)`, `osu!: Mod select (F1)`, `osu!: Chat (F8)`, `osu!: Social / extended chat (F9)`, `osu!: Settings (Ctrl+O)`, `osu!: Screenshot (F12)`, y en Windows también `osu! stable: Boss key (hide osu!) (Insert)` y `osu! stable: Reload skin (Ctrl+Alt+Shift+S)`).
   - **`Other key (F13-F24)…`**: Muestra un segundo desplegable (`Choose a key`) con teclas de función libres que no chocan con atajos de osu! ni del sistema — **F14–F19 en Linux** (omitiendo F13, F20–F23 y F24) y **F13–F23 en Windows** (omitiendo F24, que el teclado reserva para suprimir el menú Alt de Windows). <!-- src: /desktop/crates/opad-model/src/swipe.rs#free_keys -->
4. Haz clic en **Save settings**. El daemon guarda la configuración y envía `SetConfig` al teclado. <!-- src: /desktop/gui/src/pages.rs#settings, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Cómo comprobar que funcionó

Ejecuta `opadctl status` y revisa la línea `Swipes:`, que imprime la acción configurada para las cuatro direcciones (`up`, `down`, `left`, `right`). Después desliza sobre el teclado fuera de un mapa y comprueba el distintivo visual en la LCD y la acción en tu PC. <!-- src: /desktop/cli/src/main.rs#Commands, /firmware/main/input/touch_retry.c#handle_swipe -->

## Qué hacer si no funcionó

- Si cada toque actúa como Quick Retry y los deslizamientos no se ejecutan, comprueba que `opad-daemon` y `tosu` estén conectados (`opadctl status` muestra `tosu (osu!): Active`). Consulta [Uso con tosu](use-with-tosu.md) y [Solución de problemas](../troubleshooting.md).
- Si los deslizamientos de volumen cambian el volumen del sistema en lugar del de osu!, verifica que osu! sea la ventana en primer plano (Windows) o que osu!lazer esté ejecutándose (Linux). <!-- src: /desktop/gui/src/pages.rs#settings -->
