---
title: Referencia de la GUI
description: Guía de referencia de las páginas de opad-gui, navegación lateral, pestañas de Settings, página Keys de Hall Effect, Screen Designer, suite Diagnostics y página Stats.
---

`opad-gui` es una aplicación de escritorio basada en `iced` con icono en la bandeja del sistema (`--tray` inicia oculta en la bandeja, y `--page <name>` abre una página concreta). Se comunica con `opad-daemon` mediante IPC local. <!-- src: /desktop/gui/src/main.rs#parse_page_arg -->

## Páginas de navegación

La barra lateral izquierda muestra `Dashboard`, `Stats`, `Designer`, `Settings`, `Device`, `Logs` y `About`, además de `Keys` (mostrada automáticamente cuando hay conectado un teclado Hall Effect) y `Diagnostics` (cuando está activada en **Settings** -> **🛠 Diagnostics**). `Calibration` se abre desde **Keys** -> **Recalibrate**, **Settings** -> **⌨ Keypad** -> **Recalibrate Keys** o `--page calibrate`. <!-- src: /desktop/gui/src/main.rs#Page -->

| Página | Tokens de `--page` | Propósito |
|---|---|---|
| **Dashboard** | `dashboard` | Estado de conexión y modo, contadores de pulsaciones de por vida, perfil de osu! / resumen del mapa en vivo y latencia de teclas medida en el dispositivo (`p50`, `p99`, `p99.9`, `max`, `deferred`, `presses`, **Reset**). <!-- src: /desktop/gui/src/pages.rs#dashboard --> |
| **Keys** (solo HE) | `keys`, `rapid-trigger` | Actuación Hall Effect, Rapid Trigger, umbrales avanzados y visualizador en vivo del corte transversal del interruptor. <!-- src: /desktop/gui/src/keys.rs#view --> |
| **Stats** | `stats`, `kpm`, `ppm` | Ritmo de pulsación del intento actual/último e histórico en KPM (teclas por minuto). <!-- src: /desktop/gui/src/stats.rs --> |
| **Designer** | `designer` | Editor WYSIWYG del diseño de la pantalla LCD 320x240 para las vistas `Idle` y `Playing`. <!-- src: /desktop/gui/src/designer/mod.rs --> |
| **Settings** | `settings` | Cinco subpestañas para teclado, pantalla, actualizaciones/firmware, `tosu`/acerca de y conmutador de diagnósticos. <!-- src: /desktop/gui/src/pages.rs#settings --> |
| **Device** | `device` | Información de hardware (`Device ID`, `Board`, `Firmware`, `Running slot`, `Counter generation`, `Last sync`), tabla **Counter Reconciliation State**, **Sync now**, **Update firmware**, **Reset lifetime counters** y **Backup & Restore** (**Export backup** / **Import backup**). <!-- src: /desktop/gui/src/pages.rs#device --> |
| **Logs** | `logs`, `monitor` | Flujo en vivo de registros de diagnóstico del daemon, teclado, programa y `tosu`. <!-- src: /desktop/gui/src/main.rs#Page --> |
| **Diagnostics** | `diagnostics`, `test`, `testing` | Suite de pruebas de hardware y protocolo en cinco pestañas (activable desde Settings). <!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab --> |
| **Calibration** | `calibrate`, `calibration` | Asistente guiado de calibración Hall Effect de 5 pulsaciones por tecla. <!-- src: /desktop/gui/src/pages.rs#calibration --> |
| **About** | `about`, `license`, `licenses` | Versión de la aplicación, licencias de código abierto y avisos de terceros. <!-- src: /desktop/gui/src/main.rs#Page --> |

## Pestañas de ajustes (`Settings`)

La página **Settings** tiene cinco pestañas: <!-- src: /desktop/gui/src/pages.rs#settings -->

### `⌨ Keypad`
- **Keys**: **KEY 1** y **KEY 2** (más **KEY 3** y casillas **GAME KEYS** en teclados V3 de 3 teclas), desplegables **KEY 1 PIN** / **KEY 2 PIN** con **Auto-Detect** (mostrados solo en montajes cableados a mano), deslizador **DEBOUNCE LOCKOUT** (`0.5 ms`–`20.0 ms`, paso `0.5 ms`) y **HALL EFFECT CALIBRATION** (botón **Recalibrate Keys** en teclados HE). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Touchscreen swipes**: Selectores por dirección (`↑ SWIPE UP`, `↓ SWIPE DOWN`, `← SWIPE LEFT`, `SWIPE RIGHT →`) con `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, atajos de osu! u `Other key (F13-F24)…`. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/crates/opad-model/src/swipe.rs -->
- **Save settings** aplica los cambios en el teclado.

### `🖥 Display & Game`
- **Display**: Deslizador **BRIGHTNESS** (`10%`–`100%`) y deslizador **SLEEP AFTER** (`1 min`–`60 min`, `60..=3600` s en pasos de 60 s). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Result animations**: Desplegables de animación por calificación (`SS`, `S`, `A`, `B`, `C`, `D`) y botones de vista previa **Play**. <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Advanced**: Deslizador **GAMEPLAY REFRESH RATE** (`1–30 Hz`, por defecto `10 Hz`) y casilla **Start in tray at login**. <!-- src: /desktop/gui/src/pages.rs#settings -->

### `🔄 Updates & Firmware`
- **SOFTWARE UPDATES**: Versiones instalada frente a disponible para **OPad** (**Install now**, **Check automatically**) y **tosu** (**Update now**, **Update automatically**), selector **Channel** (`stable` / `beta`) y botón **Check now**. <!-- src: /desktop/gui/src/pages.rs#updates -->
- **FIRMWARE UPDATE**: Firmware instalado, ranura OTA activa (`ota_0` / `ota_1`), notas de lanzamiento, bloqueos activos y botón **Update firmware** (abre un diálogo de consentimiento explícito con **Confirm & Flash Pad**). <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->

### `ℹ About & tosu`
- **TOSU INTEGRATION**: Muestra **Binary source:**, **Status:** (`Connected (streaming telemetry)` / `Not running / Disconnected`), **Custom path:**, **Browse external tosu...** y **Use bundled** (cuando se sustituye). <!-- src: /desktop/gui/src/pages.rs#tosu_settings -->
- **ABOUT & THIRD-PARTY SOFTWARE**: Versión, licencia GPL-3.0-or-later y atribución LGPL-3.0 del `tosu` incluido. <!-- src: /desktop/gui/src/pages.rs#about_section -->

### `🛠 Diagnostics`
- Casilla **Enable Diagnostics Menu in Navigation** y botón **Open Diagnostics Menu →**. <!-- src: /desktop/gui/src/pages.rs#settings -->

## Página Keys (ajuste Hall Effect)

Se muestra únicamente cuando hay conectado un teclado Hall Effect. Todos los ajustes de recorrido son porcentajes del rango magnético calibrado de cada tecla (pasos de `0.5 %` en el deslizador, nunca milímetros según D3): <!-- src: /desktop/gui/src/keys.rs#view -->
- Barra superior: conmutador **Set keys separately**, botón **Advanced** / **Hide advanced**, botón **Recalibrate**.
- Modo básico: **ACTUATION POINT**, conmutador **Rapid trigger**, **SENSITIVITY** (`1.0 %`–`50.0 %`), conmutador **Continuous**.
- El modo avanzado añade: **PRESS SENSITIVITY**, **RELEASE SENSITIVITY**, **RELEASE POINT**, **TOP RESET** (con Continuous activo), **BOTTOM DEAD ZONE** (`0.0 %`–`30.0 %`, `0 = off`) y **Allow below the noise floor**.
- Lienzo en vivo del corte transversal del interruptor: consulta `GetHeLive` a ~50 Hz con suavizado visual One Euro y ajuste directo en los flancos (el teclado decide con lecturas crudas); se pausa durante los mapas.
- Barra inferior: **Revert** y **Save to pad**.

## Diseñador de pantalla (`Designer`)

Editor visual para la pantalla LCD `320x240` (diseños `Idle` y `Playing`), renderizado a escala `2.0x` (`SCALE = 2.0`) usando la propia interfaz LVGL del firmware compilada en `opad-ui-preview`: <!-- src: /desktop/gui/src/designer/mod.rs -->
- Hasta `32` widgets (`MAX_WIDGETS`), asas de arrastre/redimensionado (`HANDLE = 5` píxeles del teclado), ajuste fino con teclas de flecha, orden Z (**Raise** / **Lower**) y duplicar/eliminar.
- Propiedades de widget: tipo, fuente de datos, fuente tipográfica, alineación, geometría (`X`, `Y`, `W`, `H`, radio), decimales, unidad de ritmo de pulsación, etiqueta/sufijo, colores (primer plano, fondo, acento, fondo de pantalla) y banderas (`FLAG_BG_FILL`, `FLAG_BORDER`, `FLAG_HIDE_WHEN_EMPTY`, `FLAG_KEY_RATE`).
- Conmutador de vista previa de datos en vivo, simulación de pulsaciones, exportación/importación JSON de diseños, **Apply**, **Revert** y restablecimiento al diseño predeterminado integrado.

## Suite de diagnósticos (`Diagnostics`)

Cinco pestañas (`DiagnosticsTab` en `desktop/gui/src/diagnostics.rs`): <!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab -->
1. **`⌨ Switch Chatter & Input`**: Tarjetas de pulsación en vivo, detección de rebote definitivo `< 15 ms` con pitido opcional, seguimiento de fluctuación `15–35 ms`, conteo de pulsaciones simultáneas (`<= 10 ms`) y monitor de registro de depuración HE/MX.
2. **`🔌 Hardware COM & Protocol`**: Latencia de ping al daemon, salud de conexión serie y protocolo, y control de reinicio del teclado.
3. **`🖥 Screen & Backlight`**: Patrones de prueba de pantalla y controles de prueba del brillo de retroiluminación.
4. **`🔧 PCB Test`**: Ejecuta `RunBoardTest` sobre las PCBs del carrier y del módulo de entrada (ID de módulo, orientación FFC, líneas de teclas, cortocircuitos, ADS7028), asistente **Use carrier key pins** y prueba de imán en vivo para Hall Effect V3.
5. **`📦 Export Diagnostic Bundle`**: Exporta información del sistema, estadísticas USB/latencia, contadores y registros recientes para depuración.

## Página de estadísticas (`Stats`)

Muestra el ritmo de pulsación en vivo e histórico en **KPM** (teclas por minuto, distinto del BPM del mapa) para Key 1, Key 2 y canales combinados, incluyendo promedio del intento actual/último, pico de ráfaga (7 pulsaciones consecutivas), ritmo de canción y botones de periodo histórico (`7` días, `30` días, `90` días y `All time` (`0`)). <!-- src: /desktop/gui/src/stats.rs --> <!-- src: /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->
