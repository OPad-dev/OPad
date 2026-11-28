---
title: Referencia de la CLI opadctl
description: Los comandos de opadctl para las tareas habituales y después todos los comandos con su uso, sus opciones, lo que piden al daemon, lo que imprimen, su salida JSON y sus códigos de salida.
sidebar:
  order: 1
---

`opadctl` es la herramienta de línea de comandos de OPad. La mayoría de los subcomandos son peticiones a `opad-daemon` por IPC local (consulta la [Referencia del protocolo IPC](ipc.md)); los subcomandos de flasheo también funcionan sin el daemon en marcha. <!-- src: /desktop/cli/src/main.rs#main -->

## Tareas habituales

| Tarea | Comando |
|---|---|
| Comprobar que el teclado y el daemon están conectados | `opadctl status` |
| Hacer una copia de seguridad de contadores y ajustes | `opadctl backup export ~/opad-backup.json` |
| Restaurar una copia de seguridad | `opadctl backup import ~/opad-backup.json` |
| Actualizar el firmware del teclado | `opadctl update install firmware` |
| Actualizar la aplicación OPad | `opadctl update check` y después `opadctl update install app` |
| Ver tu ritmo de pulsación (KPM) | `opadctl stats` |
| Sincronizar ahora los contadores y el reloj del teclado | `opadctl counters sync` |
| Reiniciar el teclado (por ejemplo, tras cambiar su módulo de entrada) | `opadctl pad restart` |
| Reiniciar los contadores de por vida | `opadctl counters reset --yes` |
| Comprobar los permisos en Linux (udev, tosu) | `opadctl doctor` |
| Seguir los registros en directo | `opadctl logs -f` |
| Recuperar un teclado que ya no aparece como OPad | `opadctl recover <carpeta del firmware>` y vuelve a conectarlo |
<!-- src: /desktop/cli/src/main.rs#Cmd -->

## Cómo leer esta página

Los bloques `text` y las columnas **Ayuda** son la salida literal de `--help` de `opadctl` compilado desde `desktop/cli` (`cargo build -p opadctl`). Todos los subcomandos aceptan además `-h, --help`; `opadctl --help` los lista todos. <!-- src: /desktop/cli/src/main.rs#Cmd -->

| Subcomandos | Necesita `opad-daemon` |
|---|---|
| `doctor` | No. Se ejecuta en local y nunca se conecta. |
| `flash`, `recover`, `pad bootloader` | No. Si el daemon responde, `opadctl` le pide primero que libere el puerto serie; si no, imprime `Could not reach opad-daemon (...); flashing without it.` y continúa. |
| Todos los demás subcomandos | Sí. Sin él, `opadctl` termina con `Failed to connect to opad-daemon. Is the daemon running?` (exit code 3) |
<!-- src: /desktop/cli/src/main.rs#run -->

Un daemon que rechaza el handshake IPC (por ejemplo, un daemon de otra versión de la aplicación tras una actualización) cuenta como inalcanzable. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Opciones de todos los comandos

| Opción | Ayuda |
|---|---|
| `--json` | Print the result as JSON on stdout; progress and questions go to stderr |
| `-h`, `--help` | Print help |
<!-- src: /desktop/cli/src/main.rs#Cli -->

Con `--json`, la salida estándar lleva un único valor JSON y las líneas pensadas para una persona van a stderr. <!-- src: /desktop/cli/src/main.rs#emit, /desktop/cli/src/main.rs#JSON -->

| Comando | JSON en la salida estándar |
|---|---|
| `status` | La respuesta `Status` del daemon como objeto. |
| `stats` | La instantánea del ritmo de pulsación. |
| `latency` | El objeto de latencia, o `null` cuando el teclado no ha informado de ninguna. |
| `logs` | Una entrada del registro por línea. |
| `update`, `update status`, `update check` | La respuesta `UpdateStatus` del daemon, con la oferta de firmware en `firmware`. |
| `doctor` | `{"ok": …, "checks": [...]}`. |
| `pad test` | `{"ok": …, "result": …, "report": …, "checks": [...]}`. |
| Cualquier comando que cambia algo | `{"ok": true, ...}` con lo que produjo (por ejemplo `counters`, `file`, `port`, `version`). |
| Cualquier fallo | `{"ok": false, "kind": "...", "error": "..."}`, donde `kind` es `failed`, `service_unavailable`, `rejected` o `no_pad`. |
<!-- src: /desktop/cli/src/main.rs#run, /desktop/cli/src/main.rs#main, /desktop/cli/src/main.rs#Fault::name, /desktop/cli/src/main.rs#update_json, /desktop/cli/src/main.rs#run_doctor -->

## Códigos de salida

| Código | Significado |
|---|---|
| `0` | Hecho. `doctor` siempre termina con `0`: informa y no cambia nada. |
| `1` | Falló por un motivo que no es ninguno de los de abajo (también: un `pad test` fallido). |
| `2` | No se pudo leer la línea de comandos. |
| `3` | El servicio de OPad (`opad-daemon`) no está en marcha o no respondió. |
| `4` | El servicio o el teclado se negaron: durante un mapa, un valor no válido, nada listo para instalar. |
| `5` | El comando necesita un teclado y no hay ninguno conectado. `opadctl` pregunta al servicio si hay un teclado conectado; no lo deduce del texto del error. |
<!-- src: /desktop/cli/src/main.rs#Fault, /desktop/cli/src/main.rs#refusal, /desktop/cli/src/main.rs#main -->

## Nombres anteriores

Los comandos se reagruparon. Todos los nombres anteriores siguen funcionando y hacen lo mismo; solo quedan fuera de `--help`. Un `opadctl` que responde `unrecognized subcommand 'pad'` (o `counters`, `backup`, `doctor`, `logs`) es anterior a la reagrupación: dale el nombre anterior. <!-- src: /desktop/cli/src/main.rs#Cmd, /desktop/cli/src/main.rs#Action -->

| Antes | Ahora |
|---|---|
| `setup` | `doctor` |
| `sync` | `counters sync` |
| `reset --yes` | `counters reset --yes` |
| `export`, `import` | `backup export`, `backup import` |
| `monitor` | `logs` |
| `latency --reset` | `latency reset` |
| `update --check`, `update --install`, `update --channel <c>` | `update check`, `update install app`, `update channel <c>` |
| `firmware-update` | `update install firmware` |
| `restart` | `pad restart` |
| `board-test` | `pad test` |
| `animation`, `bootloader` | `pad animation`, `pad bootloader` |
<!-- src: /desktop/cli/src/main.rs#Cmd -->

## Comandos de uso diario

### opadctl status

```text
Show the pad, its counters and the OPad service at a glance

Usage: opadctl status [OPTIONS]
```

Envía `GetStatus` e imprime una línea por campo: <!-- src: /desktop/cli/src/main.rs#run -->

| Línea | Contenido |
|---|---|
| `Daemon Mode` | `Idle`, `Playing`, `Cooldown` o `Sync`. |
| `ESP32 Device` | `Connected` o `Disconnected`. |
| `Counters Source` | `Device` o `Pc`: de dónde proceden los contadores mostrados. |
| `Device ID`, `Board Profile`, `Firmware Version` | Del `HelloAck` del teclado. El ID de dispositivo es `OPAD-` seguido de 12 dígitos hexadecimales de la dirección MAC del chip. |
| `Running Slot` | `ota_0`, `ota_1` o `unknown (firmware predates the OTA layout)`. |
| `Input Module` | `Hall Effect`, solo se imprime cuando el teclado usa un backend de entrada Hall Effect. |
| `HE Calibrated` | Estado de calibración por tecla (`K1`, `K2`, y `K3` en teclados de tres teclas), solo en teclados Hall Effect. |
| `Key 1 (K1)`, `Key 2 (K2)`, `Key 3 (K3)` | Carácter de la tecla y total de pulsaciones de por vida. `Key 3` solo en teclados de tres teclas. |
| `Game Keys` | La selección de teclas de juego, solo en teclados de tres teclas. |
| `Total Presses` | Suma de los totales de por vida de las teclas de juego. |
| `Generation` | Generación de contadores. |
| `Last Sync`, `Last Backup` | Marcas de tiempo, o `Never`. |
| `Last Sync Error`, `Database Error` | Solo cuando tienen valor. |
| `Ownership` | Solo cuando el teclado pertenece a otra instalación de OPad: la sincronización de contadores queda en pausa hasta que decidas en la aplicación. |
| `Replacement` | Solo cuando un teclado nuevo sustituyó a uno conocido. |
| `Incompatible` | Solo cuando el teclado habla una versión del protocolo que esta aplicación no habla. |
| `tosu (osu!)` | `Active` u `Offline`. |
| `Key Latency` | `p50`, `p99.9` y `max` en µs, con el número de muestras. Solo cuando el teclado comunica la latencia. |
| `Key Pins` | `K1 GPIO<n>, K2 GPIO<n>`. |
| `Debounce Lockout` | En µs. |
| `Swipes` | La acción de cada dirección de deslizamiento (arriba, abajo, izquierda, derecha). |
| `Brightness`, `Sleep Timeout` | Porcentaje y segundos. |
<!-- src: /desktop/cli/src/main.rs#run, /desktop/crates/opad-model/src/lib.rs#RuntimeMode, /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

### opadctl counters sync

```text
Bring the pad's counters, this PC's counters and the pad's clock into line now

Usage: opadctl counters sync [OPTIONS]
```

Envía `ForceSync`. Si tiene éxito, imprime los conteos de por vida sincronizados de `K1` y `K2`. Un rechazo termina con el código de salida 4, o 5 cuando no hay teclado conectado. <!-- src: /desktop/cli/src/main.rs#run -->

### opadctl stats

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]
```

| Opción | Ayuda |
|---|---|
| `--period <DAYS>` | Set the history period first, in days (0 = all time). It is kept for the app and the pad too. |
<!-- src: /desktop/cli/src/main.rs#run, /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

Envía `GetTapStats`, o `SetTapHistoryPeriod` cuando se indica `--period` (de `0` a `3650` días). Imprime: <!-- src: /desktop/cli/src/main.rs#run, /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

- **Intento** (el que está en curso o, si no, el último): mapa, `AVG` (todas las pulsaciones sobre la duración de la canción, pausas incluidas), `PEAK` (las 7 pulsaciones seguidas más rápidas) con los mejores 10 s, y una tabla de `Current`, `Tap avg`, `Peak` y `Presses` para `K1`, `K2` y `Combined`.
- **Historial** del periodo: intentos, `AVG`, `PEAK` y mejores 10 s, y `Tap avg`, `Avg peak`, `Best peak` y `Presses` por canal.
<!-- src: /desktop/cli/src/main.rs#format_tap_stats, /desktop/crates/opad-model/src/tap_rate.rs#WINDOW_INTERVALS -->

### opadctl backup export

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl backup export [OPTIONS] <FILE>
```

`<FILE>`: *Path to write backup JSON file*

Envía `ExportBackup` y escribe el `JsonBackup` devuelto en `<FILE>` como JSON con formato legible. <!-- src: /desktop/cli/src/main.rs#run -->

### opadctl backup import

```text
Import configuration and counters from a validated JSON backup

Usage: opadctl backup import [OPTIONS] <FILE>
```

| Argumento u opción | Ayuda |
|---|---|
| `<FILE>` | Path to JSON backup file |
| `--yes` | Skip preview and confirmation prompt |

1. Interpreta y valida `<FILE>` en local.
2. Envía `PreviewImport` e imprime el ID de dispositivo, las teclas y los conteos de pulsaciones actuales frente a los entrantes, más avisos si el ID de dispositivo es distinto o si los contadores retroceden.
3. Pregunta `Proceed with import? [y/N]` salvo que se indique `--yes`. Sin terminal y sin `--yes`, se niega.
4. Envía `ImportBackup { confirm: true }` e imprime la generación de contadores resultante.
<!-- src: /desktop/cli/src/main.rs#run, /desktop/cli/src/main.rs#confirm -->

### opadctl counters reset

```text
Set the lifetime key counters back to zero, for good

Usage: opadctl counters reset [OPTIONS]
```

| Opción | Ayuda |
|---|---|
| `--yes` | Confirm destructive reset of lifetime key counters |

Sin `--yes`, termina con un error y no envía nada. Con `--yes`, envía `ResetCounters { confirm: true }` e imprime la nueva generación de contadores. <!-- src: /desktop/cli/src/main.rs#run -->

### opadctl update

```text
Updates of OPad, tosu and the pad's firmware. With nothing after it, shows their status

Usage: opadctl update [OPTIONS] [COMMAND]
```

| Comando | Ayuda | Qué hace |
|---|---|---|
| `status` | Show what is installed and what is available, as the app's Updates tab does | Envía `GetUpdateStatus` y `GetFirmwareUpdate`. Imprime el canal y después la versión instalada, la disponible y si las actualizaciones automáticas están activadas para OPad, tosu y el firmware del teclado, con la última comprobación, el último error y la última instalación. |
| `check` | Check the update feed now, wait for the answer and show it | Envía `CheckForUpdates` y consulta `GetUpdateStatus` cada 500 ms hasta que la comprobación termina; desiste a los 120 s. Después imprime el estado. |
| `install <PART>` | Install the available update of one part | `<PART>` es `app`, `tosu` o `firmware`; ver abajo. |
| `channel [CHANNEL]` | Show the update channel, or follow another one from now on | Con `stable` o `beta` (versiones candidatas) envía `SetUpdateChannel`, que además inicia una comprobación del canal nuevo. |
| `auto <PART> <STATE>` | Turn one part's automatic updates on or off | `<STATE>` es `on` u `off`. Envía `SetUpdateEnabled`. |
<!-- src: /desktop/cli/src/main.rs#UpdateCmd, /desktop/cli/src/main.rs#show_update_status, /desktop/cli/src/main.rs#check_for_updates, /desktop/cli/src/main.rs#set_update_channel -->

Una instalación cuyos archivos pertenecen a un gestor de paquetes muestra la versión disponible como `(this install cannot update itself)`; tosu en una instalación así muestra `(comes with OPad's own update)`. <!-- src: /desktop/cli/src/main.rs#say_component, /desktop/crates/opad-ipc/src/lib.rs#ComponentUpdate -->

**`update install app`** y **`update install tosu`** envían `InstallUpdate`. Si no hay nada listo para instalar terminan con el código de salida 4 y `There is no update of … ready to install`. El instalador de OPad reinicia OPad, así que el comando vuelve en cuanto ha arrancado. <!-- src: /desktop/cli/src/main.rs#install_update -->

**`update install firmware`** graba el teclado a partir del manifiesto firmado de la versión:

| Opción | Ayuda |
|---|---|
| `--yes` | Firmware only: consent without the interactive prompt. The firmware is still only flashed because you said so. |

1. Envía `GetFirmwareUpdate` e imprime la versión instalada, la ranura en ejecución, los bloqueos que haya, la versión disponible y sus notas. Si no hay nada más nuevo se detiene; con bloqueos termina con el código de salida 4, o 5 cuando no hay teclado conectado.
2. Imprime el texto de consentimiento y pregunta `Flash the pad now? [y/N]` salvo que se pase `--yes`.
3. Envía `InstallFirmwareUpdate { confirm: true }`. El daemon sincroniza los contadores del teclado con este PC antes de escribir nada y después informa de la versión antigua y la nueva y de la ranura en ejecución.
<!-- src: /desktop/cli/src/main.rs#install_firmware, /desktop/daemon/src/firmware_update.rs#install -->

### opadctl pad restart

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl pad restart [OPTIONS]
```

Envía `RestartPad`. El teclado guarda sus contadores, configuración, diseños y calibración, se reinicia en cuanto está inactivo y se vuelve a conectar. <!-- src: /desktop/cli/src/main.rs#run, /firmware/main/runtime/runtime.c#restart_now -->

### opadctl doctor

```text
Check this computer's setup (USB permissions, osu! memory access) and say how to fix what is missing. It changes nothing

Usage: opadctl doctor [OPTIONS]
```

Solo comprueba: <!-- src: /desktop/cli/src/main.rs#run_doctor -->

| Plataforma | Comprobaciones |
|---|---|
| Linux | Si `/etc/udev/rules.d/70-opad.rules` o `/usr/lib/udev/rules.d/70-opad.rules` coincide con la regla integrada en `opadctl`. Si falta o es distinta, imprime los comandos que la instalan (o el comando `install-udev` de la AppImage). Después, si tosu puede leer la memoria de osu!: `ptrace_scope` de Yama y `cap_sys_ptrace` de tosu (consulta [Solución de problemas](../troubleshooting.md) §2). |
| Windows | Nada que comprobar: el teclado usa los controladores HID y CDC incluidos en Windows. |
<!-- src: /desktop/cli/src/main.rs#run_doctor, /desktop/cli/src/main.rs#UDEV_RULE_PATHS -->

## Flasheo y recuperación

Funcionan con o sin el daemon (consulta [Cómo leer esta página](#cómo-leer-esta-página)). Para los procedimientos paso a paso, consulta [Recuperación](../recovery.md) §7.

`flash` y `recover` aparecen dos veces en `--help`: en el primer nivel, con los nombres que conoce cualquier versión de `opadctl`, y como `pad flash` y `pad recover`. Las dos formas ejecutan el mismo comando. Esta documentación usa los nombres cortos, para que los pasos de un teclado que no arranca sirvan con el `opadctl` que haya instalado. <!-- src: /desktop/cli/src/main.rs#Cmd, /desktop/cli/src/main.rs#PadCmd -->

### opadctl flash

```text
Flash new firmware binary onto the ESP32-S3 using espflash (same as `pad flash`)

Usage: opadctl flash [OPTIONS] <FIRMWARE>
```

| Argumento u opción | Ayuda |
|---|---|
| `<FIRMWARE>` | Path to the app image (.bin), or an ESP-IDF build directory with --full |
| `--port <PORT>` | The pad's app port, or its ROM bootloader port when several are connected (default: auto-detect) |
| `--full` | Recovery flash: also write the bootloader, partition table and OTA data |
<!-- src: /desktop/cli/src/main.rs#run -->

`<FIRMWARE>` es un archivo, o un directorio que contiene `opad-firmware.bin`. Antes de escribir nada se comprueba que la cabecera de la imagen de la aplicación sea para el ESP32-S3. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set, /desktop/cli/src/main.rs#run -->

**Actualización de la aplicación por USB (sin `--full` ni `--port`).** Con el daemon en marcha y un teclado cuyo firmware lo admite (`HelloAck.ota_chunk_max`), la imagen va por la propia conexión del teclado (`FlashOverUsb`): el teclado sigue siendo un teclado, muestra su animación de actualización y se reinicia en la imagen nueva a prueba. Si no, o con un daemon más antiguo, se flashea por el bootloader como se explica abajo. <!-- src: /desktop/cli/src/main.rs#flash_over_usb -->

**Actualización de la aplicación por el bootloader.** `opadctl` pregunta al daemon desde qué ranura funciona el teclado y escribe la imagen en la otra ranura, seguida de una imagen `otadata` que la arranca una vez a prueba. La imagen en ejecución se queda como respaldo: el firmware solo confirma la nueva después de que el anfitrión configure el USB; si no, el bootloader vuelve a la antigua. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

| Ranura en ejecución | Escribe |
|---|---|
| `ota_0` | Aplicación en `0x220000` (`ota_1`), después `otadata` en `0xf000` |
| `ota_1` | Aplicación en `0x20000` (`ota_0`), después `otadata` en `0xf000` |
| Desconocida (sin daemon, o firmware que no la comunica) | Aplicación en `0x20000` (`ota_0`) y un `otadata` en blanco, así que arranca `ota_0`; sin respaldo |
| Cualquier otra (`factory`, la disposición de particiones antigua) | Rechazado: usa `--full` |
<!-- src: /desktop/crates/opad-device/src/flash.rs#APP_PARTITION_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTADATA_OFFSET -->

**Flasheo de recuperación (`--full`).** Escribe el conjunto completo. Cada archivo se busca primero en la estructura de compilación de ESP-IDF y después junto a la imagen de la aplicación (la estructura de una versión publicada descomprimida): <!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->

| Offset | Archivo | Obligatorio |
|---|---|---|
| `0x0` | `bootloader/bootloader.bin` o `bootloader.bin` | Sí |
| `0x8000` | `partition_table/partition-table.bin` o `partition-table.bin` | Sí |
| `0xf000` | `ota_data_initial.bin` | No (se escribe si está presente) |
| `0x430000` | `opad-assets.bin`, el paquete de recursos de OPad | No (se escribe si está presente) |
| `0x930000` | `opad-user.bin`, el paquete de recursos del dueño | No (se escribe si está presente) |
| `0x20000` | `opad-firmware.bin` (o el archivo indicado) | Sí |

Un paquete de recursos se genera con `scripts/build_asset_pack.py`, que rechaza uno más grande que su partición; `opadctl` también lo rechaza. <!-- src: /desktop/crates/opad-device/src/flash.rs#ASSETS_PARTITION -->

En ambos modos, `opadctl` reinicia el teclado en el bootloader de descarga de la ROM por USB, lo flashea con `espflash` y espera hasta 15 s a que vuelva como la aplicación OPad (`303a:8415`). Un teclado que no vuelve se notifica como un fallo que remite a `opadctl recover`. <!-- src: /desktop/cli/src/main.rs#run, /desktop/daemon/src/ipc_handlers.rs#FinishFlash -->

### opadctl pad bootloader

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl pad bootloader [OPTIONS]
```

| Opción | Ayuda |
|---|---|
| `--port <PORT>` | Explicit serial port (default: auto-detect) |

Pide al daemon que libere el puerto, reinicia el teclado en el bootloader de descarga de la ROM (`303a:1001`), devuelve el puerto al daemon e imprime el puerto del bootloader. El teclado se queda en modo de descarga para otras herramientas de flasheo. <!-- src: /desktop/cli/src/main.rs#run -->

### opadctl recover

```text
Recover a pad that no longer shows up as OPad: replug it, and its next boot is caught over USB and flashed with the full set. No buttons (same as `pad recover`)

Usage: opadctl recover [OPTIONS] <FIRMWARE>
```

| Argumento u opción | Ayuda |
|---|---|
| `<FIRMWARE>` | An ESP-IDF build directory or an unpacked release (as for flash --full) |
| `--timeout <TIMEOUT>` | How long to wait for the replug, in seconds [default: 300] |
<!-- src: /desktop/cli/src/main.rs#run -->

Resuelve el mismo conjunto de archivos que `flash --full`, te pide que desconectes y vuelvas a conectar el teclado y espera hasta `--timeout` segundos. Cuando aparece durante el arranque el puerto USB-Serial-JTAG de la ROM del teclado (`303a:1001`), `opadctl` lo retiene en modo de descarga, escribe el conjunto completo, lo reinicia en la aplicación con un reinicio por watchdog RTC y espera hasta 15 s a la aplicación OPad (`303a:8415`). Una placa que ya estaba en el bus como `303a:1001` cuando empezó el comando se deja como está y se nombra en una línea `Leaving ... alone`: solo se escribe un puerto que aparece durante la espera. Consulta [Recuperación](../recovery.md) §7.7. <!-- src: /desktop/cli/src/main.rs#run, /desktop/crates/opad-device/src/flash.rs#recover -->

## Diagnóstico y pruebas

### opadctl logs

```text
Show the log of the pad, the OPad service and tosu

Usage: opadctl logs [OPTIONS]
```

| Opción | Ayuda | Qué hace |
|---|---|---|
| `-l`, `--limit <LIMIT>` | [default: 50] | Entradas por petición. |
| `-f`, `--follow` | Follow log stream in real time | Sondea `GetLogEntries` cada 500 ms desde la última entrada recibida. |
| `--level <LEVEL>` | Only this severity and above [possible values: debug, info, warn, error] | El daemon filtra antes de aplicar `limit`; `opadctl` vuelve a filtrar para los daemons que ignoran el campo. |
| `--source <SOURCE>` | Only this source [possible values: host, esp, program, tosu] | Se filtra del mismo modo. |
<!-- src: /desktop/cli/src/main.rs#LogsArgs, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### opadctl latency

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS] [COMMAND]
```

| Comando | Ayuda |
|---|---|
| `reset` | Clear the collected samples (e.g. right before playing a map) |

Sin opciones, imprime las estadísticas del teclado desde el flanco de la tecla hasta el envío HID, obtenidas de `GetStatus`: número de muestras, `p50`, `p99`, `p99.9`, `max` (µs) e informes aplazados (cambios de tecla que esperaron al siguiente sondeo USB). `opadctl latency reset` envía `ResetLatencyStats`. Sin teclado termina con el código de salida 5. <!-- src: /desktop/cli/src/main.rs#run, /protocol/opad.proto#DeviceStatus -->

### opadctl pad test

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl pad test [OPTIONS]
```

Envía `RunBoardTest`, imprime cada comprobación y termina con el código 1 cuando falla alguna. <!-- src: /desktop/cli/src/main.rs#run -->

| Opción | Ayuda | Qué hace |
|---|---|---|
| `--fix-pins` | If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again | Cuando una comprobación indica que los pines de tecla son incorrectos, envía `UpdateConfig` con esos pines, espera 300 ms y vuelve a ejecutar la prueba. Sin ella, el fallo sugiere `opadctl pad test --fix-pins`. |
| `--magnet` | Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop) | Se ejecuta tras la prueba de placa. |
<!-- src: /desktop/cli/src/main.rs#run, /desktop/cli/src/main.rs#magnet_test -->

### opadctl pad animation

```text
Play a result animation on the pad, as osu!'s results screen would

Usage: opadctl pad animation [OPTIONS] <NAME> [GRADE]
```

| Argumento | Ayuda |
|---|---|
| `<NAME>` | pop, drop or burst (or an animation id); none stops the one playing |
| `[GRADE]` | The grade it shows: SS, S, A, B, C or D [default: S] |

Ninguno distingue mayúsculas. Envía `PlayAnimation`; el daemon lo rechaza sin teclado, con un firmware sin animaciones y durante un mapa. <!-- src: /desktop/cli/src/main.rs#run, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
