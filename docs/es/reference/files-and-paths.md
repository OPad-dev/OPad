---
title: Referencia de archivos y rutas
description: Dónde guarda la suite de escritorio de OPad su base de datos, copias de seguridad, registros, punto de conexión IPC y herramientas incluidas en Linux y Windows, y el esquema SQLite.
sidebar:
  order: 7
---

Todas las rutas que la suite de escritorio lee o escribe se resuelven en `desktop/crates/opad-model/src/paths.rs`. Un daemon iniciado al iniciar sesión hereda un directorio de trabajo impredecible (a menudo `C:\Windows\System32` en Windows), así que ninguna ruta es nunca relativa: cuando no se puede resolver un directorio, la llamada falla en lugar de adivinar.
<!-- src: /desktop/crates/opad-model/src/paths.rs -->

## Ubicaciones

| Qué | Linux | Windows |
|---|---|---|
| Directorio de datos | `$XDG_DATA_HOME/opad`, si no `~/.local/share/opad` | `%APPDATA%\opad` |
| Directorio de estado (registros) | `$XDG_STATE_HOME/opad`, si no `~/.local/state/opad` | `%APPDATA%\opad` (no hay directorio de estado aparte) |
| Base de datos | `<data>/opad.db` | `<data>\opad.db` |
| Copias de seguridad automáticas | `<data>/backups/`, se conservan las 10 más recientes | `<data>\backups\` |
| Registro del daemon (un daemon que inició la GUI) | `<state>/daemon.log` | `<state>\daemon.log` |
| Registro de tosu | `<state>/tosu.log` | `<state>\tosu.log` |
| Punto de conexión IPC | `$XDG_RUNTIME_DIR/opad/daemon.sock` (alternativas en la [Referencia del protocolo IPC](ipc.md#transporte)) | `\\.\pipe\opad-ipc-<SID>` |
<!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir, /desktop/crates/opad-model/src/paths.rs#state_dir, /desktop/daemon/src/backup.rs#backup_dir, /desktop/daemon/src/backup.rs#KEEP, /desktop/crates/opad-ipc/src/transport/unix.rs#get_socket_path, /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->

## Archivos instalados

El directorio de instalación privado se localiza a partir del ejecutable en marcha, así que una sola regla cubre todos los prefijos:
<!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix, /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->

| Qué | Linux | Windows |
|---|---|---|
| Prefijo de instalación | El directorio abuelo del ejecutable (`/usr/bin/opad-daemon` → `/usr`, `~/.local/bin/opad-daemon` → `~/.local`) | El directorio del ejecutable, por defecto `%LOCALAPPDATA%\Programs\opad` (instalación por usuario) |
| Directorio privado (`install_lib_dir`) | `<prefix>/lib/opad` | `<prefix>` |
| tosu incluido | `<lib>/tosu/tosu` | `<prefix>\tosu\tosu.exe` |
| Marcador de origen de instalación | `<lib>/install-origin` | `<prefix>\install-origin` |
| `espflash` fijado | `<prefix>/lib/opad/bin/espflash` (o junto al ejecutable) | `<prefix>\espflash.exe` |
<!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary, /desktop/crates/opad-model/src/paths.rs#install_origin_path, /desktop/crates/opad-device/src/flash.rs#resolve_espflash, /packaging/windows/installer.iss -->

Si no encuentra un `espflash` incluido, el flasheador prueba `%USERPROFILE%\.cargo\bin\espflash.exe` en Windows y después `espflash` en el `PATH`.
<!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

El marcador `install-origin` indica cómo se instaló OPad, lo que decide cómo se actualiza: `windows`, `deb`, `rpm`, `aur`, `appimage`, `user` o `source`. Cualquier otro valor se interpreta como desconocido.
<!-- src: /desktop/crates/opad-update/src/origin.rs#InstallOrigin -->

## Base de datos SQLite (`opad.db`)

La base de datos se abre con `PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`. Las migraciones aplicadas se registran en `schema_migrations (version, applied_at)`; el esquema actual es v19.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#open, /desktop/crates/opad-storage/src/lib.rs#migrate -->

| Tabla | Clave | Contenido | Desde |
|---|---|---|---|
| `device_state` | `device_id` (`OPAD-<MAC>`) | `board_profile`, `firmware_version`, `counter_generation`, `lifetime_key1`, `lifetime_key2`, `lifetime_key3` (NULL hasta que se sincroniza un teclado de tres teclas), `last_seen_at`, `last_sync_at` | v1; tecla 3 v18 |
| `config` | `id` = `1` (una sola fila) | La copia de la aplicación de la configuración del teclado: teclas, antirrebote, brillo, reposo de pantalla, frecuencia de refresco en juego, `tosu_endpoint`, `press_color_rgb`, pines de tecla, acciones/teclas/modificadores de deslizamiento, `result_animations`, `key3_hid_usage`, `game_keys` | v1, columnas añadidas en v2–v17 |
| `layouts` | `screen` (`0` = Idle, `1` = Playing) | `json`, `updated_at` | v3 |
| `app_state` | `key` | `value`, `updated_at` | v7 |
| `tap_attempts` | `id` | Una fila por intento terminado: tiempos, beatmap, mods, título/artista/dificultad, por canal (`k1`, `k2`, `combined`) número de pulsaciones, intervalos, media y pico, número de pulsaciones y duración de la canción, pico de la canción | v9; canción v10, pico de la canción v11 |
<!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v1, /desktop/crates/opad-storage/src/lib.rs#apply_v3, /desktop/crates/opad-storage/src/lib.rs#apply_v7, /desktop/crates/opad-storage/src/lib.rs#apply_v18, /desktop/crates/opad-storage/src/tap_stats.rs#apply_v9, /desktop/crates/opad-layout/src/lib.rs#Screen -->

La migración v19 renombró los teclados guardados como `OSUPAD-<MAC>` (firmware hasta `1.0.0-rc`) a `OPAD-<MAC>`, conservando sus contadores.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->

### Claves de `app_state`

| Clave | Contiene |
|---|---|
| `install.id` | La identidad de esta instalación (ver más abajo) |
| `stats.history_days` | El periodo del historial de estadísticas, `0` = todo el historial |
| `update.app.enabled`, `update.tosu.enabled`, `update.firmware.auto` | Interruptores de actualización automática; si faltan = activados |
| `update.channel` | `stable` (predeterminado) o `beta` |
| `update.schedule`, `update.last_result` | El calendario de comprobaciones del actualizador y el resultado de la última instalación |
<!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY, /desktop/crates/opad-storage/src/tap_stats.rs#HISTORY_DAYS_KEY, /desktop/daemon/src/updater.rs#APP_ENABLED_KEY, /desktop/daemon/src/updater.rs#read_flag -->

### Bloqueo de escritura

El daemon desactiva las escrituras en el almacenamiento durante un mapa y su enfriamiento: entonces toda escritura devuelve `StorageError::WritesBlocked`. Los datos que deben sobrevivir, como los intentos de pulsación terminados y un nuevo periodo de historial, se mantienen en memoria y se guardan en cuanto vuelven a permitirse las escrituras.
<!-- src: /desktop/crates/opad-storage/src/lib.rs#check_writes_allowed, /desktop/daemon/src/runtime.rs, /desktop/crates/opad-ipc/src/lib.rs#TapStatsSnapshot, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

## Identidad de instalación

En la primera ejecución, el daemon genera un UUIDv4 y lo guarda como `install.id`. Sus 16 bytes son lo que un teclado registra como su propietario (`ClaimOwnership`). Es por instalación, no por máquina: desinstalar borra el directorio de datos (el desinstalador de Windows, y `uninstall-cleanup` de los paquetes de Linux para las rutas XDG predeterminadas), así que una reinstalación obtiene una identidad nueva. Protege la integridad de los contadores, no la exclusividad; no tiene nada de criptográfico.
<!-- src: /desktop/daemon/src/identity.rs, /packaging/windows/installer.iss, /packaging/linux/uninstall-cleanup.sh -->

Si no se puede abrir la base de datos, el daemon funciona sin identidad: no reclama teclados ni muestra avisos de propiedad.
<!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
