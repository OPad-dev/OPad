---
title: Referencia de la GUI
description: Las páginas de opad-gui (Dashboard, Stats, Keys, Screen, Designer, Pad, Settings, Troubleshooting), cómo guarda cada una, el menú de la bandeja y sus etiquetas y rangos.
sidebar:
  order: 2
---

`opad-gui` es una aplicación de escritorio `iced` con icono en la bandeja del sistema. Se comunica con `opad-daemon` por IPC local (consulta la [Referencia del protocolo IPC](ipc.md)) y no mantiene ninguna conexión propia con el teclado.
<!-- src: /desktop/gui/src/main.rs#main -->

| Argumento | Efecto |
|---|---|
| `--tray` | Arranca oculta en la bandeja. |
| `--page <nombre>`, `-p <nombre>` | Abre una página o una de sus pestañas (nombres abajo, sin distinguir mayúsculas). Un segundo arranque pasa la página a la instancia que ya está en marcha en vez de abrir otra. |
<!-- src: /desktop/gui/src/main.rs#parse_page_arg, /desktop/gui/src/main.rs#main, /desktop/gui/src/single_instance.rs -->

## Páginas

La barra lateral tiene ocho páginas en tres grupos, siempre las mismas. Una página que necesita el teclado lo dice en vez de desaparecer.
<!-- src: /desktop/gui/src/main.rs#Page::GROUPS, /desktop/gui/src/widgets.rs#pad_section -->

| Grupo | Página | Nombres para `--page` | Contenido |
|---|---|---|---|
| Play | **Dashboard** | `dashboard` | Pulsaciones totales, una tarjeta por tecla, perfil de osu!, el mapa seleccionado y **Key speed**. |
| Play | **Stats** | `stats`, `kpm`, `ppm` | **Key speed** en detalle y el ritmo de pulsación en KPM. |
| Your pad | **Keys** | `keys`, `rapid-trigger`; `calibrate` y `calibration` abren la calibración | Qué envía cada tecla y cómo pulsa. |
| Your pad | **Screen** | `screen`, `display` | Brillo, reposo, gestos de la pantalla táctil, animaciones de resultado. |
| Your pad | **Designer** | `designer` | Editor del diseño de la pantalla. |
| Your pad | **Pad** | `pad`, `device` | Datos del teclado, contadores, copia de seguridad. |
| App | **Settings** | `settings`; pestañas `general`, `updates`, `osu` / `tosu`, `about` / `license` / `licenses` | La aplicación en este ordenador. |
| App | **Troubleshooting** | `troubleshooting`, `diagnostics`; pestañas `logs` / `monitor`, `key-test` / `test` / `testing`, `board-test`, `connection`, `report` | Registro, pruebas y un informe para enviar. |
<!-- src: /desktop/gui/src/main.rs#Target::TOKENS, /desktop/gui/src/main.rs#Page::GROUPS -->

Debajo de las páginas, la barra lateral muestra `Pad connected` o `Pad not connected` (abre **Pad**) y `osu! connected` u `osu! not connected` (abre **Settings**, **osu! connection**).
<!-- src: /desktop/gui/src/main.rs#view -->

## Cómo guardan las páginas

| Páginas | Cómo |
|---|---|
| **Keys**, **Screen** | Una barra al pie de las dos páginas: `Unsaved changes`, **Revert**, **Save to pad**. Dice `Everything is saved on the pad.` cuando no hay nada pendiente y `Connect your pad to change its settings.` sin teclado. Al salir de estas dos páginas con cambios sin guardar pregunta **Stay**, **Discard** o **Save to pad**. |
| **Settings** | Cada control se aplica al instante; no hay botón de guardar. |
| **Designer** | **Apply to pad**, por pantalla. |
<!-- src: /desktop/gui/src/widgets.rs#save_bar, /desktop/gui/src/main.rs#go, /desktop/gui/src/main.rs#pad_dirty -->

El resultado de una acción aparece como un aviso en una esquina de la ventana. Se ven hasta tres a la vez; un aviso dura 4 s, un fallo 8 s, y más mientras el puntero está encima. El texto completo de un fallo va además a **Troubleshooting**, **Logs**.
<!-- src: /desktop/gui/src/main.rs#Tone::stays, /desktop/gui/src/main.rs#MAX_NOTICES, /desktop/gui/src/main.rs#fail -->

Una franja se queda arriba en la ventana solo mientras algo necesita respuesta: el servicio de OPad no está en marcha (**Start it**), el almacenamiento falló, el teclado y la aplicación necesitan versiones distintas, un teclado nuevo quiere los contadores del anterior, el teclado se configuró en otro ordenador (**Use it here**, **Not now**) o una actualización espera un reinicio.
<!-- src: /desktop/gui/src/main.rs#view -->

## Dashboard

- **TOTAL PRESSES** de las teclas de juego, y una tarjeta por tecla (`Key 1 · Z`) con su contador de por vida y el de este mapa.
- Qué está haciendo el teclado: `Ready`, `In a map`, `Map just ended` o `Syncing`.
- **OSU! PROFILE** y **NOW PLAYING**.
- **Key speed**: el tiempo típico del interruptor al ordenador, con **Details**, que abre **Stats**.
- Sin teclado: `No pad found` y `Plug it in with its USB cable.`
<!-- src: /desktop/gui/src/pages.rs#dashboard, /desktop/gui/src/pages.rs#mode_words, /desktop/gui/src/pages.rs#osu_cards -->

## Stats

**Key speed** es el tiempo desde que el interruptor cierra hasta que el teclado entrega la pulsación al USB, medido en el teclado: **TYPICAL** (mediana), **99% UNDER**, **99.9% UNDER**, **SLOWEST**, **PRESSES MEASURED**, **WAITED ONE USB POLL** y **Reset**.
<!-- src: /desktop/gui/src/stats.rs#key_speed_card -->

El ritmo de pulsación va en **KPM** (teclas por minuto, no los BPM del mapa).

- **Tap rate (KPM) · current attempt** (`LIVE`) o **· last attempt**: el mapa, **AVG KPM**, **PEAK KPM** (las 7 pulsaciones seguidas más rápidas), **BEST 10 S** y una tabla con el valor actual, la media al pulsar, el pico y las pulsaciones. Sus filas llevan el nombre de la tecla que las cuenta (`Key 1 · Z`, `Key 2 · X`, `Both keys`). Una repetición se marca como `REPLAY · NOT SAVED`.
- **History**: botones de periodo `7 days`, `30 days`, `90 days`, `All time`; **ATTEMPTS**, **AVG KPM**, **PEAK KPM**, **BEST 10 S**, **TAPPING AVG**, **PRESSES** y, por fila, media al pulsar, pico medio, mejor pico y pulsaciones.
<!-- src: /desktop/gui/src/stats.rs#view, /desktop/gui/src/stats.rs#channel_names, /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->

## Keys

| Sección | Se muestra | Contenido |
|---|---|---|
| **Bindings** | Siempre | Una casilla por tecla. Haz clic en una casilla y pulsa la letra (de la A a la Z) que debe enviar; Esc conserva la que tiene. En teclados de tres teclas, Key 3 también puede **Send nothing**, y **GAME KEYS** elige una o dos de Key 1–3. |
| **Key feel** | Teclados Hall Effect | La vista en vivo del interruptor y los ajustes de abajo. Dice `The keys are not calibrated` con **Calibrate** hasta que lo estén. |
| **Wiring** | Teclados cableados a mano (sin módulo), o firmware que no informa del módulo | **KEY 1 PIN**, **KEY 2 PIN** con **Auto-detect**: los pines permitidos del conector, menos el de la otra tecla. |
| **Advanced** | Siempre, plegada (**Show** / **Hide**) | Teclados Hall Effect: los ajustes finos de abajo. Los demás: **SWITCH BOUNCE FILTER**, `0.5`–`20.0` ms en pasos de `0.5` ms. |
<!-- src: /desktop/gui/src/keys.rs#view, /desktop/gui/src/keys.rs#bindings, /desktop/gui/src/keys.rs#wiring, /desktop/gui/src/keys.rs#advanced, /desktop/gui/src/main.rs#captured_letter, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

Cada ajuste Hall Effect es un porcentaje del recorrido magnético calibrado de la tecla, en pasos de `0.5` %.
<!-- src: /desktop/gui/src/keys.rs#setting -->

| Control | Se muestra | Rango |
|---|---|---|
| **Set keys separately** | Key feel | Una columna por tecla, o un control para todas |
| **ACTUATION POINT** | Key feel | Desde el punto más bajo en que la tecla puede pulsar hasta `95.0` % |
| **Rapid trigger** | Key feel | Interruptor |
| **SENSITIVITY** | Key feel, rapid trigger activado | `1.0`–`50.0` % |
| **Continuous** | Key feel, rapid trigger activado | Interruptor |
| **Recalibrate** | Key feel | Abre la calibración |
| **PRESS SENSITIVITY**, **RELEASE SENSITIVITY** | Advanced, rapid trigger activado | `1.0`–`50.0` % |
| **RELEASE POINT** | Advanced | `1.0`–`50.0` % |
| **TOP RESET** | Advanced, Continuous activado | `1.0`–`50.0` % |
| **BOTTOM DEAD ZONE** | Advanced | `0.0`–`30.0` %, `0` = desactivado |
| **Allow below the noise floor** | Advanced, firmware que informa de sus suelos de ruido | Interruptor |
<!-- src: /desktop/gui/src/keys.rs#key_controls, /desktop/gui/src/keys.rs#key_advanced, /desktop/gui/src/keys.rs#min_actuation -->

La vista en vivo del interruptor recibe del daemon las muestras del teclado según llegan (mira cada 20 ms), suaviza el dibujo con un filtro One Euro y lo ajusta a la profundidad real cuando el teclado pulsa; el teclado decide con las lecturas en bruto. Durante un mapa muestra `Waiting for the pad (paused during a map)`.
<!-- src: /desktop/gui/src/main.rs#subscription, /desktop/gui/src/keys.rs -->

### Calibración

La calibración se abre dentro de **Keys** (**Calibrate**, **Recalibrate** o `--page calibrate`) y **Back to Keys** la cierra. Mide las teclas en reposo y luego pide pulsar a fondo 5 veces Key 1, Key 2 (y Key 3 en teclados de tres teclas). Termina con `Calibration done` o `Calibration did not finish`, una frase que dice qué falló, y **Try again**. Consulta [Calibración de interruptores Hall Effect](../guides/calibrate-hall-effect.md).
<!-- src: /desktop/gui/src/pages.rs#calibration, /desktop/gui/src/main.rs#Target::CALIBRATION -->

## Screen

| Sección | Control | Rango |
|---|---|---|
| **Display** | **BRIGHTNESS** | `10`–`100` % |
| **Display** | **SLEEP AFTER** | `1`–`60` min (`60`–`3600` s, pasos de 60 s) |
| **Display** | **SCREEN UPDATES DURING A MAP** | `1`–`30` Hz (por defecto `10`) |
| **Touchscreen** | **SWIPE UP**, **SWIPE DOWN**, **SWIPE LEFT**, **SWIPE RIGHT** | Ver abajo |
| **Result animations** | Un selector de animación y un botón **Play** por nota (`SS`, `S`, `A`, `B`, `C`, `D`) | |
| **Layout** | **Edit the layout** | Abre **Designer** |
<!-- src: /desktop/gui/src/screen.rs#display, /desktop/gui/src/screen.rs#touchscreen, /desktop/gui/src/screen.rs#result_animations, /desktop/gui/src/screen.rs#view -->

Cada selector de gesto ofrece `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, los atajos de osu! (por ejemplo `Random beatmap`, `Mod select`, `Settings`) y `Other key (F13-F24)…`. Las teclas libres que ofrece son F13–F23 en Windows y F14–F19 en Linux. Rangos y valores en el cable: [Referencia de ajustes del dispositivo](device-settings.md#deslizamientos-en-la-pantalla-táctil-cfg).
<!-- src: /desktop/crates/opad-model/src/lib.rs#SwipeAction, /desktop/crates/opad-model/src/swipe.rs#swipe_choices, /desktop/crates/opad-model/src/swipe.rs#free_keys -->

## Designer

Edita los diseños `Idle` y `Playing` de la pantalla de 320 × 240, dibujados a 2× (`SCALE = 2.0`) por la propia interfaz LVGL del firmware compilada en `opad-ui-preview`.
<!-- src: /desktop/gui/src/designer/mod.rs#SCALE, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_LCD_H_RES -->

- Botones **Idle screen** y **Playing screen**. Un `•` en un botón indica que esa pantalla tiene cambios que el teclado todavía no muestra.
- Hasta 32 widgets por pantalla (`MAX_WIDGETS`), con asas para arrastrar y redimensionar (`HANDLE = 5` píxeles del teclado). Las flechas mueven la selección (Mayús = 10 px); Supr la elimina.
- Lista de widgets con **Raise**, **Lower**, **Duplicate** y **Delete**; lo de arriba en la lista se dibuja encima.
- Por widget: tipo, fuente de datos, tipografía, alineación, posición y tamaño, radio, decimales, etiqueta, sufijo, colores y **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)**.
- **Apply to pad** envía la pantalla abierta; **Apply both** envía todas las pantallas con cambios, una detrás de otra.
- **Undo** (o Ctrl+Z) retrocede por los cambios de la pantalla abierta, hasta 100. Un arrastre o una palabra escrita es un paso. También recupera lo que **Revert**, **Import**, **Reload** o **Reset to default** sustituyeron.
- **Revert** vuelve a lo que muestra el teclado. **Reset to default** hace que el teclado olvide el diseño personalizado de la pantalla abierta y dibuje de nuevo el suyo.
- **Import**, **Export** (JSON, `opad-<pantalla>.json`), **Reload**, **Live data from osu!**, **Show keys pressed**.
<!-- src: /desktop/gui/src/designer/mod.rs#update, /desktop/gui/src/designer/mod.rs#handle, /desktop/gui/src/designer/mod.rs#UNDO_DEPTH, /desktop/gui/src/designer/mod.rs#view, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS, /desktop/crates/opad-layout/src/lib.rs#FLAG_BG_FILL -->

El formato del diseño y las fuentes de datos están en [Personalizar el diseño de pantalla](../guides/customize-screen-layout.md).

## Pad

| Sección | Contenido |
|---|---|
| **This pad** | **Pad ID**, **Board**, **Firmware** (con **Update to …**, que abre **Settings**, **Updates**, cuando hay una disponible), qué está haciendo el teclado, **Last sync**. **Details** añade **Running slot**, **Counters source** y **Counter generation**. |
| **Counters** | **On this PC** junto a **On the pad** para cada tecla (Key 3 en teclados de tres teclas), con `The PC and the pad agree` o `The PC and the pad differ`. **Sync now** y **Reset lifetime counters** se desactivan sin teclado y durante un mapa. **Restore pad from PC** e **Import PC from pad** solo aparecen cuando los dos difieren. |
| **Backup** | **Export backup**, **Import backup** y **Last automatic backup**; desactivados durante un mapa. |
<!-- src: /desktop/gui/src/pages.rs#pad -->

## Settings

Pestañas, en orden: **General**, **Updates**, **osu! connection**, **About**.
<!-- src: /desktop/gui/src/settings.rs#Tab, /desktop/gui/src/settings.rs#view -->

| Pestaña | Contenido |
|---|---|
| **General** | **Start OPad at login**: arranca en la bandeja, sin abrir la ventana. |
| **Updates** | Una fila para **OPad**, otra para **tosu** y otra para **Pad firmware**, todas con la misma forma: versión instalada y disponible, **What's new**, **Update automatically** e **Install now** (**Update now** en el firmware, que pide consentimiento explícito antes de grabar y dice cuánto tiempo deja de escribir el teclado). Después **Channel** (`stable`: solo versiones finales; `beta`: también candidatas) y **Check now**. |
| **osu! connection** | `Connected` o `Not connected` con **Start**, y **tosu program**: el que está **In use**, **Choose another tosu…** y **Use the one that comes with OPad** mientras haya otro elegido. |
| **About** | Versión, licencia y código fuente de OPad; qué es tosu, su versión instalada y su código fuente; **Open source licences**. |
<!-- src: /desktop/gui/src/settings.rs#general, /desktop/gui/src/settings.rs#updates, /desktop/gui/src/settings.rs#firmware_row, /desktop/gui/src/settings.rs#osu_connection, /desktop/gui/src/settings.rs#about -->

Los tres interruptores de actualización automática vienen activados.
<!-- src: /desktop/daemon/src/updater.rs#read_flag -->

## Troubleshooting

| Pestaña | Contenido |
|---|---|
| **Logs** | El registro del teclado, del servicio de OPad, de la aplicación y de tosu, con filtros **LEVEL** y **SOURCE**, **Clear**, **Copy**, **Save log** y **Newest first** / **Oldest first**. |
| **Key test** | Tarjetas de tecla en vivo; las pulsaciones de menos de 15 ms cuentan como rebote (con pitido opcional), las de 15–35 ms como **RAPID FLUTTER**, las que caen a menos de 10 ms de la otra tecla como **SIMULTANEOUS PRESSES (NKRO)**; el monitor de pulsaciones Hall Effect o MX. |
| **Board test** | **Run the board test** (`RunBoardTest`); el arreglo **Set Key 1 = GPIO10, Key 2 = GPIO7** cuando los pines no son los de la placa base; la prueba de imán de Hall Effect V3; **Restart pad**. |
| **Connection** | Solo valores medidos: el servicio de OPad, el teclado, osu! (tosu), **Pad ID**, **Board**, **Firmware** y **App to service** con **Measure**. En Linux, un aviso cuando tosu puede no tener permiso para leer la memoria de osu!. |
| **Report** | **Save report…** y **Copy**: un archivo JSON con los datos de la aplicación, el sistema, el servicio, el teclado, la prueba de placa y los ajustes, y una vista previa de lo que contiene. |
<!-- src: /desktop/gui/src/troubleshooting.rs#Tab, /desktop/gui/src/pages.rs#logs, /desktop/gui/src/troubleshooting.rs#view_input_tester, /desktop/gui/src/troubleshooting.rs#view_board_test, /desktop/gui/src/troubleshooting.rs#view_restart, /desktop/gui/src/troubleshooting.rs#view_connection, /desktop/gui/src/troubleshooting.rs#view_export_bundle, /desktop/gui/src/troubleshooting.rs#generate_diagnostic_bundle -->

## Menú de la bandeja

Una línea de estado (por ejemplo `Pad connected · Ready`, `Pad not connected` o `Pad needs an update`), `Total presses: N`, y después **Open OPad**, **Keys**, **Troubleshooting**, **Sync pad now** (**Start OPad service** mientras el servicio no está en marcha) y **Quit OPad**.
<!-- src: /desktop/gui/src/tray.rs#TrayViewModel, /desktop/gui/src/tray.rs#TrayAction -->

Donde el escritorio no tiene bandeja del sistema, la barra lateral lo dice: cerrar la ventana cierra entonces OPad, y el teclado sigue funcionando.
<!-- src: /desktop/gui/src/main.rs#view -->
