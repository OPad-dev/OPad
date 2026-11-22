---
title: Arquitectura del Subsistema Daemon
description: Referencia de implementación para opad-daemon, la cola del device actor, el sondeador de telemetría, el despacho de IPC y la sincronización con almacenamiento SQLite.
category: reference
source_files:
  - /desktop/daemon/src/device_actor.rs
  - /desktop/daemon/src/runtime.rs
  - /desktop/daemon/src/telemetry.rs
  - /desktop/daemon/src/sync.rs
  - /desktop/daemon/src/ipc_handlers.rs
---

El binario `opad-daemon` es el supervisor central en segundo plano de la suite de escritorio de OPad. Gestiona la comunicación serie CDC, aplica una entrega ordenada de mensajes mediante un device actor, sondea estadísticas de canciones en vivo desde tosu, reconcilia los contadores de pulsaciones en SQLite y atiende solicitudes IPC de `opad-gui` y `opadctl`. <!-- src: /desktop/daemon/src/runtime.rs -->

## Device Actor Pattern (device_actor.rs)

Para eliminar condiciones de carrera entre tareas asíncronas concurrentes que envían paquetes serie al pad, todas las solicitudes del anfitrión al dispositivo se enrutan a través del device actor: <!-- src: /desktop/daemon/src/device_actor.rs -->

- **Canal de Comandos Acotado**: Los comandos (`DeviceCommand`) se colocan en un canal asíncrono de Tokio con capacidad de 256 (`QUEUE_DEPTH = 256`). <!-- src: /desktop/daemon/src/device_actor.rs#QUEUE_DEPTH -->
- **Despachador Único**: El bucle del actor extrae un comando a la vez, escribe la trama en el puerto serie, espera la confirmación del dispositivo y gestiona estados de puerto ocupado transitorios (`BUSY_RETRIES = 5`, `BUSY_BACKOFF = 20ms`). <!-- src: /desktop/daemon/src/device_actor.rs#BUSY_RETRIES -->
- **Orden Estricto**: Los diseños de pantalla, los cambios de configuración y las diferencias de telemetría del juego llegan al teclado en orden FIFO determinista.

## Runtime State Supervisor (runtime.rs)

El runtime del daemon rastrea la conexión del teclado físico, la salud de tosu y el estado de juego activo: <!-- src: /desktop/daemon/src/runtime.rs -->

- **Sondeo del Ciclo de Vida del Dispositivo**: Escanea los dispositivos del sistema periódicamente (`STATUS_POLL_INTERVAL = 1s`). Cuando se conecta un pad, inicia el handshake `Hello`, comprueba la compatibilidad de versión del protocolo e inspecciona la propiedad. <!-- src: /desktop/daemon/src/runtime.rs#STATUS_POLL_INTERVAL -->
- **Transmisión de Latido (Heartbeat)**: Emite paquetes `HostStatus` cada 1 segundo (`HOST_STATUS_INTERVAL = 1s`), informando al pad si osu! está en ejecución y tiene el foco. <!-- src: /desktop/daemon/src/runtime.rs#HOST_STATUS_INTERVAL -->
- **Aplicación del Periodo de Enfriamiento (Cooldown)**: Reproduce el periodo de enfriamiento de 5 segundos tras partida del firmware (`COOLDOWN_DURATION = 5s`). Las confirmaciones en la base de datos SQLite y las exportaciones de copias de seguridad se bloquean hasta que concluye el enfriamiento y el sistema entra en `IDLE`. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->

## Telemetry Differential Synchronization (telemetry.rs)

Durante las partidas activas, `tosu` genera paquetes JSON por WebSocket a altas frecuencias. El sincronizador de telemetría filtra las actualizaciones para evitar saturar el bus serie USB: <!-- src: /desktop/daemon/src/telemetry.rs -->

- **Limitación de Frecuencia**: Los volcados de telemetría se limitan a una frecuencia configurable por el usuario entre 1 Hz y 30 Hz (`flush_interval_playing`). <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing -->
- **Caché de Valores (`DataSync`)**: Almacena en caché los valores transmitidos previamente para cada índice de fuente de datos (combo, precisión, UR, PPM, progreso de la canción). Si una propiedad no ha cambiado, se omite de la siguiente trama `DataUpdate`, minimizando el tamaño de la carga útil en el cable. <!-- src: /desktop/daemon/src/telemetry.rs -->

## IPC Request Dispatcher (ipc_handlers.rs)

Los clientes (`opad-gui` y `opadctl`) se conectan al daemon a través de IPC local: <!-- src: /desktop/daemon/src/ipc_handlers.rs -->
- Autentica versiones de protocolo (`IPC_PROTOCOL_VERSION = 1`).
- Gestiona actualizaciones de configuración (`UpdateConfig`), sincronizaciones de contadores (`ForceSync`), preparación de diseños de pantalla (`SetLayout`) y autotests de diagnóstico (`RunBoardTest`).
- Ejecuta la heurística de propiedad `take_over_if_pad_is_ahead` cuando se conecta un pad con contadores iguales o mayores. <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead -->
- Gestiona la liberación del puerto durante el flasheo de firmware (`PrepareFlash` y `FinishFlash`).
