---
title: Counter Reconciliation, Recovery & Unbinding
description: Counter reconciliation rules, flashing procedures, bootloader entry, and recovering an un-enumerated pad.
sidebar:
  order: 2
---

## 1. Dual-Storage Model

Both the ESP32-S3 and the host PC maintain lifetime press counters for the physical keys:

- **Device (ESP32-S3)**: Counters increment in RAM in the key-event path on Core 0, with zero flash writes during keypresses. Counters are checkpointed to NVS only during `IDLE` state — when leaving `COOLDOWN`, on the idle checkpoint schedule (`counters_checkpoint_due`), or on USB host suspend (`counters_request_checkpoint`). On three-key V3 pads (MX V3 and Hall Effect V3), Key 3 keeps its own NVS key (`key3_cnt`). <!-- src: /firmware/main/counters/counters.c#counters_checkpoint, /firmware/main/runtime/runtime.c#runtime_supervisor_task, /firmware/main/app_main.c#usb_event_handler -->
- **Host PC**: Stored in SQLite (`device_state` table in `opad.db`). All SQLite writes are blocked during `PLAYING` and `COOLDOWN` (`set_writes_allowed(false)`). The daemon keeps live counts in memory and writes to SQLite during `SYNC` and `IDLE`. Key 3's lifetime count is stored in `device_state.lifetime_key3`. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage, /desktop/daemon/src/runtime.rs#RuntimeController -->

## 2. Counter Generation & Monotonic Reconciliation

Each counter record tracks:

- `device_id`: Derived from the ESP32-S3 Wi-Fi STA MAC address (`OPAD-<MAC>`). Older firmware used `OSUPAD-<MAC>`, which the host canonicalizes to `OPAD-<MAC>` and migrates in SQLite. <!-- src: /firmware/main/protocol/protocol.c#get_device_mac_string, /desktop/crates/opad-model/src/lib.rs#canonical_device_id, /desktop/crates/opad-storage/src/lib.rs#apply_v19 -->
- `counter_generation`: Monotonically incrementing 32-bit integer (starts at `1` on a fresh NVS). Incremented whenever counters are reset, restored from backup, migrated from a replaced pad, manually reconciled, or taken over from another install. <!-- src: /firmware/main/counters/counters.c#counters_init, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- `lifetime_key1` and `lifetime_key2`: Lifetime press totals for Key 1 and Key 2. On three-key V3 pads (MX V3 and Hall Effect V3), `lifetime_key3` is tracked separately from Key 1 and Key 2 reconciliation. <!-- src: /desktop/crates/opad-model/src/lib.rs#CounterState, /desktop/crates/opad-model/src/lib.rs#Key3Counters -->

### Reconciliation Rules

1. **Generational Precedence**: If `generation_A > generation_B`, the higher generation wins unconditionally. This stops an older database or backup from overwriting a reset or import. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Monotonic Merging (Same Generation)**: When generations match, the maximum count for each key is kept (`max(pc.k1, esp.k1)` and `max(pc.k2, esp.k2)`). Without `force_restore`, the firmware rejects any sync with a lower generation (`stale generation`) or a lower count in the same generation (`non-monotonic`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Power-Loss Policy**: If USB power is disconnected during an active map or cooldown before an `IDLE` checkpoint occurs, uncommitted presses since the last NVS checkpoint are lost. This trade-off keeps flash writes off the keypress path. <!-- src: /firmware/main/counters/counters.c#checkpoint_locked, /firmware/main/runtime/runtime.c#runtime_supervisor_task -->

## 3. Connect-Time Reconciliation Matrix

When the daemon connects over USB CDC and receives `HelloAck`, it first checks protocol compatibility (`protocol_version == DEVICE_PROTOCOL_VERSION`) and pad ownership (`owner_id`), then reconciles counters when `IDLE` and allowed to touch the pad: <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/sync.rs#perform_sync -->

| Condition | Action Taken |
|---|---|
| **Pad belongs to another install (`owner_id` differs)** | Ownership takeover is triggered before any sync. If the pad has at least as many presses as this PC on both keys, the daemon takes over silently keeping the pad's counters (`TakeOverIfPadIsAhead`). Otherwise counter sync is paused and the GUI shows the takeover prompt. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead --> |
| **Known pad, same generation** | Daemon merges `max(pc, esp)` for Key 1 and Key 2 and synchronizes both the pad and SQLite. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Known pad, device generation > host generation** | Host updates SQLite to match the pad's generation and counts. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Known pad, host generation > device generation** | Host sends `CounterSync` to restore its recorded counts to the pad. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters, /desktop/daemon/src/sync.rs#perform_sync --> |
| **Known pad with erased NVS (`gen <= 1`, `k1 == 0`, `k2 == 0`, while PC has presses)** | Daemon marks `nvs_restore_pending`, keeps the PC's saved configuration instead of adopting the pad's defaults, pushes the PC config, and restores counters (including Key 3 if stored) on sync. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/sync.rs#key3_sync_target --> |
| **New pad (`device_id` unknown), existing device in SQLite, `gen <= 1`, `k1 < 1000`, `k2 < 1000`, and PC is ahead** | Triggers the **Device Replacement Flow** (`pending_replacement`). Sync is paused until you answer. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/runtime.rs#pc_is_ahead --> |
| **New pad (first pad ever, or not meeting replacement criteria)** | Registered in SQLite with its reported generation and counts. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController --> |

## 4. Device Replacement Flow

When you plug in a replacement OPad on a PC that already has counter history for a previous pad:

1. If the connecting `device_id` is unknown, the database already has a previous pad, the new pad reports `counter_generation <= 1` with `lifetime_key1 < 1000` and `lifetime_key2 < 1000`, and the PC holds more presses than the new pad on at least one key (`pc_is_ahead`), the daemon sets `pending_replacement` to a known previous `device_id` and blocks sync until resolved. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController, /desktop/daemon/src/runtime.rs#pad_guard -->
2. The GUI displays a banner:
   > `This looks like a new pad. Restore counters from <old_id>?`
   with two buttons: **Restore from previous pad** and **Treat as new pad**. `opadctl status` also prints a `Replacement:` warning line. <!-- src: /desktop/gui/src/main.rs#App, /desktop/cli/src/main.rs#Commands -->
3. **If you click Restore from previous pad** (`IpcRequest::ResolveReplacement { restore: true }`):
   - The daemon sets `counter_generation` to `max(old_generation, current_generation) + 1`.
   - Copies the previous pad's lifetime Key 1, Key 2 (and Key 3 if recorded) counts to the new `device_id` in SQLite.
   - Sends `CounterSync` with `force_restore = true` to write the inherited counts onto the new pad. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
4. **If you click Treat as new pad** (`IpcRequest::ResolveReplacement { restore: false }`):
   - Registers the new pad as a separate device in SQLite with its own current counts.
   - The previous pad's row stays in SQLite as historical data. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## 5. Backup, Restore, and Reset Actions

All write operations below are refused during `PLAYING` and `COOLDOWN`, when SQLite storage is unavailable, or while an ownership takeover or replacement prompt is unresolved. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

- **Reset Counters**:
  - Run `opadctl reset --yes`, or in the GUI open **Device** -> **Reset lifetime counters**, type `RESET`, and click **Confirm Reset**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device, /desktop/gui/src/main.rs#App -->
  - Resets Key 1, Key 2, and Key 3 lifetime counts to `0`, increments `counter_generation` by 1, saves to SQLite, and sends a forced `CounterSync` to the pad (or queues a `PendingCounterReset` if the pad is disconnected so the reset is pushed on the next sync). <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request, /desktop/daemon/src/sync.rs#perform_sync -->
- **Export Backup**:
  - Run `opadctl export <file>`, or in the GUI open **Device** -> **Export backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->
  - Writes a portable JSON file (`JsonBackup`) containing:
    - `format_version`: `1` and `exported_at` (UTC timestamp).
    - `device`: `device_id`, `board_profile`, `counter_generation`.
    - `stats`: `lifetime_key1`, `lifetime_key2`, and `lifetime_key3` (on three-key pads when known).
    - `config`: `key1`, `key2`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, swipe actions/keys/modifiers for all four directions, `result_animations`, plus `key3` and `game_keys` on three-key pads. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- **Import Backup (`PreviewImport` & `ImportBackup`)**:
  - Run `opadctl import <file>` (pass `--yes` to skip the terminal prompt), or in the GUI open **Device** -> **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->
  - Two-stage safety flow:
    1. `PreviewImport`: Validates `format_version == 1`, `board_profile == "waveshare_esp32s3_touch_lcd_2"`, key usages, pin assignments, numeric ranges, and timestamp (rejecting timestamps more than 1 day in the future). Returns a diff between current and incoming state plus warnings for device ID mismatch, counter rollback, or generation bump. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
    2. `ImportBackup`: Requires explicit confirmation (`confirm: true`). In the GUI, if any incoming counter is lower than the current value, you must also tick `I understand this replaces my counters with lower values` before clicking **Apply Backup**. Preserves your local `tosu_endpoint`, bumps `counter_generation` to `max(current_gen, backup_gen) + 1`, saves to SQLite, and pushes both config and forced `CounterSync` to the pad. <!-- src: /desktop/gui/src/main.rs#App, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 5.1 Automatic backups

In addition to manual exports, `opad-daemon` writes an automatic JSON backup in the exact same `JsonBackup` format after every play session: <!-- src: /desktop/daemon/src/backup.rs#current, /desktop/daemon/src/backup.rs#write -->

| Field | Behavior |
|---|---|
| Where | `<data>/backups/opad-backup-YYYYMMDDTHHMMSSZ.json` — `$XDG_DATA_HOME/opad/backups` (default `~/.local/share/opad/backups`) on Linux, `%APPDATA%\opad\backups` on Windows. <!-- src: /desktop/daemon/src/backup.rs#backup_dir, /desktop/crates/opad-model/src/paths.rs#data_dir --> |
| When | **20 seconds** (`AUTO_BACKUP_DELAY`) after a post-play `COOLDOWN` -> `SYNC` -> `IDLE` cycle settles. Periodic idle syncs do not trigger a backup. <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY --> |
| Retention | Keeps the **10 most recent** (`KEEP = 10`) `opad-backup-*.json` files and prunes older ones. Files with other names in that folder are never deleted. <!-- src: /desktop/daemon/src/backup.rs#KEEP, /desktop/daemon/src/backup.rs#prune --> |
| Write safety | Written atomically via temporary file + `fsync` + rename (`write_atomic`). Any I/O error is logged and swallowed so backup failures never disrupt syncing or gameplay. <!-- src: /desktop/daemon/src/backup.rs#write, /desktop/daemon/src/backup.rs#write_and_log --> |
| Restore | `opadctl import <file>` or GUI **Device** -> **Import backup**. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device --> |
| Last backup time | Shown in `opadctl status` under `Last Backup`, and logged by the daemon whenever written. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/backup.rs#write_and_log --> |

The 20-second delay starts only once `IDLE` is reached, and the write re-checks `mode == RuntimeMode::Idle` before running. Starting another map during those 20 seconds cancels the pending backup rather than deferring it; the next session arms a fresh one. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

Automatic backups are armed even if the post-play sync fails, so the daemon preserves the latest counts it has in memory. They are a copy of what the daemon holds, not a substitute for a working sync. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

## 6. Deferred Operations During Gameplay

During `PLAYING` and `COOLDOWN` states:

- All writes to SQLite (`Storage::check_writes_allowed`) and ESP32-S3 NVS (`device_config.c`, `counters.c`) are blocked. <!-- src: /desktop/crates/opad-storage/src/lib.rs#Storage, /firmware/main/counters/counters.c#checkpoint_locked, /firmware/main/config/device_config.c#write_to_nvs -->
- Key mapping, key pin, and debounce config changes are queued in `PendingOperations::pending_config` and return `OperationDeferred` until `IDLE`. Brightness, sleep timeout, and `gameplay_display_hz` apply to RAM immediately while queuing persistence for `SYNC`. Layout edits and resets apply to pad RAM and queue SQLite/NVS persistence in `pending_layouts`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- Manual sync, counter reset, backup import, ownership resolution, and firmware flashing are refused during `PLAYING` and `COOLDOWN`. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- When gameplay stops (or tosu disconnects during play), `COOLDOWN` runs for **5 seconds** (`COOLDOWN_DURATION` on host, `COOLDOWN_DURATION_US` on firmware). Once cooldown expires, the daemon transitions to `SYNC` -> `IDLE`, re-enables storage writes, synchronizes counters, and drains pending operations. On firmware, the runtime supervisor flushes dirty counters, config, layouts, and Hall Effect calibration to NVS on `COOLDOWN` -> `IDLE`. <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION, /desktop/daemon/src/sync.rs#perform_sync, /firmware/main/runtime/runtime.c#COOLDOWN_DURATION_US -->

## 7. Reflashing & Unbinding a Pad

Erasing and reflashing the pad is the only way to clear its ownership record (`owner_id`). There is no unpair button in the app, and firmware rejects any all-zero `ClaimOwnership` message (`OWNER_CLAIM_REJECT_INVALID`). <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner -->

If you reached this section from the takeover prompt, you do not need to reflash just to keep using the pad: clicking **Leave it alone** keeps the pad working as a USB HID keyboard without modifying its owner or counters. Use this section when handing the pad over to someone else or when recovering a pad that will not boot. <!-- src: /desktop/gui/src/main.rs#App, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 7.1 What it costs

The 16-byte owner record (`owner_id`) lives inside the pad's NVS partition alongside the lifetime counters, device configuration, custom layouts, and Hall Effect calibration. Erasing flash clears all of them together. <!-- src: /firmware/main/config/device_config.c#device_config_claim_owner, /firmware/partitions.csv -->

> **`espflash erase-flash` erases the pad's lifetime counters, configuration, layouts, calibration, and ownership record.**

It erases the entire 16 MB SPI flash (`CONFIG_ESPTOOLPY_FLASHSIZE_16MB`), including the second-stage bootloader and partition table, so the pad stops acting as a keyboard until §7.4 flashes firmware back. <!-- src: /firmware/sdkconfig.defaults -->

The pad cannot be permanently bricked by erasing flash: the ESP32-S3 first-stage ROM bootloader resides in mask ROM, so a pad with blank flash still enumerates over USB as `303a:1001` and accepts a flash. <!-- src: /desktop/crates/opad-device/src/lib.rs#ESP_ROM_BOOTLOADER_PID, /desktop/crates/opad-device/src/flash.rs -->

### 7.2 Export first

Export a manual JSON backup before erasing anything, because it is the only copy that survives reinstalling the desktop app: <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/gui/src/pages.rs#device -->

```sh
opadctl export ~/opad-backup.json
```

Or in the GUI: **Device** -> **Export backup**. You can also check existing automatic backups from §5.1: <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/backup.rs#backup_dir -->

```sh
opadctl status | grep 'Last Backup'
ls -t ~/.local/share/opad/backups/ | head   # Windows: %APPDATA%\opad\backups
```

After reflashing, restore a backup with `opadctl import ~/opad-backup.json` (or GUI **Device** -> **Import backup**). <!-- src: /desktop/cli/src/main.rs#Commands -->

Note on ownership after erasing: an erased pad has an all-zero `owner_id`, and the first OPad install it connects to claims it silently (`Ownership::Unclaimed`). If that is the PC you just erased it from, the daemon also detects `nvs_wiped` and restores its saved counters and config (§3). If you are giving the pad to someone else, plug it into the new owner's machine first so their install claims it cleanly. <!-- src: /desktop/daemon/src/runtime.rs#RuntimeController -->

### 7.3 Put the pad in download mode and free the port

```sh
opadctl bootloader
```

`opadctl bootloader` asks `opad-daemon` to release the serial port (`IpcRequest::PrepareFlash`), triggers ROM download mode over USB, resumes the daemon (`IpcRequest::ResumeDevice`), and prints the bootloader port (`303a:1001`). It also works when `opad-daemon` is not running. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->

On Windows, serial handles are exclusive (`PortBusy`), so close any serial monitor or other process holding the COM port before running `opadctl bootloader` or `espflash`. On either OS you can list ports with `espflash list-ports` (or Windows Device Manager -> Ports (COM & LPT)). <!-- src: /desktop/crates/opad-device/src/flash.rs#FlashError, /desktop/cli/src/main.rs#prepare_flash -->

To enter download mode without pressing any buttons, `enter_bootloader` tries two triggers in order (`BOOT_TRIGGERS`): first the plain-text `BOOTLOADER\n` command over CDC, then a 1200-baud open-and-close touch (dropping DTR). The firmware handles both by detaching TinyUSB (`tud_disconnect()`), switching the internal PHY to USB-Serial-JTAG, holding SE0 for 300 ms, restoring `dp_pu = 1` before dropping pull override, waiting up to 1.5 s for a USB bus reset, setting `RTC_CNTL_FORCE_DOWNLOAD_BOOT`, and calling `esp_restart()`. If neither trigger works, use `opadctl recover` (§7.7). Pressing the BOOT or RESET buttons is never needed. <!-- src: /desktop/crates/opad-device/src/flash.rs#BOOT_TRIGGERS, /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->

### 7.4 Erase, then flash

With the pad in ROM download mode (`303a:1001`):

**Linux**

```sh
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
opadctl flash --full ~/Downloads/opad-1.0.0/   # or firmware/build
```

**Windows (PowerShell or Command Prompt)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
opadctl flash --full C:\Users\you\Downloads\opad-1.0.0\
```

Use `--before no-reset --after no-reset-no-stub` so `espflash` leaves the pad in download mode for `opadctl flash --full`. Do not unplug the pad while `erase-flash` is running. <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images -->

`--full` is required after `erase-flash` because the bootloader and partition table were erased. `resolve_flash_set` writes the images in ascending offset order — bootloader at `0x0`, partition table at `0x8000`, `ota_data_initial.bin` at `0xf000` (when present), and `opad-firmware.bin` at `0x20000` (`ota_0`) — then reboots the pad into the app using an RTC watchdog reset (`reset_to_app`). It accepts either an ESP-IDF `firmware/build` directory or a flat unpacked release directory. <!-- src: /desktop/cli/src/main.rs#resolve_flash_set, /desktop/crates/opad-device/src/flash.rs#reset_to_app -->

### 7.5 Confirm

```sh
opadctl status
```

A pad that booted cleanly reports `ESP32 Device: Connected` and `Running Slot: ota_0`. <!-- src: /desktop/cli/src/main.rs#Commands -->

On a machine that has never seen this pad, counters start at 0 with generation 1. On the machine you erased it from, the daemon restores the stored counters and re-claims ownership (§7.2). <!-- src: /firmware/main/counters/counters.c#counters_init, /desktop/daemon/src/runtime.rs#RuntimeController -->

If the pad does not come back as the OPad app within 15 seconds after a `--full` flash, `opadctl flash` reports an error. Re-run the `opadctl flash --full` step (do not erase again). If the pad has left download mode and does not enumerate as OPad (`303a:8415`), use `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

### 7.6 When flashing is interrupted

An app-only update (`opadctl flash <app.bin>` with the daemon reporting the running slot, or an update from the GUI / `opadctl firmware-update`) calls `plan_app_update` to write the inactive OTA slot first (`ota_1` if running `ota_0`, or `ota_0` if running `ota_1`) and writes `otadata` second so the bootloader tries the new slot once while keeping the previous slot `OTA_IMG_VALID`. (If `opadctl flash <app.bin>` runs without knowing the running slot — for example with no daemon running or on legacy firmware — it falls back to writing `ota_0` at `0x20000` in place with no rollback fallback.) <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update, /desktop/cli/src/main.rs#Commands -->

- **Interrupted before `otadata` is written**: The boot selection still points at the old slot, so the pad boots its previous firmware unchanged. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
- **Written, but the new image fails**: The new image boots in `ESP_OTA_IMG_PENDING_VERIFY` state. At the end of `app_main`, `confirm_image_once_usb_works` waits until the host configures USB (`tud_mounted()`). If a USB host connects (`tud_connected()`) and fails to configure USB within 20 seconds (`IMAGE_CONFIRM_TIMEOUT_US`), the firmware calls `esp_restart()`, and the bootloader marks the trial image aborted and boots the previous valid slot. A crash loop rolls back the same way. If powered with no USB host (such as a wall charger), the image remains pending until next connected to a host. When rollback occurs, `opadctl flash` reports `"The pad did not accept the new firmware (it never came up as a working USB device) and went back to the previous one on <slot>. It works as before; nothing else is needed."` <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /desktop/cli/src/main.rs#report_slot_after_flash -->

A `--full` flash (§7.4) writes `ota_0` in place with no fallback. If it is interrupted, recover the pad with `opadctl recover` (§7.7). <!-- src: /desktop/cli/src/main.rs#Commands -->

### 7.7 A pad that does not show up as OPad (`opadctl recover`)

If firmware boots but never enumerates on USB (for example `can't set config #1, error -32` in Linux `dmesg`), nothing can ask the app over CDC to enter download mode. Recover it by catching the ROM window at power-on: <!-- src: /desktop/crates/opad-device/src/flash.rs#recover, /desktop/cli/src/main.rs#Commands -->

```sh
opadctl recover firmware/build   # or an unpacked release directory
```

When prompted (`Unplug the pad's USB cable and plug it back in (waiting 300 s)...`, configurable with `--timeout`), unplug and replug the pad's USB cable. At every power-on, before the app takes over the USB PHY, the ESP32-S3 ROM USB-Serial-JTAG interface appears briefly as `303a:1001`. `flash::recover` detects that port (polling `/sys/class/tty/ttyACM*` directly on Linux because udev is too slow for the ~200 ms window, and `bootloader_ports()` on Windows), resets the chip into download mode via DTR/RTS (`usj_reset_to_download`), runs ROM `SYNC` and disables the boot watchdogs (`disable_watchdogs`), writes the full image set (`write_images`), and reboots into the app (`reset_to_app`), then waits up to 15 seconds for the OPad app port (`303a:8415`). No BOOT or RESET button is ever needed. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover, /desktop/crates/opad-device/src/flash.rs#rom_port_names, /desktop/crates/opad-device/src/flash.rs#disable_watchdogs, /desktop/cli/src/main.rs#Commands -->

Only a real power cycle brings the ROM port back — a software restart does not — so replugging the USB cable is required. <!-- src: /desktop/crates/opad-device/src/flash.rs -->
