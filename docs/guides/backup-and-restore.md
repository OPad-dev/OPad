---
title: Backup and Restore
description: How automatic JSON backups work and how to export, preview, and restore OPad configuration and lifetime counters using opadctl or opad-gui.
---

OPad exports device configuration and lifetime key press counters as portable JSON files (`JsonBackup`, `format_version: 1`). You can export and restore backups manually via `opadctl` or `opad-gui`, and `opad-daemon` also writes automatic backups after play sessions so counters survive hardware damage or replacement. <!-- src: /desktop/daemon/src/backup.rs --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->

## What you need

- `opad-daemon` running (and your OPad connected over USB when restoring onto hardware).
- The pad must be idle (not in active gameplay or post-map cooldown); backup operations are disabled during `Playing` and `Cooldown`. <!-- src: /desktop/gui/src/pages.rs#device -->

## Automatic Backups

- After a play session finishes (`COOLDOWN` -> `SYNC` -> `IDLE`), `opad-daemon` waits **20 seconds** in `IDLE` before writing an automatic backup so disk I/O never touches the gameplay path (starting another map cancels the pending timer). <!-- src: /desktop/daemon/src/backup.rs -->
- Files are written atomically (`write_atomic`) to `<data_dir>/backups/opad-backup-<YYYYMMDDTHHMMSSZ>.json` (see [Files & Paths](../reference/files-and-paths.md)). <!-- src: /desktop/daemon/src/backup.rs#backup_dir -->
- The daemon keeps the **10** newest automatic backup files (`KEEP = 10`) and prunes older ones (leaving manually named files alone). <!-- src: /desktop/daemon/src/backup.rs#KEEP -->

## What a Backup Contains

A `JsonBackup` file records: <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- `device`: `device_id`, `board_profile` (`waveshare_esp32s3_touch_lcd_2`), and `counter_generation`.
- `stats`: `lifetime_key1`, `lifetime_key2`, and optional `lifetime_key3` (included for 3-key pads when known).
- `config`: `key1`, `key2`, optional `key3` and `game_keys` (3-key pads), `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, touchscreen swipe actions/keys/modifiers, and `result_animations`.

(Custom screen layouts are exported and imported separately inside the **Designer** page in `opad-gui`.) <!-- src: /desktop/gui/src/designer/mod.rs -->

## Exporting a Backup

### Via `opadctl`
```bash
opadctl export opad-backup.json
```
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

### Via `opad-gui`
1. Open `opad-gui` and click **Device** in the sidebar. <!-- src: /desktop/gui/src/pages.rs#device -->
2. Scroll to the **Backup & Restore** card and click **Export backup**. <!-- src: /desktop/gui/src/pages.rs#device -->
3. Choose where to save the `.json` file.

## Previewing and Restoring a Backup

### Via `opadctl`
Run `opadctl import` with the backup file: <!-- src: /desktop/cli/src/main.rs#Commands::Import -->
```bash
opadctl import opad-backup.json
```
Before modifying anything, `opadctl` sends `IpcRequest::PreviewImport` to validate the file and prints a comparison showing:
- Current vs incoming lifetime counts (`Key 1`, `Key 2`, and `Key 3` if present).
- Whether `device_id` matches the connected pad.
- Warnings if importing would roll counters backward.

Type `y` at the confirmation prompt to apply (or pass `--yes` to skip the prompt: `opadctl import --yes opad-backup.json`). <!-- src: /desktop/cli/src/main.rs#Commands::Import -->

### Via `opad-gui`
1. Open `opad-gui` and click **Device** in the sidebar. <!-- src: /desktop/gui/src/pages.rs#device -->
2. In the **Backup & Restore** card, click **Import backup** and choose your `.json` file.
3. Review the preview modal (current vs incoming counters, device ID check, and any rollback warning checkbox) and confirm the import. <!-- src: /desktop/gui/src/main.rs#ImportModalState -->

When confirmed, the daemon increments `counter_generation`, updates SQLite, pushes the configuration and counters to the pad, and commits to NVS once idle. <!-- src: /desktop/daemon/src/ipc_handlers.rs -->

## How to check

- Run `opadctl status` (or check the **Device** page in `opad-gui`) and verify **Counter Reconciliation State** shows **✓ SYNCHRONIZED** with the expected `lifetime_key1`, `lifetime_key2`, and configuration values. <!-- src: /desktop/gui/src/pages.rs#device -->

## What to do if it did not work

- **Buttons are disabled or daemon rejects import**: Exit the current osu! map and wait 5 seconds for post-map cooldown to end so the pad returns to `Idle`. <!-- src: /desktop/gui/src/pages.rs#device -->
- **Validation error on import**: Check that the JSON file has `"format_version": 1` and `"board_profile": "waveshare_esp32s3_touch_lcd_2"` and valid setting ranges. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
