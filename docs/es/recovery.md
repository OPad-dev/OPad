---
title: Contadores, recuperación y desvinculación
description: Cómo reconcilian el teclado y el PC los contadores de por vida, qué hacen la copia de seguridad, la restauración y el reinicio, y cómo reflashear, desvincular o recuperar un teclado por USB sin botones.
sidebar:
  order: 2
---

Esta página explica cómo mantiene OPad a salvo tus contadores de por vida y cómo reflashear, ceder o recuperar un teclado. Todos los pasos funcionan por USB: nunca necesitas los botones BOOT o RESET de la placa. Los números de sección son fijos, porque la app y `opadctl` remiten a ellos.

## 1. Dónde se guardan los contadores

El teclado y el PC guardan cada uno los recuentos de pulsaciones de por vida.

- **En el teclado:** las pulsaciones se cuentan en RAM, en la ruta de las teclas, sin ninguna escritura en flash por pulsación. El teclado las guarda en su flash (NVS) solo mientras está en `IDLE`: cuando termina el cooldown de un mapa, según su calendario de puntos de control en reposo y cuando el PC suspende el USB. Un teclado V3 de tres teclas (MX V3 o HE V3) guarda Key 3 en su propia clave NVS (`key3_cnt`). <!-- src: /firmware/main/counters/counters.c#counters_checkpoint --> <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task --> <!-- src: /firmware/main/app_main.c#usb_event_handler -->
- **En el PC:** en la base de datos SQLite `opad.db` (tabla `device_state`, con Key 3 en `lifetime_key3`). El daemon no escribe nada en ella durante `PLAYING` y `COOLDOWN`; mantiene los recuentos en vivo en memoria y los escribe durante `SYNC` e `IDLE`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 2. Generaciones de contadores y reconciliación

Cada registro de contadores tiene:

- `device_id`: `OPAD-` más la dirección MAC Wi-Fi de la placa. El firmware antiguo usaba `OSUPAD-<MAC>`; la app lo interpreta como el mismo teclado y migra sus registros. <!-- src: /firmware/main/protocol/protocol.c#get_device_mac_string --> <!-- src: /desktop/crates/opad-model/src/lib.rs#canonical_device_id --> <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->
- `counter_generation`: un número de 32 bits que empieza en `1` en un teclado nuevo y sube cada vez que los contadores se reinician, se restauran desde una copia de seguridad, se trasladan desde un teclado sustituido, se reconcilian a mano o se recuperan de otra instalación. <!-- src: /firmware/main/counters/counters.c#counters_init --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- `lifetime_key1`, `lifetime_key2` y, en un teclado V3 de tres teclas, `lifetime_key3`, que se registra aparte de Key 1 y Key 2. <!-- src: /desktop/crates/opad-model/src/lib.rs#CounterState --> <!-- src: /desktop/crates/opad-model/src/lib.rs#Key3Counters -->

### Reglas

1. **Gana la generación más alta.** Una base de datos o una copia de seguridad más antigua nunca puede sobrescribir un reinicio o una importación. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Dentro de una misma generación, los recuentos solo suben.** Cada tecla se queda con el mayor de los dos recuentos (`max(pc, pad)`). Salvo que el PC fuerce una restauración, el teclado rechaza una sincronización con una generación inferior (`stale generation`) o con un recuento inferior en la misma generación (`non-monotonic`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Un corte de corriente durante un mapa pierde las pulsaciones desde el último guardado.** Si el teclado se queda sin alimentación durante un mapa o su cooldown, se pierden las pulsaciones desde su último guardado en `IDLE`. Es el precio de mantener las escrituras en flash fuera de la ruta de las teclas. <!-- src: /firmware/main/counters/counters.c#checkpoint_locked --> <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## 3. Qué ocurre cuando se conecta el teclado

Cuando el daemon se conecta al teclado, primero comprueba que el teclado habla la misma versión de protocolo (`DEVICE_PROTOCOL_VERSION`) y quién es su propietario (`owner_id`). Después, mientras está en `IDLE` y tiene permiso para tocar el teclado, reconcilia los contadores: <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync -->

| Situación | Qué hace el daemon |
|---|---|
| **El teclado pertenece a otra instalación de OPad** | No se sincroniza nada hasta que se resuelve la propiedad. Si el teclado tiene al menos tantas pulsaciones como este PC en ambas teclas, el daemon se lo queda sin preguntar y conserva los recuentos del teclado. Si no, la sincronización se pausa y la app te pregunta (consulta §7). <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead --> |
| **Teclado conocido, misma generación** | Conserva `max(pc, pad)` para Key 1 y Key 2 y escribe el resultado en ambos. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync --> |
| **Teclado conocido, la generación del teclado es mayor** | Actualiza la base de datos del PC a la generación y los recuentos del teclado. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> |
| **Teclado conocido, la generación del PC es mayor** | Envía sus recuentos al teclado (`CounterSync`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync --> |
| **Teclado conocido cuya flash se borró** (generación ≤ 1, ambos recuentos a 0, mientras el PC tiene pulsaciones) | Conserva los ajustes guardados en el PC en lugar de los predeterminados del teclado, se los envía al teclado y restaura los recuentos, incluido Key 3 cuando está guardado. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/sync.rs#key3_sync_target --> |
| **Teclado desconocido, cuando el PC ya tiene un teclado registrado**, y el nuevo teclado tiene generación ≤ 1, menos de 1000 pulsaciones en cada tecla y menos pulsaciones que el PC | Inicia el flujo de sustitución (§4). La sincronización se pausa hasta que respondas. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/runtime.rs#pc_is_ahead --> |
| **Cualquier otro teclado nuevo** (incluido el primero) | Lo registra con la generación y los recuentos que comunica. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |

## 4. Sustituir un teclado

Cuando conectas un OPad nuevo a un PC que tiene historial de contadores de un teclado anterior, y el nuevo teclado coincide con el caso de sustitución de §3, el daemon retiene la sincronización y pregunta. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/runtime.rs#pad_guard -->

La app muestra: <!-- src: /desktop/gui/src/main.rs#App -->

> This looks like a new pad. Restore counters from `<old_id>`?

con **Restore from previous pad** y **Treat as new pad**. `opadctl status` muestra una línea `Replacement:` mientras la pregunta está abierta. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

- **Restore from previous pad:** el daemon fija la generación en una más que la mayor de las generaciones de los dos teclados, copia los recuentos de Key 1, Key 2 (y Key 3, si está registrado) del teclado antiguo al registro del nuevo y los escribe en el teclado nuevo con un `CounterSync` forzado. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- **Treat as new pad:** el teclado nuevo se registra por separado con sus recuentos actuales. El registro del teclado antiguo se queda en la base de datos como historial. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## 5. Copia de seguridad, restauración y reinicio

El daemon rechaza todas estas acciones durante `PLAYING` y `COOLDOWN`, cuando su base de datos no está disponible y mientras haya abierta una pregunta de propiedad o de sustitución. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

- **Reiniciar los contadores:** ejecuta `opadctl reset --yes` o, en la app, abre **Device** → **Reset lifetime counters**, escribe `RESET` y haz clic en **Confirm Reset**. Key 1, Key 2 y Key 3 pasan a `0`, la generación sube en uno y el teclado recibe un `CounterSync` forzado. Con el teclado desconectado, el reinicio queda en cola y se envía en la siguiente sincronización. <!-- src: /desktop/cli/src/main.rs#Commands::Reset --> <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync -->
- **Exportar una copia de seguridad:** ejecuta `opadctl export <file>` o abre **Device** → **Export backup**. El archivo JSON contiene `format_version` (`1`), `exported_at`, el `device_id`, el `board_profile` y la `counter_generation` del teclado, los recuentos de por vida y los ajustes del teclado (teclas, debounce, brillo, suspensión, frecuencia de refresco, pines de las teclas, deslizamientos, animaciones de resultado y, en un teclado de tres teclas, `key3` y `game_keys`). <!-- src: /desktop/cli/src/main.rs#Commands::Export --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- **Importar una copia de seguridad:** ejecuta `opadctl import <file>` (añade `--yes` para saltarte la pregunta) o abre **Device** → **Import backup**. Se hace en dos pasos: <!-- src: /desktop/cli/src/main.rs#Commands::Import -->
  1. **Vista previa** (`PreviewImport`): el daemon comprueba el archivo (`format_version` 1, `board_profile` `waveshare_esp32s3_touch_lcd_2`, códigos de tecla, pines, rangos de valores y un `exported_at` como mucho un día en el futuro) y muestra los valores actuales frente a los entrantes, con avisos si el `device_id` es distinto, si los contadores bajan o si cambia la generación. Todavía no se cambia nada. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
  2. **Aplicar** (`ImportBackup`): solo después de que confirmes. Si algún recuento fuera a bajar, la app también te pide marcar **I understand this replaces my counters with lower values** antes de **Apply Backup**. Se conserva tu dirección local de tosu, la generación pasa a ser una más que la mayor entre la actual y la de la copia, y los ajustes y recuentos se escriben en la base de datos y en el teclado. <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 5.1 Copias de seguridad automáticas

El daemon también escribe una copia de seguridad por su cuenta después de cada sesión de juego, en el mismo formato: <!-- src: /desktop/daemon/src/backup.rs#write -->

| | |
|---|---|
| **Dónde** | `opad-backup-YYYYMMDDTHHMMSSZ.json` en `~/.local/share/opad/backups` (o `$XDG_DATA_HOME/opad/backups`) en Linux, `%APPDATA%\opad\backups` en Windows. <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **Cuándo** | 20 segundos después de que el teclado vuelva a `IDLE` tras un mapa (cooldown y luego sincronización). Empezar otro mapa en esos 20 segundos la cancela; la siguiente sesión programa una nueva. Las sincronizaciones en reposo nunca escriben una. <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |
| **Cuántas** | Los 10 archivos `opad-backup-*.json` más recientes. Los más antiguos se borran; los archivos con otros nombres en la carpeta nunca se tocan. <!-- src: /desktop/daemon/src/backup.rs#KEEP --> <!-- src: /desktop/daemon/src/backup.rs#prune --> |
| **Seguridad** | Se escribe en un archivo temporal, se vuelca a disco y se renombra en su sitio. Una escritura fallida se registra y nunca interrumpe la sincronización ni el juego. <!-- src: /desktop/daemon/src/backup.rs#write_and_log --> |
| **La última** | `Last Backup:` en `opadctl status`. <!-- src: /desktop/cli/src/main.rs#Commands::Status --> |

La copia se escribe aunque la sincronización posterior al mapa haya fallado, así que contiene los últimos recuentos que tiene el daemon. Es una copia de lo que tiene el daemon, no un sustituto de una sincronización que funcione. Restáurala como cualquier otra copia de seguridad. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 6. Qué espera a que termine un mapa

Durante `PLAYING` y `COOLDOWN`:

- No se escribe nada en la base de datos del PC ni en la flash del teclado. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage --> <!-- src: /firmware/main/counters/counters.c#checkpoint_locked --> <!-- src: /firmware/main/config/device_config.c#write_to_nvs -->
- Los cambios de teclas, de pines de las teclas y de debounce quedan en cola y se responden con "deferred" hasta `IDLE`. El brillo, el tiempo de suspensión y la frecuencia de refresco se aplican al momento y se guardan después. Los cambios de diseño y los reinicios se ven en el teclado al momento y se guardan después. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- Se rechazan la sincronización manual, el reinicio de contadores, la importación de copias de seguridad, las respuestas sobre propiedad y el flasheo del firmware. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

Cuando termina un mapa (o tosu se desconecta durante uno), empieza un `COOLDOWN` de 5 segundos tanto en el PC como en el teclado. Después el daemon pasa por `SYNC` hasta `IDLE`, sincroniza los contadores y aplica lo que estaba en cola, y el teclado guarda sus contadores, ajustes, diseños y calibración Hall Effect. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION --> <!-- src: /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

## 7. Reflashear y desvincular un teclado

Un teclado pertenece a la instalación de OPad que lo reclamó (`owner_id`). Borrar la flash del teclado y reflashearlo es la única forma de eliminar eso: no hay botón para desemparejar, y el firmware rechaza una reclamación todo a ceros (`OWNER_CLAIM_REJECT_INVALID`). <!-- src: /firmware/main/config/owner.c --> <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner -->

Si has llegado aquí desde la pregunta "belongs to another OPad install", no necesitas reflashear para seguir usando el teclado: **Leave it alone** lo mantiene funcionando como teclado sin tocar su propietario ni sus contadores. Usa esta sección para dar un teclado a otra persona o para recuperar uno que no arranca. <!-- src: /desktop/gui/src/main.rs#App -->

### 7.1 Lo que cuesta borrar

El registro del propietario está en la NVS del teclado junto a sus contadores de por vida, ajustes, diseños y calibración Hall Effect. Borrar la flash los elimina todos. <!-- src: /firmware/partitions.csv -->

> [!CAUTION]
> `espflash erase-flash` borra los contadores de por vida, los ajustes, los diseños, la calibración y el registro del propietario del teclado.

Borra los 16 MB enteros de la flash, incluidos el bootloader y la tabla de particiones, así que el teclado no vuelve a ser un teclado hasta que lo flashees en §7.4. <!-- src: /firmware/sdkconfig.defaults -->

No puedes inutilizar el teclado así. El primer bootloader del ESP32-S3 está en ROM, de modo que un teclado con la flash vacía sigue apareciendo por USB como `303a:1001` y acepta un flasheo. <!-- src: /desktop/crates/opad-device/src/lib.rs#ESP_ROM_BOOTLOADER_PID -->

### 7.2 Exporta primero

Exporta una copia de seguridad antes de borrar nada. Es la única copia que sobrevive a una reinstalación de la app de escritorio: <!-- src: /desktop/cli/src/main.rs#Commands::Export -->

```sh
opadctl export ~/opad-backup.json
```

o **Device** → **Export backup** en la app. También puedes usar una copia de seguridad automática (§5.1): <!-- src: /desktop/daemon/src/backup.rs#backup_dir -->

```sh
opadctl status | grep 'Last Backup'
ls -t ~/.local/share/opad/backups/ | head   # Windows: %APPDATA%\opad\backups
```

Después de reflashear, restáurala con `opadctl import ~/opad-backup.json` o **Device** → **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands::Import -->

Un teclado borrado no tiene propietario, y la primera instalación de OPad a la que se conecte lo reclama sin preguntar. Si es el PC desde el que lo borraste, el daemon también detecta la flash borrada y repone sus recuentos y ajustes guardados (§3). Si vas a regalar el teclado, conéctalo primero al PC del nuevo propietario, para que lo reclame su instalación. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

### 7.3 Pon el teclado en modo de descarga

```sh
opadctl bootloader
```

`opadctl bootloader` pide al daemon que libere el puerto serie, reinicia el teclado en el modo de descarga de la ROM del ESP32-S3 por USB, devuelve el control al daemon y muestra el puerto del bootloader (`303a:1001`). Funciona con o sin el daemon en marcha. <!-- src: /desktop/cli/src/main.rs#Commands::Bootloader --> <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->

En Windows, un puerto serie solo puede estar abierto en un programa a la vez, así que cierra antes cualquier monitor serie. Para listar los puertos, ejecuta `espflash list-ports` o mira en Device Manager, en **Ports (COM & LPT)**. <!-- src: /desktop/crates/opad-device/src/flash.rs#FlashError -->

`opadctl` llega al modo de descarga sin botones: primero envía al teclado un comando `BOOTLOADER` por su puerto serie y luego prueba el "toque" a 1200 baudios que usan las placas tipo Arduino. El firmware se desconecta entonces del USB, mantiene el bus en reset unos 300 ms para que el PC vea una desconexión real y se reinicia en modo de descarga. Si ninguna de las dos cosas funciona, usa `opadctl recover` (§7.7). <!-- src: /desktop/crates/opad-device/src/flash.rs#BOOT_TRIGGERS --> <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->

### 7.4 Borra y luego flashea

Necesitas los cuatro archivos del firmware en una carpeta: una carpeta `firmware/build` de ESP-IDF o una carpeta con los cuatro archivos `.bin` de una versión publicada del firmware (`opad-firmware.bin`, `bootloader.bin`, `partition-table.bin`, `ota_data_initial.bin`). <!-- src: /desktop/cli/src/main.rs#resolve_flash_set --> <!-- src: /.github/workflows/release-firmware.yml -->

`espflash` viene con la app: `/usr/lib/opad/bin/espflash` en un paquete de Linux, `espflash.exe` en `%LOCALAPPDATA%\Programs\opad` en Windows. Usa esa ruta si `espflash` no está en tu `PATH`. <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/windows/installer.iss -->

Con el teclado en modo de descarga (§7.3):

**Linux**

```sh
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
opadctl flash --full ~/Downloads/opad-firmware/   # or firmware/build
```

**Windows (PowerShell o Símbolo del sistema)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
opadctl flash --full C:\Users\you\Downloads\opad-firmware\
```

Usa el puerto que mostró `opadctl bootloader`. `--before no-reset --after no-reset-no-stub` mantiene el teclado en modo de descarga para el siguiente comando. No desconectes el teclado mientras se ejecuta `erase-flash`. <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images -->

`--full` hace falta después de un borrado porque el bootloader y la tabla de particiones ya no están. Escribe el bootloader en `0x0`, la tabla de particiones en `0x8000`, `ota_data_initial.bin` en `0xf000` y `opad-firmware.bin` en `0x20000` (`ota_0`), y luego reinicia el teclado con el firmware nuevo. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set --> <!-- src: /desktop/crates/opad-device/src/flash.rs#reset_to_app -->

### 7.5 Confirma

```sh
opadctl status
```

Un teclado que ha arrancado bien muestra `ESP32 Device:     Connected` y `Running Slot:     ota_0`. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

En un PC que nunca ha visto este teclado, sus recuentos empiezan en 0 con la generación 1. En el PC desde el que lo borraste, el daemon repone los recuentos guardados y lo vuelve a reclamar (§7.2). <!-- src: /firmware/main/counters/counters.c#counters_init --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

Si el teclado no vuelve como OPad en unos 15 segundos tras un flasheo `--full`, `opadctl flash` informa de un error. Repite el paso `opadctl flash --full`; no vuelvas a borrar. Si el teclado ha salido del modo de descarga y no aparece como OPad (`303a:8415`), usa `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

### 7.6 Cuando se interrumpe el flasheo

Una actualización normal (desde la app, `opadctl firmware-update` u `opadctl flash <app.bin>` cuando el daemon conoce la ranura en ejecución) escribe primero el firmware nuevo en la *otra* ranura OTA (`ota_1` si se está ejecutando `ota_0`, y al revés). Solo entonces actualiza `otadata`, de modo que el bootloader prueba la ranura nueva una vez y conserva la antigua como respaldo. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

- **Interrumpida antes de escribir `otadata`:** nada apunta todavía a la ranura nueva, así que el teclado arranca su firmware antiguo sin cambios. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
- **Escrita, pero el firmware nuevo no funciona:** el firmware nuevo arranca a prueba. Solo se confirma a sí mismo cuando el PC ha configurado su conexión USB. Si hay un PC conectado y no lo ha hecho en 20 segundos, el teclado se reinicia y el bootloader vuelve a la ranura anterior. Un firmware que falla antes de confirmarse vuelve atrás de la misma manera. En un cargador sin PC, la prueba simplemente espera hasta la próxima vez que el teclado se conecte a un ordenador. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works --> <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->

  Tras una reversión así, `opadctl flash` informa: "The pad did not accept the new firmware (it never came up as a working USB device) and went back to the previous one on `<slot>`. It works as before; nothing else is needed." <!-- src: /desktop/cli/src/main.rs#report_slot_after_flash -->

Hay dos tipos de flasheo sin respaldo, porque escriben `ota_0` en su sitio: `opadctl flash <app.bin>` cuando no se conoce la ranura en ejecución (sin daemon, o con un firmware demasiado antiguo para comunicarla) y un flasheo `--full` (§7.4). Si se interrumpe uno de ellos, recupera el teclado con `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands::Flash --> <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

### 7.7 Un teclado que no aparece como OPad (`opadctl recover`)

Si el firmware arranca pero nunca funciona por USB (en Linux, `dmesg` puede mostrar `can't set config #1, error -32`), nada puede pedirle que entre en modo de descarga. En su lugar, `opadctl recover` atrapa el teclado al encenderse: <!-- src: /desktop/crates/opad-device/src/flash.rs#recover --> <!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

1. Ejecútalo con la carpeta del firmware (como en §7.4):

   ```sh
   opadctl recover firmware/build   # or a folder with a firmware release's .bin files
   ```

2. Cuando muestre `Unplug the pad's USB cable and plug it back in (waiting 300 s)...`, desconecta el teclado y vuelve a conectarlo. (`--timeout <seconds>` cambia la espera.)
3. Espera a `✓ Recovered: the pad came back as OPad on <port>`.

En cada encendido, antes de que el firmware tome el control del USB, el bootloader USB integrado del ESP32-S3 aparece durante una fracción de segundo como `303a:1001`. `opadctl recover` lo vigila, mantiene el chip en modo de descarga en ese instante, escribe el conjunto completo del firmware, reinicia el teclado y espera hasta 15 segundos a que aparezca OPad (`303a:8415`). <!-- src: /desktop/crates/opad-device/src/flash.rs#recover --> <!-- src: /desktop/crates/opad-device/src/flash.rs#usj_reset_to_download --> <!-- src: /desktop/crates/opad-device/src/flash.rs#disable_watchdogs -->

Solo un ciclo de alimentación real vuelve a abrir esa ventana del bootloader; un reinicio por software no. Por eso vuelves a conectar el cable. Nunca hace falta un botón BOOT o RESET. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->
