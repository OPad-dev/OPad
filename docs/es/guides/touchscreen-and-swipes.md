---
title: Usar la pantalla táctil y los deslizamientos
description: La pantalla táctil es el cuarto botón de OPad, después de sus tres teclas. Un dedo apoyado es Quick Retry durante un mapa, y los deslizamientos ejecutan acciones que tú eliges, como el volumen y las teclas multimedia.
sidebar:
  order: 2
---

La pantalla táctil es el cuarto botón de OPad, después de sus tres teclas. Durante un mapa de osu!, un dedo apoyado en ella mantiene pulsado Quick Retry (`` ` ``) mientras siga ahí. Los deslizamientos ejecutan acciones que tú eliges, de forma predeterminada controles de volumen y de pista. <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->

Para distinguir un mapa de un menú, el teclado necesita la app de escritorio con tosu conectado. Sin ellos, cada toque es Quick Retry y no hay deslizamientos. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_set_host_status -->

## Cómo se comporta un toque

| Situación | Un dedo apoyado | Un deslizamiento |
|---|---|---|
| **Sin app, o tosu no conectado** | Quick Retry, al instante, mantenido hasta que levantes el dedo | Nada: sigue siendo Quick Retry |
| **Durante un mapa** | Quick Retry, mantenido hasta que levantes el dedo | Su acción de volumen o multimedia. Nunca Quick Retry, nunca una tecla del teclado |
| **Fuera de un mapa** | Nada | Su acción |
<!-- src: /firmware/main/input/touch_retry.c#touch_mode_now --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task --> <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->

### Durante un mapa

- **Quick Retry espera a que el dedo se asiente.** El teclado pulsa Quick Retry cuando el dedo se ha mantenido dentro de 12 píxeles durante 60 ms, o lleva 250 ms apoyado sin convertirse en deslizamiento. Un toque más corto que eso sigue enviando una pulsación y liberación de Quick Retry. El retry de osu! es de todas formas una pulsación mantenida, así que la espera no se nota. <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US --> <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_PX --> <!-- src: /firmware/main/input/touch_retry.c#PLAY_SWIPE_WINDOW_US --> <!-- src: /firmware/main/input/touch_retry.c#tap_retry -->
- **Un deslizamiento nunca reinicia el mapa.** En cuanto un toque se convierte en deslizamiento, ya no puede enviar Quick Retry. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->
- **Solo se ejecutan las acciones de volumen y multimedia.** Los atajos de osu! y las demás teclas del teclado se omiten, los deslizamientos de volumen envían una rueda del ratón simple sin Alt y no se dibuja nada en la pantalla del teclado. <!-- src: /firmware/main/input/touch_retry.c#step_volume --> <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

### Qué cuenta como deslizamiento

Un deslizamiento recorre al menos 30 píxeles, principalmente a lo largo de un eje: una dirección tiene que superar a la otra al menos en proporción 1,5 a 1. Un deslizamiento vertical sigue avanzando un paso por cada 30 píxeles más mientras arrastras, así que un solo arrastre largo cambia el volumen varios pasos. Cualquier otra acción se dispara una vez por toque. <!-- src: /firmware/main/input/gesture.h#GESTURE_SWIPE_PX --> <!-- src: /firmware/main/input/gesture.c#gesture_feed --> <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

## Deslizamientos predeterminados

| Deslizamiento | Acción predeterminada | Qué hace |
|---|---|---|
| Arriba | `Volume up` | El volumen de osu! cuando osu! está activo; si no, el volumen del sistema. Se repite a lo largo de un arrastre largo. |
| Abajo | `Volume down` | Lo mismo, hacia abajo. |
| Izquierda | `Previous track` | La tecla multimedia Previous Track. |
| Derecha | `Next track` | La tecla multimedia Next Track. |
<!-- src: /desktop/crates/opad-model/src/lib.rs#DEFAULT_SWIPE_UP --> <!-- src: /firmware/main/input/touch_retry.c#s_swipe_actions --> <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->

"osu! está activo" significa: <!-- src: /desktop/daemon/src/osu_window.rs -->

- **Windows:** osu! (stable o lazer) es la ventana en primer plano.
- **Linux:** osu!lazer está en ejecución. Linux no tiene una forma portable de preguntar qué ventana tiene el foco, así que mientras lazer se ejecuta, un deslizamiento de volumen envía el control de volumen de osu! (Alt+rueda) a la ventana que esté en primer plano.

## Elige tus acciones de deslizamiento

1. Conecta el teclado y abre la app de OPad.
2. Abre **Settings** → **⌨ Keypad** y desplázate hasta **Touchscreen swipes**.
3. Cada dirección tiene una lista: **↑ SWIPE UP**, **↓ SWIPE DOWN**, **← SWIPE LEFT** y **SWIPE RIGHT →**. Elige una acción: <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/crates/opad-model/src/swipe.rs#swipe_choices -->
   - **Multimedia y volumen:** `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`.
   - **Un atajo de osu!**, como `osu!: Random beatmap (F2)`, `osu!: Mod select (F1)`, `osu!: Chat (F8)`, `osu!: Settings (Ctrl+O)` u `osu!: Screenshot (F12)`. Los atajos exclusivos de osu!lazer empiezan por `osu!lazer:`; en Windows la lista también incluye `Boss key (hide osu!) (Insert)` y `Reload skin (Ctrl+Alt+Shift+S)` de osu! stable. <!-- src: /desktop/crates/opad-model/src/swipe.rs#OSU_SHORTCUTS -->
   - **`Other key (F13-F24)…`:** una segunda lista, **Choose a key**, ofrece teclas de función que no usa nada más, para asignarlas en OBS, Discord o el propio osu!. En Linux son F14–F19 (el escritorio da a F13 y F20–F23 sus propios significados); en Windows, F13–F23. F24 nunca se ofrece, porque el teclado la usa internamente tras un paso de volumen con Alt+rueda. <!-- src: /desktop/crates/opad-model/src/swipe.rs#free_keys -->
4. Haz clic en **Save settings**. El teclado guarda las acciones, así que funcionan igual en cualquier PC con OPad. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/config/device_config.c -->

> [!NOTE]
> Un teclado Hall Effect sin calibrar está bloqueado en su pantalla de calibración. Ahí, un toque solo pulsa el botón de esa pantalla: los deslizamientos están desactivados y no se envía ninguna tecla. Consulta [Calibrar interruptores Hall Effect](calibrate-hall-effect.md). <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Comprueba que ha funcionado

1. Ejecuta `opadctl status`. La línea `Swipes:` muestra la acción de cada dirección (`up`, `down`, `left`, `right`). <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
2. Fuera de un mapa, desliza el dedo sobre el teclado. Muestra una breve insignia en la dirección del deslizamiento y la acción ocurre en tu PC. <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

## Si no ha funcionado

- **Cada toque es Quick Retry y los deslizamientos no hacen nada:** el teclado no recibe noticias de tosu. Comprueba que `opadctl status` muestra `tosu (osu!): Active`; consulta [Usar OPad con tosu](use-with-tosu.md). <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->
- **Los deslizamientos de volumen cambian el volumen del sistema en lugar del de osu!:** en Windows, pon osu! en primer plano; en Linux, inicia osu!lazer. <!-- src: /desktop/daemon/src/osu_window.rs -->
- **Un deslizamiento con un atajo de osu! no hace nada en un mapa:** es así por diseño. Durante un mapa nunca se envían teclas del teclado. <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->
