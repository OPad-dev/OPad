---
title: Ingeniería de latencia
description: Cómo minimiza OPad la latencia y fluctuación de pulsación mediante dos endpoints USB HID, antirrebote inmediato, aislamiento en Core 0 y telemetría de latencia en el dispositivo.
---

En juegos de ritmo como *osu!*, la consistencia temporal se mide en milisegundos de un solo dígito, y la fluctuación (jitter) aumenta directamente el Unstable Rate (UR). La regla principal de ingeniería de OPad es **la latencia siempre gana** ([/AGENTS.md](/AGENTS.md) §1.1): la pantalla, el almacenamiento, la telemetría serie, la bandeja del sistema y `tosu` nunca entran en la ruta de pulsación.

## Doble endpoint USB HID

El anfitrión sondea los endpoints USB HID Full-Speed a intervalos de `1 ms` (`1000 Hz`, `bInterval = 1`). En un teclado convencional de un solo endpoint:

- Cuando Key 1 cambia de estado, se encola un reporte HID en el endpoint.
- Si Key 2 cambia de estado `200 µs` después (habitual en secuencias alternas rápidas), su reporte no puede enviarse hasta que el anfitrión recoja el primero, añadiendo hasta `1 ms` de fluctuación variable.

OPad evita la contención de endpoint entre las dos teclas principales de juego dividiendo los reportes de teclado entre dos endpoints IN independientes de 1000 Hz ([/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c), [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c)):

| Tecla | Interfaz USB y endpoint | Reporte HID | Notas |
|---|---|---|---|
| **Key 1** | Interfaz 0 — `EP 0x81` (`HID_INSTANCE_KEYBOARD`, 1 ms) | Reporte de teclado boot (report ID `0`) | Comparte endpoint con Quick Retry de pantalla táctil (`` ` ``, uso `0x35`) durante un mapa (`LINK_KEY1_TOUCH`). |
| **Key 2** | Interfaz 3 — `EP 0x84` (`HID_INSTANCE_MEDIA`, 1 ms) | Reporte de teclado (`MEDIA_REPORT_ID_KEYBOARD`) | Comparte endpoint con reportes multimedia/rueda de deslizamientos fuera de partida (`LINK_KEY2`). |
| **Key 3** (V3) | Enrutada dinámicamente (`usb_hid_set_key3`) | Emparejada en `LINK_KEY1_TOUCH` o `LINK_KEY2` | Usa el endpoint que la otra tecla de juego activa no esté usando, de modo que dos teclas de juego nunca comparten endpoint. |

(Las interfaces 1 y 2 son CDC-ACM usando `EP 0x82`, `0x03` y `0x83`; dado que el ESP32-S3 admite como máximo cinco endpoints IN incluido `EP0`, Key 2 comparte el endpoint IN `0x84` de la Interfaz 3.)

### Seguimiento de secuencia sin bloqueos (`hid_link_t`)

Los eventos de teclas provienen de Core 0 (`keypad_task` o el backend Hall Effect), mientras que los eventos táctiles provienen de Core 1 (`touch_retry`). Para coordinar envíos sin mutex bloqueantes en la ruta de pulsación, cada enlace de endpoint (`hid_link_t` en [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c)) mantiene contadores atómicos:

- `change_seq`: se incrementa atómicamente tras actualizar el estado de teclas para que cualquier emisor que lo leyó antes de leer las teclas entregue al menos ese cambio.
- `sent_seq`: avanza con compare-and-swap cada vez que `tud_hid_n_keyboard_report` tiene éxito. Si un endpoint está ocupado (`!tud_hid_n_ready`), el cambio queda pendiente (`sent_seq < change_seq`) y `tud_hid_report_complete_cb` lo reenvía en cuanto termina la transferencia en curso, evitando flancos perdidos o teclas atascadas.
- `pending_edge_us`: almacena la marca temporal en microsegundos del flanco de tecla más antiguo sin entregar en ese enlace para medir con exactitud la latencia diferida cuando el callback de finalización lo envía.

## Entrada mecánica: antirrebote inmediato

Los contactos de los interruptores mecánicos rebotan durante unos milisegundos tras cerrarse o abrirse. El antirrebote diferido espera a que la señal se estabilice antes de informar una pulsación, añadiendo varios milisegundos de retraso a cada toque.

OPad emplea **antirrebote inmediato (eager)** en [/firmware/main/input/debounce.c](/firmware/main/input/debounce.c) (`debounce_step`) y [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c):

```text
Physical Switch:  [Close] ~~~bounce~~~ [Held] ~~~bounce~~~ [Open]
Eager State:      [PRESS immediately] -- lockout (default 5000 us) --> [resample] ... [RELEASE immediately]
Debounce Delay:   0 us added to first edge
```

1. **Aceptación inmediata del flanco**: La interrupción GPIO (`gpio_isr_handler`, en `IRAM_ATTR`) marca el flanco con `esp_timer_get_time()` y ejecuta `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)`. Fuera de una ventana de bloqueo activa, la pulsación o liberación se acepta al instante y se despierta `keypad_task` (Core 0, prioridad `configMAX_PRIORITIES - 1`) mediante `vTaskNotifyGiveFromISR`.
2. **Ventana de bloqueo**: Aceptar una transición fija `lockout_end_us = now_us + lockout_us` (`DEBOUNCE_DEFAULT_US = 5000`, configurable de `500` a `20000` µs en [/firmware/main/input/debounce.h](/firmware/main/input/debounce.h)). Los flancos adicionales durante el bloqueo se suprimen.
3. **Remuestreo al fin del bloqueo**: Al expirar `lockout_end_us`, `keypad_task` lee una vez el GPIO físico (`DEBOUNCE_SOURCE_RESAMPLE`). Si el pin acabó en el estado opuesto durante el bloqueo (por ejemplo, un pico o una liberación ocurrida dentro del bloqueo), corrige el estado e inicia una nueva ventana de bloqueo.

## Latencia de entrada Hall Effect

Los interruptores Hall Effect carecen de rebote metálico de contactos y evitan por completo el antirrebote mecánico ([/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c)):

- **V1 (ADC1 interno por DMA)**: Tramas de 250 µs (`4 kHz`), filtradas por mediana y evaluadas dentro de la ISR de trama en IRAM en Core 0, despertando `he_task` únicamente en las transiciones de tecla.
- **V3 (ADS7028 externo por SPI3 a 40 MHz)**: Tramas de 50 µs (`20 kHz`) impulsadas por un `gptimer` hardware que despierta la tarea `he_spi` en Core 0 (`configMAX_PRIORITIES - 1`), la cual lee las 3 teclas directamente desde los registros `GPSPI3` y envía los reportes HID directamente al cambiar de estado.

## Telemetría de latencia en el dispositivo

El firmware mide el tiempo transcurrido desde la marca temporal del flanco físico (`edge_us`) hasta que TinyUSB acepta el reporte HID en [/firmware/main/input/latency_stats.c](/firmware/main/input/latency_stats.c):

- **Histograma**: 500 cubetas atómicas de `10 µs` (`BUCKET_US = 10`, `BUCKET_COUNT = 500`, cubriendo de `0` a `5000 µs`; las muestras más lentas caen en la última cubeta mientras `s_max_us` registra el máximo exacto).
- **Percentiles**: `latency_stats_get` calcula `p50`, `p99`, `p99.9`, `max_us`, el número de muestras y `deferred_reports` (flancos que tuvieron que esperar a una transferencia USB en curso). Consúltalos en cualquier momento con `opadctl latency` o reinícialos antes de un mapa con `opadctl latency --reset`.
- **Seguimiento de valores atípicos**: Cualquier muestra superior a `1000 µs` incrementa `s_outlier_count` y actualiza `s_outlier_max_us`.

## Invariantes de núcleos, memoria y flash

- **Aislamiento de núcleos**: El manejo de entradas (`keypad_task`, `he_task`, `he_spi`) y TinyUSB se ejecutan en Core 0 a máxima prioridad (`configMAX_PRIORITIES - 1`); el renderizado de interfaz LVGL, el sondeo táctil y el entramado serie se ejecutan en Core 1 ([/firmware/main/app_main.c](/firmware/main/app_main.c)).
- **Sin asignación dinámica en la ruta de pulsación**: Los estados de antirrebote, los enlaces HID y los histogramas de latencia se asignan estáticamente.
- **Ubicación en IRAM**: Las rutinas críticas (`gpio_isr_handler`, `debounce_step`, `usb_hid_handle_key_event`, `latency_stats_record` y la ISR `on_frame` del ADC V1) llevan `IRAM_ATTR` para que los fallos de caché de flash no las detengan.
- **Sin escrituras en flash durante el juego**: Los volcados NVS (contadores, configuración, calibración) se difieren mientras el estado de ejecución es `PLAYING` o `COOLDOWN` y solo se escriben en `IDLE` ([/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c)).
