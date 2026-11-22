---
title: Arquitectura del sistema
description: Arquitectura general de OPad, incluyendo el aislamiento de tareas en doble núcleo del ESP32-S3, los estados de ejecución del firmware y el modelo de actor del daemon de escritorio.
---

OPad está organizado alrededor de **la latencia siempre gana** ([/AGENTS.md](/AGENTS.md) §1.1): el escaneo de teclas y los reportes USB HID a 1000 Hz se ejecutan en Core 0 sin asignaciones en el heap, sin bloqueos compartidos con Core 1 y sin escrituras en flash durante la partida. La pantalla LCD, la persistencia NVS, los gestos táctiles y la telemetría de `tosu` permanecen fuera de la ruta de pulsación.

## Aislamiento de hardware en doble núcleo (ESP32-S3)

El ESP32-S3 ejecuta dos núcleos Xtensa LX7 a 240 MHz (`CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ_240=y` en [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)). Las tareas de FreeRTOS se fijan de forma asimétrica en [/firmware/main/app_main.c](/firmware/main/app_main.c):

```text
+-------------------------------------------------+-------------------------------------------------+
| CORE 0 (Input & USB HID)                        | CORE 1 (UI, CDC Serial, Runtime & Storage)      |
+-------------------------------------------------+-------------------------------------------------+
| • keypad_task / he_task / he_spi (MAX - 1)      | • cdc_proto / usb_cdc_task (IDLE + 3)           |
| • TinyUSB device task (MAX - 2)                 | • touch_retry_task (Priority 2)                 |
| • GPIO key ISRs / V1 ADC1 DMA / V3 SPI3 polling | • LVGL display task & ST7789 SPI2 flushes       |
| • IRAM key path, zero flash writes, zero malloc | • runtime_supervisor_task (IDLE + 1)            |
|                                                 | • Deferred NVS checkpoints & diagnostics        |
+-------------------------------------------------+-------------------------------------------------+
```

### Core 0: La ruta de pulsación
- **Tareas de entrada** (`keypad_task` en [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c), `he_task` en [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), `he_spi` en [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c)): Fijadas en Core 0 con prioridad `configMAX_PRIORITIES - 1`. Los interruptores mecánicos usan una ISR GPIO en IRAM con antirrebote inmediato; Hall Effect V1 usa DMA continuo del ADC1 interno (tramas de `4 kHz`), y Hall Effect V3 usa un `gptimer` hardware de 50 µs que sondea el ADS7028 externo por SPI3 a 40 MHz (`SPI_DMA_DISABLED`, lecturas directas de registros).
- **Tarea TinyUSB**: Fijada en Core 0 con prioridad `configMAX_PRIORITIES - 2` ([/firmware/main/app_main.c](/firmware/main/app_main.c)) para que solo las tareas de entrada tengan mayor prioridad. Controla los dos endpoints HID de 1000 Hz (`EP 0x81` y `EP 0x84`) y CDC-ACM.
- **Fijación en IRAM por el enlazador**: [/firmware/main/linker.lf](/firmware/main/linker.lf) fija `keypad_task`, `usb_hid`, `he_frame`, `he_rt`, `he_spi_source:sampler_task`, `he_spi_frame` y la ruta crítica HID/DWC2 de TinyUSB en IRAM (`noflash`) para que el tráfico de pantalla/flash en Core 1 no cause paradas de caché de instrucciones.

### Core 1: Servicios del sistema e interfaz
- **`cdc_proto` (`usb_cdc_task`)**: Fijada en Core 1 (`tskIDLE_PRIORITY + 3`) en [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c); decodifica mensajes del protocolo nanopb entramados y despacha las respuestas.
- **`touch_retry` (`touch_retry_task`)**: Fijada en Core 1 (prioridad 2) en [/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c); duerme sobre la interrupción del CST816S y sondea I2C únicamente mientras hay un dedo apoyado.
- **Pantalla y LVGL**: Renderiza diseños `320x240` hacia la ST7789 por SPI2 a 80 MHz hasta a 30 fps (`CONFIG_LV_DEF_REFR_PERIOD=33`), con el pool de 160 KB de LVGL y los búferes de UI/protocolo fuera de la ruta de teclas en los 8 MB de Octal PSRAM (`CONFIG_OPAD_LVGL_PSRAM=y`, `EXT_RAM_BSS_ATTR`) para que la RAM interna quede reservada a la entrada en Core 0, USB y los dos búferes de dibujo DMA de 40 líneas (`UI_DRAW_BUF_LINES = 40`), respaldados por 32 KB de caché de instrucciones y 64 KB de caché de datos ([/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c), [/firmware/main/ui/ui.h](/firmware/main/ui/ui.h), [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)).
- **`runtime_task` (`runtime_supervisor_task`)**: Fijada en Core 1 (`tskIDLE_PRIORITY + 1`) en [/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c); gestiona las transiciones de estado de juego, temporizadores y volcados diferidos a NVS.

## Máquina de estados de ejecución del firmware

[/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c) garantiza el aislamiento de escrituras en flash mediante tres estados de ejecución:

```text
         Host sends HostStatus / gameplay heartbeat
      +---------------------------------------------+
      |                                             |
      v                                             |
+----------+      Map Starts               +---------------+     Map Ends / 3 s Timeout
|   IDLE   | ----------------------------> |    PLAYING    | ----------------------------+
+----------+                               +---------------+                             |
      ^                                                                                  |
      |                          5 s Cooldown (COOLDOWN_DURATION_US)                     |
      +----------------------------------------------------------------------------------+
                                            COOLDOWN
```

1. **`OPAD_STATE_IDLE`**: Estado predeterminado. Muestra el diseño en reposo; la configuración, la calibración y los guardados de contadores pueden escribir en NVS.
2. **`OPAD_STATE_PLAYING`**: Se entra cuando el anfitrión informa partida activa. Todas las escrituras en NVS están bloqueadas; los cambios de configuración y contadores permanecen en RAM y se marcan como pendientes.
3. **`OPAD_STATE_COOLDOWN`**: Periodo de gracia de 5 segundos (`COOLDOWN_DURATION_US = 5,000,000` µs) tras terminar un mapa para que los reintentos rápidos no disparen escrituras en NVS. Volver a `IDLE` vuelca los cambios diferidos.
4. **Watchdog del anfitrión**: Si los latidos de partida se detienen durante 3 segundos (`GAMEPLAY_TIMEOUT_US = 3,000,000` µs), el firmware sale de `PLAYING` hacia `COOLDOWN` e `IDLE`.

## Arquitectura del daemon de escritorio

`opad-daemon` ([/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs)) coordina la comunicación serie USB CDC, la persistencia en SQLite (`opad.db`), la supervisión de `tosu`, las actualizaciones y el IPC local para `opad-gui` y `opadctl`.

### Actor de dispositivo de escritor único
Todos los comandos anfitrión-a-teclado del bucle de ejecución pasan por una única tarea ordenada en [/desktop/daemon/src/device_actor.rs](/desktop/daemon/src/device_actor.rs):
- Canal acotado de profundidad `QUEUE_DEPTH = 256` que transporta `DeviceCommand` (`TimeSync`, `ClaimOwnership`, `Config`, `HostStatus`, `DataUpdate`, `Layout`, `RequestStatus`, `PlayAnimation`).
- Cada comando se envía y espera secuencialmente para que los mensajes de configuración, diseño y telemetría nunca se adelanten entre sí.
- Los errores transitorios `DeviceError::Busy` se reintentan hasta `BUSY_RETRIES = 5` veces con `BUSY_BACKOFF = 20 ms`, salvo las diferencias obsoletas de `DataUpdate`, que en su lugar activan `resend_data` para que el siguiente volcado envíe una instantánea nueva.

### Supervisión de `tosu` y guardia de almacenamiento
`opad-daemon` supervisa `tosu` mediante [/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs), convierte las diferencias de telemetría WebSocket v2 en mensajes `DataUpdate` y `HostStatus` con límite de frecuencia ([/desktop/daemon/src/telemetry.rs](/desktop/daemon/src/telemetry.rs)), y refleja la ventana `Playing` / `Cooldown` del teclado desactivando las escrituras en SQLite (`Storage::set_writes_allowed(false)`) hasta que termina el enfriamiento post-partida.
