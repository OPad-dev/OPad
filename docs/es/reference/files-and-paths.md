---
title: Referencia de archivos y rutas
description: Ubicaciones del sistema de archivos, esquema de la base de datos SQLite, archivos de registro, rutas privadas de herramientas e identidad de instalación usadas por la suite de escritorio de OPad en Linux y Windows.
---

La suite de escritorio de OPad resuelve todas las rutas persistentes, registros y binarios privados incluidos a través de `desktop/crates/opad-model/src/paths.rs`. Como los daemons iniciados al iniciar sesión heredan directorios de trabajo impredecibles (como `C:\Windows\System32` en Windows), OPad nunca recurre a rutas relativas. <!-- src: /desktop/crates/opad-model/src/paths.rs#PathError -->

## Ubicaciones en el sistema de archivos por plataforma

| Componente | Ubicación en Linux | Ubicación en Windows | Propósito |
|---|---|---|---|
| **Directorio de datos** | `$XDG_DATA_HOME/opad` (respaldo `~/.local/share/opad`) | `%APPDATA%\opad` | Datos persistentes del usuario (`opad.db`, copias automáticas). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **Directorio de estado** | `$XDG_STATE_HOME/opad` (respaldo `~/.local/state/opad`) | `%APPDATA%\opad` | Registros de ejecución desechables. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir --> |
| **Archivo de base de datos** | `<data_dir>/opad.db` | `<data_dir>\opad.db` | Base de datos SQLite principal (modo WAL). <!-- src: /desktop/crates/opad-model/src/paths.rs#database_path --> |
| **Copias de seguridad automáticas** | `<data_dir>/backups/` | `<data_dir>\backups\` | Archivos `JsonBackup` automáticos rotativos (se conservan los 10 más recientes). <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> |
| **Registro del daemon** | `<state_dir>/daemon.log` | `<state_dir>\daemon.log` | Salida de un daemon lanzado por la GUI. <!-- src: /desktop/crates/opad-model/src/paths.rs#daemon_log_path --> |
| **Registro de tosu** | `<state_dir>/tosu.log` | `<state_dir>\tosu.log` | Salida del proceso `tosu` supervisado. <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path --> |
| **Punto de conexión IPC** | `$XDG_RUNTIME_DIR/opad/daemon.sock` (respaldos `/run/user/<uid>/opad/daemon.sock`, `/tmp/opad-<uid>/daemon.sock`) | `\\.\pipe\opad-ipc-<SID>` | Socket de dominio Unix / Named Pipe IPC local del daemon. <!-- src: /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path --> <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path --> |
| **Directorio privado de librerías** | `<install_prefix>/lib/opad` | `<install_dir>` (`%LOCALAPPDATA%\Programs\opad`) | `tosu` incluido, `espflash` fijado e `install-origin`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir --> |
| **Binario de tosu incluido** | `<install_lib_dir>/tosu/tosu` | `<install_lib_dir>\tosu\tosu.exe` | Lector de memoria de osu! incluido. <!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary --> |
| **Origen de instalación** | `<install_lib_dir>/install-origin` | `<install_lib_dir>\install-origin` | Marcador de canal de empaquetado (`windows`, `deb`, `rpm`, `aur`, `appimage`, `user`, `source`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_origin_path --> <!-- src: /desktop/crates/opad-update/src/origin.rs#InstallOrigin --> |

### Resolución de rutas en Linux

- **Directorios de datos y estado** siguen XDG (`$XDG_DATA_HOME/opad` / `$XDG_STATE_HOME/opad`, por defecto `~/.local/share/opad` y `~/.local/state/opad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **Prefijo de instalación** se deriva del directorio abuelo del ejecutable en curso (por ejemplo, `/usr/bin/opad-daemon` -> prefijo `/usr` -> `/usr/lib/opad`; `~/.local/bin/opad-daemon` -> `~/.local/lib/opad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix -->

### Resolución de rutas en Windows

- Windows no tiene un directorio de estado XDG separado, por lo que tanto los datos persistentes (`opad.db`, `backups\`) como los registros (`daemon.log`, `tosu.log`) residen en `%APPDATA%\opad`. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- El instalador de Inno Setup instala por usuario (`PrivilegesRequired=lowest`) en `%LOCALAPPDATA%\Programs\opad`, colocando juntos los binarios, `espflash.exe`, `install-origin` y `tosu\tosu.exe` en ese directorio. <!-- src: /packaging/windows/installer.iss -->

## Base de datos SQLite (`opad.db`)

`opad.db` se abre en modo Write-Ahead Logging (`PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`) y registra las migraciones en `schema_migrations` (actualmente esquema v19). <!-- src: /desktop/crates/opad-storage/src/lib.rs#open -->

### Tablas principales

- **`app_state`**: Metadatos clave-valor del anfitrión (`key`, `value`, `updated_at`), que guardan `install.id`, `tap_stats.history_days` y ajustes/programaciones del actualizador. <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v7 -->
- **`device_state`**: Filas por teclado indexadas por el `device_id` canónico (`OPAD-<MAC>`), guardando `board_profile`, `firmware_version`, `counter_generation`, `lifetime_key1`, `lifetime_key2`, `lifetime_key3`, `last_seen_at` y `last_sync_at`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1 --> <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v18 -->
- **`config`**: Fila única (`id = 1`) que almacena en caché `DeviceConfig` (códigos de tecla, pines GPIO, antirrebote, brillo, reposo, Hz de refresco en partida, `tosu_endpoint`, acciones/teclas/modificadores de deslizamiento, `result_animations`, `key3_hid_usage`, `game_keys`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1 -->
- **`layouts`**: JSON de diseños de pantalla personalizados indexados por `screen` (`Idle` / `Playing`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v3 -->
- **Tablas de estadísticas de pulsación** (`apply_v10` / `apply_v11`): Ritmos de pulsación de intentos registrados e historial. <!-- src: /desktop/crates/opad-storage/src/lib.rs#migrate -->

### Guardia de escritura

Todas las mutaciones en SQLite comprueban `writes_allowed` y devuelven `StorageError::WritesBlocked` durante `Playing` y `Cooldown`; las escrituras pendientes se vuelcan en cuanto el daemon regresa a `Idle`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#WritesBlocked -->

## Identidad de instalación (`install.id`)

En la primera ejecución, `opad-daemon` genera un UUIDv4 guardado bajo `install.id` en `app_state`: <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->

- **Ámbito por instalación**: Desinstalar OPad borra el directorio de datos de escritorio (`%APPDATA%\opad` en Windows, `~/.local/share/opad` en desinstalación de paquetes Linux), por lo que reinstalar crea un nuevo `install.id`. <!-- src: /desktop/daemon/src/identity.rs -->
- **Propiedad del teclado**: Su forma binaria de 16 bytes (`OWNER_ID_LEN = 16`) se escribe en el teclado mediante `ClaimOwnership`. <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->
- **Modo degradado**: Si no se puede abrir `opad.db`, el daemon se ejecuta sin identidad y omite reclamar teclados o actualizar los contadores del PC en lugar de cerrarse inesperadamente. <!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
