---
title: Backup and Restore
description: Exporting and restoring JSON backups of device configurations, screen layouts, and keypress counters via opadctl or the desktop GUI.
category: how-to
source_files:
  - /desktop/daemon/src/backup.rs
  - /desktop/cli/src/main.rs
  - /desktop/crates/opad-storage/src/lib.rs
---

OPad provides complete configuration, layout, and counter state backups serialized in a portable JSON format (`JsonBackup`). Backups can be exported manually or restored onto a new keypad, and `opad-daemon` generates automatic backups in the background to ensure progress is never lost if hardware is damaged or replaced. <!-- src: /desktop/daemon/src/backup.rs -->

## Automated Backups

To protect keypress records without user intervention: <!-- src: /desktop/daemon/src/backup.rs -->
- `opad-daemon` automatically saves a JSON backup following every active play session.
- **Latency Protection**: In accordance with latency invariants (Product Rule 1), no disk writes occur while playing. The daemon waits 20 seconds after the post-map cooldown concludes and the device returns to `IDLE` before committing the backup file to disk. <!-- src: /desktop/daemon/src/backup.rs -->
- Automatic backups reside in the per-user data directory (`<data_dir>/backups/`). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->

## Exporting a Backup

You can generate a backup at any time using `opadctl`: <!-- src: /desktop/cli/src/main.rs -->
```bash
opadctl backup export opad-backup.json
```
The output file contains:
- `device_id`: Hardware identifier of the backed-up pad.
- `counter_generation`: Sequence counter generation number.
- `lifetime_key1`, `lifetime_key2`, `lifetime_key3`: Total lifetime keypresses.
- `config`: Full `DeviceConfig` payload (debounce parameters, pin assignments, LED brightness).
- `layouts`: Custom JSON screen widget structures for `idle` and `playing` layouts.

### Exporting via GUI
1. Open `opad-gui` and go to **Settings > Keypad**.
2. Click **Export Backup**.
3. Choose a destination file and click **Save**.

## Previewing and Restoring a Backup

Before applying a backup to a connected keypad, `opadctl` provides a preview mode that compares the incoming backup against current pad state to detect potential counter rollback risks: <!-- src: /desktop/cli/src/main.rs -->

### Previewing
```bash
opadctl backup preview opad-backup.json
```
The preview checks:
- Whether the backup's `device_id` matches the connected pad.
- Whether restoring the backup would roll back lifetime counters to an earlier number.
- Any conflicting layout or configuration fields.

### Restoring
To apply the backup to the pad and desktop database: <!-- src: /desktop/cli/src/main.rs -->
```bash
opadctl backup import opad-backup.json --confirm
```
The daemon:
1. Validates the JSON schema structure.
2. Increments the `counter_generation` number so the incoming counters supersede existing hardware state cleanly. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
3. Transmits `SetConfig` and `SetLayout` to update pad RAM.
4. Flushes updated records to SQLite and pad NVS once idle.

### Restoring via GUI
1. In `opad-gui`, go to **Settings > Keypad**.
2. Click **Import Backup** and select the `.json` file.
3. Review the preview diff dialog displaying counter deltas and configuration changes.
4. Click **Confirm Import** to apply.
