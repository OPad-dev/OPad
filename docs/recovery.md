---
title: Counters, Recovery and Unbinding
description: How the pad and the PC reconcile lifetime counters, what backup, restore and reset do, and how to reflash, unbind or recover a pad over USB with no buttons.
sidebar:
  order: 2
---

This page explains how OPad keeps your lifetime counters safe, and how to reflash, hand over or recover a pad. Every step works over USB: you never need the board's BOOT or RESET buttons. The section numbers are fixed, because the app and `opadctl` point to them.

## 1. Where the counters live

The pad and the PC each keep the lifetime press counts.

- **On the pad:** presses are counted in RAM, in the key path, with no flash write per press. The pad saves them to its flash (NVS) only while it is `IDLE`: when a map's cooldown ends, on its idle checkpoint schedule, and when the PC suspends USB. A three-key V3 pad (MX V3 or HE V3) keeps Key 3 under its own NVS key (`key3_cnt`). <!-- src: /firmware/main/counters/counters.c#counters_checkpoint --> <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task --> <!-- src: /firmware/main/app_main.c#usb_event_handler -->
- **On the PC:** in the SQLite database `opad.db` (table `device_state`, with Key 3 in `lifetime_key3`). The daemon writes nothing to it during `PLAYING` and `COOLDOWN`; it keeps live counts in memory and writes them during `SYNC` and `IDLE`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 2. Counter generations and reconciliation

Each counter record has:

- `device_id`: `OPAD-` plus the board's Wi-Fi MAC address. Older firmware used `OSUPAD-<MAC>`; the app reads that as the same pad and migrates its records. <!-- src: /firmware/main/protocol/protocol.c#get_device_mac_string --> <!-- src: /desktop/crates/opad-model/src/lib.rs#canonical_device_id --> <!-- src: /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->
- `counter_generation`: a 32-bit number that starts at `1` on a fresh pad and goes up whenever counters are reset, restored from a backup, moved from a replaced pad, reconciled by hand, or taken over from another install. <!-- src: /firmware/main/counters/counters.c#counters_init --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- `lifetime_key1`, `lifetime_key2`, and on a three-key V3 pad `lifetime_key3`, which is tracked apart from Keys 1 and 2. <!-- src: /desktop/crates/opad-model/src/lib.rs#CounterState --> <!-- src: /desktop/crates/opad-model/src/lib.rs#Key3Counters -->

### Rules

1. **The higher generation wins.** An older database or backup can never overwrite a reset or an import. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Within one generation, counts only go up.** Each key keeps the larger of the two counts (`max(pc, pad)`). Unless the PC forces a restore, the pad rejects a sync with a lower generation (`stale generation`) or with a lower count in the same generation (`non-monotonic`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Power loss during a map loses the presses since the last save.** If the pad loses power during a map or its cooldown, presses since its last `IDLE` save are gone. That is the price of keeping flash writes off the key path. <!-- src: /firmware/main/counters/counters.c#checkpoint_locked --> <!-- src: /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## 3. What happens when the pad connects

When the daemon connects to the pad, it first checks that the pad speaks the same protocol version (`DEVICE_PROTOCOL_VERSION`) and who owns the pad (`owner_id`). Then, while `IDLE` and allowed to touch the pad, it reconciles the counters: <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync -->

| Situation | What the daemon does |
|---|---|
| **The pad belongs to another OPad install** | Nothing is synced until ownership is settled. If the pad has at least as many presses as this PC on both keys, the daemon takes it over silently and keeps the pad's counts. Otherwise sync pauses and the app asks you (see §7). <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead --> |
| **Known pad, same generation** | Keeps `max(pc, pad)` for Key 1 and Key 2 and writes the result to both. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync --> |
| **Known pad, the pad's generation is higher** | Updates the PC's database to the pad's generation and counts. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> |
| **Known pad, the PC's generation is higher** | Sends its counts to the pad (`CounterSync`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync --> |
| **Known pad whose flash was erased** (generation ≤ 1, both counts 0, while the PC has presses) | Keeps the PC's saved settings instead of the pad's defaults, sends them to the pad, and restores the counts, Key 3 included when stored. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/sync.rs#key3_sync_target --> |
| **Unknown pad, while the PC already has a pad on record**, and the new pad has generation ≤ 1, under 1000 presses on each key, and fewer presses than the PC | Starts the replacement flow (§4). Sync pauses until you answer. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/runtime.rs#pc_is_ahead --> |
| **Any other new pad** (including your first) | Registers it with the generation and counts it reports. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |

## 4. Replacing a pad

When you plug a new OPad into a PC that has counter history for an earlier pad, and the new pad matches the replacement case in §3, the daemon holds sync and asks. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> <!-- src: /desktop/daemon/src/runtime.rs#pad_guard -->

The app shows: <!-- src: /desktop/gui/src/main.rs#App -->

> This looks like a new pad. Restore counters from `<old_id>`?

with **Restore from previous pad** and **Treat as new pad**. `opadctl status` shows a `Replacement:` line while the question is open. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

- **Restore from previous pad:** the daemon sets the generation to one above the higher of the two pads' generations, copies the old pad's Key 1, Key 2 (and Key 3, if recorded) counts to the new pad's record, and writes them onto the new pad with a forced `CounterSync`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- **Treat as new pad:** the new pad is registered on its own with its current counts. The old pad's record stays in the database as history. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## 5. Backup, restore and reset

The daemon refuses all of these during `PLAYING` and `COOLDOWN`, when its database is unavailable, and while an ownership or replacement question is open. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

- **Reset counters:** run `opadctl reset --yes`, or in the app open **Device** → **Reset lifetime counters**, type `RESET` and click **Confirm Reset**. Key 1, Key 2 and Key 3 go to `0`, the generation goes up by one, and the pad gets a forced `CounterSync`. With the pad unplugged, the reset is queued and sent at the next sync. <!-- src: /desktop/cli/src/main.rs#Commands::Reset --> <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/daemon/src/sync.rs#perform_sync -->
- **Export a backup:** run `opadctl export <file>`, or open **Device** → **Export backup**. The JSON file holds `format_version` (`1`), `exported_at`, the pad's `device_id`, `board_profile` and `counter_generation`, the lifetime counts, and the pad's settings (keys, debounce, brightness, sleep, refresh rate, key pins, swipes, result animations, and on a three-key pad `key3` and `game_keys`). <!-- src: /desktop/cli/src/main.rs#Commands::Export --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- **Import a backup:** run `opadctl import <file>` (add `--yes` to skip the question), or open **Device** → **Import backup**. It happens in two steps: <!-- src: /desktop/cli/src/main.rs#Commands::Import -->
  1. **Preview** (`PreviewImport`): the daemon checks the file (`format_version` 1, `board_profile` `waveshare_esp32s3_touch_lcd_2`, key codes, pins, value ranges, and an `exported_at` no more than a day in the future) and shows current against incoming values, with warnings for a different `device_id`, counters going down, or a generation change. Nothing is changed yet. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
  2. **Apply** (`ImportBackup`): only after you confirm. If any count would go down, the app also asks you to tick **I understand this replaces my counters with lower values** before **Apply Backup**. Your local tosu address is kept, the generation becomes one above the higher of the current and backup generations, and the settings and counts are written to the database and the pad. <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 5.1 Automatic backups

The daemon also writes a backup by itself after every play session, in the same format: <!-- src: /desktop/daemon/src/backup.rs#write -->

| | |
|---|---|
| **Where** | `opad-backup-YYYYMMDDTHHMMSSZ.json` in `~/.local/share/opad/backups` (or `$XDG_DATA_HOME/opad/backups`) on Linux, `%APPDATA%\opad\backups` on Windows. <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| **When** | 20 seconds after the pad is back in `IDLE` after a map (cooldown, then sync). Starting another map in those 20 seconds cancels it; the next session arms a new one. Idle syncs never write one. <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |
| **How many** | The 10 newest `opad-backup-*.json` files. Older ones are deleted; files with other names in the folder are never touched. <!-- src: /desktop/daemon/src/backup.rs#KEEP --> <!-- src: /desktop/daemon/src/backup.rs#prune --> |
| **Safety** | Written to a temporary file, flushed and renamed into place. A failed write is logged and never interrupts syncing or play. <!-- src: /desktop/daemon/src/backup.rs#write_and_log --> |
| **Last one** | `Last Backup:` in `opadctl status`. <!-- src: /desktop/cli/src/main.rs#Commands::Status --> |

A backup is written even if the sync after the map failed, so it holds the latest counts the daemon has. It is a copy of what the daemon holds, not a replacement for a working sync. Restore one like any other backup. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 6. What waits until a map ends

During `PLAYING` and `COOLDOWN`:

- Nothing is written to the PC's database or to the pad's flash. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage --> <!-- src: /firmware/main/counters/counters.c#checkpoint_locked --> <!-- src: /firmware/main/config/device_config.c#write_to_nvs -->
- Key, key pin and debounce changes are queued and answered with "deferred" until `IDLE`. Brightness, sleep timeout and refresh rate apply at once and are saved later. Layout changes and resets show on the pad at once and are saved later. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- Manual sync, counter reset, backup import, ownership answers and firmware flashing are refused. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

When a map ends (or tosu disconnects during one), a 5-second `COOLDOWN` starts on both the PC and the pad. Then the daemon goes through `SYNC` to `IDLE`, syncs the counters and applies what was queued, and the pad saves its counters, settings, layouts and Hall Effect calibration. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION --> <!-- src: /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

## 7. Reflashing and unbinding a pad

A pad belongs to the OPad install that claimed it (`owner_id`). Erasing the pad's flash and reflashing it is the only way to clear that: there is no unpair button, and the firmware rejects an all-zero claim (`OWNER_CLAIM_REJECT_INVALID`). <!-- src: /firmware/main/config/owner.c --> <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner -->

If you came here from the "belongs to another OPad install" question, you do not need to reflash to keep using the pad: **Leave it alone** keeps it working as a keyboard without touching its owner or counters. Use this section to hand a pad to someone else, or to recover one that will not boot. <!-- src: /desktop/gui/src/main.rs#App -->

### 7.1 What erasing costs

The owner record lives in the pad's NVS next to its lifetime counters, settings, layouts and Hall Effect calibration. Erasing the flash clears all of them. <!-- src: /firmware/partitions.csv -->

> [!CAUTION]
> `espflash erase-flash` erases the pad's lifetime counters, settings, layouts, calibration and owner record.

It erases the whole 16 MB flash, bootloader and partition table included, so the pad is not a keyboard again until you flash it in §7.4. <!-- src: /firmware/sdkconfig.defaults -->

You cannot brick the pad this way. The ESP32-S3's first bootloader is in ROM, so a pad with blank flash still shows up over USB as `303a:1001` and accepts a flash. <!-- src: /desktop/crates/opad-device/src/lib.rs#ESP_ROM_BOOTLOADER_PID -->

### 7.2 Export first

Export a backup before you erase anything. It is the only copy that survives a reinstall of the desktop app: <!-- src: /desktop/cli/src/main.rs#Commands::Export -->

```sh
opadctl export ~/opad-backup.json
```

or **Device** → **Export backup** in the app. You can also use an automatic backup (§5.1): <!-- src: /desktop/daemon/src/backup.rs#backup_dir -->

```sh
opadctl status | grep 'Last Backup'
ls -t ~/.local/share/opad/backups/ | head   # Windows: %APPDATA%\opad\backups
```

After reflashing, restore it with `opadctl import ~/opad-backup.json` or **Device** → **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands::Import -->

An erased pad has no owner, and the first OPad install it connects to claims it silently. If that is the PC you erased it from, the daemon also sees the erased flash and puts back its saved counts and settings (§3). If you are giving the pad away, plug it into the new owner's PC first, so their install claims it. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

### 7.3 Put the pad in download mode

```sh
opadctl bootloader
```

`opadctl bootloader` asks the daemon to release the serial port, reboots the pad into the ESP32-S3's ROM download mode over USB, hands the daemon back control, and prints the bootloader's port (`303a:1001`). It works with or without the daemon running. <!-- src: /desktop/cli/src/main.rs#Commands::Bootloader --> <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->

On Windows a serial port can only be open in one program at a time, so close any serial monitor first. To list ports, run `espflash list-ports`, or look in Device Manager under **Ports (COM & LPT)**. <!-- src: /desktop/crates/opad-device/src/flash.rs#FlashError -->

`opadctl` reaches download mode with no buttons: it first sends the pad a `BOOTLOADER` command over its serial port, then tries the 1200-baud "touch" that Arduino-style boards use. The firmware then detaches from USB, holds the bus in reset for about 300 ms so the PC sees a real unplug, and restarts into download mode. If neither works, use `opadctl recover` (§7.7). <!-- src: /desktop/crates/opad-device/src/flash.rs#BOOT_TRIGGERS --> <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->

### 7.4 Erase, then flash

You need the firmware's four files in one folder: an ESP-IDF `firmware/build` folder, or a folder with the four `.bin` files from a firmware release (`opad-firmware.bin`, `bootloader.bin`, `partition-table.bin`, `ota_data_initial.bin`). <!-- src: /desktop/cli/src/main.rs#resolve_flash_set --> <!-- src: /.github/workflows/release-firmware.yml -->

`espflash` comes with the app: `/usr/lib/opad/bin/espflash` from a Linux package, `espflash.exe` in `%LOCALAPPDATA%\Programs\opad` on Windows. Use that path if `espflash` is not on your `PATH`. <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/windows/installer.iss -->

With the pad in download mode (§7.3):

**Linux**

```sh
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
opadctl flash --full ~/Downloads/opad-firmware/   # or firmware/build
```

**Windows (PowerShell or Command Prompt)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
opadctl flash --full C:\Users\you\Downloads\opad-firmware\
```

Use the port that `opadctl bootloader` printed. `--before no-reset --after no-reset-no-stub` keeps the pad in download mode for the next command. Do not unplug the pad while `erase-flash` runs. <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images -->

`--full` is needed after an erase because the bootloader and partition table are gone. It writes the bootloader at `0x0`, the partition table at `0x8000`, `ota_data_initial.bin` at `0xf000` and `opad-firmware.bin` at `0x20000` (`ota_0`), then restarts the pad into the new firmware. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set --> <!-- src: /desktop/crates/opad-device/src/flash.rs#reset_to_app -->

### 7.5 Confirm

```sh
opadctl status
```

A pad that booted cleanly shows `ESP32 Device:     Connected` and `Running Slot:     ota_0`. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

On a PC that has never seen this pad, its counts start at 0 with generation 1. On the PC you erased it from, the daemon puts back the stored counts and claims it again (§7.2). <!-- src: /firmware/main/counters/counters.c#counters_init --> <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

If the pad does not come back as OPad within about 15 seconds of a `--full` flash, `opadctl flash` reports an error. Run the `opadctl flash --full` step again; do not erase again. If the pad has left download mode and does not show up as OPad (`303a:8415`), use `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

### 7.6 When flashing is interrupted

A normal update (from the app, `opadctl firmware-update`, or `opadctl flash <app.bin>` while the daemon knows the running slot) writes the new firmware to the *other* OTA slot first (`ota_1` when running `ota_0`, and the other way round). Only then does it update `otadata`, so the bootloader tries the new slot once and keeps the old one as a fallback. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

- **Interrupted before `otadata` is written:** nothing points at the new slot yet, so the pad boots its old firmware unchanged. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
- **Written, but the new firmware does not work:** the new firmware starts on trial. It confirms itself only once the PC has set up its USB connection. If a PC is connected and has not done that within 20 seconds, the pad restarts and the bootloader goes back to the previous slot. A firmware that crashes before confirming goes back the same way. On a charger with no PC, the trial simply waits until the pad is next plugged into a computer. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works --> <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->

  After such a rollback, `opadctl flash` reports: "The pad did not accept the new firmware (it never came up as a working USB device) and went back to the previous one on `<slot>`. It works as before; nothing else is needed." <!-- src: /desktop/cli/src/main.rs#report_slot_after_flash -->

Two kinds of flash have no fallback, because they write `ota_0` in place: `opadctl flash <app.bin>` when the running slot is unknown (no daemon, or firmware too old to report it), and a `--full` flash (§7.4). If one of those is interrupted, recover the pad with `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands::Flash --> <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

### 7.7 A pad that does not show up as OPad (`opadctl recover`)

If the firmware boots but never works over USB (on Linux, `dmesg` may show `can't set config #1, error -32`), nothing can ask it to enter download mode. `opadctl recover` catches the pad at power-on instead: <!-- src: /desktop/crates/opad-device/src/flash.rs#recover --> <!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

1. Run it with the firmware folder (as in §7.4):

   ```sh
   opadctl recover firmware/build   # or a folder with a firmware release's .bin files
   ```

2. When it prints `Unplug the pad's USB cable and plug it back in (waiting 300 s)...`, unplug the pad and plug it back in. (`--timeout <seconds>` changes the wait.)
3. Wait for `✓ Recovered: the pad came back as OPad on <port>`.

At every power-on, before the firmware takes over USB, the ESP32-S3's built-in USB bootloader appears for a fraction of a second as `303a:1001`. `opadctl recover` watches for it, holds the chip in download mode in that moment, writes the full firmware set, restarts the pad and waits up to 15 seconds for OPad (`303a:8415`) to appear. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover --> <!-- src: /desktop/crates/opad-device/src/flash.rs#usj_reset_to_download --> <!-- src: /desktop/crates/opad-device/src/flash.rs#disable_watchdogs -->

Only a real power cycle brings that bootloader window back; a software restart does not. That is why you replug the cable. No BOOT or RESET button is ever needed. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->
