---
title: Subsistema de diagnóstico del firmware
description: Referencia de implementación del búfer circular de eventos de diagnóstico, autoprueba de placa del carrier/módulo y modo banco Hall Effect.
---

El subsistema de diagnóstico proporciona registro de eventos en búfer circular sin asignaciones dinámicas, pruebas eléctricas de PCB del carrier/módulo y un modo banco en tiempo de compilación para sensores Hall Effect.

## Búfer circular de diagnóstico (`diag.c`)

Implementado en [/firmware/main/diag/diag.c](/firmware/main/diag/diag.c):

- Búfer circular fijo `s_entries[DIAG_RING_SIZE]` de `diag_entry_t` (`timestamp_ms`, `event_id`, `level`, `arg0`, `arg1`), protegido por un spinlock FreeRTOS (`taskENTER_CRITICAL(&s_mux)`).
- `diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1)` no realiza asignaciones en el heap ni formateo de cadenas; cuando está lleno, descarta la entrada más antigua e incrementa `s_dropped`.
- `diag_drain(out_entries, max_entries, out_dropped)` vacía las entradas encoladas y reinicia `s_dropped` para que la tarea de protocolo las envíe al anfitrión en `LogEventBatch`.
- Eventos de telemetría destacados en [/firmware/main/diag/diag.h](/firmware/main/diag/diag.h) y formateados en [/desktop/crates/opad-model/src/diag.rs](/desktop/crates/opad-model/src/diag.rs):
  - `DIAG_EVENT_MEMORY = 23`: Registrado una vez tras el arranque en [/firmware/main/app_main.c](/firmware/main/app_main.c) con `arg0 =` RAM interna libre (bytes) y `arg1 =` mayor bloque interno compatible con DMA libre (bytes), visible en `opadctl monitor`.
  - `DIAG_EVENT_UI_PERF = 24`: Registrado en compilaciones `CONFIG_OPAD_UI_PERF_TEST` por [/firmware/main/diag/ui_perf_test.c](/firmware/main/diag/ui_perf_test.c) al final de cada fase (`DIAG_UI_PERF_IDLE = 0`, `DIAG_UI_PERF_SONG = 1`, `DIAG_UI_PERF_RESULT = 2`) con el número de fotogramas, el tiempo medio por fotograma y el tiempo máximo por fotograma (en décimas de milisegundo).

## Autoprueba de PCB del carrier y módulo de entrada (`board_test.c`)

Implementada en [/firmware/main/diag/board_test.c](/firmware/main/diag/board_test.c) e invocada mediante `BoardTestRequest` (`opadctl board-test` o `opad-gui` **Diagnostics** -> **🔧 PCB Test**, solo en `IDLE`):

1. **Prueba de pull de líneas de teclas (`keypad_line_test`)**: Conmuta temporalmente cada GPIO de tecla armado entre pull-up y pull-down interno (`LINE_TEST_SETTLE_US = 300` µs) para verificar si las resistencias pull-up externas de 10 kΩ de un módulo MX mantienen la línea en alto frente al pull-down, o si un interruptor está pulsado / en cortocircuito a `GND`.
2. **Lectura de ID de módulo (`board_read_module_id`)**: Muestrea tensiones abiertas y bajo carga en el pin ADC de ID de módulo (`GPIO 8`) para clasificar `MX`, `HE`, `HE_V3`, `MX_V3` o `NONE` (se omite en HE V1 donde el DMA continuo de ADC1 ocupa ADC1, usando en su lugar el módulo detectado al arrancar).
3. **Sondeo de cable invertido (`board_probe_reversed_cable`)**: Cuando ningún módulo responde en `GPIO 8` y `GPIO 2` (`BOARD_REVERSE_PROBE_GPIO`) no es pin de tecla, sondea la presencia de un cable invertido.
4. **Prueba de continuidad y puentes de líneas libres V1 (`test_spares`)**: Para módulos V1 `MX` / `HE`, prueba `GPIO 6`, `GPIO 4` y `GPIO 2` con pull-up/pull-down y lleva pines vecinos a bajo de uno en uno para detectar cortocircuitos pin a pin (`bridged_gpios`).
5. **Prueba ADS7028 de HE V3 (`he_adc_test_run`)**: Para `BOARD_MODULE_HE_V3`, pausa el muestreador SPI en flujo (`he_backend_run_paused`) si está activo, reinicia y lee registros de estado del ADS7028 y muestrea los sensores.

## Modo banco Hall Effect (`hall_bench.c`)

Compilado únicamente cuando `CONFIG_OPAD_HALL_BENCH=y` (no es una compilación de lanzamiento; detén `opad-daemon` porque transmite texto plano por CDC) ([/firmware/main/diag/hall_bench.c](/firmware/main/diag/hall_bench.c)):

- Muestrea cuatro canales ADC1 a `BENCH_SAMPLE_HZ = 64000` Hz (`BENCH_FRAME_CONVS = 64`, tramas de 1 ms): Key 1 (`ADC_CHANNEL_9`), Key 2 (`ADC_CHANNEL_6`), ID de módulo (`ADC_CHANNEL_7`) y referencia (`ADC_CHANNEL_5`, `GPIO 6`).
- Emite líneas promediadas de 1 ms `H,<ms>,<key1>,<key2>`, resúmenes de dispersión por canal cada 1 s `S,<key>,<n>,<min>,<max>,<mean>,<stddev_x100>,<overflows>` y líneas de fase de ciclo de pull `P,<phase>` para `scripts/hall_bench.py`.
- También muestra una vista en vivo de tensión/barras para 2 teclas en la LCD (`hall_bench_show_ui`, temporizador de 33 ms, midiendo la posición de reposo durante los primeros `BENCH_REST_MS = 500` ms).
