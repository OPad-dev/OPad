---
title: Referencia de crates compartidos
description: Catálogo técnico y responsabilidades de los nueve crates internos compartidos de Rust en desktop/crates/.
---

El espacio de trabajo Cargo de escritorio ([/desktop/Cargo.toml](/desktop/Cargo.toml)) divide la lógica compartida en nueve crates internos bajo `desktop/crates/`:

## Catálogo de crates

### 1. `opad-model` ([/desktop/crates/opad-model/src/lib.rs](/desktop/crates/opad-model/src/lib.rs))
Modelos de dominio, validación y utilidades multiplataforma:
- Estructuras principales: `DeviceInfo`, `DeviceConfig`, `CounterState`, `Key3Counters`, `LatencyStats`, `JsonBackup`, `HeKeyConfig`, `HeCalibrationStatus`, `HeLiveSample`.
- Submódulos: `paths` ([/desktop/crates/opad-model/src/paths.rs](/desktop/crates/opad-model/src/paths.rs)), `board_test`, `diag`, `log`, `result_anim`, `swipe`, `tap_rate`, `ui_source`.

### 2. `opad-protocol` ([/desktop/crates/opad-protocol/src/lib.rs](/desktop/crates/opad-protocol/src/lib.rs))
Códec del protocolo serie en Rust puro:
- Compila [/protocol/opad.proto](/protocol/opad.proto) mediante `protox` + `prost` en `build.rs` (sin requerir `protoc` externo).
- Implementa entramado Legacy, Marked (`0xAA 0x55`) y Checked (`0xAA 0x5A` + CRC-16/CCITT-FALSE) (`crc16`).

### 3. `opad-ipc` ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs))
IPC local entre `opad-daemon`, `opad-gui` y `opadctl`:
- Sockets de dominio Unix de flujo (`0700`/`0600`) en [/desktop/crates/opad-ipc/src/transport/unix.rs](/desktop/crates/opad-ipc/src/transport/unix.rs) y Named Pipes de Windows (`\\.\pipe\opad-ipc-<SID>` con SDDL protegida `D:P(A;;GA;;;SY)(A;;GA;;;<SID>)`) en [/desktop/crates/opad-ipc/src/transport/windows.rs](/desktop/crates/opad-ipc/src/transport/windows.rs).
- Entramado JSON UTF-8 delimitado por longitud (`MAX_REQUEST_FRAME_SIZE = 1 MiB`, `MAX_RESPONSE_FRAME_SIZE = 8 MiB`, `IPC_PROTOCOL_VERSION = 1`).

### 4. `opad-storage` ([/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs))
Persistencia SQLite (`opad.db`):
- Modo WAL y claves foráneas activadas al abrir; migraciones integradas hasta el esquema v19 (`schema_migrations`, `device_state`, `config`, `layouts`, `app_state`, estadísticas de pulsación).
- Guardia de escritura durante partida (`StorageError::WritesBlocked`) y `reconcile_counters`.

### 5. `opad-device` ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs))
Gestor de dispositivo serie y flasheador de firmware:
- Autodescubre `303a:4001`, negocia entramado Marked/Legacy (`HELLO_RETRY = 400 ms`) y expone el estado `Ownership`.
- Implementa flasheo sin botones (`flash.rs`: `plan_app_update`, `flash_firmware`, `recover`, `reboot_to_bootloader`) envolviendo `espflash` **4.6.0** fijado más el reinicio por watchdog RTC para salir del modo descarga ROM.

### 6. `opad-layout` ([/desktop/crates/opad-layout/src/lib.rs](/desktop/crates/opad-layout/src/lib.rs))
Esquema de diseño, validación (`MAX_WIDGETS = 32`, límites `320x240`) y serialización de cable/NVS coincidente con `ui_layout_t` del firmware.

### 7. `opad-ui-preview` ([/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs))
Compila el renderizador LVGL en C del firmware en el anfitrión para producir búferes RGBA con vistas previas exactas al píxel en Screen Designer.

### 8. `opad-tosu` ([/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs))
Supervisor del proceso `tosu` (`TosuSupervisor`, orden de búsqueda de binarios, anulaciones forzadas en `tosu.env`), analizador WebSocket v2 + `/precise` y diagnósticos Yama `ptrace_access()` en Linux.

### 9. `opad-update` ([/desktop/crates/opad-update/src/lib.rs](/desktop/crates/opad-update/src/lib.rs))
Verificación de actualizaciones firmadas (minisign Ed25519 + SHA-256), políticas `InstallOrigin` (`windows`, `deb`, `rpm`, `aur`, `appimage`, `user`, `source`), puerta de seguridad de partida (`may_update_now`), escrituras atómicas (`write_atomic`) y el generador de manifiestos `opad-feed`.
