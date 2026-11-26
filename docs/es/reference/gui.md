---
title: Referencia de la GUI
description: Las páginas de opad-gui, las pestañas de Settings, la página Keys, el Screen Designer, las pestañas de Diagnostics y la página Stats, con sus etiquetas y rangos.
sidebar:
  order: 2
---

`opad-gui` es una aplicación de escritorio `iced` con icono en la bandeja del sistema. Se comunica con `opad-daemon` por IPC local (consulta la [Referencia del protocolo IPC](ipc.md)) y no mantiene ninguna conexión propia con el teclado.
<!-- src: /desktop/gui/src/main.rs#main -->

| Argumento | Efecto |
|---|---|
| `--tray` | Arranca oculta en la bandeja. |
| `--page <name>`, `-p <name>` | Abre en una página (tokens más abajo, sin distinguir mayúsculas). Un segundo lanzamiento pasa la página a la instancia en marcha en lugar de abrir otra. |
<!-- src: /desktop/gui/src/main.rs#parse_page_arg, /desktop/gui/src/main.rs#main, /desktop/gui/src/single_instance.rs -->

## Páginas

La barra lateral muestra `Dashboard`, `Stats`, `Designer`, `Settings`, `Device`, `Logs` y `About`. `Keys` se añade después de `Device` mientras hay conectado un teclado con un backend Hall Effect activo, `Diagnostics` antes de `About` una vez activado en **Settings → 🛠 Diagnostics**, y `Calibration` al final mientras está abierta.
<!-- src: /desktop/gui/src/main.rs#Page::ALL, /desktop/gui/src/main.rs#view -->

| Página | Tokens de `--page` | Contenido |
|---|---|---|
| **Dashboard** | `dashboard` | Conexión y modo, pulsaciones totales, perfil de osu! y el mapa seleccionado en ese momento, y **Key latency** (`p50`, `p99`, `p99.9`, `max`, `deferred`, `presses`, **Reset**). |
| **Stats** | `stats`, `kpm`, `ppm` | Ritmo de pulsación en KPM del intento actual o del último, y el historial. |
| **Designer** | `designer` | Editor del diseño de pantalla. |
| **Settings** | `settings`, `updates` | Cinco pestañas; `updates` abre la pestaña **🔄 Updates & Firmware**. |
| **Device** | `device` | Detalles del teclado, conciliación de contadores, acciones, copia de seguridad y restauración. |
| **Keys** | `keys`, `rapid-trigger` | Actuación Hall Effect y rapid trigger. |
| **Logs** | `logs`, `monitor` | Flujo de registros con los filtros **LEVEL:** y **SOURCE:**, **Clear**, **Copy** y **Save log**. |
| **Diagnostics** | `diagnostics`, `test`, `testing` | Cinco pestañas de pruebas. |
| **Calibration** | `calibrate`, `calibration` | Calibración Hall Effect guiada. |
| **About** | `about`, `license`, `licenses` | Versión, datos del sistema y del hardware, licencias de código abierto. |
<!-- src: /desktop/gui/src/main.rs#Page::TOKENS, /desktop/gui/src/main.rs#wants_updates_tab, /desktop/gui/src/pages.rs -->

## Settings

Pestañas, por orden: **⌨ Keypad**, **🖥 Display & Game**, **🔄 Updates & Firmware**, **ℹ About & tosu**, **🛠 Diagnostics**. Las dos primeras terminan con **Save settings**, que envía los ajustes al teclado.
<!-- src: /desktop/gui/src/pages.rs#settings -->

### ⌨ Keypad

| Control | Se muestra | Rango / opciones |
|---|---|---|
| **KEY 1**, **KEY 2** | Siempre | La tecla que envía cada interruptor |
| **KEY 3**, **GAME KEYS** | Teclados de tres teclas | La tecla de la tecla 3 (vacía no envía nada); una o dos de Key 1–3 como teclas de juego |
| **KEY 1 PIN**, **KEY 2 PIN** con **Auto-Detect** | Teclados cableados a mano (sin módulo), o firmware que no comunica el módulo | Los pines de cabezal permitidos, menos el pin de la otra tecla |
| **DEBOUNCE LOCKOUT** | Siempre | `0.5`–`20.0` ms en pasos de `0.5` ms |
| **HALL EFFECT CALIBRATION**: **Recalibrate Keys** | Teclados Hall Effect | Abre **Calibration** |
| **↑ SWIPE UP**, **↓ SWIPE DOWN**, **← SWIPE LEFT**, **SWIPE RIGHT →** | Siempre | Ver más abajo |
<!-- src: /desktop/gui/src/pages.rs#settings, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

Cada selector de deslizamiento ofrece `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, los atajos de osu! (por ejemplo `Random beatmap`, `Mod select`, `Settings`) y `Other key (F13-F24)…`. Las teclas libres que se ofrecen son F13–F23 en Windows y F14–F19 en Linux. Rangos y valores en el cable: [Referencia de ajustes del dispositivo](device-settings.md#deslizamientos-en-la-pantalla-táctil-cfg).
<!-- src: /desktop/crates/opad-model/src/lib.rs#SwipeAction, /desktop/crates/opad-model/src/swipe.rs#swipe_choices, /desktop/crates/opad-model/src/swipe.rs#free_keys -->

### 🖥 Display & Game

| Sección | Control | Rango |
|---|---|---|
| Display | **BRIGHTNESS** | `10`–`100` % |
| Display | **SLEEP AFTER** | `1`–`60` min (`60`–`3600` s, pasos de 60 s) |
| Result animations | Un selector de animación y un botón **Play** por calificación (`SS`, `S`, `A`, `B`, `C`, `D`) | |
| Advanced | **GAMEPLAY REFRESH RATE** | `1`–`30` Hz (predeterminado `10`) |
| Advanced | **Start in tray at login** | Casilla |
<!-- src: /desktop/gui/src/pages.rs#settings -->

### 🔄 Updates & Firmware

- **SOFTWARE UPDATES**: una fila para OPad y otra para tosu (versión instalada y disponible, **What's new**, **Install now**), **Install OPad updates automatically**, **Install tosu updates automatically**, el selector **Channel** (`stable`: solo versiones finales; `beta`: también versiones candidatas) y **Check now**.
- **FIRMWARE UPDATE**: firmware instalado y ranura en ejecución, bloqueos, cuándo se instalará una actualización automática, notas de la versión, **Install firmware updates automatically** y **Update now**, que pide consentimiento explícito antes de flashear.
<!-- src: /desktop/gui/src/pages.rs#updates, /desktop/gui/src/pages.rs#firmware_updates -->

Los tres interruptores de actualización automática están activados por defecto.
<!-- src: /desktop/daemon/src/updater.rs#read_flag -->

### ℹ About & tosu

- **TOSU INTEGRATION**: **Binary source:**, **Status:**, **Custom path:**, **Browse external tosu...** y **Use bundled** mientras hay una ruta personalizada.
- **ABOUT & THIRD-PARTY SOFTWARE**: la licencia de OPad (GPL-3.0-or-later) y el componente tosu incluido.
<!-- src: /desktop/gui/src/pages.rs#tosu_settings, /desktop/gui/src/pages.rs#about_section -->

### 🛠 Diagnostics

**Enable Diagnostics Menu in Navigation** y **Open Diagnostics Menu →**.
<!-- src: /desktop/gui/src/pages.rs#settings -->

## Device

| Sección | Contenido |
|---|---|
| Teclado | **Device ID**, **Board**, **Firmware**, **Running slot**, **Mode**, **Counters source**, **Counter generation**, **Last sync** |
| Counter Reconciliation State | **PC Database** frente a **PAD Hardware** para generación, Key 1, Key 2 y total, con `✓ SYNCHRONIZED` o `⚠ MISMATCH DETECTED`; si no coinciden, **Restore PAD from PC** e **Import PC from PAD** |
| Acciones | **Sync now**, **Update firmware** (abre Settings), **Reset lifetime counters** |
| Backup & Restore | **Export backup**, **Import backup**; desactivados durante un mapa y su enfriamiento |
<!-- src: /desktop/gui/src/pages.rs#device -->

## Keys

Solo se muestra mientras hay conectado un teclado con un backend Hall Effect activo. Cada ajuste es un porcentaje del rango magnético calibrado de la tecla, en pasos de deslizador de `0.5` %.
<!-- src: /desktop/gui/src/keys.rs#view, /desktop/gui/src/keys.rs#setting -->

| Control | Se muestra | Rango |
|---|---|---|
| **Set keys separately** | Siempre | Una columna por tecla, o un único control para todas |
| **Advanced** / **Hide advanced**, **Recalibrate** | Siempre | |
| **ACTUATION POINT** | Siempre | Desde el punto más bajo en que puede pulsar la tecla hasta `95.0` % |
| **Rapid trigger** | Siempre | Conmutador |
| **SENSITIVITY** | Rapid trigger activado, vista básica | `1.0`–`50.0` % |
| **Continuous** | Rapid trigger activado | Conmutador |
| **PRESS SENSITIVITY**, **RELEASE SENSITIVITY** | Avanzado, rapid trigger activado | `1.0`–`50.0` % |
| **RELEASE POINT** | Avanzado | `1.0`–`50.0` % |
| **TOP RESET** | Avanzado, Continuous activado | `1.0`–`50.0` % |
| **BOTTOM DEAD ZONE** | Avanzado | `0.0`–`30.0` %, `0` = desactivada |
| **Allow below the noise floor** | Avanzado, firmware que comunica sus pisos de ruido | Conmutador |
| **Revert**, **Save to pad** | Siempre | |
<!-- src: /desktop/gui/src/keys.rs#key_controls, /desktop/gui/src/keys.rs#min_actuation -->

La vista en vivo del interruptor sondea `GetHeLive` cada 20 ms, suaviza el dibujo con un filtro One Euro y lo ajusta a la profundidad real cuando el teclado pulsa; el propio teclado decide con lecturas crudas. Durante un mapa muestra `Waiting for the pad (paused during a map)`.
<!-- src: /desktop/gui/src/main.rs#subscription, /desktop/gui/src/keys.rs -->

## Calibration

Un proceso guiado: mide la posición de reposo y después pulsa Key 1, Key 2 (y Key 3 en teclados de tres teclas) hasta el fondo 5 veces cada una. Termina con `✓ Calibration Complete` o `⚠ Calibration Failed` y **Retry Calibration**. Consulta [Calibración de interruptores Hall Effect](../guides/calibrate-hall-effect.md).
<!-- src: /desktop/gui/src/pages.rs#calibration -->

## Designer

Edita los diseños `Idle` y `Playing` de la pantalla de 320 × 240, dibujados a 2× (`SCALE = 2.0`) por la propia interfaz LVGL del firmware compilada en `opad-ui-preview`.
<!-- src: /desktop/gui/src/designer/mod.rs#SCALE, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_LCD_H_RES -->

- Hasta 32 widgets por pantalla (`MAX_WIDGETS`), con asas de arrastre y redimensionado (`HANDLE = 5` píxeles del teclado). Las flechas mueven la selección (Shift = 10 px); Supr la elimina.
- Lista de widgets con **Raise**, **Lower**, **Duplicate** y **Delete**; lo que está arriba en la lista se dibuja encima.
- Por widget: tipo, fuente de datos, fuente tipográfica, alineación, posición y tamaño, radio, decimales, etiqueta, sufijo, colores, y **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)**.
- Barra de herramientas: **Apply to pad**, **Revert**, **Reset to default**, **Export**, **Import** (JSON, `opad-<screen>.json`), **Live data from osu!**, **Show keys pressed**.
<!-- src: /desktop/gui/src/designer/mod.rs, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS, /desktop/crates/opad-layout/src/lib.rs#FLAG_BG_FILL -->

El formato del diseño y las fuentes de datos están en [Personalizar el diseño de pantalla](../guides/customize-screen-layout.md).

## Diagnostics

| Pestaña | Contenido |
|---|---|
| **⌨ Switch Chatter & Input** | Tarjetas de teclas en vivo; las pulsaciones de menos de 15 ms cuentan como rebote (con un pitido opcional), las de 15–35 ms como **RAPID FLUTTER** y las que caen a menos de 10 ms de la otra tecla como **SIMULTANEOUS PRESSES (NKRO)**; el monitor de pulsaciones Hall Effect o MX. |
| **🔌 Hardware COM & Protocol** | Latencia de ping al daemon, estado de la conexión serie y del protocolo, percentiles de latencia. |
| **🖥 Screen & Backlight** | Niveles de retroiluminación, patrones de prueba, vista previa del diseño. |
| **🔧 PCB Test** | **Run PCB Test** (`RunBoardTest`); un arreglo **Set Key 1 = GPIO10, Key 2 = GPIO7** cuando los pines no son los de la placa portadora; la prueba de imanes de Hall Effect V3; **Restart Pad**. |
| **📦 Export Diagnostic Bundle** | Un paquete JSON con datos de la aplicación, el sistema operativo, el daemon, el teclado, la prueba de placa y la configuración. |
<!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab, /desktop/gui/src/diagnostics.rs#view_board_test, /desktop/gui/src/diagnostics.rs#view_restart, /desktop/gui/src/diagnostics.rs#generate_diagnostic_bundle -->

## Stats

Ritmo de pulsación en **KPM** (teclas por minuto, no el BPM del mapa).
<!-- src: /desktop/gui/src/stats.rs -->

- **Current attempt** (`LIVE`) o **Last attempt**: el mapa, **AVG KPM**, **PEAK KPM** (las 7 pulsaciones seguidas más rápidas), **BEST 10 S** y una tabla por canal (K1, K2, Combined) con el valor actual, la media al pulsar, el pico y las pulsaciones. Una repetición se marca `REPLAY · NOT SAVED`.
- **History**: botones de periodo `7 days`, `30 days`, `90 days`, `All time`; **ATTEMPTS**, **AVG KPM**, **PEAK KPM**, **BEST 10 S**, **TAPPING AVG**, **PRESSES** y, por canal, media al pulsar, pico medio, mejor pico y pulsaciones.
<!-- src: /desktop/gui/src/stats.rs, /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->
