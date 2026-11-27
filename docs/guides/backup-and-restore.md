---
title: Backup and Restore
description: Export your OPad settings and lifetime counters to a JSON file, restore them with a preview first, and find the automatic backups the daemon writes after each session.
sidebar:
  order: 7
---

A backup is a JSON file with your pad's settings and lifetime key counters. Make one by hand with `opadctl` or the app, or use the ones the daemon writes by itself after every play session. Keep one somewhere safe: it is what brings your counts back if a pad is lost, replaced or erased. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/daemon/src/backup.rs -->

## What you need

- The desktop app installed, with the daemon running. Restoring onto the pad also needs it connected.
- No map running. Backups are disabled during a map and its 5-second cooldown. <!-- src: /desktop/gui/src/pages.rs#pad --> <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->

## What a backup contains

| Part | Contents |
|---|---|
| `device` | the pad's `device_id`, `board_profile` (`waveshare_esp32s3_touch_lcd_2`) and `counter_generation` |
| `stats` | `lifetime_key1`, `lifetime_key2`, and `lifetime_key3` on a three-key pad when its count is known |
| `config` | key codes (`key1`, `key2`, and on a three-key pad `key3` and `game_keys`), `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, the four swipe actions with their keys, and `result_animations` |
<!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackupConfig -->

Screen layouts are not in it: export those from the Designer (see [Customizing the screen layout](customize-screen-layout.md#export-and-import-a-layout)). <!-- src: /desktop/gui/src/designer/mod.rs#export_layout -->

## Export a backup

**Terminal:**

```bash
opadctl backup export opad-backup.json
```
<!-- src: /desktop/cli/src/main.rs#run -->

**App:**

1. Open the **Pad** page.
2. In **Backup**, click **Export backup**.
3. Choose where to save the file.
<!-- src: /desktop/gui/src/pages.rs#pad -->

## Restore a backup

Restoring always shows you what will change before it changes anything.

**Terminal:**

```bash
opadctl backup import opad-backup.json
```

`opadctl` first asks the daemon to check the file and prints a comparison: current against incoming counts (Key 1, Key 2, and Key 3 if present), whether the backup's `device_id` matches the connected pad, and a warning if any count would go down. Answer `y` to apply it. To skip the question in a script, add `--yes`: `opadctl backup import --yes opad-backup.json`. <!-- src: /desktop/cli/src/main.rs#run -->

**App:**

1. On the **Pad** page, in **Backup**, click **Import backup** and choose the file.
2. Check the preview: current against incoming counts and whether the pad matches. If any count would go down, tick **I understand this replaces my counters with lower values**.
3. Click **Apply Backup**.
<!-- src: /desktop/gui/src/main.rs#App -->

When you apply it, the daemon raises the counter generation so the restored counts win over older copies, saves everything to its database, and sends the settings and counts to the pad, which saves them once it is idle. Your local tosu address is kept. See [Recovery §5](../recovery.md#5-backup-restore-and-reset) for the exact rules. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Automatic backups

You do not have to do anything for these: <!-- src: /desktop/daemon/src/backup.rs --> <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY -->

- **When:** 20 seconds after a play session ends and the pad is idle again. Disk writes never happen during play. Starting another map within those 20 seconds cancels that backup; the next session makes a new one.
- **Where:** `~/.local/share/opad/backups/` on Linux (or `$XDG_DATA_HOME/opad/backups/`), `%APPDATA%\opad\backups\` on Windows, named `opad-backup-<YYYYMMDDTHHMMSSZ>.json`. <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **How many:** the 10 newest. Older ones are deleted; files with any other name in that folder are left alone. <!-- src: /desktop/daemon/src/backup.rs#KEEP -->
- **Last one:** `Last Backup:` in `opadctl status`. <!-- src: /desktop/cli/src/main.rs#run -->

Restore one like any other backup.

> [!WARNING]
> Uninstalling the desktop app deletes this folder, automatic backups included. Copy a backup somewhere else first if you want to keep it. See [Uninstalling](uninstall.md). <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Check that it worked

- Run `opadctl status` and check the key counts and settings. <!-- src: /desktop/cli/src/main.rs#run -->
- In the app, the **Pad** page's **Counters** says `The PC and the pad agree` when they do. <!-- src: /desktop/gui/src/pages.rs#pad -->

## If it did not work

- **The buttons are greyed out, or the daemon refuses:** leave the map and wait 5 seconds for the cooldown to end. A pending "this pad belongs to another install" or "new pad" question also blocks restoring until you answer it. <!-- src: /desktop/gui/src/pages.rs#pad --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- **The file is rejected:** a valid backup has `"format_version": 1`, `"board_profile": "waveshare_esp32s3_touch_lcd_2"`, settings within their ranges, and an `exported_at` no more than a day in the future. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
