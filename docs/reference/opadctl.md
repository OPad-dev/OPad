---
title: opadctl CLI Reference
description: Command-line reference and options for the opadctl pad and daemon management utility.
---

`opadctl` is the command-line management tool for OPad. It communicates with the background service (`opad-daemon`) over local IPC to query status, synchronize lifetime press counters, export and import backups, stream diagnostic logs, and configure device parameters. It can also perform direct USB operations—such as firmware flashing and device recovery—even when the daemon is stopped.
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

Most subcommands require `opad-daemon` to be running. If the daemon is unreachable, commands report an error unless they belong to the flashing group (`flash`, `recover`, `bootloader`), which are permitted to operate standalone directly against the serial port.
<!-- src: /desktop/cli/src/main.rs#main -->

---

## opadctl status

Queries and prints the current status of the daemon, connected hardware, counter reconciliation, game telemetry, and pad configuration.
<!-- src: /desktop/cli/src/main.rs#Commands::Status -->

```text
Show device and daemon status

Usage: opadctl status

Options:
  -h, --help  Print help
```

The output includes:
- **Daemon Mode**: The operating state of the daemon runtime.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **ESP32 Device**: Connection status (`Connected` or `Disconnected`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Device ID & Board Profile**: Unique hardware identifier (e.g. `OPAD-XX:XX:XX:XX:XX:XX`) and hardware profile.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Firmware Version & Running Slot**: Currently executing OTA partition slot (`ota_0` or `ota_1`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Input Module & Calibration**: Detected module type (e.g., Hall Effect) and per-key calibration state.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Key Press Counters**: Lifetime press totals for Key 1, Key 2, and optional Key 3, plus the counter generation number.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Ownership State**: Warns if the pad belongs to another installation and counter sync is paused awaiting user takeover.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **tosu (osu!)**: Connection status of the local tosu telemetry WebSocket.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Key Latency**: 50th percentile, 99.9th percentile, and maximum edge-to-HID report latency in microseconds.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
- **Configuration Values**: Active GPIO pin mappings, debounce lockout time in microseconds, swipe gestures, display brightness, and sleep timeout.
  <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

---

## opadctl sync

Triggers immediate manual reconciliation of keypress counters and synchronizes device clock time with the host PC.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

```text
Trigger manual counter reconciliation & time synchronization

Usage: opadctl sync

Options:
  -h, --help  Print help
```

Sends an `IpcRequest::ForceSync` request to the daemon. If the device is idle and counters reconcile cleanly, returns the updated lifetime values for Key 1 and Key 2.
<!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

---

## opadctl reset

Resets lifetime key press counters on both the host database and the connected pad.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

```text
Reset lifetime key counters (requires deliberate confirmation)

Usage: opadctl reset [OPTIONS]

Options:
      --yes   Confirm destructive reset of lifetime key counters
  -h, --help  Print help
```

Because counter reset is irreversible, running without `--yes` aborts execution. When confirmed, a new counter generation is allocated and stored.
<!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

---

## opadctl export

Exports pad configuration and lifetime counters to a portable JSON backup file.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl export <FILE>

Arguments:
  <FILE>  Path to write backup JSON file

Options:
  -h, --help  Print help
```

The generated JSON file contains the device ID, counter generation, lifetime statistics, and full device configuration.
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

---

## opadctl import

Imports configuration and lifetime counters from a validated JSON backup file.
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

Before applying, `opadctl` validates the backup structure and displays a preview comparison showing current vs incoming counts, device ID matching, and counter rollback warnings. In interactive mode, explicit confirmation (`y/N`) is required unless `--yes` is specified.
<!-- src: /desktop/cli/src/main.rs#Commands::Import -->

---

## opadctl monitor

Streams live diagnostic events from the daemon and device log buffers.
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

- `-l, --limit`: Number of historical log entries to display initially (default 50).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `-f, --follow`: Polls the log hub continuously every 500 ms for new incoming events.
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--level`: Filters messages by minimum severity level (`debug`, `info`, `warn`, `error`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->
- `--source`: Restricts messages to a single log source (`host`, `esp`, `program`, `tosu`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Monitor -->

---

## opadctl flash

Flashes a new firmware binary onto the ESP32-S3 over USB using `espflash`.
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

- In normal mode (without `--full`), `opadctl flash` queries the daemon for the current running OTA slot (`ota_0` or `ota_1`), plans an A/B update into the inactive alternate slot, asks the daemon to release the serial port, reboots the pad into ROM download mode hands-free, flashes the binary, and verifies that the pad re-enumerates successfully.
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->
- With `--full`, performs a complete recovery flash: writes `bootloader.bin` at `0x0000`, `partition-table.bin` at `0x8000`, `ota_data_initial.bin` at `0xf000`, and the application image at `0x20000`.
  <!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->
- `--port <PORT>`: Overrides automatic serial port detection if multiple USB serial devices are connected.
  <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

---

## opadctl recover

Recovers a pad that no longer enumerates as OPad on the USB bus without pressing any physical buttons.
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

This command waits for the user to unplug and replug the pad's USB cable. On connection, it catches the ESP32-S3 ROM bootloader during its initial enumeration window, writes the complete flash set (bootloader, partition table, OTA data, and application image), and waits up to 15 seconds for the pad to boot into the application. Physical BOOT and RESET buttons are never pressed.
<!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

---

## opadctl bootloader

Reboots the connected pad into ROM download bootloader mode hands-free over USB and leaves it there.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl bootloader [OPTIONS]

Options:
      --port <PORT>  Explicit serial port (default: auto-detect)
  -h, --help         Print help
```

Asks `opad-daemon` to release the CDC serial port, initiates software reboot into the ESP32-S3 ROM bootloader, and exits. The device remains in download mode ready for low-level development or flashing tools.
<!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

---

## opadctl firmware-update

Checks for and installs signed firmware updates from the official update feed manifest.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

```text
Update the pad's firmware from the signed release manifest (§U-3b)

Usage: opadctl firmware-update [OPTIONS]

Options:
      --yes   Consent without the interactive prompt. The firmware is still only flashed because you said so.
  -h, --help  Print help
```

Queries `opad-daemon` via `IpcRequest::GetFirmwareUpdate` to inspect installed version, available version, release notes, and active blockers. Before flashing, asks for explicit user confirmation unless `--yes` is passed. The daemon synchronizes counters to disk prior to flashing to guarantee counter integrity.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

---

## opadctl latency

Displays device-measured keypress-to-HID-report latency statistics.
<!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS]

Options:
      --reset  Clear the collected samples (e.g. right before playing a map)
  -h, --help   Print help
```

- When run without options, displays sample count, p50, p99, p99.9, and maximum latency in microseconds, along with deferred reports that had to wait for the next USB microframe.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency -->
- With `--reset`, clears sample counters on the device to start a fresh measurement window.
  <!-- src: /desktop/cli/src/main.rs#Commands::Latency -->

---

## opadctl stats

Displays tap rate statistics in keys per minute (KPM) for live or recent gameplay attempts as well as aggregated historical sessions.
<!-- src: /desktop/cli/src/main.rs#Commands::Stats -->

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]

Options:
      --period <DAYS>  Set the history period first, in days (0 = all time). It is kept for the app and the pad too.
  -h, --help           Print help
```

- Shows song-wide average KPM and peak burst KPM (fastest 7 consecutive presses) for Key 1, Key 2, and combined channels.
  <!-- src: /desktop/cli/src/main.rs#format_tap_stats -->
- `--period <DAYS>`: Configures the historical aggregation window in days (0 represents all-time history).
  <!-- src: /desktop/cli/src/main.rs#Commands::Stats -->

---

## opadctl board-test

Runs hardware diagnostics on the carrier and swappable input module PCBs.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl board-test [OPTIONS]

Options:
      --fix-pins  If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again
      --magnet    Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop)
  -h, --help      Print help
```

Verifies module identification voltages, cable orientation (detects reversed FFC cables), key line integrity, and ADS7028 ADC register communication on Hall Effect V3 modules. Both switches must be released while the test executes.
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

- `--fix-pins`: If keys are mapped to default standalone header pins instead of the V3 carrier routing, automatically sets Key 1 to GPIO10 and Key 2 to GPIO7, pauses 300 ms, and re-tests.
  <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
- `--magnet`: Initiates interactive sensor testing on Hall Effect modules, streaming live voltages and code offsets while each key magnet is brought near the sensor (up to 60 seconds).
  <!-- src: /desktop/cli/src/main.rs#magnet_test -->

---

## opadctl animation

Triggers a result screen grade animation on the pad display.
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

- `<NAME>`: Built-in animation type (`pop`, `drop`, `burst`, or numerical animation ID). Specifying `none` stops currently playing animations.
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->
- `[GRADE]`: Grade letter to render (`SS`, `S`, `A`, `B`, `C`, or `D`, defaults to `S`).
  <!-- src: /desktop/cli/src/main.rs#Commands::Animation -->

---

## opadctl restart

Commands the connected pad to save state and perform a clean reboot.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart -->

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl restart

Options:
  -h, --help  Print help
```

Sends `IpcRequest::RestartPad` to the daemon. The pad flushes persistent counters to NVS, delays restart until active gameplay ceases, and boots afresh. The pad re-enumerates and reconnects within a few seconds.
<!-- src: /desktop/cli/src/main.rs#Commands::Restart -->

---

## opadctl setup

Performs platform environment verification and system configuration checks.
<!-- src: /desktop/cli/src/main.rs#Commands::Setup -->

```text
Perform initial system setup (udev permissions & directories)

Usage: opadctl setup

Options:
  -h, --help  Print help
```

On Linux:
- Checks whether the udev rule (`/etc/udev/rules.d/70-opad.rules` or `/usr/lib/udev/rules.d/70-opad.rules`) is installed and matches the embedded package rule. If missing or altered, prints installation commands.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
- Checks Linux kernel Yama `ptrace_scope` settings and determines whether tosu possesses `CAP_SYS_PTRACE` privileges to inspect osu! process memory.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->

On Windows:
- Informs the user that no setup is needed because the device uses inbox USB HID (`hidclass.sys`) and CDC (`usbser.sys`) drivers.
  <!-- src: /desktop/cli/src/main.rs#run_setup -->
