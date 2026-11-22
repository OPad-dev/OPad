---
title: Referencia de ajustes del dispositivo
description: Catálogo completo de ajustes de configuración de OPad, tipos de datos, rangos válidos, valores predeterminados y claves de almacenamiento NVS.
sidebar:
  order: 3
---

Esta página documenta todos los ajustes de configuración almacenados por el firmware de OPad: códigos de tecla, asignaciones de pines GPIO, bloqueo de antirrebote inmediato, preferencias de pantalla, acciones de deslizamiento y parámetros Hall Effect.
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto -->

Los ajustes se aplican en RAM inmediatamente. Para evitar bloqueos de flash en la ruta de pulsación, las escrituras en NVS están bloqueadas durante `PLAYING` y `COOLDOWN`; los cambios realizados en mitad de un mapa se marcan como pendientes y se vuelcan a NVS cuando el teclado regresa a `IDLE`.
<!-- src: /firmware/main/config/device_config.c#write_to_nvs, /firmware/main/config/device_config.c#device_config_flush -->

---

## Ajustes principales de teclas y hardware (`opad_cfg` / `"cfg"`)

Almacenados en el espacio de nombres NVS `opad_cfg` bajo la clave `"cfg"` como `device_config_data_t` (versión de blob `7`).
<!-- src: /firmware/main/config/device_config.c#NVS_KEY, /firmware/main/config/device_config.h#DEVICE_CONFIG_VERSION -->

| Ajuste | Campo Protobuf | Tipo C | Rango válido | Valor predeterminado | Descripción |
|---|---|---|---|---|---|
| Uso HID de Key 1 | `key1_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1D` (`Z`) | Uso de teclado USB HID enviado para Key 1. |
| Uso HID de Key 2 | `key2_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1B` (`X`) | Uso de teclado USB HID enviado para Key 2. |
| GPIO de Key 1 | `key1_gpio` | `uint32_t` | `KEY_GPIO_ALLOWED` | `14` (P2 pin 11) | GPIO del interruptor para Key 1 (MX cableado a mano). |
| GPIO de Key 2 | `key2_gpio` | `uint32_t` | `KEY_GPIO_ALLOWED` | `9` (P2 pin 12) | GPIO del interruptor para Key 2 (MX cableado a mano). |
| Bloqueo de antirrebote | `debounce_us` | `uint32_t` | `500` – `20000` µs | `5000` µs (5 ms) | Duración del bloqueo de antirrebote inmediato tras un flanco aceptado. |
| Brillo de pantalla | `brightness` | `uint32_t` | `0` – `100` % | `100` % | Porcentaje PWM LEDC de retroiluminación LCD (el deslizador de la GUI ofrece `10` – `100` %). |
| Tiempo de reposo | `display_sleep_seconds` | `uint32_t` | `0` o `10` – `86400` s | `600` s (10 min) | Tiempo de inactividad antes de apagar la pantalla (`0` desactiva el reposo; el deslizador de la GUI ofrece `60` – `3600` s). |
| Tope de refresco en juego | `gameplay_display_hz` | `uint32_t` | `0` o `1` – `60` Hz (el anfitrión limita `1` – `30`) | `10` Hz | Frecuencia máxima de refresco de telemetría/pantalla durante la partida. |
<!-- src: /firmware/main/config/device_config.h, /firmware/main/config/config_validate.c#device_config_validate -->

### Pines GPIO permitidos para interruptores

Los interruptores cableados a mano deben usar uno de los 14 GPIOs de `KEY_GPIO_ALLOWED`:
`2`, `4`, `6`, `7`, `9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `18`, `21`.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Key 1 y Key 2 no pueden compartir GPIO. Se rechazan USB (`19`, `20`), UART0 (`43`, `44`), I2C táctil/IMU (`47`, `48`), `CAM_PWDN` (`17`) y ADC de identificación de módulo (`8`).
<!-- src: /firmware/main/config/config_validate.c#device_config_validate -->

---

## Ajustes extendidos (`opad_cfg` / `"cfg_ext"`)

Se almacenan aparte bajo `"cfg_ext"` (`device_config_ext_t`, versión `1`) para que bajar a un firmware anterior no rompa la carga del blob `"cfg"` v7.
<!-- src: /firmware/main/config/device_config.c#NVS_EXT_KEY, /firmware/main/config/device_config.h#device_config_ext_t -->

| Ajuste | Campo Protobuf | Tipo C | Rango válido | Valor predeterminado | Descripción |
|---|---|---|---|---|---|
| Uso HID de Key 3 | `key3_hid_usage` | `uint8_t` | `0` o `0x04` – `0xE7` | `0x06` (`C`) | Uso USB HID para Key 3 en módulos V3 de 3 teclas (`0` no envía nada). |
| Teclas de juego | `game_keys` | `uint8_t` | Máscara de bits `1` – `6` | `0x03` (Keys 1 y 2) | Máscara de bits de 1 o 2 teclas de juego (bit 0 = Key 1, bit 1 = Key 2, bit 2 = Key 3) contadas en totales, ritmo de pulsación y estadísticas de mapa. `0` y `0x07` (las tres) son rechazados. |
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_validate_ext -->

Cada tecla física sigue manteniendo su propio contador de por vida con independencia de `game_keys`.
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_GAME_KEYS -->

---

## Acciones de deslizamiento táctil

Almacenadas dentro del blob `"cfg"` v7 (`swipe_*_action`, `swipe_*_key`, `swipe_mods[4]`).
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

| Gesto | Acción predeterminada | Valores en el cable (`SwipeAction`) | Bits modificadores (`swipe_*_modifiers`) |
|---|---|---|---|
| Deslizar izquierda | `SWIPE_ACTION_PREV_TRACK` (`2`) | `1` = None, `2` = Prev Track, `3` = Next Track, `4` = Play/Pause, `5` = Mute, `6` = Key, `7` = Volume Up, `8` = Volume Down | Byte modificador HID estándar (`0x01` = Left Ctrl, `0x02` = Left Shift, `0x04` = Left Alt, `0x08` = Left GUI) |
| Deslizar derecha | `SWIPE_ACTION_NEXT_TRACK` (`3`) | Igual que arriba | Igual que arriba |
| Deslizar arriba | `SWIPE_ACTION_VOLUME_UP` (`7`) | Igual que arriba | Igual que arriba |
| Deslizar abajo | `SWIPE_ACTION_VOLUME_DOWN` (`8`) | Igual que arriba | Igual que arriba |
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

Cuando una acción es `SWIPE_ACTION_KEY` (`6`), el campo `swipe_*_key` correspondiente debe estar en `0x04`–`0xE7`. Durante la partida (`PLAYING`), nunca se envían acciones de deslizamiento de teclado para no inyectar modificadores ni teclas en un mapa.
<!-- src: /firmware/main/config/config_validate.c#swipe_valid, /firmware/main/input/touch_retry.c -->

---

## Ajustes Hall Effect y Rapid Trigger (`opad_cfg` / `"he_cfg"`, `"he_cfg3"`)

Almacenados en `opad_cfg` bajo `"he_cfg"` (Keys 1 y 2) y `"he_cfg3"` (Key 3) como estructuras empaquetadas `he_config_record_t` con magia `0x48454346` (`"HECF"`), versión `1`, `flags` (bits `no_floor` por tecla) y CRC-32 IEEE 802.3.
<!-- src: /firmware/main/config/device_config.c#NVS_HE_KEY, /firmware/main/config/he_config_record.h -->

Todas las profundidades y distancias se expresan en incrementos de `0.1 %` del rango magnético calibrado de la tecla (`0`–`1000`, donde `HE_PCT_FULL = 1000` es 100.0%).
<!-- src: /firmware/main/input/he_params.h#HE_PCT_FULL -->

| Ajuste | Campo Protobuf | Unidad | Rango válido | Valor predeterminado | Descripción |
|---|---|---|---|---|---|
| Profundidad de actuación | `actuation` | `0.1 %` | `0` – `1000` | `500` (`50.0 %`) | Profundidad desde la posición de reposo necesaria para la pulsación inicial. |
| Histéresis de liberación | `release_hyst` | `0.1 %` | `0` – `1000` | `100` (`10.0 %`) | Distancia ascendente por encima de la actuación necesaria para soltar con Rapid Trigger desactivado. |
| Distancia de reinicio superior | `top_reset` | `0.1 %` | `0` – `1000` | `100` (`10.0 %`) | Profundidad por encima de la cual termina Rapid Trigger continuo y vuelve a `HE_RT_ARMED`. |
| Pulsación Rapid Trigger | `rt_press` | `0.1 %` | `0` – `1000` | `150` (`15.0 %`) | Movimiento descendente necesario para volver a pulsar en modo Rapid Trigger. |
| Liberación Rapid Trigger | `rt_release` | `0.1 %` | `0` – `1000` | `150` (`15.0 %`) | Movimiento ascendente necesario para soltar en modo Rapid Trigger. |
| Zona muerta inferior | `bottom_dead_zone` | `0.1 %` | `0` – `1000` | `0` (Desactivada) | Distancia cerca del fondo de recorrido donde se detiene el seguimiento de pico (`0` la desactiva). |
| Modo Rapid Trigger | `rt_mode` | Enum | `1` – `3` | `1` (`OFF`) | `1` = Off (actuación/histéresis fijas), `2` = Normal Rapid Trigger (activo por debajo de la actuación), `3` = Continuous Rapid Trigger (activo hasta `top_reset`). |
| Permitir bajo piso de ruido | `allow_below_noise_floor` | `bool` | `false` / `true` | `false` | Baja los pisos de ruido en conteos crudos a `1` conteo para esta tecla para aplicar los ajustes tal cual (el ruido del sensor puede disparar pulsaciones). |
<!-- src: /firmware/main/config/he_config_record.h, /firmware/main/input/he_params.h, /protocol/opad.proto#HeKeySettings -->

### Aplicación del piso de ruido

Salvo que `allow_below_noise_floor` (`no_floor`) esté activado, `he_params_build` limita los umbrales de movimiento y el margen de reinicio superior por encima de los pisos de ruido en conteos crudos (`min_movement`, `min_top_reset`) y devuelve bits de estado explícitos `HE_PARAMS_*_RAISED` para que la GUI pueda mostrar el punto de pulsación efectivo real.
<!-- src: /firmware/main/input/he_params.h#he_params_build -->
