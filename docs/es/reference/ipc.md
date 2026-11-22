---
title: Referencia del Protocolo IPC
description: Especificación del transporte de comunicación entre procesos (IPC) local, enmarcado JSON, modelo de seguridad y protocolo de solicitud/respuesta entre opad-daemon, la interfaz gráfica y opadctl.
category: reference
source_files:
  - /desktop/crates/opad-ipc/src/lib.rs
  - /desktop/crates/opad-ipc/src/transport/unix.rs
  - /desktop/crates/opad-ipc/src/transport/windows.rs
  - /desktop/daemon/src/ipc_handlers.rs
---

La arquitectura de escritorio de OPad delega la interacción con el hardware, la integración con tosu, el seguimiento de contadores de pulsaciones y la sincronización en segundo plano a un servicio persistente (`opad-daemon`). Las interfaces de usuario de escritorio —incluyendo `opad-gui` y la utilidad de línea de comandos `opadctl`— se comunican con `opad-daemon` a través de un canal local de comunicación entre procesos (IPC). <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transport & Discovery

La capa IPC abstrae los mecanismos de comunicación específicos de cada sistema operativo tras interfaces unificadas `IpcStream` e `IpcListener`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcStream -->

### Linux: Unix Domain Sockets

En Linux y sistemas operativos similares a Unix, el transporte utiliza un socket de flujo de dominio Unix estándar. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#IpcStream -->

- **Orden de Resolución del Socket**:
  1. `$XDG_RUNTIME_DIR/opad/daemon.sock` (si `$XDG_RUNTIME_DIR` está definido y no está vacío). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path -->
  2. `/run/user/<uid>/opad/daemon.sock` (si el directorio `/run/user/<uid>` existe). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
  3. `/tmp/opad-<uid>/daemon.sock` (reserva). <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->
- **Permisos del Sistema de Archivos**: El directorio del socket se crea con permisos `0700` (`rwx------`) y el nodo del socket con modo `0600` (`rw-------`), restringiendo el acceso estrictamente al UID del propietario. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from -->

### Windows: Named Pipes

En Windows 10 y 11, el transporte utiliza tuberías con nombre de Windows (Named Pipes). <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#IpcStream -->

- **Ruta de la Tubería**: `\\.\pipe\opad-ipc-<sid>` (donde `<sid>` es la cadena del identificador de seguridad de Windows del token del usuario activo). <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- **Seguridad y Control de Acceso (DACL)**:
  Las tuberías con nombre creadas con atributos de seguridad por defecto pueden ser conectadas por cualquier proceso a través de distintas sesiones de inicio de sesión. Para mantener el mismo aislamiento de usuario que los permisos Unix, `opad-daemon` construye una cadena explícita de SDDL (Security Descriptor Definition Language) que contiene una lista de control de acceso discrecional (DACL): <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl -->
  ```sddl
  D:(A;;GA;;;<USER_SID>)(A;;GA;;;SY)
  ```
  - `(A;;GA;;;<USER_SID>)`: Otorga acceso Generic All (`GA`) al usuario con sesión iniciada.
  - `(A;;GA;;;SY)`: Otorga acceso Generic All (`GA`) a Local System (`SYSTEM`).
  - El acceso se deniega explícitamente a todos los demás usuarios y sesiones de menor integridad.

## Wire Framing

El transporte IPC transmite mensajes discretos de solicitud y respuesta mediante un protocolo JSON delimitado por longitud. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->

```
+-----------------------------------+-----------------------------------------+
| Length Prefix (4 bytes, u32-LE)   | JSON Payload (UTF-8, serde_json)        |
+-----------------------------------+-----------------------------------------+
```

### Framing Rules

- **Cabecera**: Entero sin signo de 32 bits de 4 bytes codificado en orden little-endian, indicando la longitud en bytes de la carga JSON subsiguiente. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request -->
- **Carga Útil**: Cadena JSON serializada correspondiente a `IpcRequest` o `IpcResponse`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest --> <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->
- **Límites de Tamaño**:
  - `MAX_REQUEST_FRAME_SIZE`: 1,048,576 bytes (1 MiB). Las solicitudes que superen este límite devuelven `IpcError::Protocol`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->
  - `MAX_RESPONSE_FRAME_SIZE`: 8,388,608 bytes (8 MiB). Protege a los clientes contra asignaciones no acotadas durante volcados de memoria o exportaciones de diagnóstico voluminosas. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_RESPONSE_FRAME_SIZE -->

## Connection Lifecycle & Handshake

Cada cliente debe completar un handshake de protocolo inmediatamente después de establecer la conexión de transporte antes de que se procese cualquier comando. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake -->

1. **Handshake del Cliente**: El cliente envía `IpcRequest::Handshake`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
   ```json
   {
     "Handshake": {
       "client_version": "1.0.0",
       "client_protocol": 1
     }
   }
   ```
2. **Evaluación del Daemon**:
   - Si `client_protocol == IPC_PROTOCOL_VERSION` (actualmente `1`), el daemon responde `IpcResponse::HandshakeAck`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->
     ```json
     {
       "HandshakeAck": {
         "daemon_version": "1.0.0",
         "daemon_protocol": 1,
         "device_connected": true,
         "device_info": { ... }
       }
     }
     ```
   - Si las versiones de protocolo divergen incompatiblemente, el daemon responde `IpcResponse::HandshakeRejected` y cierra el flujo. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

## IPC Request Catalog

La enumeración completa de solicitudes de clientes gestionadas por `opad-daemon` está definida por `IpcRequest`: <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Device Status & Configuration
- **`GetStatus`**: Consulta el estado del daemon, conexión de hardware, perfil activo, modo de juego y contadores más recientes. Devuelve `IpcResponse::Status`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`UpdateConfig(DeviceConfig)`**: Actualiza la configuración del dispositivo en la memoria del daemon y envía `SetConfig` al pad. Si el juego está activo, la escritura en NVS se pospone y se devuelve `IpcResponse::OperationDeferred` o `ConfigUpdated`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ForceSync`**: Fuerza una sincronización bidireccional inmediata entre el estado del daemon y la NVS del pad. Devuelve `IpcResponse::SyncCompleted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetCounters { confirm }`**: Restablece los contadores de pulsaciones de por vida del pad y del PC. Requiere `confirm: true`. Devuelve `IpcResponse::CountersReset`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetLatencyStats`**: Borra las estadísticas de fluctuación (jitter) de latencia USB en curso tanto en el anfitrión como en el dispositivo. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Identity & Counter Ownership
- **`ResolveTakeover { take_over, keep_device_counters }`**: Resuelve un conflicto de propiedad cuando un pad se conecta a un nuevo anfitrión. `take_over: true` adopta el pad; `keep_device_counters` determina si los totales autoritativos son los del pad o los del PC. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResolveReplacement { restore }`**: Gestiona la sustitución de un pad dañado con nuevo hardware que comparte copias de seguridad de configuración previas. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RestoreDeviceFromPc { confirm }`**: Sobrescribe los contadores del pad con los contadores de la base de datos del PC anfitrión. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ImportPcFromDevice { confirm }`**: Adopta los contadores del pad en la base de datos local del anfitrión. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Backup & Restore
- **`ExportBackup`**: Genera una estructura serializada `JsonBackup` completa que representa la configuración del dispositivo, los diseños de pantalla y el historial de contadores. Devuelve `IpcResponse::BackupExported`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`PreviewImport(JsonBackup)`**: Valida la estructura de una copia de seguridad JSON frente al pad conectado e inspecciona los riesgos de salto en contadores sin aplicar cambios. Devuelve `IpcResponse::ImportPreview`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ImportBackup { backup, confirm }`**: Aplica una copia de seguridad inspeccionada al daemon y al pad. Devuelve `IpcResponse::BackupImported`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Screen Layouts & Live UI
- **`GetLayouts`**: Obtiene las configuraciones activas de pantalla personalizadas para reposo y juego. Devuelve `IpcResponse::Layouts`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetLayout { screen, layout }`**: Transmite un nuevo diseño de widgets a la pantalla del pad para las pantallas `Idle` o `Playing`. Devuelve `IpcResponse::LayoutApplied`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResetLayout { screen }`**: Revierte el diseño de pantalla a los widgets por defecto internos. Devuelve `IpcResponse::LayoutApplied`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetUiValues`**: Sondea los valores actuales de las variables de visualización mapeadas (precisión, combo, UR, PPM, posición de la canción). Devuelve `IpcResponse::UiValues`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`PlayAnimation { grade }`**: Desencadena la animación de calificación superpuesta en la pantalla del pad. Devuelve `IpcResponse::AnimationPlayed`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Firmware Update & Port Control
- **`PrepareFlash`**: Ordena a `opad-daemon` liberar el puerto serie USB CDC y pausar el sondeo en segundo plano para que `opadctl flash` o herramientas de flasheo puedan acceder al dispositivo. Devuelve `IpcResponse::ReadyForFlash`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`FinishFlash`**: Señala que el flasheo ha concluido; el daemon se reconecta al pad y vuelve a verificar la compatibilidad de protocolo. Devuelve `IpcResponse::FlashFinished`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`ResumeDevice`**: Reanuda el sondeo del daemon sin ejecutar las rutinas de finalización de flasheo. Devuelve `IpcResponse::DeviceResumed`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetFirmwareUpdate` / `InstallFirmwareUpdate`**: Comprueba si hay versiones de firmware descargables y programa la transferencia OTA. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RestartPad`**: Reinicia el pad conectado en modo de ejecución normal una vez en reposo. Devuelve `IpcResponse::PadRestarting`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Hall Effect Calibration & Telemetry
- **`StartHeCalibration`**: Inicia la calibración guiada en múltiples pasos de Hall Effect. Devuelve `IpcResponse::HeCalibrationStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`CancelHeCalibration`**: Cancela la calibración de Hall Effect en curso. Devuelve `IpcResponse::HeCalibrationCancelled`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetHeCalibrationProgress`**: Sondea los pasos de la calibración guiada y los estados de error. Devuelve `IpcResponse::HeCalibrationProgress`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetHeLive`**: Solicita la distancia de recorrido normalizada en vivo y las lecturas magnéticas ADC en bruto (~50 Hz) para renderizado gráfico. Devuelve `IpcResponse::HeLive`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Diagnostics & Hardware Testing
- **`DetectPin { pin, timeout_ms }`**: Ordena al pad detectar el accionamiento de contacto en pines GPIO de cabecera. Devuelve `IpcResponse::PinDetected`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`RunBoardTest`**: Ejecuta la suite de autotest de placa (I2C, pantalla, controlador táctil, ADC). Devuelve `IpcResponse::BoardTest(BoardTestReport)`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetTapStats` / `SetTapHistoryPeriod`**: Consulta datos de histograma de pulsación en vivo y métricas de velocidad de pulsación. Devuelve `IpcResponse::TapStats`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`GetLogEntries { since_seq }`**: Drena eventos de registro de diagnóstico del búfer circular del daemon. Devuelve `IpcResponse::LogEntries`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`TriggerEasterEgg`**: Envía el activador de la animación easter egg. Devuelve `IpcResponse::EasterEggTriggered`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### App & tosu Updates
- **`GetUpdateStatus`**: Consulta los estados de versión del software de escritorio y de tosu. Devuelve `IpcResponse::UpdateStatus`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`CheckForUpdates`**: Desencadena la búsqueda inmediata de versiones en segundo plano. Devuelve `IpcResponse::UpdateCheckStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`InstallUpdate { component }`**: Desencadena la descarga de binarios y el reemplazo atómico de los ejecutables de la suite de escritorio. Devuelve `IpcResponse::UpdateStarted`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetUpdateChannel { channel }`**: Cambia entre los canales de publicación de software `stable` y `beta`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
- **`SetUpdateEnabled { enabled }`**: Activa o desactiva la comprobación automática de actualizaciones en segundo plano. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
