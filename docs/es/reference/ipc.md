---
title: Referencia del protocolo IPC
description: Especificación del transporte local de comunicación entre procesos (IPC), entramado JSON, modelo de seguridad Windows/Linux y protocolo de peticiones/respuestas entre opad-daemon, opad-gui y opadctl.
---

`opad-daemon` es propietario de la conexión serie USB CDC, la base de datos SQLite, la supervisión de `tosu` y las comprobaciones de actualización. `opad-gui` y `opadctl` se comunican con `opad-daemon` por un canal IPC local (`IPC_PROTOCOL_VERSION = 1`). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transporte y descubrimiento

Los transportes por plataforma se abstraen tras `IpcStream`, `IpcServerStream` e `IpcListener`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcStream -->

### Linux: sockets de dominio Unix

En Linux, IPC utiliza un socket de dominio Unix de flujo. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#IpcStream -->

- **Orden de resolución del socket**:
  1. `$XDG_RUNTIME_DIR/opad/daemon.sock` (cuando `$XDG_RUNTIME_DIR` está definido y no vacío). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path -->
  2. `/run/user/<uid>/opad/daemon.sock` (si existe `/run/user/<uid>`). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
  3. `/tmp/opad-<uid>/daemon.sock` (respaldo). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
- **Permisos**: Directorio del socket con modo `0700` y nodo del socket con modo `0600` (solo UID propietario). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->

### Windows: Named Pipes

En Windows 10/11, IPC utiliza Named Pipes de Windows por usuario. <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#IpcStream -->

- **Ruta de la tubería**: `\\.\pipe\opad-ipc-<SID>` (donde `<SID>` es la cadena SID del token del usuario actual, usando `default` como respaldo al formatear la ruta; la creación del servidor rehúsa arrancar si falla la obtención del SID). <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- **Seguridad (DACL)**: Cada instancia de tubería se crea con una DACL SDDL protegida que permite acceso únicamente a `SYSTEM` y al usuario invocador: <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl -->
  ```text
  D:P(A;;GA;;;SY)(A;;GA;;;<USER_SID>)
  ```
- **Garantía de daemon único**: La primera instancia usa `first_pipe_instance(true)` para que un segundo daemon falle con `IpcError::AlreadyRunning`. <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#create_listener -->

## Entramado en el cable

Los mensajes son tramas JSON UTF-8 delimitadas por longitud: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->

```text
+-----------------------------------+-----------------------------------------+
| Length Prefix (4 bytes, u32-LE)   | JSON Payload (UTF-8, serde_json)        |
+-----------------------------------+-----------------------------------------+
```

- `MAX_REQUEST_FRAME_SIZE`: `1,048,576` bytes (1 MiB). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->
- `MAX_RESPONSE_FRAME_SIZE`: `8,388,608` bytes (8 MiB). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_RESPONSE_FRAME_SIZE -->

## Ciclo de vida de conexión y apretón de manos

Cada cliente abre una conexión y realiza un apretón de manos (`connect_and_handshake`) antes de enviar otras peticiones: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake -->

1. **El cliente envía `IpcRequest::Handshake`**:
   ```json
   {
     "Handshake": {
       "client_version": "1.0.0",
       "client_protocol": 1
     }
   }
   ```
2. **El daemon responde**:
   - Si las versiones coinciden (`IPC_PROTOCOL_VERSION == 1`), `IpcResponse::HandshakeAck`:
     ```json
     {
       "HandshakeAck": {
         "daemon_version": "1.0.0",
         "daemon_protocol": 1,
         "device_connected": true
       }
     }
     ```
   - En caso de discrepancia, `IpcResponse::HandshakeRejected { daemon_protocol, reason }` y cierra la conexión. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

## Catálogo de peticiones IPC (`IpcRequest`)

Todas las variantes definidas en `IpcRequest` ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs)): <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Estado, configuración y contadores
- `GetStatus` -> `Status { mode, device_connected, device_info, counters, counters_source, pc_counters, esp_counters, config, last_sync_time, last_sync_error, storage_error, tosu_connected, latency, pending_replacement, pending_takeover, incompatible, last_backup, foreign_pad, he_calibration, key3 }`
- `UpdateConfig(DeviceConfig)` -> `ConfigUpdated { config, deferred_persist }` u `OperationDeferred`
- `ForceSync` -> `SyncCompleted { success, counters }`
- `ResetCounters { confirm }` -> `CountersReset { counters }`
- `ResetLatencyStats` -> limpia las estadísticas de latencia del dispositivo

### Identidad, propiedad y recuperación de contadores
- `ResolveTakeover { take_over, keep_device_counters }` -> responde a `TakeoverPrompt` para un teclado perteneciente a otra instalación
- `ResolveReplacement { restore }` -> restaura los contadores del teclado anterior en un teclado recién conectado con pocos conteos (o empieza de cero)
- `RestoreDeviceFromPc { confirm }` -> `CountersRestored { counters }`
- `ImportPcFromDevice { confirm }` -> `CountersRestored { counters }`

### Copia de seguridad y restauración
- `ExportBackup` -> `BackupExported(JsonBackup)`
- `PreviewImport(JsonBackup)` -> `ImportPreview { current, incoming, device_id_matches, is_counter_rollback, warnings }`
- `ImportBackup { backup, confirm }` -> `BackupImported { success, counters, config }`

### Diseños de pantalla e interfaz en vivo
- `GetLayouts` -> `Layouts { idle, playing }`
- `SetLayout { screen, layout }` -> `LayoutApplied { screen, message }`
- `ResetLayout { screen }` -> revierte la pantalla al diseño predeterminado del firmware
- `GetUiValues` -> `UiValues(Vec<(u8, SourceValue)>)`
- `PlayAnimation { animation, grade }` -> `AnimationPlayed` (vista previa de animación de pantalla de resultados)

### Flasheo, actualizaciones de firmware y reinicio
- `PrepareFlash` -> `ReadyForFlash { port }` (el daemon libera el puerto CDC)
- `FinishFlash` -> `FlashFinished { firmware_version, protocol_version, compatible }`
- `ResumeDevice` -> `DeviceResumed` (reanuda el sondeo sin esperar re-enumeración de la aplicación)
- `GetFirmwareUpdate` -> `FirmwareUpdateOffer(FirmwareOffer)`
- `InstallFirmwareUpdate { confirm }` -> `FirmwareUpdateFinished { from, to, firmware_version, running_partition, protocol_version, compatible }` (rechazado si `confirm: false`)
- `RestartPad` -> `PadRestarting`

### Calibración Hall Effect y vista en vivo
- `StartHeCalibration` -> `HeCalibrationStarted`
- `CancelHeCalibration` -> `HeCalibrationCancelled`
- `GetHeCalibrationProgress` -> `HeCalibrationProgress(Option<HeCalibrationProgress>)`
- `GetHeLive` -> `HeLive { supported, sample }` (mantiene la transmisión del teclado; se detiene 2 s después de la última petición)

### Diagnósticos, registros y estadísticas de pulsación
- `DetectPin { key_id, timeout_ms, exclude_gpio }` -> `PinDetected { key_id, gpio, success }`
- `RunBoardTest` -> `BoardTest(BoardTestReport)` (prueba las PCBs del carrier y del módulo de entrada: ID de módulo, dirección de cable, líneas de teclas, cortocircuitos de conector, ADS7028/sensores de HE V3)
- `GetTapStats` / `SetTapHistoryPeriod { days }` -> `TapStats(TapStatsSnapshot)`
- `GetLogEntries { since_seq, limit, level, source }` -> `LogEntries { entries, latest_seq }`
- `TriggerEasterEgg` -> `EasterEggTriggered`

### Actualizaciones de la aplicación de escritorio y `tosu`
- `GetUpdateStatus` -> `UpdateStatus { app, tosu, last_check, last_error, restart_required, channel, checking, last_result }`
- `CheckForUpdates` -> `UpdateCheckStarted`
- `InstallUpdate { component }` -> `UpdateStarted { component }`
- `SetUpdateChannel { channel }` (`"stable"` o `"beta"`)
- `SetUpdateEnabled { component, enabled }` (conmutador por componente: `App` o `Tosu`, nunca global)
