---
title: Personalizar el diseño de pantalla
description: Cómo diseñar, previsualizar, aplicar, exportar e importar diseños personalizados de pantalla Idle y Playing en opad-gui, con todas las fuentes de datos disponibles.
---

La pantalla LCD de `320x240` de OPad tiene dos diseños independientes: la **Idle screen** (mostrada fuera de un mapa) y la **Playing screen** (mostrada durante un mapa). Puedes editar ambas desde la página **Designer** de `opad-gui` usando una previsualización 2x exacta píxel a píxel renderizada por el propio motor LVGL del firmware (`opad-ui-preview`). <!-- src: /desktop/gui/src/designer/mod.rs, /firmware/main/ui/core/ui_core.h, /firmware/main/ui/ui_port.c#pad_timer_cb -->

## Qué necesitas

- `opad-gui` y `opad-daemon` en ejecución. Puedes diseñar, exportar e importar diseños incluso con el teclado desconectado; al pulsar **Apply to pad** el daemon guarda el diseño y lo envía automáticamente la próxima vez que se conecte el teclado. <!-- src: /desktop/gui/src/designer/mod.rs#update, /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- Tu OPad conectado por USB para ver los cambios en vivo en la pantalla. Si el teclado pertenece a otra instalación, asume primero la propiedad (consulta [Recuperación, reflasheo y desvinculación](../recovery.md)). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- `tosu` en ejecución si quieres previsualizar **Live data from osu!** (de lo contrario la previsualización usa valores de ejemplo integrados). <!-- src: /desktop/gui/src/designer/mod.rs#Message::LiveData -->

## Crear un diseño

1. Abre `opad-gui` y selecciona **Designer** en la barra de navegación izquierda (o lanza `opad-gui --page designer`). <!-- src: /desktop/gui/src/main.rs#Page::ALL, /desktop/gui/src/main.rs#TOKENS -->
2. Elige la pantalla que vas a editar encima de la previsualización: **Idle screen** o **Playing screen**. Cada pantalla se guarda por separado. <!-- src: /desktop/gui/src/designer/mod.rs#view, /desktop/crates/opad-layout/src/lib.rs#Screen -->
3. En **Add widget**, haz clic en un tipo de widget para añadirlo a la pantalla (hasta `32` widgets por pantalla): <!-- src: /desktop/gui/src/designer/mod.rs#palette, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS -->
   - **Text**: Muestra `Label` + valor enlazado + `Suffix`. Con `Data` en `None (static text)` muestra texto estático. Las fuentes de ritmo de pulsación pueden cambiar entre `KPM (keys per minute)` y `KPS (keys per second)`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /desktop/crates/opad-layout/src/lib.rs#WidgetKind -->
   - **Progress bar**: Barra horizontal donde `Background` es la pista y `Accent` es el relleno. Enlázala a una fuente `0..1` como `map.progress` o `play.health`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_progress -->
   - **Key card**: Tarjeta de contador para K1 o K2 (`pad.k1_map`, `pad.k1_lifetime`, `pad.k1_label` o los equivalentes de K2). `Label` es el título de la tarjeta; `Background` y `Accent` se intercambian mientras la tecla está pulsada. Activa **Show tap rate (key's live KPM or KPS)** para mostrar el ritmo en vivo de esa tecla en la esquina. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_keycard -->
   - **Status dot**: Círculo (`Accent` cuando está activo, `Background` cuando está inactivo) seguido de `Label` en color `Text / fg`. Enlázalo a una fuente `0/1` como `status.pc`, `status.tosu` o `status.osu`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_status_dot -->
   - **Rectangle**: Rectángulo redondeado relleno (`Background` con `Radius`), con un **Border** opcional de 1 px en el color `Text / fg`. <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_rect -->
   - **Grade**: Muestra la letra de calificación (`play.grade`) con los colores de rango de osu! (`SS`/`X`, `S`, `A`, `B`, `C`, `D`). <!-- src: /desktop/gui/src/designer/mod.rs#kind_help, /firmware/main/ui/core/ui_screen.c#build_grade -->
4. Coloca y ordena los widgets: <!-- src: /desktop/gui/src/designer/mod.rs#pointer_down, /desktop/gui/src/designer/overlay.rs -->
   - Arrastra cualquier widget en la previsualización para moverlo, o arrastra su tirador rosa inferior derecho para redimensionarlo. <!-- src: /desktop/gui/src/designer/mod.rs#pointer_move, /desktop/gui/src/designer/overlay.rs -->
   - Usa las teclas de flecha para desplazar el widget seleccionado `1 px` (`Shift` + flechas para `10 px`) y `Delete` para eliminarlo. <!-- src: /desktop/gui/src/designer/mod.rs#subscription -->
   - En la lista **Widgets (`n`/32)** (`top of list = drawn on top`), usa **Raise**, **Lower**, **Duplicate** (desplaza la copia `+8, +8 px`) y **Delete** para gestionar el orden de dibujo. <!-- src: /desktop/gui/src/designer/mod.rs#widget_list -->
5. Edita el widget seleccionado en **Properties**: <!-- src: /desktop/gui/src/designer/mod.rs#properties -->
   - **Type**, **Data**, **Font** (`12 px`, `14 px`, `16 px`, `20 px`, `24 px`, `32 px`, `48 px`) y **Align** (`Left`, `Center`, `Right`). <!-- src: /desktop/crates/opad-layout/src/lib.rs#Font, /desktop/crates/opad-layout/src/lib.rs#Align -->
   - **X** (`-320` a `640`), **Y** (`-240` a `480`), **Width** (`1` a `640`), **Height** (`1` a `480`), **Radius** (`0` a `120`) y **Decimals** (`Source default` o `0`–`4`). <!-- src: /desktop/gui/src/designer/mod.rs#properties -->
   - **Label** (hasta `31` bytes) y **Suffix** (hasta `11` bytes). <!-- src: /desktop/crates/opad-layout/src/lib.rs#LABEL_MAX_BYTES, /desktop/crates/opad-layout/src/lib.rs#SUFFIX_MAX_BYTES -->
   - **Text / fg**, **Background**, **Accent** y **Screen bg** (haz clic en la muestra de color o escribe un valor hexadecimal `#RRGGBB`). <!-- src: /desktop/gui/src/designer/mod.rs#color_row -->
   - Opciones adicionales: **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)** (solo en Key card) y **Unit** (`KPM (keys per minute)` o `KPS (keys per second)`, que también actualiza las palabras completas `KPM`/`PPM`/`KPS` en `Label` y `Suffix`). <!-- src: /desktop/gui/src/designer/mod.rs#properties, /desktop/crates/opad-layout/src/lib.rs#set_tap_rate_unit -->
6. Usa las casillas de previsualización debajo de la barra de acciones para probar el diseño antes de enviarlo: <!-- src: /desktop/gui/src/designer/mod.rs#view -->
   - **Live data from osu!**: Sondea `opad-daemon` cada segundo para mostrar valores reales de `tosu` y ritmo de pulsación. <!-- src: /desktop/gui/src/designer/mod.rs#subscription -->
   - **Show keys pressed**: Simula ambas teclas pulsadas para comprobar los colores de resaltado de las Key card. <!-- src: /desktop/gui/src/designer/mod.rs#Message::SimulatePress -->
7. Haz clic en **Apply to pad** (mostrado como **Apply to pad •** mientras haya cambios sin aplicar). <!-- src: /desktop/gui/src/designer/mod.rs#view -->
   - Fuera de un mapa (`Idle`), el daemon guarda el diseño en SQLite y lo envía al teclado, que lo almacena en NVS (`opad_ui`). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout, /firmware/main/ui/ui_store.c -->
   - Durante un mapa o el enfriamiento (`Playing` / `Cooldown`), el diseño se aplica a la RAM del teclado de inmediato y se persiste en disco y NVS cuando el teclado vuelve a `Idle`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout, /firmware/main/ui/ui_store.c#ui_store_save -->

Para deshacer cambios sin aplicar, haz clic en **Revert**. Para restaurar el diseño integrado del firmware en la pantalla activa, haz clic en **Reset to default** y después en **Apply to pad**. Para descartar cambios locales y recargar los diseños guardados desde el daemon, haz clic en **Reload**. <!-- src: /desktop/gui/src/designer/mod.rs#update -->

## Exportar e importar diseños

- **Export**: Selecciona **Idle screen** o **Playing screen** y haz clic en **Export**. `opad-gui` abre un diálogo para guardar con nombre predeterminado `opad-idle.json` u `opad-playing.json` y escribe el diseño en JSON formateado. <!-- src: /desktop/gui/src/designer/mod.rs#export_layout -->
- **Import**: Selecciona la pantalla de destino (**Idle screen** o **Playing screen**), haz clic en **Import** y elige un archivo `.json`. `opad-gui` lo analiza y valida con las mismas reglas que el firmware (`ui_layout_validate`), lo carga en el editor y te indica que pulses **Apply to pad** para guardarlo y enviarlo. <!-- src: /desktop/gui/src/designer/mod.rs#import_layout, /desktop/crates/opad-layout/src/lib.rs#validate -->

## Fuentes de datos seleccionables (`ui_source_t`)

Cada widget puede enlazarse a un ID de fuente de datos definido en `ui_source_t` (`firmware/main/ui/core/ui_ids.h`). El desplegable **Data** lista todas las fuentes siguientes excepto los IDs `72` y `73` (`pad.k1_down`, `pad.k2_down`), que son indicadores internos de tecla pulsada enlazados automáticamente por los widgets **Key card**. El ID `42` está reservado. <!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources -->

### Texto estático

| ID | Identificador | Etiqueta en Designer | Formato | Descripción |
|---|---|---|---|---|
| `0` | `none` | `None (static text)` | Texto | `Label` + `Suffix` estáticos sin datos en vivo. |
<!-- src: /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#STATIC_SOURCE -->

### Mapa (desde `tosu`)

| ID | Identificador | Etiqueta en Designer | Formato (decimales por defecto) | Descripción |
|---|---|---|---|---|
| `1` | `map.title` | `Map: Title` | Texto | Título del mapa. |
| `2` | `map.artist` | `Map: Artist` | Texto | Artista del mapa. |
| `3` | `map.mapper` | `Map: Mapper` | Texto | Creador del mapa. |
| `4` | `map.difficulty` | `Map: Difficulty` | Texto | Nombre de la dificultad. |
| `5` | `map.status` | `Map: Status` | Texto | Estado del mapa (p. ej., `ranked`, `loved`). |
| `6` | `map.stars` | `Map: Stars` | Fijo (`2`) | Calificación total de estrellas (con mods activos). |
| `7` | `map.stars_live` | `Map: Stars live` | Fijo (`2`) | Estrellas en vivo en la posición actual del mapa. |
| `8` | `map.ar` | `Map: AR` | Fijo (`1`) | Approach Rate. |
| `9` | `map.cs` | `Map: CS` | Fijo (`1`) | Circle Size. |
| `10` | `map.od` | `Map: OD` | Fijo (`1`) | Overall Difficulty. |
| `11` | `map.hp` | `Map: HP` | Fijo (`1`) | HP Drain Rate. |
| `12` | `map.bpm` | `Map: BPM` | Entero | BPM del mapa (no tu velocidad de pulsación). |
| `13` | `map.objects` | `Map: Objects` | Entero agrupado | Número total de objetos del mapa. |
| `14` | `map.max_combo` | `Map: Max combo` | Entero agrupado | Combo máximo posible del mapa. |
| `15` | `map.length` | `Map: Length` | Duración (`m:ss`) | Duración total del mapa (ms). |
| `16` | `map.time_elapsed` | `Map: Time elapsed` | Duración (`m:ss`) | Tiempo transcurrido del mapa (ms). |
| `17` | `map.time_remaining` | `Map: Time remaining` | Duración (`m:ss`) | Tiempo restante del mapa (ms). |
| `18` | `map.progress` | `Map: Progress` | Porcentaje (`0`, bruto `0..1`) | Progreso del mapa (usable en Progress bar). |
| `19` | `map.kiai` | `Map: Kiai` | Entero (`0`/`1`) | `1` durante kiai, `0` fuera de él. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Partida en vivo (desde `tosu` y ritmo de pulsación del daemon)

| ID | Identificador | Etiqueta en Designer | Formato (decimales por defecto) | Descripción |
|---|---|---|---|---|
| `20` | `play.pp` | `Live play: PP` | Fijo (`0`) | PP actual en vivo. |
| `21` | `play.pp_fc` | `Live play: PP fc` | Fijo (`0`) | PP estimado si se consigue full combo. |
| `22` | `play.pp_max` | `Live play: PP max` | Fijo (`0`) | PP máximo alcanzable en esta partida. |
| `23` | `play.accuracy` | `Live play: Accuracy` | Fijo (`2`) | Porcentaje de precisión actual. |
| `24` | `play.score` | `Live play: Score` | Entero agrupado | Puntuación actual. |
| `25` | `play.combo` | `Live play: Combo` | Entero agrupado | Combo actual. |
| `26` | `play.max_combo` | `Live play: Max combo` | Entero agrupado | Combo máximo alcanzado en este intento. |
| `27` | `play.grade` | `Live play: Grade` | Texto | Letra de rango actual (`SS`, `S`, `A`, `B`, `C`, `D`). |
| `28` | `play.hits_300` | `Live play: Hits 300` | Entero agrupado | Conteo de aciertos 300. |
| `29` | `play.hits_100` | `Live play: Hits 100` | Entero agrupado | Conteo de aciertos 100. |
| `30` | `play.hits_50` | `Live play: Hits 50` | Entero agrupado | Conteo de aciertos 50. |
| `31` | `play.hits_miss` | `Live play: Hits miss` | Entero agrupado | Conteo de fallos (misses). |
| `32` | `play.slider_breaks` | `Live play: Slider breaks` | Entero agrupado | Conteo de slider breaks. |
| `33` | `play.ur` | `Live play: UR` | Fijo (`1`) | Unstable Rate. |
| `34` | `play.health` | `Live play: Health` | Porcentaje (`0`, bruto `0..1`) | Barra de vida actual (usable en Progress bar). |
| `35` | `play.mods` | `Live play: Mods` | Texto | Cadena de mods activos (p. ej., `HDDT`). |
| `36` | `play.player` | `Live play: Player` | Texto | Nombre del jugador en la partida activa. |
| `37` | `play.failed` | `Live play: Failed` | Entero (`0`/`1`) | `1` si la partida ha fallado, `0` en caso contrario. |
| `51` | `play.ppm` | `Live play: Live KPM` | Entero (o `1` dec. en KPS) | Velocidad actual de pulsación combinada (`0` sin pulsar). |
| `52` | `play.ppm_avg` | `Live play: Tapping avg KPM` | Entero (o `1` dec. en KPS) | Media del intento mientras se pulsa (sin descansos). |
| `53` | `play.ppm_peak` | `Live play: PEAK KPM` | Entero (o `1` dec. en KPS) | Las 7 pulsaciones consecutivas más rápidas en este intento. |
| `54` | `play.k1_ppm` | `Live play: K1 live KPM` | Entero (o `1` dec. en KPS) | Velocidad actual de pulsación de Key 1. |
| `55` | `play.k1_ppm_avg` | `Live play: K1 tapping avg KPM` | Entero (o `1` dec. en KPS) | Media de Key 1 mientras se pulsa en este intento. |
| `56` | `play.k1_ppm_peak` | `Live play: K1 peak KPM` | Entero (o `1` dec. en KPS) | Pico de ráfaga de Key 1 en este intento. |
| `57` | `play.k2_ppm` | `Live play: K2 live KPM` | Entero (o `1` dec. en KPS) | Velocidad actual de pulsación de Key 2. |
| `58` | `play.k2_ppm_avg` | `Live play: K2 tapping avg KPM` | Entero (o `1` dec. en KPS) | Media de Key 2 mientras se pulsa en este intento. |
| `59` | `play.k2_ppm_peak` | `Live play: K2 peak KPM` | Entero (o `1` dec. en KPS) | Pico de ráfaga de Key 2 en este intento. |
| `84` | `play.ppm_song` | `Live play: AVG KPM` | Entero (o `1` dec. en KPS) | Ritmo global de la canción (de la primera a la última nota, descansos incluidos). |
| `86` | `play.ppm_song_peak` | `Live play: Best 10 s KPM` | Entero (o `1` dec. en KPS) | Ventana de 10 segundos más rápida de la canción en este intento. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Perfil y sesión (desde `tosu`)

| ID | Identificador | Etiqueta en Designer | Formato (decimales por defecto) | Descripción |
|---|---|---|---|---|
| `40` | `profile.name` | `Profile & session: Name` | Texto | Nombre de usuario de osu! conectado. |
| `41` | `profile.rank` | `Profile & session: Rank` | Entero agrupado | Rango global. |
| `43` | `profile.pp` | `Profile & session: PP` | Fijo (`0`) | PP total del perfil. |
| `44` | `profile.accuracy` | `Profile & session: Accuracy` | Fijo (`2`) | Precisión del perfil. |
| `45` | `profile.playcount` | `Profile & session: Playcount` | Entero agrupado | Partidas totales del perfil. |
| `46` | `profile.level` | `Profile & session: Level` | Fijo (`0`) | Nivel del perfil. |
| `47` | `profile.country` | `Profile & session: Country` | Texto | Código de país. |
| `48` | `session.playtime` | `Profile & session: Playtime` | Duración (`m:ss` / `h:mm:ss`) | Tiempo jugado en la sesión actual (ms). |
| `49` | `session.playcount` | `Profile & session: Playcount` | Entero agrupado | Mapas jugados en esta sesión. |
| `50` | `game.state` | `Profile & session: State` | Texto | Estado actual de osu! (p. ej., `Playing`, `Song select`). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

### Teclado (locales al dispositivo, funcionan sin anfitrión)

| ID | Identificador | Etiqueta en Designer | Formato | Descripción |
|---|---|---|---|---|
| `60` | `pad.k1_map` | `Pad: K1 map` | Entero agrupado | Pulsaciones de la primera tecla de juego en el mapa actual. |
| `61` | `pad.k2_map` | `Pad: K2 map` | Entero agrupado | Pulsaciones de la segunda tecla de juego en el mapa actual. |
| `62` | `pad.total_map` | `Pad: Total map` | Entero agrupado | Pulsaciones combinadas de teclas de juego en el mapa actual. |
| `63` | `pad.k1_lifetime` | `Pad: K1 lifetime` | Entero agrupado | Contador de por vida de la primera tecla de juego. |
| `64` | `pad.k2_lifetime` | `Pad: K2 lifetime` | Entero agrupado | Contador de por vida de la segunda tecla de juego. |
| `65` | `pad.total_lifetime` | `Pad: Total lifetime` | Entero agrupado | Total de por vida de las teclas de juego activas. |
| `66` | `pad.kps` | `Pad: KPS` | Entero | Pulsaciones por segundo en ventana móvil de 1 s locales al teclado. |
| `67` | `pad.k1_label` | `Pad: K1 label` | Texto | Carácter asignado a la primera tecla de juego (p. ej., `Z`). |
| `68` | `pad.k2_label` | `Pad: K2 label` | Texto | Carácter asignado a la segunda tecla de juego (p. ej., `X`). |
| `69` | `pad.clock` | `Pad: Clock` | Texto | Reloj local (`HH:MM`, sincronizado desde el anfitrión). |
| `70` | `pad.date` | `Pad: Date` | Texto | Fecha local (p. ej., `Sat 12 Sep`). |
| `71` | `pad.uptime` | `Pad: Uptime` | Duración (`m:ss` / `h:mm:ss`) | Tiempo desde el arranque del teclado (ms). |
| `72` | `pad.k1_down` | *(interno — solo Key card)* | Entero (`0`/`1`) | Estado de resaltado de Key 1 pulsada (oculto del desplegable). |
| `73` | `pad.k2_down` | *(interno — solo Key card)* | Entero (`0`/`1`) | Estado de resaltado de Key 2 pulsada (oculto del desplegable). |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#all_sources -->

### Historial (desde SQLite del daemon, independiente de `tosu`)

| ID | Identificador | Etiqueta en Designer | Formato (decimales por defecto) | Descripción |
|---|---|---|---|---|
| `74` | `history.ppm_avg` | `History: Tapping avg KPM` | Entero (o `1` dec. en KPS) | Media histórica de pulsación en el periodo seleccionado. |
| `75` | `history.ppm_peak` | `History: Best PEAK KPM` | Entero (o `1` dec. en KPS) | Mejor pico de ráfaga de 7 pulsaciones en el periodo seleccionado. |
| `76` | `history.k1_ppm_avg` | `History: K1 tapping avg KPM` | Entero (o `1` dec. en KPS) | Media histórica de pulsación de Key 1. |
| `77` | `history.k1_ppm_peak` | `History: K1 best peak KPM` | Entero (o `1` dec. en KPS) | Mejor pico histórico de Key 1. |
| `78` | `history.k2_ppm_avg` | `History: K2 tapping avg KPM` | Entero (o `1` dec. en KPS) | Media histórica de pulsación de Key 2. |
| `79` | `history.k2_ppm_peak` | `History: K2 best peak KPM` | Entero (o `1` dec. en KPS) | Mejor pico histórico de Key 2. |
| `83` | `history.period` | `History: Period` | Texto | Etiqueta de la ventana histórica activa (p. ej., `LAST 30 DAYS`, `ALL TIME`). |
| `85` | `history.ppm_song` | `History: AVG KPM` | Entero (o `1` dec. en KPS) | Media histórica del ritmo global de canción. |
| `87` | `history.ppm_song_peak` | `History: Best 10 s KPM` | Entero (o `1` dec. en KPS) | Mejor ventana de 10 s de canción en el periodo seleccionado. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#tap_rate_label -->

### Conexión (indicadores de estado, `0`/`1`)

| ID | Identificador | Etiqueta en Designer | Formato | Descripción |
|---|---|---|---|---|
| `80` | `status.pc` | `Connection: PC` | Entero (`0`/`1`) | `1` cuando `opad-daemon` está conectado por USB CDC. |
| `81` | `status.tosu` | `Connection: Tosu` | Entero (`0`/`1`) | `1` cuando `opad-daemon` está conectado a `tosu`. |
| `82` | `status.osu` | `Connection: Osu` | Entero (`0`/`1`) | `1` cuando `tosu` detecta un proceso `osu!` en ejecución. |
<!-- src: /firmware/main/ui/core/ui_ids.h, /firmware/main/ui/core/ui_data.c, /desktop/crates/opad-layout/src/lib.rs#source_info -->

## Cómo comprobar que funcionó

- Al pulsar **Apply to pad**, la línea de estado bajo la previsualización cambia a `Applied to the pad` (o `Layout applied to pad RAM; will be saved after gameplay` durante un mapa) y el indicador `•` desaparece del botón. <!-- src: /desktop/gui/src/designer/mod.rs#Message::Applied -->
- La LCD del teclado muestra de inmediato el nuevo diseño de **Idle screen** (y cambia a tu diseño de **Playing screen** al empezar un mapa en osu!). <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
- Desconecta y vuelve a conectar el teclado: los diseños personalizados se recargan automáticamente desde NVS (`opad_ui`). <!-- src: /firmware/main/ui/ui_port.c#ui_init, /firmware/main/ui/ui_store.c -->

## Si no funcionó

- **`This pad's firmware shows tap rates in KPM only; update the pad's firmware to show them in KPS`**: Algún widget usa **Unit** = `KPS (keys per second)` (`FLAG_PER_SECOND`), que el firmware anterior ignora. Cambia el widget a `KPM (keys per minute)` o sigue [Actualización del firmware](update-firmware.md). <!-- src: /desktop/daemon/src/ipc_handlers.rs#IpcRequest::SetLayout -->
- **`Daemon unavailable (...)` o `No live data yet (is tosu running?)`**: Comprueba que `opad-daemon` y `tosu` estén en ejecución; consulta [Solución de problemas](../troubleshooting.md) y [Uso con tosu](use-with-tosu.md). <!-- src: /desktop/gui/src/designer/mod.rs#update -->
- **Diseño rechazado al importar o aplicar**: Comprueba que el diseño tenga como máximo `32` widgets, dimensiones válidas (`Width` 1–640, `Height` 1–480), `Label` ≤ 31 bytes, `Suffix` ≤ 11 bytes e IDs válidos de `ui_source_t`. <!-- src: /desktop/crates/opad-layout/src/lib.rs#validate -->
- **El teclado está bloqueado en la pantalla roja de calibración**: Un teclado Hall Effect sin calibrar oculta las pantallas normales hasta completar la calibración; consulta [Calibración de interruptores Hall Effect](calibrate-hall-effect.md). <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
