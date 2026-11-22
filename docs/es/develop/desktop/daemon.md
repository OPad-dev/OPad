---
title: Arquitectura del subsistema daemon
description: Referencia de implementación de opad-daemon, cola del actor de dispositivo, supervisor de estado de ejecución, envío diferencial de telemetría, copias automáticas y manejadores IPC.
---

`opad-daemon` es el servicio Tokio en segundo plano que coordina el tráfico USB CDC, la telemetría de `tosu`, la persistencia en SQLite, las copias automáticas, las actualizaciones de software/firmware y el IPC local ([/desktop/daemon/src/main.rs](/desktop/daemon/src/main.rs), [/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs)).

## Actor de dispositivo (`device_actor.rs`)

Todos los comandos anfitrión-a-teclado en ejecución se serializan a través de una sola tarea de actor en [/desktop/daemon/src/device_actor.rs](/desktop/daemon/src/device_actor.rs):

- **Canal FIFO acotado**: Capacidad `QUEUE_DEPTH = 256` que transporta `DeviceCommand` (`TimeSync`, `ClaimOwnership`, `Config`, `HostStatus`, `DataUpdate`, `Layout`, `RequestStatus`, `PlayAnimation`).
- **Despacho secuencial y reintentos**: Espera cada comando en orden. Ante `DeviceError::Busy`, reintenta hasta `BUSY_RETRIES = 5` veces con `BUSY_BACKOFF = 20 ms`, salvo las diferencias obsoletas de `DataUpdate`, que en su lugar marcan `resend_data` para que el siguiente volcado transmita una instantánea completa nueva.

## Supervisor de estado de ejecución (`runtime.rs`)

Definido en [/desktop/daemon/src/runtime.rs](/desktop/daemon/src/runtime.rs):

- **Sondeo y latidos**: `STATUS_POLL_INTERVAL = 1 s`, `HOST_STATUS_INTERVAL = 1 s` (informa `tosu_connected`, `is_playing`, `play_id`, `osu_active`).
- **Enfriamiento y guardia de escritura**: Refleja los 5 s de enfriamiento del firmware (`COOLDOWN_DURATION = 5 s`), bloqueando las escrituras en SQLite (`Storage::set_writes_allowed(false)`) durante `Playing` y `Cooldown`.
- **Copias de seguridad automáticas**: Programa `AUTO_BACKUP_DELAY = 20 s` tras asentarse la sincronización post-partida en `Idle` y escribe `opad-backup-<timestamp>.json` en `<data_dir>/backups/` (conservando los 10 archivos más recientes mediante [/desktop/daemon/src/backup.rs](/desktop/daemon/src/backup.rs)).
- **Seguimiento de ritmo de pulsación**: Sigue los intentos en vivo (prefiriendo las marcas exactas de pulsación del teclado frente a las marcas gruesas de `tosu`), ignora repeticiones u otros teclados (`TOSU_ONLY_PRESSES = 8`) y almacena en búfer hasta `MAX_UNSAVED_ATTEMPTS = 500` mientras las escrituras están pausadas.

## Sincronización diferencial de telemetría (`telemetry.rs`)

[/desktop/daemon/src/telemetry.rs](/desktop/daemon/src/telemetry.rs) (`DataSync`) ingiere valores de `tosu` v2 y emite solo las entradas `(source_id, SourceValue)` modificadas, limitadas a `flush_interval_playing(hz)` (`1`–`30 Hz`, por defecto `10 Hz` / `100 ms`) en partida y `FLUSH_INTERVAL_IDLE = 500 ms` en reposo.

## Despachador de peticiones IPC (`ipc_handlers.rs`)

[/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs) atiende peticiones autenticadas de `IPC_PROTOCOL_VERSION = 1`:
- Ejecuta `take_over_if_pad_is_ahead` en teclados de otro propietario cuando el teclado está igual o por delante del PC en ambas teclas; en caso contrario aplica `pad_guard()` hasta que el usuario resuelva `TakeoverPrompt`.
- Coordina las cesiones de puerto `PrepareFlash` / `FinishFlash` / `ResumeDevice` para `opadctl` y gestiona las actualizaciones de firmware impulsadas por el anfitrión ([/desktop/daemon/src/firmware_update.rs](/desktop/daemon/src/firmware_update.rs)).
