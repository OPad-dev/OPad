---
title: Subsistema de diagnóstico del firmware
description: Referencia de implementación del búfer circular de eventos de diagnóstico, la autoprueba de la placa portadora y del módulo, y el modo de banco de pruebas Hall Effect con streaming.
sidebar:
  order: 8
---

El subsistema de diagnóstico ofrece un registro de eventos en búfer circular sin reservas de memoria, pruebas eléctricas de las PCB de la placa portadora y del módulo, y un modo de banco de pruebas de sensores Hall Effect que se activa en tiempo de compilación.

## Búfer circular de diagnóstico (`diag.c`)

Implementado en [/firmware/main/diag/diag.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/diag.c):

- Búfer circular fijo `s_entries[DIAG_RING_SIZE]` (`DIAG_RING_SIZE = 64`) de `diag_entry_t` (`timestamp_ms`, `event_id`, `level`, `arg0`, `arg1`), protegido por un spinlock de FreeRTOS (`taskENTER_CRITICAL(&s_mux)`).
- `diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1)` no reserva memoria del heap ni formatea cadenas; cuando está lleno, descarta la entrada más antigua e incrementa `s_dropped`.
- `diag_drain(out_entries, max_entries, out_dropped)` saca las entradas en cola y pone a cero `s_dropped` para que la tarea del protocolo las envíe al host en `LogEventBatch`.
- Los IDs de evento son formato de protocolo (solo se añaden), definidos en [/firmware/main/diag/diag.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/diag.h) y formateados para `opadctl monitor` en [/desktop/crates/opad-model/src/diag.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/diag.rs):

| ID | Evento | ID | Evento |
|---|---|---|---|
| 1 | `BOOT` (arg0 = motivo del reinicio) | 13 | `COUNTER_SYNC_REJECTED` |
| 2 | `HID_MOUNTED` | 14 | `LAYOUT_REJECTED` (arg0 = pantalla) |
| 3 | `HID_UNMOUNTED` | 15 | `DEFERRED_WRITE_FLUSHED` |
| 4 | `HID_SUSPENDED` | 16 | `DISPLAY_SLEEP` |
| 5 | `CDC_OPENED` | 17 | `DISPLAY_WAKE` |
| 6 | `CDC_CLOSED` | 18 | `LATENCY_OUTLIER` (arg0 = µs) |
| 7 | `NVS_INIT_FAILED` | 19 | `CDC_WRITE_DROPPED` |
| 8 | `NVS_ERASED` | 20 | `BUFFER_OVERFLOW` |
| 9 | `LCD_INIT_FAILED` | 21 | `UNKNOWN_HOST_MSG` (arg0 = etiqueta del campo) |
| 10 | `DECODE_FAILED` | 22 | `DECODE_BYTES` |
| 11 | `FRAME_TOO_LARGE` (bytes de CDC descartados; arg1 = motivo `DIAG_FRAME_DROP_*`, incluidos parcial obsoleto y CRC incorrecto) | 23 | `MEMORY` |
| 12 | `CONFIG_REJECTED` | 24 | `UI_PERF` |

Dos de ellos necesitan más detalle:
  - `DIAG_EVENT_MEMORY = 23`: se registra una vez tras el arranque en [/firmware/main/app_main.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/app_main.c) con `arg0 =` RAM interna libre (bytes) y `arg1 =` mayor bloque interno libre apto para DMA (bytes); se ve en `opadctl monitor`.
  - `DIAG_EVENT_UI_PERF = 24`: se registra en las compilaciones con `CONFIG_OPAD_UI_PERF_TEST` desde [/firmware/main/diag/ui_perf_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/ui_perf_test.c) al final de cada fase (`DIAG_UI_PERF_IDLE = 0`, `DIAG_UI_PERF_SONG = 1`, `DIAG_UI_PERF_RESULT = 2`) con el número de fotogramas, el tiempo medio de fotograma y el fotograma más largo (en unidades de 0,1 ms).

## Autoprueba de las PCB de la placa portadora y del módulo de entrada (`board_test.c`)

Implementada en [/firmware/main/diag/board_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/board_test.c) y lanzada con el mensaje del host `run_board_test` (`opadctl board-test`, o la página **Diagnostics** de la GUI). La tarea del protocolo la rechaza fuera de `IDLE` (`"not while a map is played"`) y mientras se ejecuta la detección de pines (`"pin detection is running"`), porque la prueba reconfigura los pines de tecla ([/firmware/main/protocol/protocol.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/protocol/protocol.c)). Pasos:

1. **Prueba de pull de las líneas de tecla (`keypad_line_test`)**: cambia temporalmente cada GPIO de tecla armado entre pull-up y pull-down internos (`LINE_TEST_SETTLE_US = 300` µs) para comprobar si los pull-ups externos de 10 kΩ de un módulo MX mantienen la línea alta frente al pull-down, o si un interruptor está mantenido o cortocircuitado a `GND`.
2. **Lectura del ID del módulo (`board_read_module_id`)**: muestrea las tensiones en abierto y con carga en el pin ADC del ID del módulo (`GPIO 8`) para clasificarlo como `MX`, `HE`, `HE_V3`, `MX_V3` o `NONE` (se omite en HE V1, donde el DMA continuo del ADC1 es dueño del ADC1, y se usa en su lugar el módulo del arranque).
3. **Sondeo de cable invertido (`board_probe_reversed_cable`)**: cuando ningún módulo responde en `GPIO 8` y `GPIO 2` (`BOARD_REVERSE_PROBE_GPIO`) no es un pin de tecla, busca un cable invertido.
4. **Prueba de continuidad y puentes de las líneas libres de V1 (`test_spares`)**: en los módulos V1 `MX` / `HE`, prueba `GPIO 6`, `GPIO 4` y `GPIO 2` con pull-up/pull-down y lleva a nivel bajo los pines vecinos de uno en uno para detectar cortos entre pines (`bridged_gpios`).
5. **Prueba del ADS7028 de HE V3 (`he_adc_test_run`, [/firmware/main/diag/he_adc_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/he_adc_test.c))**: en `BOARD_MODULE_HE_V3`, reinicia el ADS7028, lee sus registros de estado y muestrea los sensores. Si el muestreador SPI está en marcha, se pausa durante la prueba (`he_backend_run_paused`, soltando antes las teclas); si en cambio en ese módulo están armados los pines de tecla MX, la prueba se omite con `"key input is on: restart the pad"`, porque seleccionar el ADC (CS es GPIO10) pulsaría una tecla.

## Modo de banco de pruebas Hall Effect (`hall_bench.c`)

Solo se compila con `CONFIG_OPAD_HALL_BENCH=y` (no es una compilación de release; para `opad-daemon`, porque envía texto plano por CDC) ([/firmware/main/diag/hall_bench.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/hall_bench.c)):

- Muestrea cuatro canales del ADC1 a `BENCH_SAMPLE_HZ = 64000` Hz (`BENCH_FRAME_CONVS = 64`, tramas de 1 ms): Key 1 (`ADC_CHANNEL_9`), Key 2 (`ADC_CHANNEL_6`), ID del módulo (`ADC_CHANNEL_7`) y referencia (`ADC_CHANNEL_5`, `GPIO 6`).
- Ejecuta su propia tarea `hall_bench` (núcleo 0, `configMAX_PRIORITIES - 3`) y emite líneas promediadas cada 1 ms `H,<ms>,<key1>,<key2>`, resúmenes de dispersión por canal cada 1 s `S,<key>,<n>,<min>,<max>,<mean>,<stddev_x100>,<overflows>`, líneas de fase del ciclo de pull `P,<phase>` y líneas de error `E,start|restart|read,...` para `scripts/hall_bench.py`.
- También muestra en la LCD una vista en vivo de tensión/barra de las 2 teclas (`hall_bench_show_ui`, temporizador de 33 ms, midiendo el reposo durante los primeros `BENCH_REST_MS = 500` ms).
