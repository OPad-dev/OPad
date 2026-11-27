---
title: Referencia del protocolo IPC
description: Transporte, entramado, handshake y catálogo de peticiones del canal IPC local entre opad-daemon, opad-gui y opadctl en Linux y Windows.
sidebar:
  order: 6
---

`opad-daemon` es dueño del puerto serie del teclado, la base de datos SQLite, tosu y los actualizadores. `opad-gui` y `opadctl` llegan a él por un canal local por usuario definido en `desktop/crates/opad-ipc`. La versión del protocolo es `IPC_PROTOCOL_VERSION = 1`.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IPC_PROTOCOL_VERSION -->

## Transporte

| | Linux | Windows 10/11 |
|---|---|---|
| Canal | Socket de dominio Unix de flujo | Named pipe |
| Ruta | `$XDG_RUNTIME_DIR/opad/daemon.sock`; si esa variable no está definida o está vacía, `/run/user/<uid>/opad/daemon.sock` cuando existe `/run/user/<uid>`, y si no, `/tmp/opad-<uid>/daemon.sock` | `\\.\pipe\opad-ipc-<SID>`, con el SID del usuario actual (`default` si no se puede leer el SID) |
| Acceso | El directorio del socket es `0700` y debe pertenecer al usuario; el socket es `0600` | DACL protegida `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)`: solo `SYSTEM` y el usuario. El daemon se niega a crear la tubería si no puede leer el SID. |
| Segundo daemon | Un daemon que encuentra un socket que aún responde falla con `opad-daemon is already running`; un socket obsoleto se elimina | La primera instancia de la tubería se crea con `first_pipe_instance(true)`, así que un segundo daemon falla con el mismo error |
<!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#socket_path_from, /desktop/crates/opad-ipc/src/transport/unix.rs#create_listener, /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path, /desktop/crates/opad-ipc/src/transport/windows.rs#pipe_security_sddl, /desktop/crates/opad-ipc/src/transport/windows.rs#create_instance -->

## Entramado

Cada mensaje es una longitud `u32` little-endian de 4 bytes seguida de esa misma cantidad de bytes de JSON en UTF-8 (`serde_json`, enums con etiqueta externa). Un cliente envía una petición y lee una respuesta; una conexión [suscrita](#suscripciones) es la excepción.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#send_request, /desktop/crates/opad-ipc/src/lib.rs#read_request -->

| Límite | Valor |
|---|---|
| `MAX_REQUEST_FRAME_SIZE` | 1 MiB (1.048.576 bytes) |
| `MAX_RESPONSE_FRAME_SIZE` | 8 MiB (8.388.608 bytes) |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->

## Handshake

Toda conexión empieza con `Handshake` (`connect_and_handshake`). El daemon no atiende nada más hasta que uno tenga éxito: cualquier otra primera petición recibe `Error("Handshake required: ...")` y la conexión se cierra.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake, /desktop/daemon/src/ipc_handlers.rs#refuse_before_handshake, /desktop/daemon/src/ipc_handlers.rs#serve_connection -->

```json
{ "Handshake": { "client_version": "1.0.0-rc.6", "client_protocol": 1 } }
```

`client_version` es la versión del crate del cliente (`CARGO_PKG_VERSION`). El daemon responde:

```json
{ "HandshakeAck": { "daemon_version": "1.0.0-rc.6", "daemon_protocol": 1, "device_connected": true } }
```

Responde `HandshakeRejected { daemon_protocol, reason }` y cierra la conexión cuando:

- `client_protocol` no es `1`; o
- `client_version` difiere de la versión del daemon. La GUI, el daemon y la CLI se publican juntos, así que esto significa que la aplicación se actualizó en disco mientras el daemon antiguo sigue en marcha; el motivo indica al usuario que reinicie `opad-daemon`.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Las peticiones añadidas después de publicar IPC v1 (`ResumeDevice`, `CheckForUpdates`, `SetUpdateChannel`, `GetTapStats`, `RunBoardTest`, `PlayAnimation`, `RestartPad`, `Subscribe`) llegaron sin subir la versión. Un daemon más antiguo no puede interpretarlas y cierra la conexión, lo que los clientes tratan como «no disponible».
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

## Peticiones

Cualquier petición puede recibir también `Error(String)`. `OperationRejected { reason }` significa rechazada; `OperationDeferred { reason }` significa rechazada por ahora, normalmente porque hay un mapa o su enfriamiento en curso.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcResponse, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Estado, configuración y contadores

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `GetStatus` | `Status { mode, device_connected, device_info, counters, counters_source, pc_counters, esp_counters, config, last_sync_time, last_sync_error, storage_error, tosu_connected, latency, pending_replacement, pending_takeover, incompatible, last_backup, foreign_pad, he_calibration, key3 }` | |
| `UpdateConfig(DeviceConfig)` | `ConfigUpdated { config, deferred_persist }` | Puede ser `OperationDeferred` u `OperationRejected`. |
| `ForceSync` | `SyncCompleted { success, counters }` | |
| `ResetCounters { confirm }` | `CountersReset { counters }` | Se rechaza sin `confirm: true`. |
| `ResetLatencyStats` | `HandshakeAck { .. }` | El daemon responde con un `HandshakeAck` en lugar de una variante propia. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Propiedad y recuperación de contadores

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `ResolveTakeover { take_over, keep_device_counters }` | `CountersRestored { counters }` | Responde al aviso de un teclado que pertenece a otra instalación. `take_over: false` deja el teclado como está y recibe `OperationRejected`. `keep_device_counters` elige los contadores del teclado en lugar de los de este PC. Se aplaza durante un mapa. |
| `ResolveReplacement { restore }` | `CountersReset { counters }` | Un teclado nuevo sustituyó a uno conocido: restaura en él los contadores del teclado anterior o empieza de cero. |
| `RestoreDeviceFromPc { confirm }` | `CountersRestored { counters }` | Escribe los contadores de este PC en el teclado. Se rechaza sin `confirm: true`. |
| `ImportPcFromDevice { confirm }` | `CountersRestored { counters }` | Lleva los contadores del teclado a la base de datos de este PC. Se rechaza sin `confirm: true`. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Los reinicios, restauraciones y sustituciones de contadores se rechazan durante un mapa o su enfriamiento. Las peticiones que escriben en el teclado reciben `Error` mientras el teclado pertenezca a otra instalación o haya un aviso de toma de control o de sustitución esperando respuesta.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#refuse_if_pad_guarded, /desktop/daemon/src/runtime.rs#pad_guard -->

### Copia de seguridad y restauración

| Petición | Respuesta de éxito |
|---|---|
| `ExportBackup` | `BackupExported(JsonBackup)` |
| `PreviewImport(JsonBackup)` | `ImportPreview { current, incoming, device_id_matches, is_counter_rollback, warnings }` |
| `ImportBackup { backup, confirm }` | `BackupImported { success, counters, config }` |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### Diseños de pantalla y animaciones

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `GetLayouts` | `Layouts { idle, playing }` | Diseños personalizados guardados en el daemon; `None` = el diseño integrado del teclado. |
| `SetLayout { screen, layout }` | `LayoutApplied { screen, message }` | Valida, guarda y envía el diseño. |
| `ResetLayout { screen }` | `LayoutApplied { screen, message }` | Olvida el diseño personalizado; el teclado vuelve a su diseño predeterminado. |
| `GetUiValues` | `UiValues(Vec<(u8, SourceValue)>)` | Los últimos valores de las fuentes de datos, para la vista previa en vivo del diseñador. |
| `PlayAnimation { animation, grade }` | `AnimationPlayed` | `Error` sin teclado, con un firmware sin animaciones o durante un mapa. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### Flasheo, firmware y reinicio

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `PrepareFlash` | `ReadyForFlash { port }` | El daemon libera el puerto serie. Se rechaza durante un mapa o el enfriamiento. |
| `FinishFlash` | `FlashFinished { firmware_version, protocol_version, compatible }` | El daemon vuelve a buscar el teclado y lo espera hasta 15 s. |
| `ResumeDevice` | `DeviceResumed` | Devuelve el puerto sin esperar, tras `opadctl pad bootloader` o un flasheo fallido. |
| `GetFirmwareUpdate` | `FirmwareUpdateOffer(FirmwareOffer)` | Solo lee; se puede sondear sin riesgo. |
| `InstallFirmwareUpdate { confirm }` | `FirmwareUpdateFinished { from, to, firmware_version, running_partition, protocol_version, compatible }` | Se rechaza con `confirm: false`. Sincroniza los contadores con este PC antes de escribir. |
| `RestartPad` | `PadRestarting` | `Error` sin teclado, con un firmware sin reinicio o durante un mapa. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#FirmwareOffer, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Si una conexión que pausó el teclado para un flasheo se cierra sin devolverlo, el daemon reanuda el teclado por su cuenta.
<!-- src: /desktop/daemon/src/ipc_handlers.rs#serve_connection -->

### Hall Effect

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `StartHeCalibration` | `HeCalibrationStarted` | |
| `CancelHeCalibration` | `HeCalibrationCancelled` | |
| `GetHeCalibrationProgress` | `HeCalibrationProgress(Option<HeCalibrationProgress>)` | |
| `GetHeLive` | `HeLive { supported, sample }` | Mantiene la transmisión del teclado; se detiene 2 s después de la última petición. `supported` es falso en teclados MX y firmware antiguo; `sample` es `None` durante un mapa. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#IpcResponse -->

### Diagnósticos, registros y estadísticas

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `DetectPin { key_id, timeout_ms, exclude_gpio }` | `PinDetected { key_id, gpio, success }` | |
| `RunBoardTest` | `BoardTest(BoardTestReport)` | `Error` sin teclado o con un firmware sin la prueba. |
| `GetTapStats` | `TapStats(TapStatsSnapshot)` | `current`, `last`, `history`, `period_days`, `unsaved_attempts`. |
| `SetTapHistoryPeriod { days }` | `TapStats(TapStatsSnapshot)` | `0` = todo el historial. Se guarda cuando vuelven a permitirse las escrituras en el almacenamiento. |
| `GetLogEntries { since_seq, limit, level, source }` | `LogEntries { entries, latest_seq }` | `level` y `source` son opcionales y se filtran antes de aplicar `limit`. |
| `TriggerEasterEgg` | `EasterEggTriggered` | |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest, /desktop/crates/opad-ipc/src/lib.rs#TapStatsSnapshot -->

### Actualizaciones de la aplicación y de tosu

| Petición | Respuesta de éxito | Notas |
|---|---|---|
| `GetUpdateStatus` | `UpdateStatus { app, tosu, last_check, last_error, restart_required, pending_version, channel, checking, last_result }` | `app` y `tosu` son `ComponentUpdate`. |
| `CheckForUpdates` | `UpdateCheckStarted` | La comprobación se ejecuta en segundo plano; sondea `GetUpdateStatus`. |
| `SetUpdateChannel { channel }` | `UpdateCheckStarted` | `stable` o `beta`; también inicia una comprobación del nuevo canal. |
| `SetUpdateEnabled { component, enabled }` | `ConfigUpdated { .. }` | `component` es `app`, `tosu` o `firmware` (actualizaciones automáticas del firmware mientras no se usa el teclado). Cada interruptor es independiente; no hay uno global. |
| `InstallUpdate { component }` | `UpdateStarted { component }` | Solo se envía porque una persona pulsó Install. Se aplaza salvo que el daemon esté en `Idle`. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#UpdateComponent, /desktop/crates/opad-ipc/src/lib.rs#ComponentUpdate, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Suscripciones

`Subscribe { watches }` mantiene la conexión abierta: el daemon responde `Subscribed` y a partir de ahí envía por su cuenta la respuesta a cada petición vigilada, una vez al empezar y de nuevo cada vez que difiere de la última que envió. Un cliente que muestra estado en vivo lo usa en lugar de preguntar con un temporizador.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest::Subscribe, /desktop/crates/opad-ipc/src/lib.rs#can_be_watched, /desktop/daemon/src/ipc_handlers.rs#serve_connection, /desktop/daemon/src/ipc_handlers.rs#ActiveWatch -->

```json
{ "Subscribe": { "watches": [ { "request": "GetStatus", "every_ms": 100 }, { "request": "GetHeLive", "every_ms": 20 } ] } }
```

| Regla | Detalle |
|---|---|
| Qué se puede vigilar | Peticiones que solo leen: `GetStatus`, `GetLayouts`, `GetUiValues`, `GetUpdateStatus`, `GetFirmwareUpdate`, `GetTapStats`, `GetLogEntries`, `GetHeCalibrationProgress`, `GetHeLive`. Cualquier otra petición en la lista hace que se rechace todo el `Subscribe` con `Error`, y la conexión sigue como antes. |
| `every_ms` | Cada cuánto mira el daemon si hay un cambio; nunca más a menudo que cada 20 ms (`MIN_WATCH_INTERVAL_MS`). Como máximo 16 vigilancias (`MAX_WATCHES`). |
| Qué se envía | La misma respuesta que recibe la petición cuando se hace directamente, así que el cliente distingue las respuestas por su tipo. No se envía nada mientras una respuesta sigue igual. |
| `GetLogEntries` | Cada entrada se envía una vez: cada respuesta lleva las entradas posteriores a la última enviada (hasta `limit`), y una respuesta sin entradas solo se envía la primera vez. |
| `GetHeLive` | Mantiene el teclado transmitiendo mientras se vigila. |
| Otras peticiones | Una conexión suscrita solo acepta otro `Subscribe`, que sustituye las vigilancias (una lista vacía las detiene). Cualquier otra cosa se responde con `Error` y no se ejecuta; envíala por una conexión propia. |
| Un daemon sin `Subscribe` | Cierra la conexión en vez de responder. `opad_ipc::subscribe` lo indica con `Ok(false)` y el cliente pregunta con un temporizador. |
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#IpcRequest::Subscribe, /desktop/crates/opad-ipc/src/lib.rs#can_be_watched, /desktop/daemon/src/ipc_handlers.rs#serve_connection, /desktop/daemon/src/ipc_handlers.rs#ActiveWatch -->

`opad-gui` se suscribe a lo que muestra su página abierta ([opad-gui](../develop/desktop/gui.md#bucle-de-actualización-y-el-flujo)); `opadctl` pregunta petición a petición.
<!-- src: /desktop/gui/src/main.rs#watches, /desktop/cli/src/main.rs#run -->
