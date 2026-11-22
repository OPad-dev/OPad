---
title: Referencia de Archivos y Rutas
description: Especificación de todas las ubicaciones en el sistema de archivos, rutas de base de datos, archivos de registro, rutas de socket y estructuras de directorios por SO utilizadas por la suite de escritorio de OPad.
category: reference
source_files:
  - /desktop/crates/opad-model/src/paths.rs
  - /desktop/daemon/src/identity.rs
  - /desktop/crates/opad-storage/src/lib.rs
---

La suite de escritorio de OPad resuelve estrictamente todas las rutas persistentes, archivos de base de datos, estados de procesos y binarios empaquetados mediante funciones auxiliares centrales para garantizar un comportamiento predecible en entornos Linux y Windows 10/11. Los daemons iniciados en el arranque de sesión suelen heredar directorios de trabajo impredecibles (como `C:\Windows\System32` en Windows), por lo que nunca se utilizan rutas relativas. <!-- src: /desktop/crates/opad-model/src/paths.rs#PathError -->

## Filesystem Locations by Platform

| Componente | Ubicación en Linux | Ubicación en Windows | Propósito |
|---|---|---|---|
| **Directorio de Datos** | `$XDG_DATA_HOME/opad` (o `~/.local/share/opad`) | `%APPDATA%\opad` | Datos de usuario persistentes (base de datos SQLite). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **Directorio de Estado** | `$XDG_STATE_HOME/opad` (o `~/.local/state/opad`) | `%APPDATA%\opad` | Registros de ejecución desechables. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir --> |
| **Archivo de Base de Datos** | `<data_dir>/opad.db` | `<data_dir>\opad.db` | Base de datos SQLite principal. <!-- src: /desktop/crates/opad-model/src/paths.rs#database_path --> |
| **Registro del Daemon** | `<state_dir>/daemon.log` | `<state_dir>\daemon.log` | Salida estándar/error del proceso daemon. <!-- src: /desktop/crates/opad-model/src/paths.rs#daemon_log_path --> |
| **Registro de tosu** | `<state_dir>/tosu.log` | `<state_dir>\tosu.log` | Salida estándar/error de la instancia supervisada de tosu. <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path --> |
| **Directorio Lib Privado** | `<install_prefix>/lib/opad` | `<install_dir>` | Herramientas auxiliares privadas y marcadores. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir --> |
| **Binario tosu Empaquetado** | `<install_lib_dir>/tosu/tosu` | `<install_lib_dir>\tosu\tosu.exe` | Binario empaquetado para la integración con la memoria de osu!. <!-- src: /desktop/crates/opad-model/src/paths.rs#bundled_tosu_binary --> |
| **Origen de Instalación** | `<install_lib_dir>/install-origin` | `<install_lib_dir>\install-origin` | Marcador del canal de empaquetado (`deb`, `rpm`, `tar`, `msi`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_origin_path --> |

### Linux Path Resolution

En sistemas Linux, la resolución de rutas se adhiere a la especificación de directorios base de XDG: <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **Directorio de datos**: Evalúa `$XDG_DATA_HOME/opad`. Si no está definido o está vacío, recurre a `$HOME/.local/share/opad`.
- **Directorio de estado**: Evalúa `$XDG_STATE_HOME/opad`. Si no está definido o está vacío, recurre a `$HOME/.local/state/opad`.
- **Prefijo de Instalación**: Deducido del árbol de directorios que contiene el binario en ejecución (`opad-daemon` u `opad-gui`). Por ejemplo, `/usr/bin/opad-daemon` genera el prefijo `/usr` y la ruta de bibliotecas `/usr/lib/opad`. Una compilación local en `~/.local/bin/opad-daemon` genera `~/.local/lib/opad`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_prefix -->

### Windows Path Resolution

En Windows 10 y 11: <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- Windows no cuenta con un directorio de estado separado de tipo XDG. Tanto los datos como los registros de ejecución residen bajo `%APPDATA%\opad` (típicamente `C:\Users\<Usuario>\AppData\Roaming\opad`). Esto asegura que el desinstalador tenga una única carpeta de datos a eliminar. <!-- src: /desktop/crates/opad-model/src/paths.rs#state_dir -->
- Los ejecutables de la aplicación, el tosu empaquetado y los marcadores de origen residen juntos en el directorio raíz de instalación (por ejemplo, `C:\Program Files\OPad`). <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->

## SQLite Database (opad.db)

Todo el estado persistente del escritorio se gestiona en una única base de datos SQLite inicializada en modo Write-Ahead Logging (`PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage::open -->

### Schema Tables

La base de datos mantiene el versionado de esquema en `schema_migrations`: <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage -->

- **`app_state`**: Tabla clave-valor que almacena metadatos de la instalación de escritorio: <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage -->
  - `install.id`: Cadena UUIDv4 generada aleatoriamente que identifica de forma única la instalación de escritorio. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->
  - `tap_stats.history_days`: Ventana móvil de retención configurada para las estadísticas de velocidad de pulsación. <!-- src: /desktop/crates/opad-storage/src/lib.rs#HISTORY_DAYS_KEY -->
- **`device_state`**: Almacena el estado persistente específico del dispositivo indexado por número de serie:
  - Cadena del ID de propietario del dispositivo.
  - Conteos de pulsaciones de por vida reconciliados (`lifetime_key1`, `lifetime_key2` y el opcional `lifetime_key3`).
  - Números de secuencia de generación de contadores.
- **`config`**: Estructuras de configuración de dispositivo en caché (`DeviceConfig`) serializadas por ID de serie del dispositivo.
- **`layouts`**: Diseños de pantalla personalizados en JSON para las pantallas `idle` y `playing`.

### Write Invariants

Para evitar que la E/S de almacenamiento interfiera con los tiempos críticos de latencia, las transacciones de base de datos se bloquean durante el juego activo (`PLAYING`) y las fases de enfriamiento (`StorageError::WritesBlocked`). Las actualizaciones pendientes se almacenan en búfer en memoria y se vuelcan una vez que el dispositivo regresa a `IDLE`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#StorageError::WritesBlocked -->

## Install Identity (install.id)

El daemon de escritorio genera una identidad UUIDv4 persistente en su primera ejecución y la almacena bajo la clave `install.id` en `app_state`. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->

- **Alcance**: Por instalación, no por máquina. Si el usuario desinstala y elige "eliminar todos los datos", la base de datos y la identidad se eliminan. Reinstalar genera una identidad nueva. <!-- src: /desktop/daemon/src/identity.rs -->
- **Propiedad**: La representación binaria de 16 bytes se utiliza en los comandos `ClaimOwnership` para reclamar pads de hardware. Los pads registran este ID de propietario de 16 bytes en la memoria flash no volátil (NVS). <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->
- **Operación Degradada**: Si el almacenamiento falla o `opad.db` no puede abrirse, el daemon funciona degradado sin una identidad en lugar de cerrarse abruptamente; simplemente no reclama dispositivos ni solicita reconciliación de contadores. <!-- src: /desktop/daemon/src/identity.rs#load_or_create -->
