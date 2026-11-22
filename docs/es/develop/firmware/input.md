---
title: Subsistema de entrada del firmware
description: Referencia de implementación de la tubería de entrada del firmware, antirrebote mecánico inmediato, muestreo Hall Effect V1/V3 y despacho unificado de eventos de tecla.
---

El subsistema de entrada lee interruptores mecánicos físicos y sensores Hall Effect, ejecuta el antirrebote o Rapid Trigger, actualiza los contadores monotónicos de pulsaciones y envía reportes USB HID. Todas las tareas de la ruta de teclas se ejecutan en **Core 0** con prioridad `configMAX_PRIORITIES - 1`.

## Tubería de teclado mecánico (`keypad.c` y `debounce.c`)

Implementada en [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c) y [/firmware/main/input/debounce.c](/firmware/main/input/debounce.c):

- **ISR GPIO (`gpio_isr_handler`, `IRAM_ATTR`)**: Se dispara en los flancos de interruptor configurados (por defecto `GPIO 14` para Key 1, `GPIO 9` para Key 2; carrier MX V3 usa `GPIO 10`, `GPIO 7`, `GPIO 6`). Marca el flanco con `esp_timer_get_time()`, llama a `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)`, incrementa los contadores aceptados vía `key_event_press_accepted` y despierta `keypad_task` mediante `vTaskNotifyGiveFromISR`.
- **Antirrebote inmediato (`debounce_step`)**:
  - `DEBOUNCE_SOURCE_EDGE`: Si `level != state->accepted_pressed` y `now_us >= state->lockout_end_us`, acepta la transición de inmediato (`DEBOUNCE_ACTION_PRESS` o `DEBOUNCE_ACTION_RELEASE`) y establece `lockout_end_us = now_us + lockout_us` (`DEBOUNCE_DEFAULT_US = 5000`, rango `500`–`20000` µs en [/firmware/main/input/debounce.h](/firmware/main/input/debounce.h)).
  - `DEBOUNCE_SOURCE_RESAMPLE`: Ejecutado por `keypad_task` al expirar `lockout_end_us`; lee el GPIO una vez y corrige `accepted_pressed` (iniciando un nuevo bloqueo) si el pin quedó en el estado opuesto.
- **Preparación diferida de configuración**: `keypad_set_config` prepara los cambios de pin/código de tecla y los aplica en `keypad_task` solo cuando las teclas están sueltas, de modo que los códigos nunca cambian en mitad de una pulsación mantenida y solo `keypad_task` toca los GPIOs de teclas.
- **Detección de pines y prueba de líneas**: `keypad_task` también atiende la detección interactiva de pines (`detect_scan_step`, `DETECT_CONFIRM_US = 15000` µs) y `keypad_line_test` (comprobación pull-up/pull-down para `board_test.c`).

## Tubería Hall Effect (`he_backend.c`, `he_spi_source.c`, `he_rt.c`)

Implementada en [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c) y [/firmware/main/input/he_rt.c](/firmware/main/input/he_rt.c):

- **ADC interno V1 (`HE_SOURCE_INTERNAL_ADC`)**: DMA continuo de ADC1 alternando `ADC_CHANNEL_9` (Key 1) y `ADC_CHANNEL_6` (Key 2) a `HE_SAMPLE_HZ = 64000` Hz (`HE_FRAME_CONVS = 16`, tramas de 250 µs / 4 kHz). La ISR en IRAM `on_frame` calcula la mediana de cada canal, ejecuta `he_rt_step` y notifica a `he_task` solo en flancos de estado de tecla.
- **ADS7028 externo V3 (`HE_SOURCE_SPI_ADC`)**: Un `gptimer` hardware despierta la tarea `he_spi` de Core 0 cada `HE_SPI_FRAME_US = 50` µs (20 kHz). `he_spi` lee 4 palabras de 32 bits protegidas con CRC por SPI3 a 40 MHz (`SPI_DMA_DISABLED`, bucle directo sobre registros `GPSPI3` `ads7028_read16_again` en [/firmware/main/input/ads7028.c](/firmware/main/input/ads7028.c)) para `AIN3`/`AIN4`/`AIN5` (Keys 3/2/1), ejecuta `he_rt_step` y llama directamente a `key_event_report`. La captura de calibración se diezma 5:1 (`SPI_CAL_DECIMATE = 5`) para igualar el corredor a 4 kHz.
- **Rapid Trigger de 3 estados (`he_rt_step`)**: Transiciona entre `HE_RT_ARMED`, `HE_RT_PRESSED` y `HE_RT_RT_RELEASED`, exigiendo `need_release` tras el arranque, cambios de parámetros o paradas del ADC (`he_rt_fault`).

## Despacho unificado de eventos de tecla (`key_event.c`)

Tanto el backend mecánico como el Hall Effect convergen en [/firmware/main/input/key_event.c](/firmware/main/input/key_event.c):

- **`key_event_press_accepted` (`IRAM_ATTR`)**: Incrementa atómicamente `s_lifetime_presses[key_index]` y `s_map_presses[key_index]`, registra `s_last_press_us` y empuja las pulsaciones de teclas de juego (filtradas por `s_game_keys`) a `press_log_push`.
- **`key_event_report` (`IRAM_ATTR`)**: Llama primero a `usb_hid_handle_key_event(key_index, pressed, edge_us)` en [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c); registra la latencia de entrega vía `latency_stats_record` (o `latency_stats_record_deferred` cuando el endpoint está ocupado), conmuta `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM` si está habilitado y notifica actividad a la interfaz (`ui_notify_activity`) al pulsar.
