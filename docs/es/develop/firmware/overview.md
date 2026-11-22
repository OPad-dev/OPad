---
title: Visión General y Arquitectura del Firmware
description: Arquitectura interna del firmware para ESP32-S3, topologías de tareas FreeRTOS, particionado de memoria, ajustes de sdkconfig y banderas de prueba en tiempo de compilación.
category: explanation
source_files:
  - /firmware/main/app_main.c
  - /firmware/main/runtime/runtime.c
  - /firmware/sdkconfig.defaults
  - /firmware/main/Kconfig.projbuild
---

El firmware de OPad está escrito en C99 utilizando ESP-IDF **v5.5.2** sobre el microcontrolador de doble núcleo ESP32-S3 (Xtensa LX7 a 240 MHz). Está estructurado en torno a un aislamiento estricto de hardware: el escaneo de entrada crítico en el tiempo y el envío de informes USB HID se ejecutan en Core 0, mientras que el renderizado de pantalla, el enmarcado del protocolo serie CDC, los gestos táctiles y la gestión de almacenamiento NVS se ejecutan en Core 1. <!-- src: /firmware/main/app_main.c -->

## FreeRTOS Task Layout and Priorities

Las tareas están asignadas estáticamente a núcleos de CPU y priorizadas para asegurar que el procesamiento de pulsaciones nunca sea interrumpido por tareas secundarias en segundo plano: <!-- src: /firmware/sdkconfig.defaults -->

| Nombre de Tarea | ID Núcleo | Prioridad | Tamaño Pila | Módulo Fuente | Propósito |
|---|---|---|---|---|---|
| **`keypad_task`** | Core 0 | `MAX_PRIORITIES - 1` | 4096 B | `input/keypad.c` | Antirrebote mecánico por flancos y eventos de teclas. <!-- src: /firmware/main/input/keypad.c --> |
| **`he_task`** | Core 0 | `MAX_PRIORITIES - 1` | 4096 B | `input/he_backend.c` | Conversión ADC de Hall Effect y pasos de estado de Rapid Trigger. <!-- src: /firmware/main/input/he_backend.c --> |
| **`tusb`** | Core 0 | `MAX_PRIORITIES - 1` | Por Defecto | Pila TinyUSB | Envío de informes de endpoint USB HID y sondeo SOF a 1000 Hz. <!-- src: /firmware/main/app_main.c --> |
| **`cdc_proto`** | Core 1 | `IDLE + 3` | 6144 B | `usb/usb_cdc.c` | Enmarcado serie CDC-ACM, decodificación Nanopb proto. <!-- src: /firmware/main/usb/usb_cdc.c --> |
| **`touch_retry`** | Core 1 | `2` (Baja) | 3072 B | `input/touch_retry.c` | Lectura táctil I2C CST816S y seguimiento de gestos. <!-- src: /firmware/main/input/touch_retry.c --> |
| **`runtime_task`**| Core 1 | `IDLE + 1` | 4096 B | `runtime/runtime.c` | Watchdog de tiempo límite de mapa, volcados diferidos a NVS. <!-- src: /firmware/main/runtime/runtime.c --> |
| **`gui` / `lvgl`**| Core 1 | `IDLE + 2` | Dinámico | `ui/ui_store.c` | Cauce de pantalla LVGL, volcados SPI DMA ST7789. <!-- src: /firmware/main/ui/ui_store.c --> |

## Critical Configuration Options (sdkconfig.defaults)

Las invariantes de rendimiento del firmware se aplican mediante ajustes de Kconfig (`firmware/sdkconfig.defaults`): <!-- src: /firmware/sdkconfig.defaults -->

- **`CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y`**: Velocidad máxima de reloj de CPU (240 MHz). Los benchmarks demuestran que la latencia de envío de informes de entrada a USB baja de ~240 µs (a 160 MHz con tráfico de pantalla) a menos de 70 µs a 240 MHz.
- **`CONFIG_FREERTOS_HZ=1000`**: Frecuencia de ticks del SO a 1000 Hz permitiendo una resolución de planificación de FreeRTOS de 1 ms, coincidiendo con el intervalo de sondeo USB de 1000 Hz. <!-- src: /firmware/sdkconfig.defaults -->
- **`CONFIG_GPIO_CTRL_FUNC_IN_IRAM=y`**: Fija las funciones de acceso a registros GPIO en la rápida IRAM interna, eliminando fallos de caché de flash durante la gestión de interrupciones de interruptores. <!-- src: /firmware/sdkconfig.defaults -->
- **`CONFIG_TINYUSB=y`**: Utiliza el hardware nativo USB full-speed del ESP32-S3 con los controladores compuestos HID + CDC de TinyUSB.

## Memory Partitioning & Linker Rules

El mapa de memoria separa la RAM rápida interna (IRAM/DRAM) de la PSRAM SPI externa: <!-- src: /firmware/main/app_main.c -->
- **Ubicación en IRAM (`linker.lf`)**: Los controladores ISR de GPIO, las funciones de antirrebote anticipado (`debounce_step`), las interrupciones de trama de Hall Effect (`on_frame`) y los colectores de estadísticas de latencia (`latency_stats_record`) están fijados en IRAM.
- **Asignación en PSRAM (`CONFIG_OPAD_LVGL_PSRAM`)**: Los grupos de memoria LVGL y los búferes de pantalla RGB565 de 320x240 se asignan desde la PSRAM externa de 2 MB (`opad_lvgl_pool_alloc`), preservando la SRAM interna para el heap de FreeRTOS y las pilas de las tareas. <!-- src: /firmware/CMakeLists.txt -->

## Compile-Time Benchmarking & Diagnostics Flags

`firmware/main/Kconfig.projbuild` proporciona interruptores de prueba para aislar subsistemas durante la validación de latencia: <!-- src: /firmware/main/Kconfig.projbuild -->

- **`CONFIG_OPAD_BENCH_HID_ONLY`**: Desactiva LVGL, las tareas serie CDC y los supervisores de runtime para medir la latencia base pura del hardware de entrada (pruebas de Etapa A). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_BENCH_NO_DISPLAY`**: Ejecuta las comunicaciones CDC y la sincronización de telemetría desactivando las transferencias SPI DMA al LCD (pruebas de Etapa B). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_TEST_FAIL_LCD`**: Fuerza que la inicialización de la pantalla falle, verificando el comportamiento de reserva del teclado sin pantalla (Regla de Producto 2). <!-- src: /firmware/main/Kconfig.projbuild -->
- **`CONFIG_OPAD_TEST_FAIL_NVS`**: Simula el fallo de la memoria flash SPI para verificar el seguimiento de contadores volátil solo en RAM. <!-- src: /firmware/main/Kconfig.projbuild -->
