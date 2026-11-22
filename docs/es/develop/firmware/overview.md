---
title: Visión general y arquitectura del firmware
description: Arquitectura interna del firmware ESP32-S3, tareas y prioridades de FreeRTOS, colocación en IRAM por el enlazador, distribución de memoria y opciones Kconfig de prueba.
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
- `CONFIG_SPIRAM=y`, `CONFIG_SPIRAM_MODE_OCT=y`, `CONFIG_SPIRAM_SPEED_80M=y`: Habilita los 8 MB de Octal PSRAM de la placa.
- `CONFIG_LV_MEM_SIZE_KILOBYTES=160`: Pool dedicado de 160 KB para LVGL para que las asignaciones de UI nunca compitan con el heap del sistema.
- `CONFIG_PARTITION_TABLE_CUSTOM=y` (`partitions.csv`) y `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`: Dos ranuras OTA de 2 MB (`ota_0`, `ota_1`) con reversión hasta que USB se monte.

## Colocación en IRAM y distribución de memoria

- **Fragmento del enlazador IRAM ([/firmware/main/linker.lf](/firmware/main/linker.lf))**: Coloca `keypad:keypad_task`, `usb_hid`, `he_frame`, `he_rt`, `he_spi_source:sampler_task`, `he_spi_frame` y la ruta caliente HID/USBD/`dcd_dwc2`/`tu_hwfifo_write` de TinyUSB en IRAM (`noflash`) para que la actividad de caché en Core 1 nunca retrase la entrega de teclas.
- **Pool de LVGL y búferes de dibujo**: Por defecto (`CONFIG_OPAD_LVGL_PSRAM=n`), el pool de 160 KB de LVGL reside en RAM interna; activar `CONFIG_OPAD_LVGL_PSRAM` mueve el pool de LVGL a PSRAM manteniendo siempre los búferes de dibujo LCD en RAM interna compatible con DMA ([/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild)).

## Opciones Kconfig en tiempo de compilación (`Kconfig.projbuild`)

Definidas en [/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild) bajo **OPad Testing and Benchmark**:
- `CONFIG_OPAD_TEST_FAIL_LCD` / `CONFIG_OPAD_TEST_FAIL_NVS`: Compilaciones de inyección de fallos para verificar la operación como teclado sin pantalla y contadores solo en RAM.
- `CONFIG_OPAD_BENCH_HID_ONLY` / `CONFIG_OPAD_BENCH_NO_DISPLAY`: Modos de referencia de latencia Etapa A (solo HID + teclado) y Etapa B (CDC activo, pantalla desactivada).
- `CONFIG_OPAD_BENCH_DEBUG_GPIO` / `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM` (por defecto `4`): Conmuta un GPIO justo tras enviar HID para correlación con analizador lógico.
- `CONFIG_OPAD_HE_BACKEND`: Habilita la entrada Hall Effect V1 por ADC1 DMA (`select ADC_CONTINUOUS_ISR_IRAM_SAFE`).
- `CONFIG_OPAD_HALL_BENCH` / `CONFIG_OPAD_HE_TRACE` / `CONFIG_OPAD_HE_DEBUG`: Flujo crudo de sensores por CDC, captura de trazas de ~150 KB/s y registro diagnóstico `+hedebug` por pulsación.
- `CONFIG_OPAD_UI_PERF_TEST` / `CONFIG_OPAD_LVGL_PSRAM`: Bucle sintético de rendimiento de UI con superposiciones de FPS/memoria y pool LVGL opcional en PSRAM.
- `CONFIG_OPAD_DEBUG_I2C_SCAN`: Sondea las 126 direcciones I2C del bus táctil al arrancar (~1.3 s adicionales de arranque).
