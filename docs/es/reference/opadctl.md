---
title: Referencia de la CLI opadctl
description: Referencia de línea de comandos y opciones para la utilidad de gestión del teclado y del daemon opadctl.
sidebar:
  order: 1
---

`opadctl` es la herramienta de gestión por línea de comandos de OPad. Se comunica con el servicio en segundo plano (`opad-daemon`) por IPC local para consultar el estado, sincronizar contadores de pulsaciones de por vida, exportar e importar copias de seguridad, transmitir registros de diagnóstico y controlar funciones del teclado. Los comandos de flasheo (`flash`, `recover`, `bootloader`) también pueden operar directamente por serie USB cuando el daemon está detenido.
<!-- src: /desktop/cli/src/main.rs#main -->

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

La mayoría de los subcomandos requieren que `opad-daemon` esté en ejecución. `flash`, `recover` y `bootloader` funcionan incluso si el daemon no está ejecutándose (cuando sí lo está, le piden que libere primero el puerto serie). `setup` se ejecuta localmente sin requerir el daemon.
<!-- src: /desktop/cli/src/main.rs#main -->

---

## opadctl status

Consulta e imprime el estado actual del daemon, el hardware conectado, la reconciliación de contadores, la telemetría del juego y la configuración del teclado.
<!-- src: /desktop/cli/src/main.rs#Commands::Status -->

```text
Show device and daemon status

Usage: opadctl status

Options:
  -h, --help  Print help
```

La salida incluye:
- **Daemon Mode**: El estado operativo del entorno de ejecución del daemon.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **ESP32 Device**: Estado de conexión (`Connected` o `Disconnected`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Device ID & Board Profile**: Identificador de hardware derivado de la dirección MAC del ESP32-S3 (formato `OPAD-XXXXXXXXXXXX`, 12 dígitos hexadecimales) y perfil de placa.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status --> <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->
- **Firmware Version & Running Slot**: Versión del firmware y ranura de partición OTA activa (`ota_0` u `ota_1`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Input Module & Calibration**: Tipo de módulo detectado (por ejemplo, Mechanical o Hall Effect) y estado de calibración por tecla.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Key Press Counters**: Totales de pulsaciones de por vida para Key 1, Key 2 y Key 3 (cuando está presente), más el número de generación de contadores.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Ownership State**: Advierte si el teclado pertenece a otra instalación y la sincronización de contadores está en pausa a la espera de que el usuario asuma la propiedad.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **tosu (osu!)**: Estado de conexión del WebSocket local de telemetría de `tosu`.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Key Latency**: Percentil 50 (`p50`), percentil 99.9 (`p99.9`) y latencia máxima desde el flanco de tecla hasta el reporte HID en microsegundos.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Configuration Values**: Asignaciones activas de pines GPIO, tiempo de bloqueo de antirrebote en microsegundos, acciones de deslizamiento, brillo de pantalla y tiempo de reposo.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

---

## opadctl sync

Dispara una reconciliación manual inmediata de los contadores de pulsaciones y sincroniza la hora del reloj con el PC anfitrión.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

```text
Trigger manual counter reconciliation & time synchronization

Usage: opadctl sync

Options:
  -h, --help  Print help
```

Envía `IpcRequest::ForceSync` al daemon. Si el dispositivo está en reposo y los contadores se reconcilian limpiamente, imprime los conteos de por vida sincronizados.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

---

## opadctl reset

Reinicia los contadores de pulsaciones de por vida tanto en la base de datos del anfitrión como en el teclado conectado.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

```text
Reset lifetime key counters (requires deliberate confirmation)

Usage: opadctl reset [OPTIONS]

Options:
      --yes   Confirm destructive reset of lifetime key counters
  -h, --help  Print help
```

Como el reinicio de contadores es irreversible, ejecutarlo sin `--yes` aborta la operación. Al confirmarlo, se asigna y almacena una nueva generación de contadores.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

---

## opadctl export

Exporta la configuración del teclado y los contadores de por vida a un archivo de copia de seguridad JSON portable.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl export <FILE>

Arguments:
  <FILE>  Path to write backup JSON file

Options:
  -h, --help  Print help
```

El archivo JSON generado contiene el ID de dispositivo, la generación de contadores, los conteos de por vida y la configuración del dispositivo.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

---

## opadctl import

Importa la configuración y los contadores de por vida desde un archivo de copia de seguridad JSON validado.
<!-- src: /desktop/cli/src/main.rs#Commands::Import -->

```text
Import configuration and counters from a validated JSON backup

Usage: opadctl import [OPTIONS] <FILE>

Arguments:
  <FILE>  Path to JSON backup file

Options:
      --yes   Skip preview and confirmation prompt
  -h, --help  Print help
```

Antes de aplicarla, `opadctl` valida la estructura de la copia de seguridad y muestra una previsualización comparando los conteos actuales frente a los entrantes, la coincidencia del ID de dispositivo y las advertencias de retroceso de contadores. En modo interactivo se requiere confirmación explícita (`y/N`) salvo que se especifique `--yes`.
<!-- src: /desktop/cli/src/main.rs#Commands::Import -->

---

## opadctl monitor

Transmite en vivo los eventos de diagnóstico desde los búferes de registro del daemon y del dispositivo.
<!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->

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

- `-l, --limit`: Número de entradas históricas de registro a mostrar inicialmente (por defecto `50`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `-f, --follow`: Sondea continuamente el concentrador de registros cada 500 ms buscando nuevos eventos entrantes.
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--level`: Filtra mensajes por nivel mínimo de severidad (`debug`, `info`, `warn`, `error`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--source`: Restringe los mensajes a una sola fuente de registro (`host`, `esp`, `program`, `tosu`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->

---

## opadctl flash

Flashea un nuevo binario de firmware en el ESP32-S3 por USB usando `espflash`.
<!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

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

- En modo normal (sin `--full`), `opadctl flash` consulta al daemon la ranura OTA activa (`ota_0` u `ota_1`), planifica una actualización A/B en la ranura alternativa inactiva, pide al daemon que libere el puerto serie, reinicia el teclado en modo de descarga ROM sin botones, flashea el binario y verifica que el teclado vuelva a enumerar como OPad (`303a:8415`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->
- Con `--full`, realiza un flasheo completo de recuperación: escribe `bootloader.bin` en `0x0000`, `partition-table.bin` en `0x8000`, `ota_data_initial.bin` en `0xf000` y la imagen de aplicación en `0x20000`.
  <!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->
- `--port <PORT>`: Anula la detección automática de puerto serie si hay varios dispositivos serie USB conectados.
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

---

## opadctl recover

Recupera un teclado que dejó de enumerar como OPad en el bus USB sin pulsar ningún botón físico.
<!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

```text
Recover a pad that no longer shows up as OPad: replug it, and its next boot is caught over USB and flashed with the full set. No buttons

Usage: opadctl recover [OPTIONS] <FIRMWARE>

Arguments:
  <FIRMWARE>  An ESP-IDF build directory or an unpacked release (as for flash --full)

Options:
      --timeout <TIMEOUT>  How long to wait for the replug, in seconds [default: 300]
  -h, --help               Print help
```

Este comando espera a que el usuario desconecte y vuelva a conectar el cable USB del teclado (hasta `--timeout`, por defecto 300 s). Al conectarse, captura el bootloader ROM del ESP32-S3 (`303a:1001`) durante su ventana inicial de enumeración, escribe el conjunto completo de flash (bootloader, tabla de particiones, datos OTA e imagen de aplicación), reinicia el chip mediante el watchdog RTC y espera hasta 15 segundos a que el teclado arranque en la aplicación (`303a:8415`). Nunca se pulsan los botones físicos BOOT ni RESET.
<!-- src: /desktop/cli/src/main.rs#Commands::Recover --> <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->

---

## opadctl bootloader

Reinicia el teclado conectado en modo de descarga del bootloader ROM sin botones por USB y lo deja en ese estado.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl bootloader [OPTIONS]

Options:
      --port <PORT>  Explicit serial port (default: auto-detect)
  -h, --help         Print help
```

Pide a `opad-daemon` que libere el puerto serie CDC, inicia el reinicio por software hacia el bootloader ROM del ESP32-S3 (`303a:1001`) y termina. El dispositivo permanece en modo de descarga listo para herramientas de desarrollo o flasheo de bajo nivel.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

---

## opadctl firmware-update

Comprueba e instala actualizaciones de firmware firmadas desde el manifiesto del feed oficial de actualizaciones.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

```text
Update the pad's firmware from the signed release manifest (§U-3b)

Usage: opadctl firmware-update [OPTIONS]

Options:
      --yes   Consent without the interactive prompt. The firmware is still only flashed because you said so.
  -h, --help  Print help
```

Consulta a `opad-daemon` mediante `IpcRequest::GetFirmwareUpdate` para inspeccionar la versión instalada, la versión disponible, las notas de lanzamiento y los bloqueos activos. Antes de flashear, solicita confirmación explícita del usuario salvo que se pase `--yes`. El daemon sincroniza los contadores en disco antes de flashear para preservar su integridad.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

---

## opadctl update

Muestra, comprueba o instala una actualización de la aplicación de escritorio de OPad desde el feed de actualizaciones firmado.
<!-- src: /desktop/cli/src/main.rs#Commands::Update -->

```text
Show, check for or install an update of the OPad app from the update feed, as the app's Updates page does

Usage: opadctl update [OPTIONS]

Options:
      --channel <CHANNEL>  Follow this channel from now on: stable, or beta for release candidates
      --check              Check the feed now and wait for the answer
      --install            Download, verify and install the available update. The installer restarts OPad, so this returns once it has started.
  -h, --help               Print help
```

- `--channel <CHANNEL>`: Cambia el daemon para seguir el canal `stable` o `beta` (`IpcRequest::SetUpdateChannel`), lo que también inicia una comprobación del nuevo canal.
  <!-- src: /desktop/cli/src/main.rs#Commands::Update -->
- `--check`: Dispara `IpcRequest::CheckForUpdates` y sondea `IpcRequest::GetUpdateStatus` cada 500 ms hasta que la comprobación termina o agota el tiempo de espera tras 120 segundos.
  <!-- src: /desktop/cli/src/main.rs#Commands::Update -->
- `--install`: Envía `IpcRequest::InstallUpdate` para la aplicación de escritorio si hay una actualización lista para instalar; regresa en cuanto el actualizador inicia la instalación (OPad se reinicia al terminar). Las instalaciones gestionadas por gestor de paquetes marcadas como solo notificación (como Arch) muestran `(update it with your package manager)` en lugar de instalar.
  <!-- src: /desktop/cli/src/main.rs#Commands::Update -->

---

## opadctl latency

Muestra las estadísticas de latencia desde la pulsación de tecla hasta el reporte HID medidas en el propio dispositivo.
<!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS]

Options:
      --reset  Clear the collected samples (e.g. right before playing a map)
  -h, --help   Print help
```

- Al ejecutarse sin opciones, muestra el número de muestras, `p50`, `p99`, `p99.9` y latencia máxima en microsegundos, junto con los reportes diferidos que tuvieron que esperar a la siguiente trama USB de 1 ms.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency --> <!-- src: /firmware/main/input/latency_stats.c -->
- Con `--reset`, limpia los contadores de muestras en el dispositivo para iniciar una nueva ventana de medición.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

---

## opadctl stats

Muestra estadísticas de ritmo de pulsación en teclas por minuto (KPM) para el intento actual o más reciente, así como para sesiones históricas agregadas.
<!-- src: /desktop/cli/src/main.rs#Commands::Stats -->

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]

Options:
      --period <DAYS>  Set the history period first, in days (0 = all time). It is kept for the app and the pad too.
  -h, --help           Print help
```

- Muestra el KPM medio de pulsación, el pico de ráfaga KPM (las 7 pulsaciones consecutivas más rápidas, `WINDOW_INTERVALS = 6` intervalos) y el KPM global de la canción para Key 1, Key 2 y combinado.
  <!-- src: /desktop/cli/src/main.rs#format_tap_stats --> <!-- src: /desktop/crates/opad-model/src/tap_rate.rs -->
- `--period <DAYS>`: Configura la ventana de agregación histórica en días (`0` representa todo el historial; máximo `3650` días). El ajuste persiste tanto en la aplicación de escritorio como en el teclado.
  <!-- src: /desktop/cli/src/main.rs#Commands::Stats --> <!-- src: /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

---

## opadctl board-test

Ejecuta diagnósticos de hardware en las PCBs del carrier y del módulo de entrada intercambiable.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl board-test [OPTIONS]

Options:
      --fix-pins  If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again
      --magnet    Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop)
  -h, --help      Print help
```

Verifica las tensiones de identificación del módulo, la orientación del cable (detecta cables FFC invertidos), la integridad de los pull-up de las líneas de teclas y la comunicación de registros del ADC ADS7028 en módulos Hall Effect V3. Suelta todos los interruptores mientras se ejecuta la prueba.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

- `--fix-pins`: Si las teclas están asignadas a los pines predeterminados de cabezal autónomo en lugar del enrutado del carrier V3, configura Key 1 en `GPIO10` y Key 2 en `GPIO7`, espera 300 ms y vuelve a probar.
  <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
- `--magnet`: Ejecuta comprobaciones en vivo de los sensores en módulos Hall Effect V3, consultando lecturas mientras se acerca un imán a cada tecla por turno (hasta 60 segundos, `Ctrl-C` para detener).
  <!-- src: /desktop/cli/src/main.rs#magnet_test -->

---

## opadctl animation

Dispara una animación de calificación de pantalla de resultados en la pantalla del teclado.
<!-- src: /desktop/cli/src/main.rs#Commands::Animation -->

```text
Play a result animation on the pad, as osu!'s results screen would

Usage: opadctl animation <NAME> [GRADE]

Arguments:
  <NAME>   pop, drop or burst (or an animation id); none stops the one playing
  [GRADE]  The grade it shows: SS, S, A, B, C or D [default: S]

Options:
  -h, --help  Print help
```

- `<NAME>`: Tipo de animación integrada (`pop`, `drop`, `burst` o ID numérico de animación). Especificar `none` detiene cualquier animación en reproducción.
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->
- `[GRADE]`: Letra de calificación a mostrar (`SS`, `S`, `A`, `B`, `C` o `D`; por defecto `S`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->

---

## opadctl restart

Ordena al teclado conectado guardar su estado y realizar un reinicio limpio.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart -->

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl restart

Options:
  -h, --help  Print help
```

Envía `IpcRequest::RestartPad` al daemon. El teclado vuelca los contadores persistentes a NVS, espera hasta que no haya ningún mapa en juego y se reinicia limpiamente.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart -->

---

## opadctl setup

Realiza la verificación del entorno de plataforma y comprobaciones de configuración del sistema.
<!-- src: /desktop/cli/src/main.rs#Commands::Setup -->

```text
Perform initial system setup (udev permissions & directories)

Usage: opadctl setup

Options:
  -h, --help  Print help
```

En Linux:
- Comprueba si la regla udev (`/etc/udev/rules.d/70-opad.rules` o `/usr/lib/udev/rules.d/70-opad.rules`) está instalada y coincide con la regla integrada, imprimiendo comandos de instalación si falta o difiere.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
- Comprueba el parámetro `ptrace_scope` de Yama en el núcleo Linux y si `tosu` dispone de `cap_sys_ptrace` para inspeccionar la memoria del proceso `osu!`.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->

En Windows:
- Informa que no se necesita configuración manual de controladores porque OPad usa los controladores integrados USB HID y CDC (`usbser.sys`) de Windows.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
