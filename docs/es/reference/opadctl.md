---
title: Referencia de CLI opadctl
description: Referencia de línea de comandos y opciones para la utilidad de gestión de pad y daemon opadctl.
---

`opadctl` es la herramienta de gestión por línea de comandos para OPad. Se comunica con el servicio en segundo plano (`opad-daemon`) a través de IPC local para consultar el estado, sincronizar los contadores de pulsaciones totales, exportar e importar copias de seguridad, transmitir registros de diagnóstico y configurar parámetros del dispositivo. También puede realizar operaciones USB directas —como el flasheo de firmware y la recuperación del dispositivo— incluso cuando el daemon está detenido.
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

La mayoría de los subcomandos requieren que `opad-daemon` esté en ejecución. Si el daemon no está accesible, los comandos reportan un error a menos que pertenezcan al grupo de flasheo (`flash`, `recover`, `bootloader`), a los que se les permite operar de forma autónoma directamente contra el puerto serie.
<!-- src: /desktop/cli/src/main.rs#main -->

---

## opadctl status

Consulta e imprime el estado actual del daemon, el hardware conectado, la reconciliación de contadores, la telemetría del juego y la configuración del pad.
<!-- src: /desktop/cli/src/main.rs#Commands::Status -->

```text
Show device and daemon status

Usage: opadctl status

Options:
  -h, --help  Print help
```

La salida incluye:
- **Modo del Daemon**: El estado operativo del runtime del daemon.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Dispositivo ESP32**: Estado de conexión (`Connected` o `Disconnected`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **ID del Dispositivo y Perfil de Placa**: Identificador único de hardware (por ejemplo, `OPAD-XX:XX:XX:XX:XX:XX`) y perfil de hardware.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Versión de Firmware y Ranura en Ejecución**: Ranura de partición OTA en ejecución actualmente (`ota_0` u `ota_1`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Módulo de Entrada y Calibración**: Tipo de módulo detectado (por ejemplo, Hall Effect) y estado de calibración por tecla.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Contadores de Pulsaciones de Teclas**: Totales de pulsaciones a lo largo de la vida útil para Key 1, Key 2 y la opcional Key 3, además del número de generación del contador.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Estado de Propiedad**: Advierte si el pad pertenece a otra instalación y la sincronización de contadores está en pausa esperando la confirmación del usuario.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **tosu (osu!)**: Estado de conexión del WebSocket local de telemetría de tosu.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Latencia de Tecla**: Percentil 50, percentil 99.9 y latencia máxima entre el flanco y el informe HID en microsegundos.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Valores de Configuración**: Mapeos de pines GPIO activos, tiempo de bloqueo de rebote (debounce lockout) en microsegundos, gestos de deslizamiento, brillo de pantalla y tiempo de inactividad para suspensión.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

---

## opadctl sync

Desencadena la reconciliación manual inmediata de los contadores de pulsaciones y sincroniza la hora del dispositivo con el PC anfitrión.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

```text
Trigger manual counter reconciliation & time synchronization

Usage: opadctl sync

Options:
  -h, --help  Print help
```

Envía una solicitud `IpcRequest::ForceSync` al daemon. Si el dispositivo está inactivo y los contadores se reconcilian limpiamente, devuelve los valores actualizados de vida útil para Key 1 y Key 2.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

---

## opadctl reset

Restablece los contadores de pulsaciones de por vida tanto en la base de datos del anfitrión como en el pad conectado.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

```text
Reset lifetime key counters (requires deliberate confirmation)

Usage: opadctl reset [OPTIONS]

Options:
      --yes   Confirm destructive reset of lifetime key counters
  -h, --help  Print help
```

Debido a que el restablecimiento de contadores es irreversible, ejecutar sin `--yes` cancela la ejecución. Cuando se confirma, se asigna y almacena una nueva generación de contadores.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

---

## opadctl export

Exporta la configuración del pad y los contadores de pulsaciones a un archivo de copia de seguridad JSON portátil.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl export <FILE>

Arguments:
  <FILE>  Path to write backup JSON file

Options:
  -h, --help  Print help
```

El archivo JSON generado contiene el ID del dispositivo, la generación de contadores, las estadísticas de vida útil y la configuración completa del dispositivo.
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

Antes de aplicar los cambios, `opadctl` valida la estructura de la copia de seguridad y muestra una vista previa comparativa indicando los conteos actuales frente a los entrantes, la coincidencia del ID del dispositivo y advertencias de retroceso de contadores. En modo interactivo, se requiere confirmación explícita (`y/N`) a menos que se especifique `--yes`.
<!-- src: /desktop/cli/src/main.rs#Commands::Import -->

---

## opadctl monitor

Transmite eventos de diagnóstico en vivo desde los búferes de registro del daemon y del dispositivo.
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

- `-l, --limit`: Número de entradas históricas de registro a mostrar inicialmente (por defecto 50).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `-f, --follow`: Sondea continuamente el concentrador de registros cada 500 ms en busca de nuevos eventos entrantes.
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--level`: Filtra mensajes por nivel mínimo de severidad (`debug`, `info`, `warn`, `error`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--source`: Restringe mensajes a una sola fuente de registro (`host`, `esp`, `program`, `tosu`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->

---

## opadctl flash

Flashea un nuevo binario de firmware en el ESP32-S3 a través de USB utilizando `espflash`.
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

- En modo normal (sin `--full`), `opadctl flash` consulta al daemon la ranura OTA actualmente en ejecución (`ota_0` u `ota_1`), planifica una actualización A/B en la ranura alternativa inactiva, solicita al daemon que libere el puerto serie, reinicia el pad en modo de descarga ROM de forma automatizada, flashea el binario y verifica que el pad re-enumere correctamente.
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->
- Con `--full`, realiza un flasheo completo de recuperación: escribe `bootloader.bin` en `0x0000`, `partition-table.bin` en `0x8000`, `ota_data_initial.bin` en `0xf000` y la imagen de aplicación en `0x20000`.
  <!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->
- `--port <PORT>`: Anula la detección automática del puerto serie si hay múltiples dispositivos serie USB conectados.
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

---

## opadctl recover

Recupera un pad que ya no enumera como OPad en el bus USB sin necesidad de presionar ningún botón físico.
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

Este comando espera a que el usuario desconecte y vuelva a conectar el cable USB del pad. Al conectarse, captura el bootloader ROM del ESP32-S3 durante su ventana inicial de enumeración, escribe el conjunto completo de flasheo (bootloader, tabla de particiones, datos OTA e imagen de aplicación) y espera hasta 15 segundos a que el pad arranque en la aplicación. Los botones físicos BOOT y RESET nunca se presionan.
<!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

---

## opadctl bootloader

Reinicia el pad conectado en modo bootloader de descarga ROM de forma automatizada por USB y lo deja en ese estado.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl bootloader [OPTIONS]

Options:
      --port <PORT>  Explicit serial port (default: auto-detect)
  -h, --help         Print help
```

Pide a `opad-daemon` que libere el puerto serie CDC, inicia el reinicio por software en el bootloader ROM del ESP32-S3 y finaliza. El dispositivo permanece en modo de descarga listo para herramientas de desarrollo de bajo nivel o flasheo.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

---

## opadctl firmware-update

Comprueba e instala actualizaciones firmadas de firmware desde el manifiesto oficial de actualización.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

```text
Update the pad's firmware from the signed release manifest (§U-3b)

Usage: opadctl firmware-update [OPTIONS]

Options:
      --yes   Consent without the interactive prompt. The firmware is still only flashed because you said so.
  -h, --help  Print help
```

Consulta a `opad-daemon` mediante `IpcRequest::GetFirmwareUpdate` para inspeccionar la versión instalada, la versión disponible, las notas de la versión y los bloqueadores activos. Antes de flashear, solicita confirmación explícita del usuario a menos que se pase `--yes`. El daemon sincroniza los contadores en disco antes del flasheo para garantizar la integridad de los contadores.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

---

## opadctl latency

Muestra estadísticas de latencia medidas en el dispositivo entre la pulsación de la tecla y el informe HID.
<!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS]

Options:
      --reset  Clear the collected samples (e.g. right before playing a map)
  -h, --help   Print help
```

- Cuando se ejecuta sin opciones, muestra el conteo de muestras, p50, p99, p99.9 y la latencia máxima en microsegundos, junto con los informes diferidos que tuvieron que esperar al siguiente microframe USB.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency -->
- Con `--reset`, limpia los contadores de muestras en el dispositivo para iniciar una nueva ventana de medición.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

---

## opadctl stats

Muestra estadísticas de frecuencia de pulsación en teclas por minuto (KPM) para los intentos de juego en vivo o recientes, así como para sesiones históricas agregadas.
<!-- src: /desktop/cli/src/main.rs#Commands::Stats -->

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]

Options:
      --period <DAYS>  Set the history period first, in days (0 = all time). It is kept for the app and the pad too.
  -h, --help           Print help
```

- Muestra el KPM promedio de la canción y el KPM de ráfaga máxima (las 7 pulsaciones consecutivas más rápidas) para Key 1, Key 2 y canales combinados.
  <!-- src: /desktop/cli/src/main.rs#format_tap_stats -->
- `--period <DAYS>`: Configura la ventana histórica de agregación en días (0 representa el historial completo).
  <!-- src: /desktop/cli/src/main.rs#Commands::Stats -->

---

## opadctl board-test

Ejecuta diagnósticos de hardware en las PCB del carrier y del módulo de entrada intercambiable.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl board-test [OPTIONS]

Options:
      --fix-pins  If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again
      --magnet    Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop)
  -h, --help      Print help
```

Verifica voltajes de identificación de módulo, orientación del cable (detecta cables FFC invertidos), integridad de líneas de teclas y comunicación con registros del ADC ADS7028 en módulos Hall Effect V3. Ambos interruptores deben estar liberados mientras se ejecuta la prueba.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

- `--fix-pins`: Si las teclas están mapeadas a pines de cabecera independientes por defecto en lugar del enrutamiento del carrier V3, asigna automáticamente Key 1 a GPIO10 y Key 2 a GPIO7, pausa 300 ms y vuelve a probar.
  <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
- `--magnet`: Inicia la prueba interactiva de sensores en módulos Hall Effect, transmitiendo voltajes y desfases de código en vivo mientras se acerca un imán a cada sensor por turno (hasta 60 segundos).
  <!-- src: /desktop/cli/src/main.rs#magnet_test -->

---

## opadctl animation

Desencadena una animación de calificación de pantalla de resultados en la pantalla del pad.
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

- `<NAME>`: Tipo de animación integrada (`pop`, `drop`, `burst` o ID de animación numérico). Especificar `none` detiene las animaciones en reproducción.
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->
- `[GRADE]`: Letra de calificación a renderizar (`SS`, `S`, `A`, `B`, `C` o `D`, por defecto `S`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->

---

## opadctl restart

Ordena al pad conectado que guarde el estado y realice un reinicio limpio.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart -->

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl restart

Options:
  -h, --help  Print help
```

Envía `IpcRequest::RestartPad` al daemon. El pad vuelca los contadores persistentes a NVS, pospone el reinicio hasta que cese el juego activo y se reinicia limpiamente. El pad re-enumera y se reconecta en unos pocos segundos.
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
- Comprueba si la regla udev (`/etc/udev/rules.d/70-opad.rules` o `/usr/lib/udev/rules.d/70-opad.rules`) está instalada y coincide con la regla incrustada en el paquete. Si falta o está modificada, imprime los comandos de instalación.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
- Comprueba la configuración de `ptrace_scope` de Yama en el kernel Linux y determina si tosu posee privilegios `CAP_SYS_PTRACE` para inspeccionar la memoria del proceso de osu!.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->

En Windows:
- Informa al usuario de que no se requiere configuración ya que el dispositivo utiliza los controladores integrados en el sistema de USB HID (`hidclass.sys`) y CDC (`usbser.sys`).
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
