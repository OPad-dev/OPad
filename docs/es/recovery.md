---
title: Reconciliación de contadores, recuperación y desvinculación
description: Reglas de reconciliación de contadores, procedimientos de flasheo, entrada en el bootloader y recuperación de un teclado que no enumera.
---

## 1. Modelo de almacenamiento dual

Tanto el ESP32-S3 como el PC anfitrión mantienen contadores de pulsaciones de por vida para las teclas físicas:

- **Dispositivo (ESP32-S3)**: Los contadores se incrementan en RAM dentro de la ruta de eventos de tecla en Core 0, sin ninguna escritura en flash durante las pulsaciones. Los contadores se guardan en NVS únicamente en estado `IDLE` — al salir de `COOLDOWN`, según la programación periódica en reposo (`counters_checkpoint_due`) o al suspender el anfitrión USB (`counters_request_checkpoint`). En los teclados Hall Effect V3 de tres teclas, Key 3 conserva su propia clave NVS (`key3_cnt`). <!-- src: /firmware/main/counters/counters.c#counters_checkpoint, /firmware/main/runtime/runtime.c#runtime_supervisor_task, /firmware/main/app_main.c#usb_event_handler -->
- **PC anfitrión**: Almacenados en SQLite (tabla `device_state` en `opad.db`). Todas las escrituras en SQLite están bloqueadas durante `PLAYING` y `COOLDOWN` (`set_writes_allowed(false)`). El daemon mantiene los conteos en vivo en memoria y escribe en SQLite durante `SYNC` e `IDLE`. El conteo de por vida de Key 3 se guarda en `device_state.lifetime_key3`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage, /desktop/daemon/src/runtime.rs#RuntimeController -->

## 2. Generación de contadores y reconciliación monotónica

Cada registro de contador sigue estos campos:

- `device_id`: Derivado de la dirección MAC Wi-Fi STA del ESP32-S3 (`OPAD-<MAC>`). El firmware más antiguo usaba `OSUPAD-<MAC>`, que el anfitrión canoniza a `OPAD-<MAC>` y migra en SQLite. <!-- src: /firmware/main/protocol/protocol.c#get_device_mac_string, /desktop/crates/opad-model/src/lib.rs#canonical_device_id, /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->
- `counter_generation`: Entero de 32 bits que aumenta de forma monotónica (comienza en `1` en una NVS nueva). Se incrementa cada vez que los contadores se reinician, se restauran desde una copia de seguridad, se migran desde un teclado reemplazado, se reconcilian manualmente o se asumen desde otra instalación. <!-- src: /firmware/main/counters/counters.c#counters_init, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- `lifetime_key1` y `lifetime_key2`: Totales de pulsaciones de por vida para Key 1 y Key 2. En teclados Hall Effect V3 de tres teclas, `lifetime_key3` se registra por separado de la reconciliación de Key 1 y Key 2. <!-- src: /desktop/crates/opad-model/src/lib.rs#CounterState, /desktop/crates/opad-model/src/lib.rs#Key3Counters -->

### Reglas de reconciliación

1. **Precedencia generacional**: Si `generation_A > generation_B`, la generación mayor gana incondicionalmente. Esto impide que una base de datos o copia de seguridad antigua sobrescriba un reinicio o una importación. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Fusión monotónica (misma generación)**: Cuando las generaciones coinciden, se conserva el valor máximo para cada tecla (`max(pc.k1, esp.k1)` y `max(pc.k2, esp.k2)`). Sin `force_restore`, el firmware rechaza cualquier sincronización con una generación menor (`stale generation`) o con un conteo inferior en la misma generación (`non-monotonic`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Política ante pérdida de energía**: Si la alimentación USB se desconecta durante un mapa activo o durante el enfriamiento antes de un punto de guardado `IDLE`, se pierden las pulsaciones sin guardar desde el último guardado en NVS. Este compromiso mantiene las escrituras en flash fuera de la ruta de pulsación. <!-- src: /firmware/main/counters/counters.c#checkpoint_locked, /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## 3. Matriz de reconciliación al conectar

Cuando el daemon se conecta por USB CDC y recibe `HelloAck`, primero comprueba la compatibilidad del protocolo (`protocol_version == DEVICE_PROTOCOL_VERSION`) y la propiedad del teclado (`owner_id`), y después reconcilia los contadores cuando está en `IDLE` y tiene permiso para operar sobre el teclado: <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/sync.rs#perform_sync -->

| Condición | Acción ejecutada |
|---|---|
| **El teclado pertenece a otra instalación (`owner_id` distinto)** | Se activa la toma de propiedad antes de cualquier sincronización. Si el teclado tiene al menos tantas pulsaciones como este PC en ambas teclas, el daemon asume la propiedad en silencio conservando los contadores del teclado (`TakeOverIfPadIsAhead`). De lo contrario, se pausa la sincronización de contadores y la GUI muestra el aviso de toma de propiedad. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead --> |
| **Teclado conocido, misma generación** | El daemon fusiona `max(pc, esp)` para Key 1 y Key 2 y sincroniza tanto el teclado como SQLite. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Teclado conocido, generación del dispositivo > generación del anfitrión** | El anfitrión actualiza SQLite para igualar la generación y los conteos del teclado. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Teclado conocido, generación del anfitrión > generación del dispositivo** | El anfitrión envía `CounterSync` para restaurar sus conteos registrados en el teclado. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Teclado conocido con NVS borrada (`gen <= 1`, `k1 == 0`, `k2 == 0`, mientras el PC tiene pulsaciones)** | El daemon marca `nvs_restore_pending`, conserva la configuración guardada en el PC en lugar de adoptar los valores predeterminados del teclado, envía la configuración del PC y restaura los contadores (incluido Key 3 si estaba guardado) al sincronizar. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/sync.rs#key3_sync_target --> |
| **Teclado nuevo (`device_id` desconocido), dispositivo previo en SQLite, `gen <= 1`, `k1 < 1000`, `k2 < 1000` y el PC va por delante** | Activa el **flujo de reemplazo de dispositivo** (`pending_replacement`). La sincronización se pausa hasta que respondas. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/runtime.rs#pc_is_ahead --> |
| **Teclado nuevo (primer teclado registrado o sin cumplir criterios de reemplazo)** | Se registra en SQLite con su generación y conteos reportados. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |

## 4. Flujo de reemplazo de dispositivo

Cuando conectas un OPad de reemplazo en un PC que ya tiene historial de contadores de un teclado anterior:

1. Si el `device_id` conectado es desconocido, la base de datos ya tiene un teclado previo, el nuevo teclado reporta `counter_generation <= 1` con `lifetime_key1 < 1000` y `lifetime_key2 < 1000`, y el PC tiene más pulsaciones que el nuevo teclado en al menos una tecla (`pc_is_ahead`), el daemon establece `pending_replacement` al `device_id` previo conocido y bloquea la sincronización hasta que se resuelva. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/runtime.rs#pad_guard -->
2. La GUI muestra un aviso:
   > `This looks like a new pad. Restore counters from <old_id>?`
   con dos botones: **Restore from previous pad** y **Treat as new pad**. `opadctl status` también imprime una línea de advertencia `Replacement:`. <!-- src: /desktop/gui/src/main.rs#App, /desktop/cli/src/main.rs#Commands -->
3. **Si haces clic en Restore from previous pad** (`IpcRequest::ResolveReplacement { restore: true }`):
   - El daemon establece `counter_generation` en `max(old_generation, current_generation) + 1`.
   - Copia los conteos de por vida de Key 1, Key 2 (y Key 3 si estaba registrado) del teclado anterior al nuevo `device_id` en SQLite.
   - Envía `CounterSync` con `force_restore = true` para escribir los conteos heredados en el nuevo teclado. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
4. **Si haces clic en Treat as new pad** (`IpcRequest::ResolveReplacement { restore: false }`):
   - Registra el nuevo teclado como un dispositivo independiente en SQLite con sus propios conteos actuales.
   - La fila del teclado anterior permanece en SQLite como dato histórico. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## 5. Acciones de copia de seguridad, restauración y reinicio

Todas las operaciones de escritura siguientes se rechazan durante `PLAYING` y `COOLDOWN`, cuando el almacenamiento SQLite no está disponible o mientras exista un aviso pendiente de toma de propiedad o reemplazo. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

- **Reiniciar contadores**:
  - Ejecuta `opadctl reset --yes`, o en la GUI abre **Device** -> **Reset lifetime counters**, escribe `RESET` y haz clic en **Confirm Reset**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device, /desktop/gui/src/main.rs#App -->
  - Pone a `0` los contadores de por vida de Key 1, Key 2 y Key 3, incrementa `counter_generation` en 1, guarda en SQLite y envía un `CounterSync` forzado al teclado (o encola `PendingCounterReset` si el teclado está desconectado para enviarlo en la siguiente sincronización). <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request, /desktop/daemon/src/sync.rs#perform_sync -->
- **Exportar copia de seguridad**:
  - Ejecuta `opadctl export <file>`, o en la GUI abre **Device** -> **Export backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->
  - Escribe un archivo JSON portable (`JsonBackup`) que contiene:
    - `format_version`: `1` y `exported_at` (marca temporal UTC).
    - `device`: `device_id`, `board_profile`, `counter_generation`.
    - `stats`: `lifetime_key1`, `lifetime_key2` y `lifetime_key3` (en teclados de tres teclas cuando se conoce).
    - `config`: `key1`, `key2`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, acciones/teclas/modificadores de deslizamiento para las cuatro direcciones, `result_animations`, además de `key3` y `game_keys` en teclados de tres teclas. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- **Importar copia de seguridad (`PreviewImport` e `ImportBackup`)**:
  - Ejecuta `opadctl import <file>` (añade `--yes` para omitir la confirmación en terminal), o en la GUI abre **Device** -> **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->
  - Flujo de seguridad en dos fases:
    1. `PreviewImport`: Valida `format_version == 1`, `board_profile == "waveshare_esp32s3_touch_lcd_2"`, usos de teclas, asignaciones de pines, rangos numéricos y marca temporal (rechaza marcas más de 1 día en el futuro). Devuelve una diferencia entre el estado actual y el entrante más advertencias por discrepancia de ID de dispositivo, retroceso de contadores o salto de generación. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
    2. `ImportBackup`: Requiere confirmación explícita (`confirm: true`). En la GUI, si algún contador entrante es menor que el valor actual, también debes marcar `I understand this replaces my counters with lower values` antes de hacer clic en **Apply Backup**. Conserva tu `tosu_endpoint` local, eleva `counter_generation` a `max(current_gen, backup_gen) + 1`, guarda en SQLite y envía tanto la configuración como un `CounterSync` forzado al teclado. <!-- src: /desktop/gui/src/main.rs#App, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 5.1 Copias de seguridad automáticas

Además de las exportaciones manuales, `opad-daemon` escribe una copia de seguridad JSON automática en el mismo formato `JsonBackup` tras cada sesión de juego: <!-- src: /desktop/daemon/src/backup.rs#current, /desktop/daemon/src/backup.rs#write -->

| Campo | Comportamiento |
|---|---|
| Ubicación | `<data>/backups/opad-backup-YYYYMMDDTHHMMSSZ.json` — `$XDG_DATA_HOME/opad/backups` (por defecto `~/.local/share/opad/backups`) en Linux, `%APPDATA%\opad\backups` en Windows. <!-- src: /desktop/daemon/src/backup.rs#backup_dir, /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| Cuándo | **20 segundos** (`AUTO_BACKUP_DELAY`) después de completarse un ciclo post-partida `COOLDOWN` -> `SYNC` -> `IDLE`. Las sincronizaciones periódicas en reposo no disparan una copia de seguridad. <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY --> |
| Retención | Conserva los **10 archivos más recientes** (`KEEP = 10`) `opad-backup-*.json` y elimina los más antiguos. Los archivos con otros nombres en esa carpeta nunca se borran. <!-- src: /desktop/daemon/src/backup.rs#KEEP, /desktop/daemon/src/backup.rs#prune --> |
| Seguridad de escritura | Se escribe de forma atómica mediante archivo temporal + `fsync` + renombrado (`write_atomic`). Cualquier error de E/S se registra y se absorbe para que un fallo de copia nunca interrumpa la sincronización ni el juego. <!-- src: /desktop/daemon/src/backup.rs#write, /desktop/daemon/src/backup.rs#write_and_log --> |
| Restauración | `opadctl import <file>` o en la GUI **Device** -> **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device --> |
| Última copia | Se muestra en `opadctl status` bajo `Last Backup`, y el daemon la registra cada vez que la escribe. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/backup.rs#write_and_log --> |

La espera de 20 segundos solo comienza una vez alcanzado `IDLE`, y la escritura vuelve a comprobar `mode == RuntimeMode::Idle` antes de ejecutarse. Empezar otro mapa durante esos 20 segundos cancela la copia pendiente en lugar de posponerla; la siguiente sesión programará una nueva. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

Las copias automáticas se programan incluso si falla la sincronización post-partida, de modo que el daemon preserva los últimos conteos que tiene en memoria. Son una copia de lo que guarda el daemon, no un sustituto de una sincronización funcional. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 6. Operaciones diferidas durante el juego

Durante los estados `PLAYING` y `COOLDOWN`:

- Todas las escrituras en SQLite (`Storage::check_writes_allowed`) y en la NVS del ESP32-S3 (`device_config.c`, `counters.c`) están bloqueadas. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage, /firmware/main/counters/counters.c#checkpoint_locked, /firmware/main/config/device_config.c#write_to_nvs -->
- Los cambios de asignación de teclas, pines de teclas y antirrebote se encolan en `PendingOperations::pending_config` y devuelven `OperationDeferred` hasta `IDLE`. El brillo, el tiempo de reposo de pantalla y `gameplay_display_hz` se aplican inmediatamente en RAM mientras encolan la persistencia para `SYNC`. Las ediciones y reinicios de diseño se aplican en RAM del teclado y encolan la persistencia SQLite/NVS en `pending_layouts`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- La sincronización manual, el reinicio de contadores, la importación de copias, la resolución de propiedad y el flasheo de firmware se rechazan durante `PLAYING` y `COOLDOWN`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- Al terminar la partida (o si `tosu` se desconecta jugando), `COOLDOWN` dura **5 segundos** (`COOLDOWN_DURATION` en el anfitrión, `COOLDOWN_DURATION_US` en el firmware). Al expirar el enfriamiento, el daemon pasa a `SYNC` -> `IDLE`, rehabilita las escrituras de almacenamiento, sincroniza los contadores y vacía las operaciones pendientes. En el firmware, el supervisor de ejecución guarda en NVS los contadores, configuración, diseños y calibración Hall Effect pendientes en la transición `COOLDOWN` -> `IDLE`. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION, /desktop/daemon/src/sync.rs#perform_sync, /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

## 7. Reflasheo y desvinculación de un teclado

Borrar y reflashear el teclado es la única forma de eliminar su registro de propiedad (`owner_id`). No existe un botón de desvinculación en la aplicación, y el firmware rechaza cualquier mensaje `ClaimOwnership` con ceros (`OWNER_CLAIM_REJECT_INVALID`). <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner -->

Si llegaste a esta sección desde el aviso de toma de propiedad, no necesitas reflashear solo para seguir usando el teclado: hacer clic en **Leave it alone** mantiene el teclado funcionando como teclado USB HID sin modificar su propietario ni sus contadores. Usa esta sección al entregar el teclado a otra persona o al recuperar un teclado que no arranca. <!-- src: /desktop/gui/src/main.rs#App, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 7.1 Qué implica

El registro de propietario de 16 bytes (`owner_id`) vive dentro de la partición NVS del teclado junto con los contadores de por vida, la configuración del dispositivo, los diseños personalizados y la calibración Hall Effect. Borrar la flash elimina todo en conjunto. <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner, /firmware/partitions.csv -->

> **`espflash erase-flash` borra los contadores de por vida, la configuración, los diseños, la calibración y el registro de propiedad del teclado.**

Borra los 16 MB completos de flash SPI (`CONFIG_ESPTOOLPY_FLASHSIZE_16MB`), incluidos el bootloader de segunda etapa y la tabla de particiones, de modo que el teclado deja de actuar como teclado hasta que el §7.4 vuelva a flashear el firmware. <!-- src: /firmware/sdkconfig.defaults -->

El teclado no puede quedar inutilizado de forma permanente al borrar la flash: el bootloader ROM de primera etapa del ESP32-S3 reside en ROM de máscara, por lo que un teclado con flash vacía sigue enumerando por USB como `303a:1001` y acepta flasheo. <!-- src: /desktop/crates/opad-device/src/lib.rs#ESP_ROM_BOOTLOADER_PID, /desktop/crates/opad-device/src/flash.rs -->

### 7.2 Exporta primero

Exporta una copia de seguridad JSON manual antes de borrar nada, ya que es la única copia que sobrevive a una reinstalación de la aplicación de escritorio: <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->

```sh
opadctl export ~/opad-backup.json
```

O en la GUI: **Device** -> **Export backup**. También puedes revisar las copias automáticas existentes de §5.1: <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/backup.rs#backup_dir -->

```sh
opadctl status | grep 'Last Backup'
ls -t ~/.local/share/opad/backups/ | head   # Windows: %APPDATA%\opad\backups
```

Tras reflashear, restaura la copia con `opadctl import ~/opad-backup.json` (o en la GUI **Device** -> **Import backup**). <!-- src: /desktop/cli/src/main.rs#Commands -->

Nota sobre la propiedad tras el borrado: un teclado borrado tiene un `owner_id` de ceros, y la primera instalación de OPad a la que se conecte lo reclama en silencio (`Ownership::Unclaimed`). Si se trata del mismo PC desde el que lo borraste, el daemon también detecta `nvs_wiped` y restaura sus contadores y configuración guardados (§3). Si vas a entregar el teclado a otra persona, conéctalo primero al equipo del nuevo propietario para que su instalación lo reclame limpiamente. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

### 7.3 Pon el teclado en modo de descarga y libera el puerto

```sh
opadctl bootloader
```

`opadctl bootloader` pide a `opad-daemon` que libere el puerto serie (`IpcRequest::PrepareFlash`), activa el modo de descarga ROM por USB, reanuda el daemon (`IpcRequest::ResumeDevice`) e imprime el puerto del bootloader (`303a:1001`). También funciona cuando `opad-daemon` no se está ejecutando. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->

En Windows los identificadores serie son exclusivos (`PortBusy`), así que cierra cualquier monitor serie u otro proceso que mantenga abierto el puerto COM antes de ejecutar `opadctl bootloader` o `espflash`. En cualquier sistema operativo puedes listar los puertos con `espflash list-ports` (o en Windows Administrador de dispositivos -> Puertos (COM y LPT)). <!-- src: /desktop/crates/opad-device/src/flash.rs#FlashError, /desktop/cli/src/main.rs#prepare_flash -->

Para entrar en modo de descarga sin pulsar ningún botón, `enter_bootloader` prueba dos disparadores en orden (`BOOT_TRIGGERS`): primero el comando en texto plano `BOOTLOADER\n` por CDC, y después una apertura y cierre a 1200 baudios (bajando DTR). El firmware maneja ambos desconectando TinyUSB (`tud_disconnect()`), cambiando el PHY interno a USB-Serial-JTAG, manteniendo SE0 durante 300 ms, restaurando `dp_pu = 1` antes de soltar la anulación de pull, esperando hasta 1.5 s un reinicio del bus USB, activando `RTC_CNTL_FORCE_DOWNLOAD_BOOT` y llamando a `esp_restart()`. Si ninguno funciona, usa `opadctl recover` (§7.7). Nunca es necesario pulsar los botones BOOT o RESET. <!-- src: /desktop/crates/opad-device/src/flash.rs#BOOT_TRIGGERS, /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->

### 7.4 Borrar y luego flashear

Con el teclado en modo de descarga ROM (`303a:1001`):

**Linux**

```sh
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
opadctl flash --full ~/Downloads/opad-1.0.0/   # or firmware/build
```

**Windows (PowerShell o Símbolo del sistema)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
opadctl flash --full C:\Users\you\Downloads\opad-1.0.0\
```

Usa `--before no-reset --after no-reset-no-stub` para que `espflash` deje el teclado en modo de descarga para `opadctl flash --full`. No desconectes el teclado mientras `erase-flash` esté en ejecución. <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images -->

`--full` es obligatorio después de `erase-flash` porque el bootloader y la tabla de particiones fueron borrados. `resolve_flash_set` escribe las imágenes en orden ascendente de desplazamiento — bootloader en `0x0`, tabla de particiones en `0x8000`, `ota_data_initial.bin` en `0xf000` (cuando está presente) y `opad-firmware.bin` en `0x20000` (`ota_0`) — y luego reinicia el teclado hacia la aplicación mediante un reinicio del watchdog RTC (`reset_to_app`). Acepta tanto un directorio `firmware/build` de ESP-IDF como un directorio de lanzamiento desempaquetado. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set, /desktop/crates/opad-device/src/flash.rs#reset_to_app -->

### 7.5 Confirmar

```sh
opadctl status
```

Un teclado que arrancó correctamente informa `ESP32 Device: Connected` y `Running Slot: ota_0`. <!-- src: /desktop/cli/src/main.rs#Commands -->

En un equipo que nunca haya visto este teclado, los contadores comienzan en 0 con generación 1. En el equipo desde el que lo borraste, el daemon restaura los contadores guardados y vuelve a reclamar la propiedad (§7.2). <!-- src: /firmware/main/counters/counters.c#counters_init, /desktop/daemon/src/runtime.rs#RuntimeController -->

Si el teclado no vuelve como la aplicación OPad en un plazo de 15 segundos tras un flasheo `--full`, `opadctl flash` informa un error. Vuelve a ejecutar el paso `opadctl flash --full` (no borres otra vez). Si el teclado salió del modo de descarga y no enumera como OPad (`303a:4001`), usa `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 7.6 Cuando se interrumpe el flasheo

Una actualización solo de aplicación (`opadctl flash <app.bin>` con el daemon informando la ranura activa, o una actualización desde la GUI / `opadctl firmware-update`) llama a `plan_app_update` para escribir primero en la ranura OTA inactiva (`ota_1` si corre en `ota_0`, o `ota_0` si corre en `ota_1`) y escribir `otadata` en segundo lugar para que el bootloader pruebe la nueva ranura una vez manteniendo la anterior como `OTA_IMG_VALID`. (Si `opadctl flash <app.bin>` se ejecuta sin conocer la ranura activa — por ejemplo sin daemon en ejecución o en firmware antiguo — recurre a escribir `ota_0` en su sitio en `0x20000` sin respaldo de reversión.) <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update, /desktop/cli/src/main.rs#Commands -->

- **Interrumpido antes de escribir `otadata`**: La selección de arranque sigue apuntando a la ranura anterior, por lo que el teclado arranca su firmware previo sin cambios. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
- **Escrito, pero la nueva imagen falla**: La nueva imagen arranca en estado `ESP_OTA_IMG_PENDING_VERIFY`. Al final de `app_main`, `confirm_image_once_usb_works` espera hasta que el anfitrión configure el USB (`tud_mounted()`). Si un anfitrión USB se conecta (`tud_connected()`) y no configura el USB en 20 segundos (`IMAGE_CONFIRM_TIMEOUT_US`), el firmware llama a `esp_restart()`, y el bootloader marca la imagen de prueba como abortada y arranca la ranura válida anterior. Un bucle de reinicios revierte del mismo modo. Si se alimenta sin anfitrión USB (como un cargador de pared), la imagen queda pendiente hasta conectarse a un anfitrión. Cuando ocurre una reversión, `opadctl flash` informa `"The pad did not accept the new firmware (it never came up as a working USB device) and went back to the previous one on <slot>. It works as before; nothing else is needed."` <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /desktop/cli/src/main.rs#report_slot_after_flash -->

Un flasheo `--full` (§7.4) escribe `ota_0` directamente sin respaldo. Si se interrumpe, recupera el teclado con `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands -->

### 7.7 Un teclado que no aparece como OPad (`opadctl recover`)

Si el firmware arranca pero nunca enumera por USB (por ejemplo `can't set config #1, error -32` en `dmesg` de Linux), nada puede pedir a la aplicación por CDC que entre en modo de descarga. Recupéralo capturando la ventana de la ROM al encender: <!-- src: /desktop/crates/opad-device/src/flash.rs#recover, /desktop/cli/src/main.rs#Commands -->

```sh
opadctl recover firmware/build   # or an unpacked release directory
```

Cuando aparezca el mensaje (`Unplug the pad's USB cable and plug it back in (waiting 300 s)...`, configurable con `--timeout`), desconecta y vuelve a conectar el cable USB del teclado. En cada encendido, antes de que la aplicación tome el control del PHY USB, la interfaz ROM USB-Serial-JTAG del ESP32-S3 aparece brevemente como `303a:1001`. `flash::recover` detecta ese puerto (sondeando directamente `/sys/class/tty/ttyACM*` en Linux porque udev es demasiado lento para la ventana de ~200 ms, y `bootloader_ports()` en Windows), reinicia el chip en modo de descarga vía DTR/RTS (`usj_reset_to_download`), ejecuta `SYNC` de ROM y desactiva los watchdogs de arranque (`disable_watchdogs`), escribe el conjunto completo de imágenes (`write_images`), reinicia en la aplicación (`reset_to_app`) y espera hasta 15 segundos al puerto de la aplicación OPad (`303a:4001`). Nunca se necesita ningún botón BOOT ni RESET. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover, /desktop/crates/opad-device/src/flash.rs#rom_port_names, /desktop/crates/opad-device/src/flash.rs#disable_watchdogs, /desktop/cli/src/main.rs#Commands -->

Solo un ciclo de alimentación real hace reaparecer el puerto ROM — un reinicio por software no lo hace —, por lo que es obligatorio desconectar y volver a conectar el cable USB. <!-- src: /desktop/crates/opad-device/src/flash.rs -->
