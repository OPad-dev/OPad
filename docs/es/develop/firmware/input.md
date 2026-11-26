---
title: Subsistema de entrada del firmware
description: Referencia de implementación de la cadena de entrada del firmware, el antirrebote mecánico inmediato, la Key 3, el muestreo Hall Effect V1/V3 y Rapid Trigger, el bloqueo por calibración, la vigilancia del módulo y el despacho unificado de eventos de tecla.
sidebar:
  order: 2
---

El subsistema de entrada lee interruptores mecánicos o sensores Hall Effect, decide las pulsaciones (antirrebote o Rapid Trigger), las cuenta y entrega cada cambio a la capa USB HID. Todas las tareas de esta ruta se ejecutan en el **núcleo 0** a `configMAX_PRIORITIES - 1`, desde IRAM ([visión general](overview.md#la-ruta-de-una-pulsación)). Fuente: [/firmware/main/input/](https://github.com/OPad-dev/OPad/tree/main/firmware/main/input).

> [!IMPORTANT]
> Cualquier cambio bajo `firmware/main/input/` debe decir cómo afecta a la latencia de las teclas y cómo se comprobó ([/AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1), y necesita una prueba en un pad real ([Lista de pruebas](../../testing-checklist.md) §1).

## Qué fuente de teclas se ejecuta

`app_main` elige la fuente de teclas una sola vez, al arrancar, a partir del ID del módulo (`board_detect_module()` en [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c), umbrales en [/firmware/main/input/module_id.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_id.h)):

| Módulo (`board_module_type_t`) | Tensión de ID | Las teclas vienen de |
|---|---|---|
| `BOARD_MODULE_NONE` (cableado a mano) | ~0 V | `keypad.c` en los pines configurados (por defecto GPIO14 / GPIO9) |
| `BOARD_MODULE_MX` (MX V1) | ~0,30 V | `keypad.c`; los pines por defecto pasan a GPIO10 / GPIO7 (`BOARD_MX_KEY1_GPIO` / `BOARD_MX_KEY2_GPIO`) |
| `BOARD_MODULE_MX_V3` | ~2,24 V | `keypad.c` en GPIO10 / GPIO7, más la Key 3 en GPIO6 (`keypad_set_key3_gpio(BOARD_MX3_KEY3_GPIO)`); teclas desactivadas si el cable está dado la vuelta |
| `BOARD_MODULE_HE` (HE V1) | ~1,06 V | `he_backend.c` con `HE_SOURCE_ESP_ADC`, solo si se compila con `CONFIG_OPAD_HE_BACKEND`; si no, teclas desactivadas con un aviso en la LCD |
| `BOARD_MODULE_HE_V3` | ~1,65 V | `he_backend.c` con `HE_SOURCE_ADS7028` (siempre compilado); los pines de tecla son las líneas SPI del módulo |

En los módulos HE y en los no soportados, el teclado se desactiva antes de armar su ISR (`keypad_set_input_enabled(false)`) y los pines se dejan en modo analógico o desconectados, para que las salidas analógicas de los sensores nunca puedan escribir.

## Teclado mecánico (`keypad.c`, `debounce.c`)

[/firmware/main/input/keypad.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/keypad.c), [/firmware/main/input/debounce.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.c):

- **ISR (`gpio_isr_handler`, `IRAM_ATTR`)**: ante cualquier flanco de un pin de tecla, toma `esp_timer_get_time()`, lee el pin y llama a `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)` bajo `s_keypad_spinlock`. Una pulsación aceptada llama a `key_event_press_accepted()` ahí mismo; cualquier cambio aceptado despierta a `keypad_task` con `vTaskNotifyGiveFromISR`. La ISR no envía reports HID.
- **Antirrebote inmediato (`debounce_step`, C puro, probado en el host)**:
  - `DEBOUNCE_SOURCE_EDGE`: un nivel distinto de `accepted_pressed` fuera del bloqueo se acepta al instante (`DEBOUNCE_ACTION_PRESS` / `DEBOUNCE_ACTION_RELEASE`) e inicia un bloqueo de `lockout_us`; los flancos dentro de él se ignoran. Rango de `DEBOUNCE_MIN_US = 500` a `DEBOUNCE_MAX_US = 20000`, por defecto `DEBOUNCE_DEFAULT_US = 5000` ([/firmware/main/input/debounce.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.h)).
  - `DEBOUNCE_SOURCE_RESAMPLE`: cuando termina el bloqueo, `keypad_task` lee el pin una vez; si se ha quedado en el estado contrario a `accepted_pressed`, se aplica esa transición (y se cuenta, si es una pulsación) con un bloqueo nuevo.
- **Bucle de `keypad_task`**: duerme hasta que lo notifican o hasta que termina el bloqueo más próximo, y entonces (1) remuestrea los bloqueos vencidos, (2) llama a `key_event_report()` para cada tecla cuyo estado difiere del informado, (3) aplica una configuración escalonada, (4) avanza la detección de pines y (5) ejecuta una prueba de líneas si se ha pedido.
- **Configuración escalonada (`keypad_set_config`)**: los cambios de keycode y de pines esperan a que se suelten todas las teclas, de modo que una tecla mantenida nunca cambia de keycode y solo `keypad_task` toca los GPIO de las teclas. Si un cambio de pines falla, se mantienen armados los pines anteriores y se registra `DIAG_EVENT_CONFIG_REJECTED`.
- **Detección de pines (`detect_scan_step`)**: pull-ups en todos los pines libres permitidos (`KEY_GPIO_ALLOWED` en [/firmware/main/config/config_validate.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/config_validate.c)); gana el pin que se mantiene a nivel bajo durante `DETECT_CONFIRM_US = 15000` µs. Se sondea cada 10 ms entre eventos de tecla, así que las teclas siguen funcionando.
- **Prueba de líneas (`keypad_line_test`)**: cambia el pull-up de cada pin de tecla por un pull-down durante `LINE_TEST_SETTLE_US = 300` µs para detectar un pull-up externo o un cortocircuito, y después reinicia el antirrebote a partir de los pines. La usa la [prueba de placa](diag.md#autoprueba-de-las-pcb-de-la-placa-portadora-y-del-módulo-de-entrada-board_testc).

## Entrada Hall Effect (`he_backend.c`)

[/firmware/main/input/he_backend.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_backend.c) gestiona las dos fuentes HE y pasa cada trama por un único paso, `process_frame()`: mediana por tecla, registro de salud, `he_rt_step()` por tecla y `key_event_press_accepted()` en un evento de bajada. Registro de decisiones de diseño: [/docs/specs/v2-hall-effect-decisions.md](../../../specs/v2-hall-effect-decisions.md); conceptos: [Hall Effect y Rapid Trigger](../../explanation/hall-effect.md).

| | V1 (`HE_SOURCE_ESP_ADC`) | V3 (`HE_SOURCE_ADS7028`) |
|---|---|---|
| Muestreo | DMA continuo del ADC1, `ADC_CHANNEL_9` (Key 1, GPIO10) y `ADC_CHANNEL_6` (Key 2, GPIO7) alternando a `HE_SAMPLE_HZ = 64000` | Un GPTimer despierta a `he_spi` cada `HE_SPI_FRAME_US = 50` µs (20 kHz); autosecuencia del ADS7028 sobre AIN5 / AIN4 / AIN3 = Keys 1 / 2 / 3 |
| Trama | `HE_FRAME_CONVS = 16` (8 por tecla), una trama cada 250 µs; mediana de las lecturas válidas (`he_frame.c`) | 4 conversiones leídas a 40 MHz por SPI3 (sin DMA; tras la primera lectura, directamente sobre los registros mediante `ads7028_read16_again`), descartando la primera por obsoleta; cada una con CRC comprobado (`ads7028.c`) y asignada a su tecla por el ID de canal (`he_spi_frame.c`) |
| Quién informa | La ISR del ADC en IRAM, `on_frame`, avanza el motor y notifica a `he_task` solo cuando cambia una tecla; `he_task` llama a `publish()` | La propia tarea del muestreador llama a `publish()` (`on_spi_frame`), evitando un traspaso que costaba ~100 µs en la cola |
| Teclas | 2 | 3 |
| Compilación | `CONFIG_OPAD_HE_BACKEND` | siempre |

- **`publish()`** (IRAM, bajo `s_publish_mutex`) llama a `key_event_report()` una vez por cada tecla que ha cambiado, en orden, desde la tarea que publique.
- **Salud (`he_health.c`, C puro)**: `he_task` comprueba cada 2 ms (`HE_CHECK_TICKS`). Una tecla sin lectura válida durante `HE_STALL_US = 5000` µs se suelta y se ignora; si no llega ninguna trama durante ese tiempo se sueltan todas las teclas y se reinicia la fuente con una espera creciente de 10 ms a 1 s (`HE_RESTART_BACKOFF_MIN_US` / `_MAX_US`). Las dos teclas ausentes durante `HE_ABSENT_US = 100 ms` significa que se ha quitado el módulo. Estos valores están marcados como provisionales (OPEN G) en el código.
- **Motor de Rapid Trigger (`he_rt.c`, C puro, probado en el host)**: estados `HE_RT_ARMED`, `HE_RT_PRESSED`, `HE_RT_RT_RELEASED`. El modo normal pulsa en `actuation` y suelta en `release`; Rapid Trigger suelta tras subir `rt_release` cuentas desde el pico y vuelve a pulsar tras bajar `rt_press` desde el valle mientras esté más hundida que `release` (o que `top_reset` en modo continuo). `need_release` hace que una tecla mantenida al arrancar, tras un cambio de parámetros o tras un fallo (`he_rt_fault`) tenga que subir antes de poder pulsar ([/firmware/main/input/he_rt.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_rt.h)).
- **Umbrales (`he_params.c`)**: los ajustes son pasos del 0,1 % del recorrido calibrado (`HE_PCT_FULL = 1000`), convertidos a cuentas en bruto fuera de la ruta de teclas y con un mínimo en un límite de ruido (`HE_PROVISIONAL_LIMITS`).

### Calibración y el bloqueo (decisión D7)

Un pad HE sin calibrar está bloqueado: `he_backend_reload_params()` da a cada tecla `HE_PARAMS_NEVER` (umbrales que nunca pulsan) mientras `he_cal_is_locked_decision()` sea verdadero o haya una calibración en curso, la pantalla muestra la superposición de bloqueo ([UI](ui.md#superposición-de-calibración-hall-effect-he_cal_uic)) y el táctil solo maneja el botón de esa pantalla ([táctil](touch.md#bloqueo-por-calibración)). Mientras está bloqueado no se envía ninguna tecla.

| Archivo | Función |
|---|---|
| [/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c), [he_cal_record.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_record.c) | Registros de calibración A/B en NVS ([configuración y NVS](config-and-nvs.md#registros-de-calibración-hall-effect)) |
| [/firmware/main/input/he_cal_runner.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_runner.c) | Etapas `REST`, `KEY1`, `KEY2`, `KEY3` (V3), `SUCCESS` / `FAILED`; 5 pulsaciones completas por tecla |
| [/firmware/main/input/he_cal_capture.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_capture.c) | Ventana de reposo (dispersión ≤ 36, media 1500..2500), mediana de 5 picos, `rest_pressed_margin = 72` |
| [/firmware/main/input/he_cal_button.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_button.c) | Detección de toque en el botón en pantalla: bajada y subida dentro, desplazamiento < `GESTURE_SWIPE_PX` |
| [/firmware/main/input/he_cal_barrier.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_barrier.h), [/firmware/main/usb/usb_report_barrier.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_report_barrier.c) | `he_backend_start_calibration()` suprime las teclas, espera (≤ 100 ms cada cosa) a que el publicador envíe las sueltas y a que se vacíen los callbacks de TinyUSB, y solo entonces empieza la captura; si algo falla, se restauran los umbrales anteriores |
| [/firmware/main/input/he_cal_touch_guard.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_touch_guard.h) | Rechaza un inicio mientras haya un toque en curso ("Release the touchscreen first") |

Las muestras de calibración llegan a 4 kHz con las dos fuentes: V3 deja pasar una trama de cada `HE_CAL_EVERY_V3 = 5`.

## Eventos de tecla unificados (`key_event.c`)

Todas las fuentes convergen en [/firmware/main/input/key_event.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/key_event.c):

- **`key_event_press_accepted()` (`IRAM_ATTR`)**: incrementos atómicos relajados de `s_lifetime_presses[k]` (64 bits) y `s_map_presses[k]` (32 bits, que el host pone a cero en cada intento), guarda `s_last_press_us[k]` y, para una tecla de juego (máscara `s_game_keys`, [/firmware/main/input/game_keys.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/game_keys.h)), añade la marca de tiempo al registro de pulsaciones.
- **`key_event_report()` (`IRAM_ATTR`)**: llama **primero** a `usb_hid_handle_key_event()` ([USB](usb.md#envío-hid-con-endpoints-separados-usb_hidc)); después, `latency_stats_record()` si ha ido bien o `latency_stats_record_deferred()` si el endpoint estaba ocupado, conmuta el GPIO de depuración del banco si está activado y, en una pulsación, llama a `ui_notify_activity()` (un único store atómico).
- **Registro de pulsaciones ([/firmware/main/input/press_log.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/press_log.c))**: anillo sin bloqueos de un solo productor con `PRESS_LOG_CAPACITY = 128` bajadas de tecla con marcas de tiempo del pad, que la tarea del protocolo vacía para la frecuencia de toques del host; si el registro está lleno, descarta la pulsación más reciente y la cuenta.
- **Estadísticas de latencia ([/firmware/main/input/latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c))**: histograma sin bloqueos de flanco a envío (p50/p99/p99.9/máx., número de diferidos), lo que muestra `opadctl latency`.

## Vigilancia del módulo (`module_monitor.c`, `module_watch.c`)

Un módulo cambiado en caliente no debe permitir que las salidas analógicas escriban. En los pads que no son MX, `module_mon` (núcleo 1, `tskIDLE_PRIORITY + 1`) comprueba la línea de ID cada 5 ms (`board_module_present_fast`) y desactiva las teclas tras `MODULE_WATCH_FAST_READS = 3` lecturas discrepantes seguidas; solo vuelven tras `MODULE_WATCH_RESTORE_READS = 3` lecturas completas del módulo del arranque. Un módulo distinto requiere reiniciar. Con la entrada HE en marcha, el ADC1 pertenece al backend, así que el ID solo se lee pausándolo (`he_backend_probe_module`) mientras las dos teclas están ausentes ([/firmware/main/input/module_monitor.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_monitor.c), [/firmware/main/input/module_watch.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/module_watch.h)).

## Ganchos del firmware de depuración

Las compilaciones con `CONFIG_OPAD_HE_DEBUG` registran cada pulsación HE con las lecturas previas (`he_debug.c`) o, en MX, cada toque y cada cambio de pin que el pad no envió (`mx_debug.c`). Fuera de esa compilación, cada gancho no hace nada ([/firmware/main/input/he_debug.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_debug.h), [/firmware/main/input/mx_debug.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/mx_debug.h)).
