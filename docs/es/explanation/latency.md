---
title: Ingeniería de latencia
description: De dónde vienen la latencia y el jitter de pulsación en un teclado USB, y cómo OPad elimina cada fuente con endpoints HID separados, antirrebote inmediato, aislamiento del núcleo 0 y medición en el propio teclado.
sidebar:
  order: 2
---

En osu!, lo que duele no es el retraso medio sino su *variación*: una tecla que a veces llega 1 ms tarde ensancha la dispersión del error de acierto (Unstable Rate) aunque la media sea baja. La regla de OPad es que **la latencia siempre gana** ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1): la pantalla, el almacenamiento, la telemetría serie, la bandeja del sistema y tosu nunca entran en la ruta de pulsación.

Una pulsación pasa por cuatro etapas antes de que el juego la vea. Detrás de cada una hay una decisión de diseño:

```mermaid
flowchart LR
    A["El interruptor se cierra"] -->|"antirrebote: 0 µs en el primer flanco"| B["Tarea de entrada (núcleo 0)"]
    B -->|"su propio endpoint, sin esperar a la otra tecla"| C["Informe HID en cola"]
    C -->|"el anfitrión sondea cada 1 ms"| D["Anfitrión / osu!"]
```

El teclado mide la parte que controla, desde el flanco del interruptor hasta que el informe queda en cola en TinyUSB, y la presenta como histograma (consulta [Medición en el propio teclado](#medición-en-el-propio-teclado)). El último salto, hasta 1 ms de sondeo USB, corresponde al anfitrión.

## Antirrebote sin retraso

Los contactos de un interruptor rebotan durante unos milisegundos al cerrarse o abrirse. La solución habitual espera a que la señal lleve un rato estable antes de informar, lo que añade esa espera a *cada* pulsación.

OPad hace el antirrebote **de forma inmediata** ([debounce.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.c), `debounce_step`): el primer flanco se da por bueno al instante y el rebote que le sigue se ignora.

```text
Switch contact:   ___|‾|_|‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|_|‾|________
Reported:         ___|‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|_____________
                     ^ press, 0 µs added           ^ release, 0 µs added
                     |-- lockout (5 ms) --| resample
```

1. **El flanco se acepta en el acto.** La interrupción GPIO (`gpio_isr_handler` en [keypad.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/keypad.c), `IRAM_ATTR`) pone marca de tiempo al flanco con `esp_timer_get_time()` y llama a `debounce_step` con `DEBOUNCE_SOURCE_EDGE`. Fuera de un bloqueo el cambio se acepta y se despierta a `keypad_task` con `vTaskNotifyGiveFromISR`.
2. **Sigue un bloqueo.** Aceptar un cambio inicia un bloqueo (`DEBOUNCE_DEFAULT_US = 5000`, configurable entre 500 y 20 000 µs, [debounce.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.h)). Los flancos dentro de él son rebote y se descartan.
3. **El pin se vuelve a leer al terminar.** `keypad_task` lee el pin una vez (`DEBOUNCE_SOURCE_RESAMPLE`). Si no coincide con el estado aceptado (una liberación ocurrida durante el bloqueo, o una interferencia), el estado se corrige y empieza un bloqueo nuevo. Ningún cambio se pierde; como mucho se retrasa un bloqueo.

El compromiso es que un pico de ruido lo bastante largo para disparar la interrupción cuenta como pulsación, y se corrige (se suelta) un bloqueo después. Con interruptores reales y un cable corto eso es raro; el bloqueo es configurable para los montajes en los que no lo sea.

Los interruptores Hall Effect no tienen contactos y se saltan todo esto; consulta [Hall Effect y Rapid Trigger](hall-effect.md).

## Un endpoint por tecla de juego

El anfitrión sondea un endpoint USB HID full-speed una vez por `bInterval`, aquí 1 ms (1000 Hz). El dispositivo solo puede tener un informe esperando en un endpoint: un segundo informe espera a que el anfitrión haya recogido el primero.

En un stream, las dos teclas cambian con mucho menos de 1 ms de diferencia. En un teclado con un solo endpoint, el cambio de la tecla 2 hecho 200 µs después del de la tecla 1 espera al siguiente sondeo para que se recoja el informe de la tecla 1: hasta 1 ms de retraso que depende del momento, justo el tipo de jitter que duele.

OPad da a cada tecla de juego su propio endpoint ([usb_descriptors.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_descriptors.c), [usb_hid.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_hid.c)):

| Tecla | Interfaz y endpoint | Informe | Comparte el endpoint con |
|---|---|---|---|
| **Tecla 1** | Interfaz 0, `EP 0x81`, 1 ms (`HID_INSTANCE_KEYBOARD`) | Informe de teclado de arranque, sin ID de informe | El Quick Retry de la pantalla táctil (`` ` ``) y, fuera de un mapa, las teclas de deslizamiento (`LINK_KEY1_TOUCH`) |
| **Tecla 2** | Interfaz 3, `EP 0x84`, 1 ms (`HID_INSTANCE_MEDIA`) | Informe de teclado, `MEDIA_REPORT_ID_KEYBOARD` | Las teclas multimedia de deslizamiento y la rueda (`LINK_KEY2`) |
| **Tecla 3** (HE V3) | La de las dos en la que no esté la otra tecla de juego (`usb_hid_set_key3`) | El mismo que ese enlace | Va con la tecla 2, salvo que la tecla 2 sea de juego y la tecla 1 no |

Las interfaces 1 y 2 son el puerto serie CDC-ACM (`EP 0x82`, `0x03`, `0x83`). El ESP32-S3 permite cinco endpoints IN contando EP0, así que no queda sitio para una tercera interfaz HID: la tecla 2 comparte el endpoint de la interfaz multimedia, donde solo compiten con ella los deslizamientos (que nunca se envían durante un mapa).

El resultado es que el informe de una tecla solo espera a *su propio* informe anterior, que tiene al menos un bloqueo de antirrebote de antigüedad y hace tiempo que se recogió.

### Seguimiento de secuencias sin bloqueos (`hid_link_t`)

Los eventos de tecla llegan del núcleo 0, pero los de la pantalla táctil llegan del núcleo 1 (`touch_retry`) y acaban en el informe de la tecla 1. Un mutex entre ellos permitiría que una tarea del núcleo 1 retrasara una tecla. En su lugar, cada informe tiene un `hid_link_t` con tres atómicos:

- `change_seq` lo incrementa quien cambia una tecla en ese informe. Un envío que lo cargó antes de leer las teclas tiene garantizado llevar al menos ese cambio.
- `sent_seq` registra el cambio más reciente que llevó un envío correcto, y lo sube con compare-and-swap el emisor que lo consiga. Un cambio está pendiente mientras `sent_seq` va por detrás de `change_seq`. A diferencia de una bandera de "pendiente", un emisor nunca puede borrar el cambio de otro.
- `pending_edge_us` guarda el instante del flanco del cambio más antiguo sin entregar, así que su latencia se mide desde el flanco del interruptor aunque se envíe tarde.

Si el endpoint está ocupado (`tud_hid_n_ready` falso), el cambio queda pendiente, y `tud_hid_report_complete_cb` lo envía en cuanto termina la transferencia en curso. Un cambio puede llegar un informe tarde, pero nunca perderse, y una tecla nunca puede quedarse atascada.

## Mantener libre el núcleo 0

Los dos mecanismos anteriores solo funcionan si nada más retrasa la tarea de entrada. [Arquitectura del sistema](architecture.md) cubre el reparto de núcleos; las reglas de la ruta de teclas son:

- **Núcleo y prioridad**: las tareas de entrada se ejecutan en el núcleo 0 a `configMAX_PRIORITIES - 1`, TinyUSB un nivel por debajo. LVGL, el protocolo serie y el tacto van en el núcleo 1.
- **IRAM**: `gpio_isr_handler`, `debounce_step`, `usb_hid_handle_key_event`, `latency_stats_record` y el callback de trama ADC de V1 llevan `IRAM_ATTR`, y [linker.lf](https://github.com/OPad-dev/OPad/blob/main/firmware/main/linker.lf) coloca `keypad_task`, `usb_hid` y la ruta HID de TinyUSB en IRAM, así que un fallo de caché de flash provocado por el núcleo 1 no puede detenerlos.
- **Sin asignación de memoria**: el estado del antirrebote, los enlaces HID y el histograma de latencia son estáticos.
- **Ninguna escritura en flash durante un mapa**: las escrituras NVS esperan a `IDLE` ([runtime.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/runtime/runtime.c)).

## Medición en el propio teclado

[latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c) registra, para cada cambio de tecla, el tiempo desde la marca del flanco hasta el momento en que TinyUSB acepta el informe:

- **Histograma**: 500 cubetas atómicas de 10 µs (`BUCKET_US`, `BUCKET_COUNT`), que cubren de 0 a 5 ms; lo más lento cae en la última cubeta. Por tanto `p50`, `p99` y `p99.9` son múltiplos de 10 µs (el borde superior de una cubeta), mientras que `max_us` es exacto.
- **Informes diferidos**: cuántos cambios tuvieron que esperar a una transferencia en curso.
- **Valores atípicos**: toda muestra por encima de 1000 µs se cuenta y se guarda su máximo; la tarea del protocolo los comunica al anfitrión como diagnósticos `DIAG_EVENT_LATENCY_OUTLIER`.

`opadctl latency` lee las cifras y `opadctl latency --reset` las borra antes de una prueba. La medición se detiene en TinyUSB: lo que el anfitrión haga después con el informe no está en estas cifras. [Medición de latencia](../develop/latency-testing.md) describe las etapas del benchmark y el filtro de publicación construido sobre ellas.
