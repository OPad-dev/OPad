---
title: Visión general y arquitectura del firmware
description: Arquitectura interna del firmware ESP32-S3, tareas y prioridades de FreeRTOS, colocación en IRAM por el enlazador, distribución de memoria y opciones Kconfig de prueba.
sidebar:
  order: 1
---

El firmware de OPad está escrito en C para ESP-IDF **v5.5.2** dirigido a la placa Waveshare ESP32-S3-Touch-LCD-2 (Xtensa LX7 de doble núcleo a 240 MHz, 16 MB Flash, 8 MB Octal PSRAM) ([/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults), [/firmware/main/app_main.c](/firmware/main/app_main.c)).

## Distribución y prioridades de tareas FreeRTOS

| Tarea | Núcleo | Prioridad | Pila | Fuente | Propósito |
|---|---|---|---|---|---|
| `keypad_task` | Core 0 | `configMAX_PRIORITIES - 1` | 4096 B | [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c) | Remuestreo de antirrebote mecánico, reportes HID, detección de pines, prueba de líneas |
| `he_task` | Core 0 | `configMAX_PRIORITIES - 1` | 4096 B | [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c) | Despacho de flancos Hall Effect V1, watchdog de parada, pausa para sondeo |
| `he_spi` | Core 0 | `configMAX_PRIORITIES - 1` | 3072 B | [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c) | Muestreador SPI3 ADS7028 a 50 µs de Hall Effect V3 y publicación directa de teclas |
| TinyUSB (`tusb`) | Core 0 | `configMAX_PRIORITIES - 2` | Por defecto TinyUSB | [/firmware/main/app_main.c](/firmware/main/app_main.c) | Endpoints USB HID a 1000 Hz (`0x81`, `0x84`) y CDC-ACM |
| `cdc_proto` | Core 1 | `tskIDLE_PRIORITY + 3` | 6144 B | [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c) | Recepción, despacho y transmisión del protocolo nanopb entramado |
| `touch_retry` | Core 1 | `2` | 3072 B | [/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c) | Sondeo I2C del CST816S con dedo apoyado, Quick Retry y acciones de deslizamiento |
| Tarea del puerto LVGL | Core 1 | Por defecto `esp_lvgl_port` | `esp_lvgl_port` | [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c) | `pad_timer_cb` cada 20 ms, renderizado ST7789 por SPI2, superposición de calibración HE |
| `runtime_task` | Core 1 | `tskIDLE_PRIORITY + 1` | 4096 B | [/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c) | Máquina de estados `IDLE`/`PLAYING`/`COOLDOWN` y volcados NVS diferidos |

## Ajustes críticos de `sdkconfig.defaults`

En [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults):
- `CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y`: Reloj de CPU a 240 MHz (junto con la colocación en IRAM de la ruta de teclas, reduce la latencia tecla-a-USB bajo carga de pantalla de ~240 µs a 160 MHz a ~70 µs).
- `CONFIG_FREERTOS_HZ=1000`: Tick del sistema operativo a 1 kHz alineado con el sondeo USB de 1 ms.
- `CONFIG_GPIO_CTRL_FUNC_IN_IRAM=y`: Coloca los ayudantes de control GPIO en IRAM para la ISR de teclas y `keypad_task`.
- `CONFIG_TINYUSB_HID_COUNT=2`, `CONFIG_TINYUSB_CDC_COUNT=1`, `CONFIG_TINYUSB_CDC_TX_BUFSIZE=2048`, `CONFIG_TINYUSB_SUSPEND_CALLBACK=y`.
- `CONFIG_SPIRAM=y`, `CONFIG_SPIRAM_MODE_OCT=y`, `CONFIG_SPIRAM_SPEED_80M=y`, `CONFIG_SPIRAM_ALLOW_BSS_SEG_EXTERNAL_MEMORY=y`: Habilita los 8 MB de Octal PSRAM de la placa y permite colocar búferes estáticos fuera de la ruta de teclas en PSRAM con `EXT_RAM_BSS_ATTR`.
- `CONFIG_LV_MEM_SIZE_KILOBYTES=160` y `CONFIG_LV_DEF_REFR_PERIOD=33`: Pool dedicado de 160 KB para LVGL (en PSRAM por defecto mediante `CONFIG_OPAD_LVGL_PSRAM=y`) y límite de refresco de pantalla de 30 fps (periodo de 33 ms).
- `CONFIG_ESP32S3_INSTRUCTION_CACHE_32KB=y`, `CONFIG_ESP32S3_DATA_CACHE_64KB=y`, `CONFIG_ESP32S3_DATA_CACHE_LINE_64B=y`: Cachés máximas del ESP32-S3 para acelerar el código/fuentes en flash y los búferes de LVGL en PSRAM.
- `CONFIG_PARTITION_TABLE_CUSTOM=y` (`partitions.csv`) y `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`: Dos ranuras OTA de 2 MB (`ota_0`, `ota_1`) con reversión hasta que USB se monte.

## Colocación en IRAM y distribución de memoria

- **Fragmento del enlazador IRAM ([/firmware/main/linker.lf](/firmware/main/linker.lf))**: Coloca `keypad:keypad_task`, `usb_hid`, `he_frame`, `he_rt`, `he_spi_source:sampler_task`, `he_spi_frame` y la ruta caliente HID/USBD/`dcd_dwc2`/`tu_hwfifo_write` de TinyUSB en IRAM (`noflash`) para que la actividad de caché en Core 1 nunca retrase la entrega de teclas.
- **División entre RAM interna y PSRAM**: Solo las teclas, el USB y los búferes DMA permanecen en RAM interna. Por defecto (`CONFIG_OPAD_LVGL_PSRAM=y`), el pool de 160 KB de LVGL reside en PSRAM, y los búferes estáticos fuera de la ruta de teclas (`s_layouts` en `ui_port.c`, `s_pending_layouts` en `ui_store.c`, `s_subjects`/`s_str_buf`/`s_str_prev` en `ui_data.c` y `s_parser`/`msg`/`layout` en `protocol.c`) se colocan en PSRAM mediante `EXT_RAM_BSS_ATTR`. Los dos búferes de dibujo LCD de 40 líneas (`UI_DRAW_BUF_LINES = 40`, `25.6 KB` cada uno) permanecen en RAM interna compatible con DMA ([/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild), [/firmware/main/ui/ui.h](/firmware/main/ui/ui.h), [/firmware/main/protocol/protocol.c](/firmware/main/protocol/protocol.c)). Al arrancar, [/firmware/main/app_main.c](/firmware/main/app_main.c) registra `DIAG_EVENT_MEMORY` con la RAM interna libre y el mayor bloque DMA libre.

## Opciones Kconfig en tiempo de compilación (`Kconfig.projbuild`)

Definidas en [/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild) bajo **OPad Testing and Benchmark**:
- `CONFIG_OPAD_TEST_FAIL_LCD` / `CONFIG_OPAD_TEST_FAIL_NVS`: Compilaciones de inyección de fallos para verificar la operación como teclado sin pantalla y contadores solo en RAM.
- `CONFIG_OPAD_BENCH_HID_ONLY` / `CONFIG_OPAD_BENCH_NO_DISPLAY`: Modos de referencia de latencia Etapa A (solo HID + teclado) y Etapa B (CDC activo, pantalla desactivada).
- `CONFIG_OPAD_BENCH_DEBUG_GPIO` / `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM` (por defecto `4`): Conmuta un GPIO justo tras enviar HID para correlación con analizador lógico.
- `CONFIG_OPAD_HE_BACKEND`: Habilita la entrada Hall Effect V1 por ADC1 DMA (`select ADC_CONTINUOUS_ISR_IRAM_SAFE`).
- `CONFIG_OPAD_HALL_BENCH` / `CONFIG_OPAD_HE_TRACE` / `CONFIG_OPAD_HE_DEBUG`: Flujo crudo de sensores por CDC, captura de trazas de ~150 KB/s y registro diagnóstico `+hedebug` por pulsación.
- `CONFIG_OPAD_UI_PERF_TEST` / `CONFIG_OPAD_LVGL_PSRAM` (por defecto `y`): Bucle sintético de rendimiento de UI con superposiciones de FPS/memoria y reporte de tiempos de fotograma `DIAG_EVENT_UI_PERF`; asignación en PSRAM para el pool de memoria de LVGL.
- `CONFIG_OPAD_DEBUG_I2C_SCAN`: Sondea las 126 direcciones I2C del bus táctil al arrancar (~1.3 s adicionales de arranque).
