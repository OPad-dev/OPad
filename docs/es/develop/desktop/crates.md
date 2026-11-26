---
title: Referencia de los crates compartidos
description: Responsabilidades, tipos principales y constantes de los nueve crates de biblioteca de desktop/crates/.
sidebar:
  order: 5
---

El workspace de escritorio ([/desktop/Cargo.toml](https://github.com/OPad-dev/OPad/blob/main/desktop/Cargo.toml)) reparte la lógica compartida en nueve crates de biblioteca bajo `desktop/crates/`. Cómo encajan entre sí: [Visión general de la suite de escritorio](overview.md).

## 1. `opad-model`

[/desktop/crates/opad-model/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/lib.rs). Tipos de dominio compartidos por todos los binarios, sin depender de los demás crates:

- Structs principales: `DeviceInfo`, `DeviceConfig`, `CounterState`, `Key3Counters`, `LatencyStats`, `JsonBackup`, `HeKeyConfig`, `HeCalibrationStatus`, `HeLiveSample`; `DEFAULT_KEY3_HID_USAGE = 0x06`.
- Submódulos: `paths` (rutas de datos, configuración y socket por sistema operativo, [/desktop/crates/opad-model/src/paths.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/paths.rs)), `board_test` (evalúa las medidas en bruto de la PCB del pad), `diag` (pone nombre y formato a los eventos de diagnóstico del firmware), `log`, `result_anim`, `swipe` (acciones de deslizamiento y `OSU_SHORTCUTS`, contrastadas con los `SWIPE_LABELS` del firmware), `tap_rate`, `ui_source`.

## 2. `opad-protocol`

[/desktop/crates/opad-protocol/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-protocol/src/lib.rs). El códec serie:

- `build.rs` compila [/protocol/opad.proto](https://github.com/OPad-dev/OPad/blob/main/protocol/opad.proto) con `protox` + `prost`; sin `protoc` externo.
- Framing legacy, marcado (`0xAA 0x55`) y comprobado (`0xAA 0x5A` + CRC-16/CCITT-FALSE, `crc16`). Formato: [Protocolo serie](../../reference/serial-protocol.md).

## 3. `opad-ipc`

[/desktop/crates/opad-ipc/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/lib.rs). IPC local entre el daemon, la GUI y `opadctl`:

- Socket de flujo Unix (directorio `0700`, socket `0600`) en [/desktop/crates/opad-ipc/src/transport/unix.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/transport/unix.rs); named pipe de Windows `\\.\pipe\opad-ipc-<SID>` con la DACL protegida `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)` en [/desktop/crates/opad-ipc/src/transport/windows.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ipc/src/transport/windows.rs).
- Prefijo de longitud `u32` little-endian + JSON UTF-8 (`MAX_REQUEST_FRAME_SIZE = 1 MiB`, `MAX_RESPONSE_FRAME_SIZE = 8 MiB`, `IPC_PROTOCOL_VERSION = 1`). Catálogo de mensajes: [Referencia de IPC](../../reference/ipc.md).

## 4. `opad-storage`

[/desktop/crates/opad-storage/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-storage/src/lib.rs). Persistencia SQLite en `opad.db`:

- Modo WAL y claves foráneas al abrir; migraciones embebidas hasta el esquema v19 (`schema_migrations`, `device_state`, `config`, `layouts`, `app_state`, `tap_attempts` y las demás tablas de estadísticas de toques de [/desktop/crates/opad-storage/src/tap_stats.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-storage/src/tap_stats.rs)).
- La protección de escritura durante la partida: mientras el daemon tiene las escrituras desactivadas (`PLAYING`, `COOLDOWN`), toda escritura devuelve `StorageError::WritesBlocked`. También `reconcile_counters`.

## 5. `opad-device`

[/desktop/crates/opad-device/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-device/src/lib.rs). El gestor del dispositivo serie:

- **Descubrimiento**: un puerto es un pad cuando su VID es de Espressif (`ESPRESSIF_VID = 0x303A`) o de pid.codes (`PIDCODES_VID = 0x1209`) **y** su cadena de producto contiene "OPad" o su número de serie empieza por `OPAD-` (o `OSUPAD-` en firmware antiguo). **No** se fija en el PID, así que se conectan tanto `303a:8415` como el `303a:4001` de las release candidates; `OPAD_APP_PID = 0x8415` es solo una constante. A un puerto sin cadenas se le envía un Hello de sondeo. Se omiten el bootloader de la ROM (`ESP_ROM_BOOTLOADER_PID = 0x1001`) y los puertos serie del sistema. Los puertos se vuelven a explorar cada 400 ms (`PORT_SCAN_INTERVAL`).
- **Handshake**: los Hello alternan framing marcado y legacy cada 400 ms (`HELLO_RETRY`) hasta que el pad responde; un `HelloAck` con `frame_crc` pasa los comandos a tramas comprobadas ([Versiones de la app y del pad](../../explanation/app-and-pad-versions.md)).
- **Eventos**: `DeviceEvent` en un canal broadcast (`EVENT_BUFFER = 256`): `Connected`, `Ownership { owner_id }`, `Disconnected`, `Counters`, `StatusUpdate` y otros. Qué significa la propiedad lo decide el daemon.
- **Flasheo** ([/desktop/crates/opad-device/src/flash.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-device/src/flash.rs)), envolviendo `espflash` fijado en **4.6.0**:
  - `enter_bootloader` prueba `BOOT_TRIGGERS` (el comando `BOOTLOADER` y después el toque a 1200 baudios) y espera a `303a:1001`;
  - `plan_app_update` + `otadata_image` escriben la nueva imagen en la ranura que no está en ejecución (`APP_PARTITION_OFFSET = 0x20000` u `OTA_1_OFFSET = 0x220000`) y apuntan `otadata` (`OTADATA_OFFSET = 0xf000`) a ella como imagen nueva, para que el bootloader revierta si nunca se confirma;
  - `write_images`, `flash`, `recover` (el conjunto completo de imágenes para un pad que no se enumera como OPad, [Recuperación](../../recovery.md)), `wait_for_port`, `wait_until_openable`;
  - `reset_to_app` sale del modo de descarga con un reset por watchdog del RTC mediante su propio `WRITE_REG` (el `--after watchdog-reset` de espflash no hace nada en el S3);
  - `check_esp32s3_image` / `check_esp32s3_image_file` rechazan una imagen para otro chip.

## 6. `opad-layout`

[/desktop/crates/opad-layout/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-layout/src/lib.rs). Esquema de layouts, validación (`SCREEN_W` × `SCREEN_H` = 320×240, `MAX_WIDGETS = 32`) y empaquetado para el protocolo y la NVS que coincide con el `ui_layout_t` del firmware ([UI del firmware](../firmware/ui.md#layouts-y-persistencia-diferida-ui_storec)).

## 7. `opad-ui-preview`

[/desktop/crates/opad-ui-preview/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ui-preview/src/lib.rs). Compila el `ui/core` del firmware y LVGL para el host (`csrc/preview.c`) y renderiza búferes RGBA, para que el Screen Designer de la GUI muestre lo que dibujará el pad. Si faltan LVGL o el `sdkconfig` del firmware, `build.rs` avisa y compila en su lugar `csrc/preview_stub.c`.

## 8. `opad-tosu`

[/desktop/crates/opad-tosu/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-tosu/src/lib.rs). Integración con `tosu` (fijado en **v4.26.2**):

- `TosuManager`: cliente WebSocket de los sockets v2 y precise (`parse_tosu_v2_json`, `parse_precise_keys`) que publica la telemetría, los recuentos de teclas y un indicador de conexión.
- `spawn_tosu_supervisor` / `TosuSupervisor`: ejecuta `tosu` como proceso hijo, localizado por `find_tosu_binary()` (`$OPAD_TOSU_PATH`, `~/.local/opt/tosu/tosu`, `tosu` en el `$PATH` y, en último lugar, la copia incluida, para que un tosu instalado por el usuario gane siempre), con ajustes forzados en `tosu.env`.
- `ptrace_access()` / `yama_ptrace_scope()`: explica en Linux cuándo Yama impide a `tosu` leer la memoria de osu!.

## 9. `opad-update`

[/desktop/crates/opad-update/src/lib.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/lib.rs). Actualizaciones firmadas de la app, `tosu` y el firmware ([Actualizaciones y reversión automática](../../explanation/updates-and-rollback.md)):

| Módulo | Función |
|---|---|
| `verify` | Firmas minisign Ed25519 (`RELEASE_KEY`; `TEST_KEY` solo con `OPAD_TRUST_TEST_KEY=1` o `/etc/opad/trust-test-key`) y hashes SHA-256 |
| `manifest`, `client`, `http` | Lectura y descarga del feed; `client` respeta `OPAD_MANIFEST_URL` |
| `origin` | `InstallOrigin` (`Windows`, `Deb`, `Rpm`, `Aur`, `AppImage`, `User`, `Source`, `Unknown`) decide cómo se aplica una actualización de la app |
| `gate`, `schedule`, `rollout` | `may_update_now` (nunca durante la partida), el horario del usuario, despliegue escalonado |
| `download`, `app`, `tosu`, `firmware`, `helper`, `version` | Descargas con `write_atomic`, aplicación por componente, el protocolo del helper con privilegios, comparación de versiones |

Binarios: `opad-feed` ([/desktop/crates/opad-update/src/bin/opad-feed.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/bin/opad-feed.rs): genera, firma y verifica feeds) y `opad-update-helper` (se ejecuta como root mediante `pkexec` en Linux; comprueba un paquete `.deb` / `.rpm` / de Arch frente al feed firmado y lo instala, [/desktop/crates/opad-update/src/helper.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-update/src/helper.rs)).
