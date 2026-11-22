---
title: Referencia de Ajustes del Dispositivo
description: Catálogo completo de ajustes de configuración de OPad, tipos de datos, rangos válidos, valores por defecto y almacenamiento NVS.
---

Esta página documenta cada ajuste almacenado por el firmware de OPad, incluyendo asignaciones de teclado, tiempos de rebote (debounce), preferencias de pantalla, enlaces de gestos de deslizamiento y parámetros de Rapid Trigger para Hall Effect.
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto -->

Todos los ajustes se aplican inmediatamente a la memoria volátil (RAM). Para proteger la ruta crítica de pulsación de teclas, las escrituras en almacenamiento no volátil (NVS) están estrictamente prohibidas durante el juego activo (estados `PLAYING` y `COOLDOWN`). Los cambios realizados mientras un mapa está activo se mantienen en RAM y se vuelcan automáticamente a la memoria flash una vez que el dispositivo regresa a `IDLE`.
<!-- src: /firmware/main/config/device_config.c#write_to_nvs, /firmware/main/config/device_config.c#device_config_flush -->

---

## Core Key & Hardware Settings

Almacenados en el espacio de nombres NVS `opad_cfg` bajo la clave `"cfg"` dentro de la estructura `device_config_data_t`.
<!-- src: /firmware/main/config/device_config.c#NVS_KEY -->

| Ajuste | Campo Protobuf | Tipo C | Rango Válido | Valor por Defecto | Descripción |
|---|---|---|---|---|---|
| Uso HID Key 1 | `key1_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1D` ('Z') | Código de uso de teclado USB HID transmitido para la tecla física Key 1. |
| Uso HID Key 2 | `key2_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1B` ('X') | Código de uso de teclado USB HID transmitido para la tecla física Key 2. |
| GPIO Key 1 | `key1_gpio` | `uint32_t` | Lista de GPIOs permitidos | `14` (Cabecera P2, pin 11) | Pin de entrada GPIO del microcontrolador asignado al interruptor Key 1. |
| GPIO Key 2 | `key2_gpio` | `uint32_t` | Lista de GPIOs permitidos | `9` (Cabecera P2, pin 12) | Pin de entrada GPIO del microcontrolador asignado al interruptor Key 2. |
| Bloqueo de Rebote | `debounce_us` | `uint32_t` | `500` – `20000` µs | `5000` µs (5 ms) | Periodo de bloqueo de rebote anticipado (eager debounce) en microsegundos. Un flanco dispara un informe instantáneo; los rebotes posteriores dentro de esta ventana se ignoran. |
| Brillo de Pantalla | `brightness` | `uint32_t` | `0` – `100` % | `100` % | Porcentaje de brillo de la retroiluminación LCD. Controlado por PWM de hardware. |
| Tiempo de Suspensión | `display_sleep_seconds` | `uint32_t` | `0` o `10` – `86400` s | `600` s (10 min) | Tiempo de inactividad de pantalla en segundos. Establecer `0` desactiva la suspensión por completo. |
| Límite de Refresco en Juego | `gameplay_display_hz` | `uint32_t` | `0` o `1` – `60` Hz | `10` Hz | Frecuencia máxima de refresco de pantalla mientras se juega un beatmap, reduciendo la carga de CPU en Core 1. |
<!-- src: /firmware/main/config/device_config.h, /firmware/main/config/config_validate.c -->

### Allowed Switch GPIO Pins

Las teclas conectadas directamente a las cabeceras de la placa con un interruptor a masa deben usar uno de los pines GPIO soportados:
`2`, `4`, `6`, `7`, `9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `18`, `21`.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Key 1 y Key 2 no pueden compartir el mismo pin GPIO. Los pines utilizados por USB (`19`, `20`), consola UART0 (`43`, `44`), I2C de táctil/IMU (`47`, `48`), apagado de cámara (`17`) y el divisor de ID de módulo (`8`) están reservados y la validación de configuración los rechaza.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/main/config/config_validate.c#device_config_validate -->

---

## Extended Settings (Three-Key & Game Key Selection)

Almacenados en el espacio de nombres NVS `opad_cfg` bajo la clave `"cfg_ext"` dentro de la estructura `device_config_ext_t`. Este registro NVS separado garantiza la retrocompatibilidad si el firmware se degrada a versiones anteriores.
<!-- src: /firmware/main/config/device_config.c#NVS_EXT_KEY, /firmware/main/config/device_config.h#device_config_ext_t -->

| Ajuste | Campo Protobuf | Tipo C | Rango Válido | Valor por Defecto | Descripción |
|---|---|---|---|---|---|
| Uso HID Key 3 | `key3_hid_usage` | `uint8_t` | `0` o `0x04` – `0xE7` | `0x06` ('C') | Código de uso de teclado USB HID para Key 3 en módulos de tres teclas (Hall Effect V3). Establecer `0` desactiva la transmisión de la tecla. |
| Teclas de Juego | `game_keys` | `uint8_t` | Máscara de bits (`1` – `6`) | `0x03` (Keys 1 y 2) | Máscara de bits que selecciona qué teclas cuentan para los totales de sesión, la tasa KPM y las estadísticas del mapa (bit 0 = Key 1, bit 1 = Key 2, bit 2 = Key 3). Exactamente una o dos teclas deben estar activas; las tres activas a la vez es rechazado. |
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_validate_ext -->

Cada tecla física mantiene su propio contador de por vida independientemente de si está seleccionada en `game_keys`.
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_GAME_KEYS -->

---

## Touchscreen Swipe Actions

La pantalla táctil actúa como el tercer botón. Cuando no se está jugando un mapa, se admiten cuatro gestos de deslizamiento direccionales configurables.
<!-- src: /protocol/opad.proto#SwipeAction, /firmware/main/config/device_config.h -->

| Gesto | Acción por Defecto | Valores de Acción | Bits Modificadores |
|---|---|---|---|
| Deslizar Izquierda | `SWIPE_ACTION_PREV_TRACK` (2) | `1` = None, `2` = Prev Track, `3` = Next Track, `4` = Play/Pause, `5` = Mute, `6` = Key, `7` = Volume Up, `8` = Volume Down | `0x01` = Ctrl, `0x02` = Shift, `0x04` = Alt, `0x08` = GUI |
| Deslizar Derecha | `SWIPE_ACTION_NEXT_TRACK` (3) | Igual que arriba | Igual que arriba |
| Deslizar Arriba | `SWIPE_ACTION_VOLUME_UP` (7) | Igual que arriba | Igual que arriba |
| Deslizar Abajo | `SWIPE_ACTION_VOLUME_DOWN` (8) | Igual que arriba | Igual que arriba |
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

Cuando una acción se establece en `SWIPE_ACTION_KEY` (6), el `swipe_*_key` correspondiente define el código de uso HID transmitido, acompañado de `swipe_*_modifiers`. Durante el juego (estado `PLAYING`), los informes de teclas por deslizamiento se suprimen para evitar interferencias; solo se pueden informar Key 1, Key 2 y Quick Retry.
<!-- src: /protocol/opad.proto#SwipeAction, /firmware/main/config/device_config.c#device_config_apply -->

---

## Hall Effect & Rapid Trigger Settings

Almacenados en el espacio de nombres NVS `opad_cfg` bajo `"he_cfg"` (Keys 1 y 2) y `"he_cfg3"` (Key 3) como registros empaquetados fijos (`he_config_record_t`) con validación de integridad CRC-32.
<!-- src: /firmware/main/config/device_config.c#NVS_HE_KEY, /firmware/main/config/he_config_record.h -->

Las distancias de recorrido se especifican en unidades normalizadas de 0.1% del recorrido calibrado de la tecla (`0` – `1000`, donde `1000` = 100.0% de recorrido completo).
<!-- src: /firmware/main/input/he_params.h#HE_PCT_FULL -->

| Ajuste | Campo Protobuf | Unidad | Rango Válido | Valor por Defecto | Descripción |
|---|---|---|---|---|---|
| Profundidad de Actuación | `actuation` | 0.1% | `0` – `1000` | `500` (50.0%) | Distancia desde el reposo necesaria para activar la pulsación inicial. |
| Histéresis de Liberación | `release_hyst` | 0.1% | `0` – `1000` | `100` (10.0%) | Distancia que debe subir el vástago tras la actuación para liberarse en modo simple. |
| Distancia de Top Reset | `top_reset` | 0.1% | `0` – `1000` | `100` (10.0%) | Distancia desde el reposo donde el Rapid Trigger continuo se desactiva y se reinicia a la actuación inicial. |
| Pulsación Rapid Trigger | `rt_press` | 0.1% | `0` – `1000` | `150` (15.0%) | Recorrido descendente necesario para registrar una re-pulsación inmediata mientras está presionado. |
| Liberación Rapid Trigger | `rt_release` | 0.1% | `0` – `1000` | `150` (15.0%) | Recorrido ascendente necesario para registrar una liberación inmediata mientras está presionado. |
| Zona Muerta Inferior | `bottom_dead_zone` | 0.1% | `0` – `1000` | `0` (Desactivado) | Distancia de limitación al final del recorrido donde las lecturas del sensor dejan de rastrear el movimiento. |
| Modo Rapid Trigger | `rt_mode` | Enum | `1` – `3` | `1` (`OFF`) | `1` = Modo simple (actuación/histéresis fijas), `2` = Rapid Trigger normal (con top reset), `3` = Rapid Trigger continuo. |
| Permitir Bajo Noise Floor | `allow_below_noise_floor` | Booleano | `true` / `false` | `false` | Cuando está habilitado, omite los límites de seguridad de conteo ADC en bruto. Establecer esto en true puede causar pulsaciones fantasma por ruido ambiental. |
<!-- src: /firmware/main/config/he_config_record.h, /firmware/main/input/he_params.h, /protocol/opad.proto#HeKeySettings -->

### Noise Floor Clamping

Por defecto, el firmware aplica límites de ruido basados en conteos ADC en bruto: los umbrales de movimiento deben superar el ruido de reposo medido del sensor (`noise_floor_rest`) y el ruido de movimiento (`noise_floor_movement`). Si los ajustes configurados caen por debajo de estos umbrales físicos, el generador de parámetros los limita automáticamente a márgenes seguros para evitar pulsaciones fantasma.
<!-- src: /firmware/main/input/he_params.h, /firmware/main/input/he_params.c -->
