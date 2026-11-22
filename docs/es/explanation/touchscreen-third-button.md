---
title: La pantalla táctil como tercer botón
description: Explicación de la integración del controlador táctil capacitivo CST816S, Quick Retry con dedo apoyado, clasificación de gestos y aislamiento durante la partida.
sidebar:
  order: 3
---

En la Waveshare ESP32-S3-Touch-LCD-2, la pantalla táctil capacitiva funciona como **el tercer botón** (AGENTS.md §1.4). Ofrece una pulsación limpia, un estado sostenido mientras el dedo permanece apoyado y una liberación limpia, manteniendo la sobrecarga de CPU e I2C fuera de la ruta de pulsación ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).

## Arquitectura de hardware e interrupciones

El subsistema táctil utiliza un controlador táctil capacitivo CST816 por I2C (`CST816_I2C_ADDR = 0x15` a 100 kHz en `I2C_NUM_0`) cableado a **GPIO 48 (SDA)** y **GPIO 47 (SCL)**, con una línea de interrupción activa-baja en **GPIO 46** ([`board_pins.h`](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h), [`touch_retry.c`](/firmware/main/input/touch_retry.c)). La tarea corre en Core 1 para no competir nunca con la entrada de teclas ni TinyUSB en Core 0.

Las coordenadas del panel (vertical `240x320`) se transforman con `TOUCH_SWAP_XY = 1` y `TOUCH_MIRROR_Y = 1` a coordenadas horizontales `320x240` para que las direcciones coincidan con la orientación del teclado ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).

### Modelo de ejecución de bajo consumo de CPU

- **Reposo en interrupción**: Cuando no hay ningún dedo apoyado, `touch_retry_task` bloquea en `ulTaskNotifyTake()` esperando la ISR de flanco descendente de GPIO 46 (`touch_int_isr`), con un respaldo de seguridad `TOUCH_IDLE_CHECK_MS = 1000` ms (recurriendo a sondeo a 100 Hz solo si falla la configuración de la ISR). No se realizan lecturas I2C en reposo ([`touch_retry.c`](/firmware/main/input/touch_retry.c)).
- **Sondeo a 100 Hz mientras el dedo está apoyado**: Al iniciarse un toque, la tarea sondea cada `TOUCH_POLL_MS = 10` ms (100 Hz) para seguir el desplazamiento del dedo y detectar la liberación, ya que el pin INT del CST816 puede emitir pulsos por reporte en vez de permanecer en bajo.
- **Antirrebote de liberación**: Soltar requiere `TOUCH_RELEASE_READS = 2` lecturas vacías consecutivas (~20 ms) antes de soltar el estado mantenido, evitando que una lectura ruidosa corte una pulsación sostenida.

## Modos táctiles sensibles al contexto

Al tocar la pantalla, [`touch_mode_now()`](/firmware/main/input/touch_retry.c) elige uno de tres modos:

| Modo | Condición | Acción de toque | Acción de deslizamiento |
|---|---|---|---|
| **`TOUCH_MODE_PLAY`** | El anfitrión envió `HostStatus` con `tosu_connected = true` dentro de `HOST_STATUS_TIMEOUT_US` (3 s) y el estado del dispositivo es `PLAYING`. | Un dedo apoyado (o un toque corto sin deslizar) envía Quick Retry (`` ` ``). | Los deslizamientos multimedia/volumen se ejecutan sin añadir Alt ni otras teclas; los deslizamientos de teclado se suprimen. |
| **`TOUCH_MODE_MENU`** | El anfitrión envió `HostStatus` con `tosu_connected = true` dentro de 3 s y el estado no es `PLAYING`. | Los toques y pulsaciones mantenidas no envían nada. | Los deslizamientos ejecutan las acciones configuradas de volumen, multimedia, atajos de osu! o teclas libres. |
| **`TOUCH_MODE_RETRY`** | Sin anfitrión conectado, `tosu` desconectado o sin `HostStatus` durante > 3 s. | Todo toque pulsa y mantiene inmediatamente Quick Retry (`` ` ``). | Deslizamientos desactivados. |

### El dedo apoyado en partida (Quick Retry)

En `TOUCH_MODE_PLAY`, Quick Retry no se activa al instante del primer contacto para que un deslizamiento nunca reinicie un mapa por accidente ([`touch_retry_task()`](/firmware/main/input/touch_retry.c)):

- Si el dedo se mueve menos de `PLAY_RETRY_STILL_PX = 12` píxeles (distancia de Chebyshev, [`gesture_distance()`](/firmware/main/input/gesture.c)) durante `PLAY_RETRY_STILL_US = 60,000` µs (60 ms), se clasifica como dedo apoyado y `usb_hid_set_touch_retry(true)` activa Quick Retry (`` ` ``).
- Si el dedo permanece apoyado durante `PLAY_SWIPE_WINDOW_US = 250,000` µs (250 ms) sin convertirse en deslizamiento, también activa Quick Retry.
- Si el dedo se levanta antes de 60 ms sin deslizar, [`tap_retry()`](/firmware/main/input/touch_retry.c) envía una pulsación y liberación de Quick Retry.
- En cuanto se dispara un deslizamiento, `retry_pending` se borra para que ese toque nunca envíe Quick Retry.

## Clasificación de deslizamientos y gestos multimedia

El seguimiento de movimiento reside en [`firmware/main/input/gesture.c`](/firmware/main/input/gesture.c) y [`gesture.h`](/firmware/main/input/gesture.h):

- **Umbral y bloqueo de eje**: Partiendo de `(origin_x, origin_y)`, el dedo debe desplazarse al menos `GESTURE_SWIPE_PX = 30` píxeles en un eje con relación mínima 1.5:1 (`2 * |dy| >= 3 * |dx|` para vertical, `2 * |dx| >= 3 * |dy|` para horizontal) antes de fijarse en ese eje. El movimiento diagonal espera un desplazamiento más claro.
- **Repetición vertical frente a disparo único horizontal**: Los arrastres verticales avanzan un paso cada `30` píxeles (`origin_y` avanza `±30`), permitiendo ajustar el volumen de forma continua en arrastres largos (e invertir la dirección); los deslizamientos horizontales se disparan una sola vez por toque (`horizontal_fired`). Las acciones que no son de volumen solo se disparan en el primer paso (`!repeat`).
- **Asignaciones predeterminadas** ([`touch_retry.c`](/firmware/main/input/touch_retry.c), [`opad-model/src/lib.rs`](/desktop/crates/opad-model/src/lib.rs)):
  - **Arriba / Abajo**: `Volume up` / `Volume down`. Cuando el anfitrión activa `HostStatus.osu_active` (osu! está en primer plano en Windows u osu!lazer está ejecutándose en Linux), el propio teclado ajusta el volumen de osu! mediante rueda de ratón HID (`usb_media_wheel`): rueda sola durante un mapa (`playing == true`) y Left Alt + rueda en menús (`playing == false`). Cuando `osu_active` es falso, envía USB HID Consumer Control Volume Up / Down (`usb_media_consumer_tap`).
  - **Izquierda / Derecha**: `Previous track` / `Next track` mediante USB HID Consumer Control.
- **Protección del menú Alt en Windows**: En los menús, [`step_volume()`](/firmware/main/input/touch_retry.c) mantiene Left Alt (`HID_MOD_LEFT_ALT = 0x04`) durante todos los pasos de rueda de un arrastre. Antes de soltar Alt al levantar el dedo, [`release_alt()`](/firmware/main/input/touch_retry.c) pulsa `HID_KEY_F24` (`0x73`) y espera la entrega HID (`TOUCH_KEY_WAIT_MS = 30` ms) para que Windows no interprete una pulsación aislada de Alt como apertura del menú de ventana (`SC_KEYMENU`).

## Invariante de aislamiento durante la partida

De acuerdo con AGENTS.md §1.4:

1. **Nunca Alt ni teclas extra en mitad de un mapa**: Durante `TOUCH_MODE_PLAY`, [`step_volume()`](/firmware/main/input/touch_retry.c) usa la rueda de ratón sin Left Alt, y [`run_swipe_action()`](/firmware/main/input/touch_retry.c) ignora `SWIPE_ACTION_KEY` para que el reporte de teclado solo transporte las teclas físicas y Quick Retry.
2. **Pantalla limpia durante el juego**: [`handle_swipe()`](/firmware/main/input/touch_retry.c) solo llama a `ui_show_swipe()` cuando `!playing`; no se dibuja nada adicional en la LCD durante un mapa.
3. **Teclados Hall Effect sin calibrar**: Cuando `he_cal_suppress_input(he_backend_is_locked(), he_backend_is_calibrating())` es verdadero (registro de decisiones D7 en [`v2-hall-effect-decisions.md`](../../specs/v2-hall-effect-decisions.md)), [`touch_retry_task()`](/firmware/main/input/touch_retry.c) dirige los toques exclusivamente a la interfaz de calibración (`he_cal_ui_touch_sample`), desactiva los deslizamientos y no envía reportes de teclado ni multimedia.
