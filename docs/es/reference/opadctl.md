---
title: Referencia de la CLI opadctl
description: Todos los subcomandos de opadctl con su salida de --help literal, lo que piden al daemon y lo que imprimen.
sidebar:
  order: 1
---

`opadctl` es la herramienta de línea de comandos de OPad. La mayoría de los subcomandos son peticiones a `opad-daemon` por IPC local (consulta la [Referencia del protocolo IPC](ipc.md)); los subcomandos de flasheo también funcionan sin el daemon en marcha.
<!-- src: /desktop/cli/src/main.rs#main -->

Los textos de ayuda de esta página son la salida literal de `--help` de `opadctl` compilado desde `desktop/cli` (`cargo build -p opadctl`).
<!-- src: /desktop/cli/src/main.rs#Commands -->

```text
OPad CLI management tool

Usage: opadctl <COMMAND>

Commands:
  status           Show device and daemon status
  sync             Trigger manual counter reconciliation & time synchronization
  reset            Reset lifetime key counters (requires deliberate confirmation)
  export           Export device configuration and lifetime counters to a JSON file
  import           Import configuration and counters from a validated JSON backup
  monitor          Stream live device and daemon diagnostic events
  flash            Flash new firmware binary onto the ESP32-S3 using espflash
  recover          Recover a pad that no longer shows up as OPad: replug it, and its next boot is caught over USB and flashed with the full set. No buttons
  bootloader       Reboot device into ROM download bootloader mode hands-free over USB
  firmware-update  Update the pad's firmware from the signed release manifest (§U-3b)
  update           Show, check for or install an update of the OPad app from the update feed, as the app's Updates page does
  latency          Show key-press-to-HID latency measured on the device
  stats            Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history
  board-test       Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs
  animation        Play a result animation on the pad, as osu!'s results screen would
  restart          Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played
  setup            Perform initial system setup (udev permissions & directories)
  help             Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

## Requisito del daemon

| Subcomandos | Necesita `opad-daemon` |
|---|---|
| `setup` | No. Se ejecuta en local y nunca se conecta. |
| `flash`, `recover`, `bootloader` | No. Si el daemon responde, `opadctl` le pide primero que libere el puerto serie; si no, imprime `Could not reach opad-daemon (...); flashing without it.` y continúa. |
| Todos los demás subcomandos | Sí. Sin él, `opadctl` termina con `Failed to connect to opad-daemon. Is the daemon running?` |
<!-- src: /desktop/cli/src/main.rs#main -->

Un daemon que rechaza el handshake IPC (por ejemplo, un daemon de otra versión de la aplicación tras una actualización) cuenta como inalcanzable.
<!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

---

## opadctl status

```text
Show device and daemon status

Usage: opadctl status

Options:
  -h, --help  Print help
```

Envía `GetStatus` e imprime una línea por campo:
<!-- src: /desktop/cli/src/main.rs#Commands::Status -->

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
<!-- src: /desktop/cli/src/main.rs#Commands::Status, /desktop/crates/opad-model/src/lib.rs#RuntimeMode, /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

---

## opadctl sync

```text
Trigger manual counter reconciliation & time synchronization

Usage: opadctl sync

Options:
  -h, --help  Print help
```

Envía `ForceSync`. Si tiene éxito, imprime los conteos de por vida sincronizados de `K1` y `K2`; un rechazo imprime `✗ Operation rejected: <reason>`.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

---

## opadctl reset

```text
Reset lifetime key counters (requires deliberate confirmation)

Usage: opadctl reset [OPTIONS]

Options:
      --yes   Confirm destructive reset of lifetime key counters
  -h, --help  Print help
```

Sin `--yes`, termina con un error y no envía nada. Con `--yes`, envía `ResetCounters { confirm: true }` e imprime la nueva generación de contadores.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

---

## opadctl export

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl export <FILE>

Arguments:
  <FILE>  Path to write backup JSON file

Options:
  -h, --help  Print help
```

Envía `ExportBackup` y escribe el `JsonBackup` devuelto en `<FILE>` como JSON con formato legible.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

---

## opadctl import

```text
Import configuration and counters from a validated JSON backup

Usage: opadctl import [OPTIONS] <FILE>

Arguments:
  <FILE>  Path to JSON backup file

Options:
      --yes   Skip preview and confirmation prompt
  -h, --help  Print help
```

1. Interpreta y valida `<FILE>` en local.
2. Envía `PreviewImport` e imprime el ID de dispositivo, las teclas y los conteos de pulsaciones actuales frente a los entrantes, más avisos si el ID de dispositivo es distinto o si los contadores retroceden.
3. Pregunta `Proceed with import? [y/N]` salvo que se indique `--yes`. Sin terminal y sin `--yes`, se niega.
4. Envía `ImportBackup { confirm: true }` e imprime la generación de contadores resultante.
<!-- src: /desktop/cli/src/main.rs#Commands::Import, /desktop/cli/src/main.rs#confirm -->

---

## opadctl monitor

```text
Stream live device and daemon diagnostic events

Usage: opadctl monitor [OPTIONS]

Options:
  -l, --limit <LIMIT>    [default: 50]
  -f, --follow           Follow log stream in real time
      --level <LEVEL>    Only this severity and above [possible values: debug, info, warn, error]
      --source <SOURCE>  Only this source [possible values: host, esp, program, tosu]
  -h, --help             Print help
```

| Opción | Efecto |
|---|---|
| `-l`, `--limit` | Entradas por petición (predeterminado `50`). |
| `-f`, `--follow` | Sondea `GetLogEntries` cada 500 ms desde la última entrada recibida. |
| `--level` | Gravedad mínima. El daemon filtra antes de aplicar `limit`; `opadctl` vuelve a filtrar para los daemons que ignoran el campo. |
| `--source` | Una única fuente de registros, filtrada del mismo modo. |
<!-- src: /desktop/cli/src/main.rs#Commands::Monitor, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

---

## opadctl flash

```text
Flash new firmware binary onto the ESP32-S3 using espflash

Usage: opadctl flash [OPTIONS] <FIRMWARE>

Arguments:
  <FIRMWARE>  Path to the app image (.bin), or an ESP-IDF build directory with --full

Options:
      --port <PORT>  The pad's app port, or its ROM bootloader port when several are connected (default: auto-detect)
      --full         Recovery flash: also write the bootloader, partition table and OTA data
  -h, --help         Print help
```

`<FIRMWARE>` es un archivo, o un directorio que contiene `opad-firmware.bin`. Antes de escribir nada se comprueba que la cabecera de la imagen de la aplicación sea para el ESP32-S3.
<!-- src: /desktop/cli/src/main.rs#resolve_flash_set, /desktop/cli/src/main.rs#Commands::Flash -->

**Actualización de la aplicación (sin `--full`).** `opadctl` pregunta al daemon desde qué ranura funciona el teclado y escribe la imagen en la otra ranura, seguida de una imagen `otadata` que la arranca una vez a prueba. La imagen en ejecución se queda como respaldo: el firmware solo confirma la nueva después de que el anfitrión configure el USB; si no, el bootloader vuelve a la antigua.
<!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

| Ranura en ejecución | Escribe |
|---|---|
| `ota_0` | Aplicación en `0x220000` (`ota_1`), después `otadata` en `0xf000` |
| `ota_1` | Aplicación en `0x20000` (`ota_0`), después `otadata` en `0xf000` |
| Desconocida (sin daemon, o firmware que no la comunica) | Aplicación en `0x20000` en el mismo sitio, sin respaldo |
<!-- src: /desktop/crates/opad-device/src/flash.rs#APP_PARTITION_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTADATA_OFFSET -->

**Flasheo de recuperación (`--full`).** Escribe el conjunto completo. Cada archivo se busca primero en la estructura de compilación de ESP-IDF y después junto a la imagen de la aplicación (la estructura de una versión publicada descomprimida):
<!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->

| Offset | Archivo | Obligatorio |
|---|---|---|
| `0x0` | `bootloader/bootloader.bin` o `bootloader.bin` | Sí |
| `0x8000` | `partition_table/partition-table.bin` o `partition-table.bin` | Sí |
| `0xf000` | `ota_data_initial.bin` | No (se escribe si está presente) |
| `0x20000` | `opad-firmware.bin` (o el archivo indicado) | Sí |

En ambos modos, `opadctl` reinicia el teclado en el bootloader de descarga de la ROM por USB, lo flashea con `espflash` y espera hasta 15 s a que vuelva como la aplicación OPad (`303a:8415`). Un teclado que no vuelve se notifica como un fallo que remite a `opadctl recover`.
<!-- src: /desktop/cli/src/main.rs#Commands::Flash, /desktop/daemon/src/ipc_handlers.rs#FinishFlash -->

---

## opadctl recover

```text
Recover a pad that no longer shows up as OPad: replug it, and its next boot is caught over USB and flashed with the full set. No buttons

Usage: opadctl recover [OPTIONS] <FIRMWARE>

Arguments:
  <FIRMWARE>  An ESP-IDF build directory or an unpacked release (as for flash --full)

Options:
      --timeout <TIMEOUT>  How long to wait for the replug, in seconds [default: 300]
  -h, --help               Print help
```

Resuelve el mismo conjunto de archivos que `flash --full`, te pide que desconectes y vuelvas a conectar el teclado y espera hasta `--timeout` segundos. Cuando aparece durante el arranque el puerto USB-Serial-JTAG de la ROM del teclado (`303a:1001`), `opadctl` lo retiene en modo de descarga, escribe el conjunto completo, lo reinicia en la aplicación con un reinicio por watchdog RTC y espera hasta 15 s a la aplicación OPad (`303a:8415`). Consulta [Recuperación](../recovery.md) §7.7.
<!-- src: /desktop/cli/src/main.rs#Commands::Recover, /desktop/crates/opad-device/src/flash.rs#recover -->

---

## opadctl bootloader

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl bootloader [OPTIONS]

Options:
      --port <PORT>  Explicit serial port (default: auto-detect)
  -h, --help         Print help
```

Pide al daemon que libere el puerto, reinicia el teclado en el bootloader de descarga de la ROM (`303a:1001`), devuelve el puerto al daemon e imprime el puerto del bootloader. El teclado se queda en modo de descarga para otras herramientas de flasheo.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

---

## opadctl firmware-update

```text
Update the pad's firmware from the signed release manifest (§U-3b)

Usage: opadctl firmware-update [OPTIONS]

Options:
      --yes   Consent without the interactive prompt. The firmware is still only flashed because you said so.
  -h, --help  Print help
```

1. Envía `GetFirmwareUpdate` e imprime la versión instalada, la ranura en ejecución, los bloqueos que haya, la versión disponible y sus notas. Si no hay nada más reciente, se detiene; si hay bloqueos, termina con un error.
2. Imprime el texto de consentimiento y pregunta `Flash the pad now? [y/N]` salvo que se indique `--yes`.
3. Envía `InstallFirmwareUpdate { confirm: true }`. El daemon sincroniza los contadores del teclado con este PC antes de escribir nada y después comunica la versión antigua y la nueva y la ranura en ejecución.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate, /desktop/daemon/src/firmware_update.rs#install -->

---

## opadctl update

```text
Show, check for or install an update of the OPad app from the update feed, as the app's Updates page does

Usage: opadctl update [OPTIONS]

Options:
      --channel <CHANNEL>  Follow this channel from now on: stable, or beta for release candidates
      --check              Check the feed now and wait for the answer
      --install            Download, verify and install the available update. The installer restarts OPad, so this returns once it has started.
  -h, --help               Print help
```

Las opciones se ejecutan en este orden y después se imprime el estado (canal, instalada, disponible, última comprobación, último error, última instalación):
<!-- src: /desktop/cli/src/main.rs#Commands::Update -->

| Opción | Efecto |
|---|---|
| `--channel <CHANNEL>` | Envía `SetUpdateChannel`. Cambiar de canal también inicia una comprobación del nuevo. |
| `--check` | Envía `CheckForUpdates` y sondea `GetUpdateStatus` cada 500 ms hasta que termina la comprobación; se rinde tras 120 s. |
| `--install` | Envía `InstallUpdate { component: App }`. Falla con `There is no update ready to install` cuando no hay ninguna lista. |

Una instalación cuyos archivos pertenecen a un gestor de paquetes muestra la versión disponible como `(this install cannot update itself)`.
<!-- src: /desktop/cli/src/main.rs#Commands::Update, /desktop/crates/opad-ipc/src/lib.rs#ComponentUpdate -->

---

## opadctl latency

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS]

Options:
      --reset  Clear the collected samples (e.g. right before playing a map)
  -h, --help   Print help
```

Sin opciones, imprime las estadísticas del teclado desde el flanco de la tecla hasta el envío HID, obtenidas de `GetStatus`: número de muestras, `p50`, `p99`, `p99.9`, `max` (µs) e informes aplazados (cambios de tecla que esperaron al siguiente sondeo USB). Con `--reset`, envía `ResetLatencyStats`.
<!-- src: /desktop/cli/src/main.rs#Commands::Latency, /protocol/opad.proto#DeviceStatus -->

---

## opadctl stats

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]

Options:
      --period <DAYS>  Set the history period first, in days (0 = all time). It is kept for the app and the pad too.
  -h, --help           Print help
```

Envía `GetTapStats`, o `SetTapHistoryPeriod` cuando se indica `--period` (de `0` a `3650` días). Imprime:
<!-- src: /desktop/cli/src/main.rs#Commands::Stats, /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

- **Intento** (el que está en curso o, si no, el último): mapa, `AVG` (todas las pulsaciones sobre la duración de la canción, pausas incluidas), `PEAK` (las 7 pulsaciones seguidas más rápidas) con los mejores 10 s, y una tabla de `Current`, `Tap avg`, `Peak` y `Presses` para `K1`, `K2` y `Combined`.
- **Historial** del periodo: intentos, `AVG`, `PEAK` y mejores 10 s, y `Tap avg`, `Avg peak`, `Best peak` y `Presses` por canal.
<!-- src: /desktop/cli/src/main.rs#format_tap_stats, /desktop/crates/opad-model/src/tap_rate.rs#WINDOW_INTERVALS -->

---

## opadctl board-test

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl board-test [OPTIONS]

Options:
      --fix-pins  If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again
      --magnet    Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop)
  -h, --help      Print help
```

Envía `RunBoardTest` e imprime cada comprobación. Termina con un error cuando falla alguna.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

| Opción | Efecto |
|---|---|
| `--fix-pins` | Cuando una comprobación indica que los pines de tecla no son los de la placa portadora, envía `UpdateConfig` con Key 1 = `GPIO10` y Key 2 = `GPIO7`, espera 300 ms y vuelve a ejecutar la prueba. Sin ella, el fallo sugiere `opadctl board-test --fix-pins`. |
| `--magnet` | Tras la prueba de placa, observa los sensores de un módulo Hall Effect V3 mientras sostienes un imán sobre cada tecla por turnos. |
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest, /desktop/cli/src/main.rs#magnet_test -->

---

## opadctl animation

```text
Play a result animation on the pad, as osu!'s results screen would

Usage: opadctl animation <NAME> [GRADE]

Arguments:
  <NAME>   pop, drop or burst (or an animation id); none stops the one playing
  [GRADE]  The grade it shows: SS, S, A, B, C or D [default: S]

Options:
  -h, --help  Print help
```

`<NAME>` y `[GRADE]` no distinguen mayúsculas. Envía `PlayAnimation`; el daemon lo rechaza sin teclado, con un firmware sin animaciones y durante un mapa.
<!-- src: /desktop/cli/src/main.rs#Commands::Animation, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

---

## opadctl restart

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl restart

Options:
  -h, --help  Print help
```

Envía `RestartPad`. El teclado guarda sus contadores, configuración, diseños y calibración, se reinicia en cuanto está inactivo y se vuelve a conectar.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart, /firmware/main/runtime/runtime.c#restart_now -->

---

## opadctl setup

```text
Perform initial system setup (udev permissions & directories)

Usage: opadctl setup

Options:
  -h, --help  Print help
```

Solo comprueba; no cambia nada.
<!-- src: /desktop/cli/src/main.rs#run_setup -->

| Plataforma | Comprobaciones |
|---|---|
| Linux | Si `/etc/udev/rules.d/70-opad.rules` o `/usr/lib/udev/rules.d/70-opad.rules` coincide con la regla integrada en `opadctl`. Si falta o es distinta, imprime los comandos que la instalan (o el comando `install-udev` de la AppImage). Después, si tosu puede leer la memoria de osu!: `ptrace_scope` de Yama y `cap_sys_ptrace` de tosu (consulta [Solución de problemas](../troubleshooting.md) §2). |
| Windows | Nada que comprobar: el teclado usa los controladores HID y CDC incluidos en Windows. |
<!-- src: /desktop/cli/src/main.rs#run_setup, /desktop/cli/src/main.rs#UDEV_RULE_PATHS -->
