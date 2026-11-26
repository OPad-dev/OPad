---
title: opadctl CLI Reference
description: The opadctl commands for common tasks, then every subcommand with its usage, options, what it asks the daemon for, and what it prints.
sidebar:
  order: 1
---

`opadctl` is OPad's command-line tool. Most subcommands are requests to `opad-daemon` over local IPC (see [IPC Protocol Reference](ipc.md)); the flashing subcommands also work with no daemon running. <!-- src: /desktop/cli/src/main.rs#main -->

## Common tasks

| Task | Command |
|---|---|
| Check that the pad and the daemon are connected | `opadctl status` |
| Back up counters and settings | `opadctl export ~/opad-backup.json` |
| Restore a backup | `opadctl import ~/opad-backup.json` |
| Update the pad's firmware | `opadctl firmware-update` |
| Update the OPad app | `opadctl update --check`, then `opadctl update --install` |
| See your tap rate (KPM) | `opadctl stats` |
| Sync counters and the pad's clock now | `opadctl sync` |
| Restart the pad (e.g. after swapping its input module) | `opadctl restart` |
| Reset lifetime counters | `opadctl reset --yes` |
| Check Linux permissions (udev, tosu) | `opadctl setup` |
| Watch the logs live | `opadctl monitor -f` |
| Recover a pad that no longer shows up as OPad | `opadctl recover <firmware folder>`, then replug it |
<!-- src: /desktop/cli/src/main.rs#Commands -->

## How to read this page

The `text` blocks and the **Help** columns are verbatim from the `--help` output of `opadctl` built from `desktop/cli` (`cargo build -p opadctl`). Every subcommand also takes `-h, --help`; `opadctl --help` lists them all. <!-- src: /desktop/cli/src/main.rs#Commands -->

| Subcommands | Needs `opad-daemon` |
|---|---|
| `setup` | No. It runs locally and never connects. |
| `flash`, `recover`, `bootloader` | No. If the daemon answers, `opadctl` asks it to release the serial port first; if not, it prints `Could not reach opad-daemon (...); flashing without it.` and goes on. |
| Every other subcommand | Yes. Without it, `opadctl` exits with `Failed to connect to opad-daemon. Is the daemon running?` |
<!-- src: /desktop/cli/src/main.rs#main -->

A daemon that rejects the IPC handshake (for example a daemon of another app version after an update) counts as unreachable. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#connect_and_handshake, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Everyday commands

### opadctl status

```text
Show device and daemon status

Usage: opadctl status
```

Sends `GetStatus` and prints one line per field: <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

| Line | Content |
|---|---|
| `Daemon Mode` | `Idle`, `Playing`, `Cooldown` or `Sync`. |
| `ESP32 Device` | `Connected` or `Disconnected`. |
| `Counters Source` | `Device` or `Pc`: where the shown counters came from. |
| `Device ID`, `Board Profile`, `Firmware Version` | From the pad's `HelloAck`. The device ID is `OPAD-` followed by 12 hex digits of the chip's MAC address. |
| `Running Slot` | `ota_0`, `ota_1`, or `unknown (firmware predates the OTA layout)`. |
| `Input Module` | `Hall Effect`, only when the pad runs a Hall Effect input backend. |
| `HE Calibrated` | Per-key calibration state (`K1`, `K2`, and `K3` on three-key pads), Hall Effect pads only. |
| `Key 1 (K1)`, `Key 2 (K2)`, `Key 3 (K3)` | Key character and lifetime press total. `Key 3` only on three-key pads. |
| `Game Keys` | The game-key selection, three-key pads only. |
| `Total Presses` | Sum of the game keys' lifetime totals. |
| `Generation` | Counter generation. |
| `Last Sync`, `Last Backup` | Timestamps, or `Never`. |
| `Last Sync Error`, `Database Error` | Only when set. |
| `Ownership` | Only when the pad belongs to another OPad install: counter sync is paused until you decide in the app. |
| `Replacement` | Only when a new pad replaced a known one. |
| `Incompatible` | Only when the pad speaks a protocol version this app does not. |
| `tosu (osu!)` | `Active` or `Offline`. |
| `Key Latency` | `p50`, `p99.9` and `max` in µs, with the sample count. Only when the pad reports latency. |
| `Key Pins` | `K1 GPIO<n>, K2 GPIO<n>`. |
| `Debounce Lockout` | In µs. |
| `Swipes` | The action of each swipe direction (up, down, left, right). |
| `Brightness`, `Sleep Timeout` | Percent and seconds. |
<!-- src: /desktop/cli/src/main.rs#Commands::Status, /desktop/crates/opad-model/src/lib.rs#RuntimeMode, /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

### opadctl sync

```text
Trigger manual counter reconciliation & time synchronization

Usage: opadctl sync
```

Sends `ForceSync`. On success it prints the synchronized `K1` and `K2` lifetime counts; a refusal prints `✗ Operation rejected: <reason>`. <!-- src: /desktop/cli/src/main.rs#Commands::Sync -->

### opadctl stats

```text
Show your tap rate in KPM (keys per minute, not the map's BPM): the current or last attempt and the history

Usage: opadctl stats [OPTIONS]
```

| Option | Help |
|---|---|
| `--period <DAYS>` | Set the history period first, in days (0 = all time). It is kept for the app and the pad too. |
<!-- src: /desktop/cli/src/main.rs#Commands::Stats, /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

Sends `GetTapStats`, or `SetTapHistoryPeriod` when `--period` is given (`0` to `3650` days), and prints: <!-- src: /desktop/cli/src/main.rs#Commands::Stats, /desktop/crates/opad-model/src/tap_rate.rs#MAX_HISTORY_DAYS -->

- **Attempt** (the live one, else the last): map, `AVG` (every press over the song time, breaks included), `PEAK` (fastest 7 presses in a row) with the best 10 s, and a table of `Current`, `Tap avg`, `Peak` and `Presses` for `K1`, `K2` and `Combined`.
- **History** over the period: attempts, `AVG`, `PEAK` and best 10 s, and `Tap avg`, `Avg peak`, `Best peak` and `Presses` per channel.
<!-- src: /desktop/cli/src/main.rs#format_tap_stats, /desktop/crates/opad-model/src/tap_rate.rs#WINDOW_INTERVALS -->

### opadctl export

```text
Export device configuration and lifetime counters to a JSON file

Usage: opadctl export <FILE>
```

`<FILE>`: *Path to write backup JSON file*

Sends `ExportBackup` and writes the returned `JsonBackup` to `<FILE>` as pretty-printed JSON. <!-- src: /desktop/cli/src/main.rs#Commands::Export -->

### opadctl import

```text
Import configuration and counters from a validated JSON backup

Usage: opadctl import [OPTIONS] <FILE>
```

| Argument or option | Help |
|---|---|
| `<FILE>` | Path to JSON backup file |
| `--yes` | Skip preview and confirmation prompt |

1. Parses and validates `<FILE>` locally.
2. Sends `PreviewImport` and prints current versus incoming device ID, keys and press counts, plus warnings for a different device ID or a counter rollback.
3. Asks `Proceed with import? [y/N]` unless `--yes` is given. Without a terminal and without `--yes`, it refuses.
4. Sends `ImportBackup { confirm: true }` and prints the resulting counter generation.
<!-- src: /desktop/cli/src/main.rs#Commands::Import, /desktop/cli/src/main.rs#confirm -->

### opadctl reset

```text
Reset lifetime key counters (requires deliberate confirmation)

Usage: opadctl reset [OPTIONS]
```

| Option | Help |
|---|---|
| `--yes` | Confirm destructive reset of lifetime key counters |

Without `--yes` it exits with an error and sends nothing. With `--yes` it sends `ResetCounters { confirm: true }` and prints the new counter generation. <!-- src: /desktop/cli/src/main.rs#Commands::Reset -->

### opadctl firmware-update

```text
Update the pad's firmware from the signed release manifest (§U-3b)

Usage: opadctl firmware-update [OPTIONS]
```

| Option | Help |
|---|---|
| `--yes` | Consent without the interactive prompt. The firmware is still only flashed because you said so. |

1. Sends `GetFirmwareUpdate` and prints the installed version, running slot, any blockers, the available version and its notes. With nothing newer it stops; with blockers it exits with an error.
2. Prints the consent text and asks `Flash the pad now? [y/N]` unless `--yes` is given.
3. Sends `InstallFirmwareUpdate { confirm: true }`. The daemon syncs the pad's counters to this PC before it writes anything, then reports the old and new version and the running slot.
<!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate, /desktop/daemon/src/firmware_update.rs#install -->

### opadctl update

```text
Show, check for or install an update of the OPad app from the update feed, as the app's Updates page does

Usage: opadctl update [OPTIONS]
```

The options run in this order, then the status (channel, installed, available, last check, last error, last install) is printed: <!-- src: /desktop/cli/src/main.rs#Commands::Update -->

| Option | Help | What it does |
|---|---|---|
| `--channel <CHANNEL>` | Follow this channel from now on: stable, or beta for release candidates | Sends `SetUpdateChannel`. Changing the channel also starts a check of the new one. |
| `--check` | Check the feed now and wait for the answer | Sends `CheckForUpdates` and polls `GetUpdateStatus` every 500 ms until the check finishes; gives up after 120 s. |
| `--install` | Download, verify and install the available update. The installer restarts OPad, so this returns once it has started. | Sends `InstallUpdate { component: App }`. Fails with `There is no update ready to install` when none is ready. |

An install whose files belong to a package manager shows the available version as `(this install cannot update itself)`. <!-- src: /desktop/cli/src/main.rs#Commands::Update, /desktop/crates/opad-ipc/src/lib.rs#ComponentUpdate -->

### opadctl restart

```text
Restart the pad, e.g. after swapping its input module. It saves its counters and restarts once no map is being played

Usage: opadctl restart
```

Sends `RestartPad`. The pad saves its counters, config, layouts and calibration, restarts once it is idle, and reconnects. <!-- src: /desktop/cli/src/main.rs#Commands::Restart, /firmware/main/runtime/runtime.c#restart_now -->

### opadctl setup

```text
Perform initial system setup (udev permissions & directories)

Usage: opadctl setup
```

Despite the help text, it only checks and changes nothing: <!-- src: /desktop/cli/src/main.rs#run_setup -->

| Platform | Checks |
|---|---|
| Linux | Whether `/etc/udev/rules.d/70-opad.rules` or `/usr/lib/udev/rules.d/70-opad.rules` matches the rule built into `opadctl`. If missing or different, prints the commands that install it (or the AppImage's `install-udev` command). Then whether tosu can read osu!'s memory: Yama `ptrace_scope` and tosu's `cap_sys_ptrace` (see [Troubleshooting](../troubleshooting.md) §2). |
| Windows | Nothing to check: the pad uses the inbox HID and CDC drivers. |
<!-- src: /desktop/cli/src/main.rs#run_setup, /desktop/cli/src/main.rs#UDEV_RULE_PATHS -->

## Flashing and recovery

These work with or without the daemon (see [How to read this page](#how-to-read-this-page)). For the step-by-step procedures, see [Recovery](../recovery.md) §7.

### opadctl flash

```text
Flash new firmware binary onto the ESP32-S3 using espflash

Usage: opadctl flash [OPTIONS] <FIRMWARE>
```

| Argument or option | Help |
|---|---|
| `<FIRMWARE>` | Path to the app image (.bin), or an ESP-IDF build directory with --full |
| `--port <PORT>` | The pad's app port, or its ROM bootloader port when several are connected (default: auto-detect) |
| `--full` | Recovery flash: also write the bootloader, partition table and OTA data |
<!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

`<FIRMWARE>` is a file, or a directory that holds `opad-firmware.bin`. The app image's header is checked for the ESP32-S3 before anything is written. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set, /desktop/cli/src/main.rs#Commands::Flash -->

**App update (no `--full`).** `opadctl` asks the daemon which slot the pad runs from and writes the image to the other slot, followed by an `otadata` image that boots it once on trial. The running image stays as the fallback: the firmware confirms the new one only after the host configures USB, otherwise the bootloader returns to the old one. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

| Running slot | Writes |
|---|---|
| `ota_0` | App at `0x220000` (`ota_1`), then `otadata` at `0xf000` |
| `ota_1` | App at `0x20000` (`ota_0`), then `otadata` at `0xf000` |
| Unknown (no daemon, or firmware that does not report it) | App at `0x20000` in place, with no fallback |
<!-- src: /desktop/crates/opad-device/src/flash.rs#APP_PARTITION_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET, /desktop/crates/opad-device/src/flash.rs#OTADATA_OFFSET -->

**Recovery flash (`--full`).** Writes the whole set. Each file is looked for in the ESP-IDF build layout first, then flat next to the app image (the layout of an unpacked release): <!-- src: /desktop/cli/src/main.rs#resolve_flash_set -->

| Offset | File | Required |
|---|---|---|
| `0x0` | `bootloader/bootloader.bin` or `bootloader.bin` | Yes |
| `0x8000` | `partition_table/partition-table.bin` or `partition-table.bin` | Yes |
| `0xf000` | `ota_data_initial.bin` | No (written when present) |
| `0x20000` | `opad-firmware.bin` (or the file given) | Yes |

In both modes `opadctl` reboots the pad into the ROM download bootloader over USB, flashes it with `espflash`, then waits up to 15 s for it to come back as the OPad app (`303a:8415`). A pad that does not come back is reported as a failure that points at `opadctl recover`. <!-- src: /desktop/cli/src/main.rs#Commands::Flash, /desktop/daemon/src/ipc_handlers.rs#FinishFlash -->

### opadctl bootloader

```text
Reboot device into ROM download bootloader mode hands-free over USB

Usage: opadctl bootloader [OPTIONS]
```

| Option | Help |
|---|---|
| `--port <PORT>` | Explicit serial port (default: auto-detect) |

Asks the daemon to release the port, reboots the pad into the ROM download bootloader (`303a:1001`), hands the port back to the daemon and prints the bootloader's port. The pad stays in download mode for other flashing tools. <!-- src: /desktop/cli/src/main.rs#Commands::Bootloader -->

### opadctl recover

```text
Recover a pad that no longer shows up as OPad: replug it, and its next boot is caught over USB and flashed with the full set. No buttons

Usage: opadctl recover [OPTIONS] <FIRMWARE>
```

| Argument or option | Help |
|---|---|
| `<FIRMWARE>` | An ESP-IDF build directory or an unpacked release (as for flash --full) |
| `--timeout <TIMEOUT>` | How long to wait for the replug, in seconds [default: 300] |
<!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

Resolves the same file set as `flash --full`, then asks you to unplug and replug the pad and waits up to `--timeout` seconds. When the pad's ROM USB-Serial-JTAG port (`303a:1001`) appears during boot, `opadctl` holds it in download mode, writes the full set, resets it into the app with an RTC watchdog reset, and waits up to 15 s for the OPad app (`303a:8415`). See [Recovery](../recovery.md) §7.7. <!-- src: /desktop/cli/src/main.rs#Commands::Recover, /desktop/crates/opad-device/src/flash.rs#recover -->

## Diagnostics and testing

### opadctl monitor

```text
Stream live device and daemon diagnostic events

Usage: opadctl monitor [OPTIONS]
```

| Option | Help | What it does |
|---|---|---|
| `-l`, `--limit <LIMIT>` | [default: 50] | Entries per request. |
| `-f`, `--follow` | Follow log stream in real time | Polls `GetLogEntries` every 500 ms from the last entry received. |
| `--level <LEVEL>` | Only this severity and above [possible values: debug, info, warn, error] | The daemon filters before `limit` applies; `opadctl` filters again for daemons that ignore the field. |
| `--source <SOURCE>` | Only this source [possible values: host, esp, program, tosu] | Filtered the same way. |
<!-- src: /desktop/cli/src/main.rs#Commands::Monitor, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->

### opadctl latency

```text
Show key-press-to-HID latency measured on the device

Usage: opadctl latency [OPTIONS]
```

| Option | Help |
|---|---|
| `--reset` | Clear the collected samples (e.g. right before playing a map) |

Without options it prints the pad's key-edge-to-HID-submit statistics from `GetStatus`: sample count, `p50`, `p99`, `p99.9`, `max` (µs), and deferred reports (key changes that waited for the next USB poll). With `--reset` it sends `ResetLatencyStats`. <!-- src: /desktop/cli/src/main.rs#Commands::Latency, /protocol/opad.proto#DeviceStatus -->

### opadctl board-test

```text
Test the carrier and input module PCBs: module ID, cable direction, key lines, connector shorts, and a Hall Effect V3 module's ADC and sensors. Release both keys while it runs

Usage: opadctl board-test [OPTIONS]
```

Sends `RunBoardTest`, prints each check, and exits with an error when any check fails. <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->

| Option | Help | What it does |
|---|---|---|
| `--fix-pins` | If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and Key 2 = GPIO7, then test again | When a check says the key pins are wrong, sends `UpdateConfig` with those pins, waits 300 ms and runs the test again. Without it, the failure suggests `opadctl board-test --fix-pins`. |
| `--magnet` | Hall Effect V3: then watch the sensors live while you hold a magnet over each key in turn (up to 60 s, Ctrl-C to stop) | Runs after the board test. |
<!-- src: /desktop/cli/src/main.rs#Commands::BoardTest, /desktop/cli/src/main.rs#magnet_test -->

### opadctl animation

```text
Play a result animation on the pad, as osu!'s results screen would

Usage: opadctl animation <NAME> [GRADE]
```

| Argument | Help |
|---|---|
| `<NAME>` | pop, drop or burst (or an animation id); none stops the one playing |
| `[GRADE]` | The grade it shows: SS, S, A, B, C or D [default: S] |

Both are case-insensitive. Sends `PlayAnimation`; the daemon refuses it without a pad, with firmware that has no animations, and during a map. <!-- src: /desktop/cli/src/main.rs#Commands::Animation, /desktop/crates/opad-ipc/src/lib.rs#IpcRequest -->
