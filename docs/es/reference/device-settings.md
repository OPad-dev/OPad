---
title: Referencia de ajustes del dispositivo
description: Todos los ajustes que guarda el firmware de OPad, con su campo en el cable, tipo, rango válido, valor predeterminado y registro NVS.
sidebar:
  order: 3
---

El teclado guarda sus ajustes en NVS y los comunica en cada `HelloAck` y `ConfigAck` como un `ConfigPayload` (consulta la [Referencia del protocolo serie](serial-protocol.md)). La aplicación de escritorio los modifica con `SetConfig`.
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#ConfigPayload -->

Un cambio se aplica en RAM al instante. La escritura en NVS solo ocurre en `IDLE`: durante `PLAYING` y `COOLDOWN` el registro se marca como pendiente y se escribe en cuanto el teclado vuelve a estar inactivo, de modo que ninguna escritura en flash retrasa nunca una pulsación.
<!-- src: /firmware/main/config/device_config.c#write_to_nvs, /firmware/main/config/device_config.c#write_he_to_nvs, /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## Registros NVS

Todos los ajustes viven en el espacio de nombres `opad_cfg`. El firmware hasta `1.0.0-rc` usaba `osupad_cfg`; al arrancar, el firmware copia una vez el espacio de nombres antiguo al nuevo y nunca modifica el antiguo, así que un firmware revertido sigue encontrando sus datos.
<!-- src: /firmware/main/config/nvs_ns.h, /firmware/main/config/nvs_ns.c -->

| Clave | Tipo C | Contiene |
|---|---|---|
| `cfg` | `device_config_data_t`, empaquetado, `version` = `7` | Teclas 1 y 2, antirrebote, pantalla, pines de tecla, ID de propietario, deslizamientos |
| `cfg_ext` | `device_config_ext_t`, empaquetado, `version` = `1` | Uso HID de la tecla 3 y las teclas de juego |
| `he_cfg` | `he_config_record_t` | Ajustes Hall Effect de las teclas 1 y 2 |
| `he_cfg3` | `he_config_record_t` | Ajustes Hall Effect de la tecla 3 (en `keys[0]`, copiados a `keys[1]`) |
<!-- src: /firmware/main/config/device_config.c#NVS_KEY, /firmware/main/config/device_config.c#NVS_EXT_KEY, /firmware/main/config/device_config.c#NVS_HE_KEY, /firmware/main/config/device_config.c#NVS_HE3_KEY -->

El blob `cfg` nunca crece más allá de v7: los ajustes añadidos después van a `cfg_ext` o a un registro propio, para que un firmware revertido a una versión anterior pueda seguir leyendo `cfg`. Los blobs `cfg` más antiguos (v1 a v6) se reconocen por su longitud y se cargan con valores predeterminados para los campos que faltan.
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_blob_layout_ok -->

> [!NOTE]
> En el cable, `0` significa «mantener el valor actual» en todos los campos de `ConfigPayload` salvo `brightness`, `display_sleep_seconds` y los modificadores de deslizamiento, que siempre se aplican tal como llegan. `key3_hid_usage` y `game_keys` son `optional`: si faltan, se mantiene el valor actual.
<!-- src: /firmware/main/protocol/protocol.c#handle_host_message, /protocol/opad.proto#ConfigPayload -->

## Teclas, antirrebote y pantalla (`cfg`)

| Ajuste | Campo en el cable | Rango | Predeterminado |
|---|---|---|---|
| Uso HID de la tecla 1 | `key1_hid_usage` | `0x04`–`0xE7` | `0x1D` (`Z`) |
| Uso HID de la tecla 2 | `key2_hid_usage` | `0x04`–`0xE7` | `0x1B` (`X`) |
| GPIO de la tecla 1 | `key1_gpio` | Un pin de [Pines de tecla permitidos](#pines-de-tecla-permitidos) | `14` (cabezal P2 pin 11) |
| GPIO de la tecla 2 | `key2_gpio` | Un pin de [Pines de tecla permitidos](#pines-de-tecla-permitidos), distinto del de la tecla 1 | `9` (cabezal P2 pin 12) |
| Bloqueo de antirrebote | `debounce_us` | `500`–`20000` µs | `5000` µs |
| Brillo | `brightness` | `0`–`100` % | `100` % |
| Reposo de pantalla | `display_sleep_seconds` | `0` (nunca) o `10`–`86400` s | `600` s |
| Tope de refresco en juego | `gameplay_display_hz` | `0` o `1`–`60` Hz en el teclado; la aplicación acepta `1`–`30` | `10` Hz |
<!-- src: /firmware/main/config/device_config.h, /firmware/main/config/config_validate.c#device_config_validate, /firmware/main/input/debounce.h#DEBOUNCE_MIN_US -->

Todos los campos son `uint32_t` en el blob. El teclado solo guarda y comunica `gameplay_display_hz`; el daemon lo lee para limitar con qué frecuencia envía datos de pantalla durante un mapa.
<!-- src: /firmware/main/protocol/protocol.c#fill_config_payload, /desktop/daemon/src/runtime.rs, /desktop/crates/opad-model/src/lib.rs#validate_settings -->

Los deslizadores de la aplicación abarcan menos de lo que acepta el teclado: brillo `10`–`100` %, reposo de pantalla `60`–`3600` s, tope de refresco `1`–`30` Hz (consulta la [Referencia de la GUI](gui.md)).
<!-- src: /desktop/gui/src/pages.rs -->

Cuando el teclado arranca con un módulo de entrada MX y ambos pines de tecla siguen en los valores predeterminados del cableado a mano (`14`, `9`), los cambia a los pines de la placa portadora (`10`, `7`) y lo guarda. Los pines que alguien eligió no se tocan.
<!-- src: /firmware/main/app_main.c -->

El blob también contiene el ID de propietario de 16 bytes (`owner_id`, todo a cero = sin reclamar), que escribe `ClaimOwnership` y solo en `IDLE`.
<!-- src: /firmware/main/config/device_config.h#device_config_data_t, /firmware/main/config/device_config.h#device_config_claim_owner -->

### Pines de tecla permitidos

Una tecla cableada a mano puede usar uno de los 14 GPIO de `KEY_GPIO_ALLOWED`:

`2`, `4`, `6`, `7`, `9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `18`, `21`
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Excluidos a propósito: `19`/`20` (USB D-/D+), `43`/`44` (consola UART0), `47`/`48` (I2C de la pantalla táctil y la IMU), `17` (`CAM_PWDN`, pull-down de 10 kΩ) y `8` (ADC de identificación de módulo). Un firmware compilado con `CONFIG_OPAD_BENCH_DEBUG_GPIO` también rechaza ese pin de depuración. Las posiciones en los cabezales están en la [Referencia de pines](pinout.md).
<!-- src: /firmware/main/config/config_validate.c#device_config_key_gpio_supported -->

## Tecla 3 y teclas de juego (`cfg_ext`)

| Ajuste | Campo en el cable | Tipo C | Rango | Predeterminado |
|---|---|---|---|---|
| Uso HID de la tecla 3 | `key3_hid_usage` | `uint8_t` | `0` (no envía nada) o `0x04`–`0xE7` | `0x06` (`C`) |
| Teclas de juego | `game_keys` | `uint8_t` | `1`–`6`: una o dos de las teclas 1–3 | `0x03` (teclas 1 y 2) |
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_validate_ext -->

`game_keys` es una máscara de bits: bit 0 = tecla 1, bit 1 = tecla 2, bit 2 = tecla 3. Solo las pulsaciones de las teclas de juego cuentan para el total, el ritmo de pulsación y las estadísticas de mapa, y solo ellas se envían como eventos `KeyPress`. Cada tecla conserva igualmente su propio contador de por vida. `0` y `0x07` (las tres) se rechazan: osu!standard se juega con dos teclas.
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_GAME_KEYS, /protocol/opad.proto#ConfigPayload -->

## Deslizamientos en la pantalla táctil (`cfg`)

| Dirección | Campo de acción | Campo de tecla | Campo de modificadores | Acción predeterminada |
|---|---|---|---|---|
| Arriba | `swipe_up_action` | `swipe_up_key` | `swipe_up_modifiers` | `7` Subir volumen |
| Abajo | `swipe_down_action` | `swipe_down_key` | `swipe_down_modifiers` | `8` Bajar volumen |
| Izquierda | `swipe_left_action` | `swipe_left_key` | `swipe_left_modifiers` | `2` Pista anterior |
| Derecha | `swipe_right_action` | `swipe_right_key` | `swipe_right_modifiers` | `3` Pista siguiente |
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_SWIPE_UP, /protocol/opad.proto#ConfigPayload -->

| `SwipeAction` | Valor | Efecto |
|---|---|---|
| `SWIPE_ACTION_KEEP` | `0` | Solo en el cable: mantiene la acción actual. Nunca se guarda. |
| `SWIPE_ACTION_NONE` | `1` | Nada |
| `SWIPE_ACTION_PREV_TRACK` | `2` | Pista anterior |
| `SWIPE_ACTION_NEXT_TRACK` | `3` | Pista siguiente |
| `SWIPE_ACTION_PLAY_PAUSE` | `4` | Reproducir / pausar |
| `SWIPE_ACTION_MUTE` | `5` | Silenciar |
| `SWIPE_ACTION_KEY` | `6` | La tecla de teclado de esa dirección, con sus modificadores |
| `SWIPE_ACTION_VOLUME_UP` | `7` | Subir volumen: el de osu! mientras osu! está activo; si no, el del sistema |
| `SWIPE_ACTION_VOLUME_DOWN` | `8` | Bajar volumen, del mismo modo |
<!-- src: /protocol/opad.proto#SwipeAction, /firmware/main/config/device_config.h#swipe_action_t -->

- Un campo de tecla es un uso HID en `0x04`–`0xE7`, o `0`. Debe ser distinto de cero cuando la acción es `SWIPE_ACTION_KEY`.
- Los modificadores son el byte de modificadores HID: `0x01` Ctrl, `0x02` Shift, `0x04` Alt, `0x08` GUI. Se guardan en `swipe_mods[4]` en el orden arriba, abajo, izquierda, derecha.
- Durante un mapa (`PLAYING`), un deslizamiento `SWIPE_ACTION_KEY` no envía nada, de modo que un deslizamiento nunca mete una tecla ni un modificador en el informe de teclado en mitad de un mapa.
<!-- src: /firmware/main/config/config_validate.c#swipe_valid, /firmware/main/config/device_config.h#device_config_data_t, /firmware/main/input/touch_retry.c#run_swipe_action -->

## Teclas Hall Effect (`he_cfg`, `he_cfg3`)

Cada registro es un `he_config_record_t` empaquetado: magia `0x48454346` (`"HECF"`), `version` `1`, `flags`, dos `he_key_config_record_t` y un CRC-32 (IEEE 802.3, polinomio `0xEDB88320`). El bit *k* de `flags` es el `no_floor` de la tecla *k*; el firmware anterior a este campo lo ignora. Un registro que no supera la comprobación de magia, versión, CRC o rango se carga con los valores de fábrica.
<!-- src: /firmware/main/config/he_config_record.h -->

Las profundidades y distancias van en décimas de porcentaje (0,1 %) del rango magnético calibrado de la tecla: `1000` (`HE_PCT_FULL`) es una pulsación completa normal. Es una fracción de la señal magnética, no del recorrido físico.
<!-- src: /firmware/main/input/he_params.h#HE_PCT_FULL -->

| Ajuste | Campo en el cable (`HeKeySettings`) | Rango guardado | Predeterminado |
|---|---|---|---|
| Punto de actuación | `actuation` | `0`–`1000` | `500` (50,0 %) |
| Histéresis de liberación | `release_hyst` | `0`–`1000` | `100` (10,0 %) |
| Reinicio superior | `top_reset` | `0`–`1000` | `100` (10,0 %) |
| Pulsación de rapid trigger | `rt_press` | `0`–`1000` | `150` (15,0 %) |
| Liberación de rapid trigger | `rt_release` | `0`–`1000` | `150` (15,0 %) |
| Zona muerta inferior | `bottom_dead_zone` (`optional`) | `0`–`1000`, `0` = desactivada | `0` (desactivada) |
| Modo de rapid trigger | `rt_mode` | `HeRtMode` | Desactivado |
| Permitir bajo el piso de ruido | `allow_below_noise_floor` (`optional`) | `bool` | `false` |
<!-- src: /firmware/main/config/he_config_record.h#HE_CONFIG_DEFAULT_ACTUATION, /firmware/main/config/config_validate.c#device_config_validate_he, /protocol/opad.proto#HeKeySettings -->

Los cinco campos de recorrido son `uint16_t` en el registro; en el cable `0` significa mantener el valor actual, así que la aplicación envía `1`–`1000`. Los valores predeterminados son provisionales (`docs/specs/v2-hall-effect-decisions.md` D4).
<!-- src: /firmware/main/protocol/protocol.c#apply_he_settings, /firmware/main/config/he_config_record.h -->

| `HeRtMode` | Valor | Se guarda como | Comportamiento |
|---|---|---|---|
| `HE_RT_MODE_KEEP` | `0` | — | Solo en el cable: mantiene el modo actual |
| `HE_RT_MODE_OFF` | `1` | `rapid_trigger = 0`, `continuous = 0` | Actuación e histéresis de liberación fijas |
| `HE_RT_MODE_NORMAL` | `2` | `rapid_trigger = 1`, `continuous = 0` | Rapid trigger por debajo del punto de actuación |
| `HE_RT_MODE_CONTINUOUS` | `3` | `rapid_trigger = 1`, `continuous = 1` | Rapid trigger hasta la profundidad de reinicio superior |
<!-- src: /protocol/opad.proto#HeRtMode, /firmware/main/protocol/protocol.c#apply_he_settings -->

### Pisos de ruido

Cuando el teclado convierte los ajustes en umbrales crudos (`he_params_build`), mantiene cada umbral de movimiento (histéresis, pulsación y liberación de rapid trigger) y el margen de liberación sobre el reposo en o por encima de unos pisos de ruido en conteos crudos. Si un ajuste cae por debajo de un piso, se usa el piso y un bit `HE_PARAMS_*_RAISED` lo indica. El teclado comunica sus pisos en `HeCalibrationStatus` (`noise_floor_rest`, `noise_floor_movement`) para que la aplicación pueda mostrar el punto de pulsación real. `allow_below_noise_floor` baja los pisos de una tecla a 1 conteo, y entonces el propio ruido del sensor puede pulsar esa tecla. El rapid trigger permanece desactivado hasta que la tecla tiene una calibración válida.
<!-- src: /firmware/main/input/he_params.h#he_params_build, /protocol/opad.proto#HeCalibrationStatus -->
