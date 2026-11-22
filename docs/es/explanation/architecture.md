---
title: Arquitectura del Sistema
description: Arquitectura integral de OPad, detallando el aislamiento de hardware de doble núcleo en ESP32-S3, tareas de firmware, máquinas de estado del daemon de escritorio y comunicación secuenciada con el dispositivo.
---

OPad está diseñado con un principio rector fundamental: **la latencia siempre gana** (Regla de Producto 1). Cada capa de hardware, firmware y arquitectura de escritorio está diseñada para garantizar que los contactos de interruptores mecánicos y las actuaciones de Hall Effect lleguen al anfitrión USB en intervalos sub-milisegundo y sin fluctuaciones (jitter). Nada se interpone en la ruta de pulsación de teclas: la pantalla LCD, el almacenamiento flash (NVS), los gestos de la pantalla táctil y la integración de juego de tosu permanecen estrictamente aislados. <!-- src: /README.md -->

## Dual-Core Hardware Isolation (ESP32-S3)

El ESP32-S3 contiene dos núcleos Xtensa LX7 que funcionan a 240 MHz. OPad utiliza la asignación asimétrica de tareas (task pinning) en FreeRTOS para evitar que las operaciones en segundo plano se apropien de la CPU e interrumpan el escaneo de teclas o los informes USB HID. <!-- src: /firmware/main/app_main.c -->

```
+-------------------------------------------------+-------------------------------------------------+
| CORE 0 (Entrada y USB HID)                      | CORE 1 (UI, Serie CDC, Runtime y Almacenamiento)|
+-------------------------------------------------+-------------------------------------------------+
| • Tarea Keypad / HE (Prioridad MAX - 1)         | • Tarea TinyUSB CDC (Prioridad IDLE + 3)        |
| • Tarea TinyUSB Core (Prioridad MAX - 1)        | • Tarea Táctil CST816S (Prioridad 2)            |
| • ISRs de GPIO Directas / Sampler DMA ADS7028   | • Motor de Pantalla LVGL y PWM de Luz           |
| • Cero escrituras flash, cero logs, cero malloc | • Supervisor de Runtime (Prioridad IDLE + 1)    |
|                                                 | • Volcados NVS Diferidos y Diagnósticos         |
+-------------------------------------------------+-------------------------------------------------+
```

### Core 0: The Keypress Path
Core 0 controla el cauce de entrada crítico en el tiempo: <!-- src: /firmware/main/app_main.c -->
- **`keypad_task` / `he_task`**: Asignadas a Core 0 con prioridad `configMAX_PRIORITIES - 1`. Para interruptores mecánicos, responde directamente a las interrupciones GPIO disparadas por flanco y ejecuta un antirrebote anticipado (eager debounce) instantáneo. Para módulos Hall Effect, muestrea conversiones ADC a alta velocidad y calcula las transiciones de estado de Rapid Trigger. <!-- src: /firmware/main/input/keypad.c#keypad_task -->
- **`tinyusb_task`**: Asignada a Core 0 con alta prioridad (`tusb_cfg.task.xCoreID = 0`). Gestiona el sondeo del endpoint USB HID a 1000 Hz, enviando inmediatamente informes de teclado estándar de 8 bytes cada vez que cambia el estado de un interruptor. <!-- src: /firmware/main/app_main.c -->
- **Invariantes de Ejecución**: El código que corre en Core 0 nunca realiza asignaciones dinámicas en el heap (`malloc`), nunca toma mutexes bloqueantes en poder de Core 1 y nunca interactúa con la memoria flash SPI (NVS).

### Core 1: System Services & UI
Core 1 gestiona todas las funciones auxiliares no críticas: <!-- src: /firmware/main/runtime/runtime.c -->
- **`cdc_proto` (`usb_cdc_task`)**: Asignada a Core 1 (`tskIDLE_PRIORITY + 3`). Codifica y decodifica mensajes de Protocol Buffers sobre la interfaz serie CDC-ACM. <!-- src: /firmware/main/usb/usb_cdc.c#usb_cdc_task -->
- **`touch_retry` (`touch_retry_task`)**: Asignada a Core 1 (`Prioridad 2`). Sondea el controlador táctil I2C CST816S durante los contactos activos y detecta gestos de deslizamiento. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->
- **Motor de Pantalla**: Controla la pantalla LCD ST7789 de 320x240 utilizando LVGL. Los refrescos de pantalla y las transferencias SPI DMA se ejecutan en Core 1, completamente fuera de la ruta de entrada.
- **`runtime_task` (`runtime_supervisor_task`)**: Asignada a Core 1 (`tskIDLE_PRIORITY + 1`). Supervisa los estados del dispositivo, comprueba tiempos de inactividad de mapas y ejecuta escrituras diferidas en NVS al regresar a `IDLE`. <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## Firmware Runtime State Machine

El supervisor de ejecución del firmware (`firmware/main/runtime/runtime.c`) rastrea el estado activo del jugador para aplicar reglas estrictas de protección de la memoria flash: <!-- src: /firmware/main/runtime/runtime.c#s_state -->

```
        El host envía GameplayDisplayState (>= 1 Hz)
     +---------------------------------------------+
     |                                             |
     v                                             |
+----------+      Primer Paquete del Mapa  +---------------+      Fin del Mapa / Parada
|   IDLE   | ----------------------------> |    PLAYING    | -------------------------+
+----------+                               +---------------+                          |
     ^                                                                                |
     |                       Ventana de Enfriamiento de 5 segundos                    |
     +--------------------------------------------------------------------------------+
                                         COOLDOWN
```

### State Definitions
1. **`OPAD_STATE_IDLE`**: Estado de reposo por defecto. El pad muestra el diseño de pantalla inactivo. Las actualizaciones de configuración recibidas por serie se guardan inmediatamente en flash NVS. <!-- src: /firmware/main/runtime/runtime.c#OPAD_STATE_IDLE -->
2. **`OPAD_STATE_PLAYING`**: Sesión activa de beatmap de osu! en curso. El pad pasa a `PLAYING` al recibir el primer paquete `GameplayDisplayState` del daemon del anfitrión. Todas las escrituras en flash (NVS) se bloquean estrictamente para evitar paradas por invalidación de caché de CPU. Las actualizaciones de configuración se aplican solo en RAM y se marcan como pendientes (dirty). <!-- src: /firmware/main/runtime/runtime.c#OPAD_STATE_PLAYING -->
3. **`OPAD_STATE_COOLDOWN`**: Periodo de gracia de 5 segundos tras finalizar el mapa (`COOLDOWN_DURATION_US = 5,000,000`). Evita que los reintentos inmediatos o reinicios de canciones causen picos de latencia por escritura. Una vez transcurridos los 5 segundos sin nuevos paquetes de juego, el pad regresa a `IDLE` y vuelca los ajustes pendientes a NVS. <!-- src: /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

### Host Watchdog
Si el daemon del anfitrión se bloquea o `tosu` se cierra inesperadamente durante un mapa, el watchdog del firmware (`GAMEPLAY_TIMEOUT_US = 3,000,000`) detecta la pérdida del latido de juego a 1 Hz tras 3 segundos y regresa automáticamente el pad a `COOLDOWN` e `IDLE`. <!-- src: /firmware/main/runtime/runtime.c#GAMEPLAY_TIMEOUT_US -->

## Desktop Daemon Architecture

El daemon de escritorio (`opad-daemon`) actúa como coordinador central entre el hardware físico, el proceso de osu! (vía `tosu`) y las interfaces de usuario (`opad-gui` y `opadctl`). <!-- src: /desktop/daemon/src/runtime.rs -->

### The Device Actor Pattern
En implementaciones iniciales, múltiples tareas asíncronas escribían de forma concurrente en el puerto serie, causando condiciones de carrera y entrega de tramas fuera de orden. En la arquitectura actual, toda la comunicación del anfitrión hacia el dispositivo se serializa a través de un actor dedicado (`desktop/daemon/src/device_actor.rs`): <!-- src: /desktop/daemon/src/device_actor.rs -->

- **Cola de Comandos Ordenada**: Los comandos (`DeviceCommand`) se colocan en un canal acotado de 256 elementos (`QUEUE_DEPTH = 256`). <!-- src: /desktop/daemon/src/device_actor.rs#QUEUE_DEPTH -->
- **Despacho Monohilo**: La tarea del actor extrae comandos secuencialmente, espera la transmisión serie y el acuse de recibo, y gestiona los estados de ocupado transitorios con retroceso exponencial (`BUSY_RETRIES = 5`, `BUSY_BACKOFF = 20ms`). <!-- src: /desktop/daemon/src/device_actor.rs#BUSY_RETRIES -->
- **Eliminación de Carreras**: Las diferencias de telemetría, los cambios de diseño y las escrituras de configuración siempre llegan al pad en un orden predecible.

### Supervision of tosu
`opad-daemon` supervisa `tosu` como un proceso hijo: <!-- src: /desktop/daemon/src/runtime.rs -->
- Inicia `tosu` desde el directorio privado de bibliotecas (`bundled_tosu_binary`).
- Sondea la interfaz WebSocket de `tosu` para obtener métricas de juego en vivo (metadatos de canciones, precisión, combo, UR, KPM).
- Traduce los eventos del juego en envoltorios protobuf `GameplayDisplayState` y `DataUpdate`, reenviándolos a través del actor del dispositivo hacia el motor de pantalla del pad.
- Cuando los beatmaps comienzan o terminan, el daemon reproduce la máquina de estados del firmware, respetando `COOLDOWN_DURATION` antes de confirmar registros en la base de datos SQLite. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->
