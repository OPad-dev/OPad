//! Automatic JSON counter backups.
//!
//! `ExportBackup` over IPC has always existed, but it is manual: a pad that
//! dies between exports takes its lifetime counters with it, and the person
//! who most needs the backup is the one who never ran the command. So the
//! daemon writes one by itself, in the same `JsonBackup` format, 20 seconds
//! after a play session has finished settling.
//!
//! **Why 20 seconds and not immediately.** P1-3 and §11.3: no storage write
//! may happen during PLAYING or COOLDOWN. A backup is a storage write, and a
//! disk touch during a map is exactly the class of jitter that invariant
//! exists to exclude. The sequence is COOLDOWN → SYNC → IDLE; the delay is
//! armed only once the post-play sync has settled and the mode is IDLE, and
//! the write is re-checked against IDLE when it actually fires. The window
//! also absorbs the common case of a player starting the next map straight
//! away — that cancels the pending write rather than racing it.
//!
//! **Why it may never fail loudly.** A backup exists to help after something
//! has already gone wrong. Taking the daemon down, or blocking the sync,
//! because a disk was full would make the safety net the failure. Every error
//! here is logged and swallowed.

use chrono::{DateTime, SecondsFormat, Utc};
use opad_model::{paths, DeviceInfo, JsonBackup};
use opad_storage::Storage;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

use crate::runtime::DaemonState;

/// Builds the backup for the counters the daemon currently holds.
///
/// Shared by `ExportBackup` over IPC and by the automatic write, so the two
/// cannot produce different documents for the same state. `None` means there
/// is nothing worth backing up yet: no pad has ever connected and no counters
/// were loaded from the database.
pub fn current(
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
) -> Option<JsonBackup> {
    let st = state.lock().unwrap();
    if !st.device_connected && st.device_info.is_none() && st.counters.device_id.is_empty() {
        return None;
    }
    let info = st
        .device_info
        .clone()
        .or_else(|| {
            storage.lock().unwrap().as_ref().and_then(|s| {
                s.list_device_states()
                    .ok()?
                    .into_iter()
                    .find(|(i, _)| i.device_id == st.counters.device_id)
                    .map(|(i, _)| i)
            })
        })
        .unwrap_or_else(|| DeviceInfo {
            device_id: st.counters.device_id.clone(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: String::new(),
            protocol_version: 1,
            running_partition: None,
        });
    Some(JsonBackup::new(&info, &st.counters, &st.config))
}

/// How many backups to keep. Ten covers a long session's worth of plays
/// without turning the directory into an unbounded log.
pub const KEEP: usize = 10;

const PREFIX: &str = "osupad-backup-";
const SUFFIX: &str = ".json";

/// `<data>/backups`, beside `osupad.db` so one directory holds everything the
/// uninstaller has to know about (§W2-3, §W0-4).
pub fn backup_dir() -> Result<PathBuf, paths::PathError> {
    Ok(paths::data_dir()?.join("backups"))
}

/// Writes one backup and prunes the directory back to [`KEEP`].
///
/// Atomic in the same sense as `opad_update::download::stage_bytes`: a
/// temporary file in the destination directory, fsynced, then renamed into
/// place, so a kill at any point leaves whole files only. Returns the path so
/// the caller can log it.
pub fn write(backup: &JsonBackup, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;

    let name = file_name_for(backup.exported_at);
    let target = dir.join(&name);
    let tmp = dir.join(format!(".{name}.incoming"));

    let json = serde_json::to_vec_pretty(backup)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(&json)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, &target)?;
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }

    prune(dir, KEEP);
    Ok(target)
}

/// The whole operation, with every failure absorbed (see the module note).
///
/// Returns the timestamp recorded in the backup on success, so the daemon can
/// surface it in `Status` without re-reading the directory.
pub fn write_and_log(backup: &JsonBackup) -> Option<DateTime<Utc>> {
    let dir = match backup_dir() {
        Ok(d) => d,
        Err(e) => {
            warn!("Automatic backup skipped: {}", e);
            return None;
        }
    };
    match write(backup, &dir) {
        Ok(path) => {
            info!(
                "Automatic counter backup written: {} ({} / {})",
                path.display(),
                backup.stats.lifetime_key1,
                backup.stats.lifetime_key2
            );
            Some(backup.exported_at)
        }
        Err(e) => {
            // Never fatal, and never propagated into the sync path
            warn!(
                "Automatic backup failed ({}); continuing: {}",
                dir.display(),
                e
            );
            None
        }
    }
}

/// Deletes all but the `keep` newest backups.
///
/// The names sort chronologically, so this is a name sort rather than a stat
/// of every file — and it only ever considers files this module wrote, so a
/// hand-made export dropped in the directory is left alone.
pub fn prune(dir: &Path, keep: usize) {
    let mut ours: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && is_ours(p))
            .collect(),
        Err(e) => {
            warn!("Could not list {} to prune backups: {}", dir.display(), e);
            return;
        }
    };
    if ours.len() <= keep {
        return;
    }
    ours.sort();
    let doomed = ours.len() - keep;
    for path in ours.into_iter().take(doomed) {
        if let Err(e) = std::fs::remove_file(&path) {
            warn!("Could not remove the old backup {}: {}", path.display(), e);
        }
    }
}

/// The newest backup's timestamp, read from the file names.
///
/// Called once at startup so `opadctl status` reports a real last-backup
/// time across a daemon restart rather than "never".
pub fn newest(dir: &Path) -> Option<DateTime<Utc>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_ours(p))
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    parse_stamp(names.last()?)
}

fn is_ours(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(PREFIX) && n.ends_with(SUFFIX))
}

fn file_name_for(at: DateTime<Utc>) -> String {
    format!("{PREFIX}{}{SUFFIX}", at.format("%Y%m%dT%H%M%SZ"))
}

fn parse_stamp(name: &str) -> Option<DateTime<Utc>> {
    let stamp = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let naive = chrono::NaiveDateTime::parse_from_str(stamp, "%Y%m%dT%H%M%SZ").ok()?;
    Some(naive.and_utc())
}

/// RFC 3339, matching every other timestamp on the IPC surface
pub fn format_stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opad_model::{CounterState, DeviceConfig, DeviceInfo};

    fn backup_at(secs: i64, key1: u64) -> JsonBackup {
        let mut b = JsonBackup::new(
            &DeviceInfo {
                device_id: "pad-1".into(),
                board_profile: "waveshare_esp32s3_touch_lcd_2".into(),
                firmware_version: "1.0.0".into(),
                protocol_version: 1,
                running_partition: Some("ota_0".into()),
            },
            &CounterState {
                device_id: "pad-1".into(),
                lifetime_key1: key1,
                lifetime_key2: 20594,
                ..Default::default()
            },
            &DeviceConfig::default(),
        );
        b.exported_at = DateTime::from_timestamp(1_780_000_000 + secs, 0).unwrap();
        b
    }

    #[test]
    fn a_backup_round_trips_through_the_file_it_writes() {
        let dir = tempfile::tempdir().unwrap();
        let backup = backup_at(0, 3745);
        let path = write(&backup, dir.path()).unwrap();

        let read: JsonBackup = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(read, backup);
        read.validate().expect("what we write must import again");
        // The name carries the timestamp, so the directory sorts chronologically
        assert_eq!(
            path.file_name().unwrap().to_str().unwrap(),
            file_name_for(backup.exported_at)
        );
    }

    #[test]
    fn rotation_keeps_exactly_the_ten_newest() {
        let dir = tempfile::tempdir().unwrap();
        // 25 backups, one per minute, written oldest first
        for i in 0..25 {
            write(&backup_at(i * 60, 1000 + i as u64), dir.path()).unwrap();
        }

        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left.len(), KEEP, "rotation must keep exactly {KEEP}");
        // …and they must be the *newest* ten, not the first ten
        assert_eq!(left[0], file_name_for(backup_at(15 * 60, 0).exported_at));
        assert_eq!(
            left[KEEP - 1],
            file_name_for(backup_at(24 * 60, 0).exported_at)
        );
        assert_eq!(newest(dir.path()), Some(backup_at(24 * 60, 0).exported_at));
    }

    #[test]
    fn nothing_the_daemon_did_not_write_is_ever_pruned() {
        let dir = tempfile::tempdir().unwrap();
        // A hand-made export the user dropped here, and an unrelated file
        std::fs::write(dir.path().join("my-export.json"), b"{}").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();
        for i in 0..15 {
            write(&backup_at(i * 60, 0), dir.path()).unwrap();
        }
        assert!(dir.path().join("my-export.json").exists());
        assert!(dir.path().join("notes.txt").exists());
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            KEEP + 2,
            "only our own backups are rotated"
        );
    }

    #[test]
    fn a_failed_write_is_reported_rather_than_panicking() {
        // A file where the directory should be: create_dir_all fails, and the
        // caller must get an Err instead of an unwind.
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("not-a-directory");
        std::fs::write(&blocked, b"in the way").unwrap();
        assert!(write(&backup_at(0, 0), &blocked).is_err());
    }

    #[test]
    fn no_backups_reads_as_none_not_as_the_epoch() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(newest(dir.path()), None);
        std::fs::write(dir.path().join("my-export.json"), b"{}").unwrap();
        assert_eq!(newest(dir.path()), None);
    }

    #[test]
    fn a_partly_written_backup_is_never_left_under_a_real_name() {
        let dir = tempfile::tempdir().unwrap();
        write(&backup_at(0, 3745), dir.path()).unwrap();
        // The temp name is hidden and is not a backup as far as rotation or
        // "newest" are concerned, so an interrupted write cannot be mistaken
        // for the latest good one.
        let tmp = dir.path().join(format!(
            ".{}.incoming",
            file_name_for(backup_at(0, 0).exported_at)
        ));
        assert!(!tmp.exists(), "the temporary file must be renamed away");
        assert!(!is_ours(&tmp));
    }
}
