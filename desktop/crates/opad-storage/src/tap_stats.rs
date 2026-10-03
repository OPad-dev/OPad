//! Tap rate (PPM) history: one summarised row per attempt, never the live
//! samples. The rows keep each channel's valid interval count and total
//! interval time, so any period's average is rebuilt exactly and weighted by
//! how long the player tapped, not by how many attempts there were.

use crate::{Storage, StorageError};
use chrono::{DateTime, Duration, TimeZone, Utc};
use opad_model::tap_rate::{
    validate_history_days, AttemptStats, ChannelHistory, ChannelStats, TapHistory,
    DEFAULT_HISTORY_DAYS,
};
use rusqlite::params;

/// `app_state` key of the statistics history period, in days (0 = all time)
pub const HISTORY_DAYS_KEY: &str = "stats.history_days";

/// The three channels' column prefixes, in `TapChannel` order
const CHANNELS: [&str; 3] = ["k1", "k2", "combined"];

impl Storage {
    /// v9: tap rate history, one row per attempt (issue #2)
    pub(crate) fn apply_v9(&self) -> Result<(), StorageError> {
        let channel_columns: String = CHANNELS
            .iter()
            .map(|c| {
                format!(
                    "{c}_press_count INTEGER NOT NULL,
                    {c}_valid_intervals INTEGER NOT NULL,
                    {c}_interval_ms_sum REAL NOT NULL,
                    {c}_average_ppm REAL,
                    {c}_peak_ppm REAL,"
                )
            })
            .collect();
        self.conn.execute_batch(&format!(
            "BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS tap_attempts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at INTEGER NOT NULL,
                ended_at INTEGER NOT NULL,
                attempt_duration_ms INTEGER NOT NULL,
                beatmap_id INTEGER,
                beatmap_md5 TEXT,
                ruleset TEXT,
                mods TEXT,
                title TEXT NOT NULL DEFAULT '',
                artist TEXT NOT NULL DEFAULT '',
                difficulty TEXT NOT NULL DEFAULT '',
                {channel_columns}
                recorded_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS tap_attempts_started_at ON tap_attempts (started_at);
            CREATE INDEX IF NOT EXISTS tap_attempts_beatmap_md5 ON tap_attempts (beatmap_md5);
            INSERT INTO schema_migrations (version, applied_at) VALUES (9, datetime('now'));
            COMMIT;"
        ))?;
        Ok(())
    }

    /// v10: the song rate, every press over the song time from the first note
    /// to the last, breaks included. Rows from v9 have none (0 ms) and are
    /// left out of its history.
    pub(crate) fn apply_v10(&self) -> Result<(), StorageError> {
        // ADD COLUMN has no IF NOT EXISTS: skip it for a table that has the
        // columns already (v9 run again on a newer table)
        let has_song: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('tap_attempts') WHERE name = 'song_ms'",
            [],
            |row| row.get(0),
        )?;
        let add_columns = if has_song {
            ""
        } else {
            "ALTER TABLE tap_attempts ADD COLUMN song_press_count INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE tap_attempts ADD COLUMN song_ms REAL NOT NULL DEFAULT 0;"
        };
        self.conn.execute_batch(&format!(
            "BEGIN TRANSACTION;
            {add_columns}
            INSERT INTO schema_migrations (version, applied_at) VALUES (10, datetime('now'));
            COMMIT;"
        ))?;
        Ok(())
    }

    /// v11: the song peak, the most presses in any 10 s of song
    pub(crate) fn apply_v11(&self) -> Result<(), StorageError> {
        let has_peak: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('tap_attempts') WHERE name = 'song_peak_ppm'",
            [],
            |row| row.get(0),
        )?;
        self.conn.execute_batch(&format!(
            "BEGIN TRANSACTION;
            {}
            INSERT INTO schema_migrations (version, applied_at) VALUES (11, datetime('now'));
            COMMIT;",
            if has_peak {
                ""
            } else {
                "ALTER TABLE tap_attempts ADD COLUMN song_peak_ppm REAL;"
            }
        ))?;
        Ok(())
    }

    /// Saves finished attempts in one transaction. Attempts still being
    /// played (no `ended_at`) are skipped. Returns how many were written.
    pub fn save_tap_attempts(&self, attempts: &[AttemptStats]) -> Result<usize, StorageError> {
        self.check_writes_allowed()?;
        let tx = self.conn.unchecked_transaction()?;
        let mut written = 0;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO tap_attempts (
                    started_at, ended_at, attempt_duration_ms,
                    beatmap_id, beatmap_md5, ruleset, mods, title, artist, difficulty,
                    k1_press_count, k1_valid_intervals, k1_interval_ms_sum, k1_average_ppm, k1_peak_ppm,
                    k2_press_count, k2_valid_intervals, k2_interval_ms_sum, k2_average_ppm, k2_peak_ppm,
                    combined_press_count, combined_valid_intervals, combined_interval_ms_sum,
                    combined_average_ppm, combined_peak_ppm,
                    song_press_count, song_ms, song_peak_ppm,
                    recorded_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                          ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                          ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, datetime('now'))",
            )?;
            for a in attempts {
                let (Some(ended_at), Some(duration)) = (a.ended_at, a.duration_ms()) else {
                    continue;
                };
                let b = &a.beatmap;
                let (k1, k2, all) = (&a.k1, &a.k2, &a.combined);
                insert.execute(params![
                    a.started_at.timestamp_millis(),
                    ended_at.timestamp_millis(),
                    duration,
                    b.beatmap_id,
                    b.checksum,
                    b.ruleset,
                    b.mods,
                    b.title,
                    b.artist,
                    b.difficulty,
                    k1.presses,
                    k1.valid_intervals,
                    k1.interval_ms_sum,
                    k1.average_ppm,
                    k1.peak_ppm,
                    k2.presses,
                    k2.valid_intervals,
                    k2.interval_ms_sum,
                    k2.average_ppm,
                    k2.peak_ppm,
                    all.presses,
                    all.valid_intervals,
                    all.interval_ms_sum,
                    all.average_ppm,
                    all.peak_ppm,
                    a.song.presses,
                    a.song.ms,
                    a.song.peak_ppm,
                ])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Aggregates the attempts that started within the last `period_days`
    /// before `now` (0 = all of them)
    pub fn tap_history(
        &self,
        period_days: u32,
        now: DateTime<Utc>,
    ) -> Result<TapHistory, StorageError> {
        let since = if period_days == 0 {
            i64::MIN
        } else {
            (now - Duration::days(i64::from(period_days))).timestamp_millis()
        };
        let mut history = TapHistory {
            period_days,
            ..Default::default()
        };
        history.attempts = self.conn.query_row(
            "SELECT COUNT(*) FROM tap_attempts WHERE started_at >= ?1",
            params![since],
            |row| row.get::<_, i64>(0),
        )? as u64;
        for (prefix, out) in
            CHANNELS
                .iter()
                .zip([&mut history.k1, &mut history.k2, &mut history.combined])
        {
            *out = self.conn.query_row(
                &format!(
                    "SELECT SUM({prefix}_valid_intervals), SUM({prefix}_interval_ms_sum),
                            AVG({prefix}_peak_ppm), MAX({prefix}_peak_ppm),
                            SUM({prefix}_press_count)
                     FROM tap_attempts WHERE started_at >= ?1"
                ),
                params![since],
                |row| {
                    let intervals: Option<i64> = row.get(0)?;
                    let interval_ms: Option<f64> = row.get(1)?;
                    Ok(ChannelHistory {
                        average_ppm: match (intervals, interval_ms) {
                            (Some(n), Some(ms)) if n > 0 && ms > 0.0 => {
                                Some(60_000.0 * n as f64 / ms)
                            }
                            _ => None,
                        },
                        average_peak_ppm: row.get(2)?,
                        best_peak_ppm: row.get(3)?,
                        total_presses: row.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as u64,
                    })
                },
            )?;
        }
        (history.song_ppm, history.best_song_peak_ppm) = self.conn.query_row(
            "SELECT SUM(song_press_count), SUM(song_ms), MAX(song_peak_ppm)
             FROM tap_attempts WHERE started_at >= ?1 AND song_ms > 0",
            params![since],
            |row| {
                let presses: Option<i64> = row.get(0)?;
                let ms: Option<f64> = row.get(1)?;
                let rate = match (presses, ms) {
                    (Some(n), Some(ms)) if ms > 0.0 => Some(60_000.0 * n as f64 / ms),
                    _ => None,
                };
                Ok((rate, row.get(2)?))
            },
        )?;
        Ok(history)
    }

    /// The most recent attempts, newest first (for a future per-map view and
    /// for checking what was recorded)
    pub fn recent_tap_attempts(&self, limit: usize) -> Result<Vec<AttemptStats>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT started_at, ended_at, beatmap_id, beatmap_md5, ruleset, mods,
                    title, artist, difficulty,
                    k1_press_count, k1_valid_intervals, k1_interval_ms_sum, k1_average_ppm, k1_peak_ppm,
                    k2_press_count, k2_valid_intervals, k2_interval_ms_sum, k2_average_ppm, k2_peak_ppm,
                    combined_press_count, combined_valid_intervals, combined_interval_ms_sum,
                    combined_average_ppm, combined_peak_ppm, song_press_count, song_ms,
                    song_peak_ppm
             FROM tap_attempts ORDER BY started_at DESC, id DESC LIMIT ?1",
        )?;
        let millis = |ms: i64| Utc.timestamp_millis_opt(ms).single().unwrap_or_default();
        let rows = stmt.query_map(params![limit as i64], |row| {
            let channel = |base: usize| -> rusqlite::Result<ChannelStats> {
                Ok(ChannelStats {
                    current_ppm: None,
                    presses: row.get(base)?,
                    valid_intervals: row.get(base + 1)?,
                    interval_ms_sum: row.get(base + 2)?,
                    average_ppm: row.get(base + 3)?,
                    peak_ppm: row.get(base + 4)?,
                })
            };
            Ok(AttemptStats {
                started_at: millis(row.get(0)?),
                ended_at: Some(millis(row.get(1)?)),
                beatmap: opad_model::tap_rate::BeatmapRef {
                    beatmap_id: row.get(2)?,
                    checksum: row.get(3)?,
                    ruleset: row.get(4)?,
                    mods: row.get(5)?,
                    title: row.get(6)?,
                    artist: row.get(7)?,
                    difficulty: row.get(8)?,
                },
                k1: channel(9)?,
                k2: channel(14)?,
                combined: channel(19)?,
                song: opad_model::tap_rate::SongRate {
                    presses: row.get(24)?,
                    ms: row.get(25)?,
                    peak_ppm: row.get(26)?,
                },
                replay: false,
            })
        })?;
        rows.collect::<Result<_, _>>().map_err(Into::into)
    }

    /// The configured history period; the default when unset or unreadable
    pub fn tap_history_days(&self) -> Result<u32, StorageError> {
        Ok(self
            .get_app_state(HISTORY_DAYS_KEY)?
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|d| validate_history_days(*d).is_ok())
            .unwrap_or(DEFAULT_HISTORY_DAYS))
    }

    pub fn set_tap_history_days(&self, days: u32) -> Result<(), StorageError> {
        validate_history_days(days).map_err(StorageError::Migration)?;
        self.set_app_state(HISTORY_DAYS_KEY, &days.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opad_model::tap_rate::BeatmapRef;

    /// An attempt with `intervals` valid intervals of `interval_ms` on every
    /// channel, and the given peak
    fn attempt(
        started_at: DateTime<Utc>,
        intervals: u32,
        interval_ms: f64,
        peak: f64,
    ) -> AttemptStats {
        let channel = ChannelStats {
            current_ppm: None,
            average_ppm: Some(60_000.0 / interval_ms),
            peak_ppm: Some(peak),
            presses: intervals + 1,
            valid_intervals: intervals,
            interval_ms_sum: f64::from(intervals) * interval_ms,
        };
        AttemptStats {
            started_at,
            ended_at: Some(started_at + Duration::seconds(10)),
            beatmap: BeatmapRef {
                title: "Map".into(),
                checksum: Some("abc".into()),
                ..Default::default()
            },
            k1: channel.clone(),
            k2: channel.clone(),
            combined: channel,
            song: opad_model::tap_rate::SongRate {
                presses: intervals + 1,
                ms: 10_000.0,
                peak_ppm: Some(peak),
            },
            replay: false,
        }
    }

    #[test]
    fn history_is_weighted_by_tapping_time() {
        let storage = Storage::open_in_memory().unwrap();
        let now = Utc::now();
        // A short attempt at 300 PPM and a long one at 150 PPM
        let short = attempt(now - Duration::hours(1), 10, 200.0, 310.0);
        let long = attempt(now - Duration::hours(2), 1000, 400.0, 170.0);
        assert_eq!(storage.save_tap_attempts(&[short, long]).unwrap(), 2);

        let h = storage.tap_history(30, now).unwrap();
        assert_eq!(h.attempts, 2);
        // Not (300 + 150) / 2 = 225: 1010 intervals over 402 s
        let avg = h.combined.average_ppm.unwrap();
        assert!((avg - 60_000.0 * 1010.0 / 402_000.0).abs() < 0.01, "{avg}");
        assert!(avg < 155.0);
        assert_eq!(h.combined.best_peak_ppm, Some(310.0));
        assert_eq!(h.combined.average_peak_ppm, Some(240.0));
        assert_eq!(h.k1.total_presses, 11 + 1001);
        // Song rate: 1012 presses over 20 s of song, weighted the same way
        let song = h.song_ppm.unwrap();
        assert!((song - 60_000.0 * 1012.0 / 20_000.0).abs() < 0.01, "{song}");
        assert_eq!(h.best_song_peak_ppm, Some(310.0));
    }

    #[test]
    fn periods_select_by_start_time() {
        let storage = Storage::open_in_memory().unwrap();
        let now = Utc::now();
        let ages = [1, 10, 60, 400];
        let attempts: Vec<_> = ages
            .iter()
            .map(|d| attempt(now - Duration::days(*d), 10, 300.0, 200.0))
            .collect();
        storage.save_tap_attempts(&attempts).unwrap();
        let count = |days| storage.tap_history(days, now).unwrap().attempts;
        assert_eq!(count(7), 1);
        assert_eq!(count(30), 2);
        assert_eq!(count(90), 3);
        assert_eq!(count(0), 4);
    }

    #[test]
    fn empty_history_has_no_rates() {
        let storage = Storage::open_in_memory().unwrap();
        let h = storage.tap_history(30, Utc::now()).unwrap();
        assert_eq!(h.attempts, 0);
        assert_eq!(h.combined, ChannelHistory::default());
        assert_eq!(h.period_days, 30);
    }

    #[test]
    fn unfinished_attempts_and_blocked_writes_are_not_saved() {
        let storage = Storage::open_in_memory().unwrap();
        let mut live = attempt(Utc::now(), 10, 300.0, 200.0);
        live.ended_at = None;
        assert_eq!(storage.save_tap_attempts(&[live]).unwrap(), 0);

        storage.set_writes_allowed(false);
        let done = attempt(Utc::now(), 10, 300.0, 200.0);
        assert!(matches!(
            storage.save_tap_attempts(std::slice::from_ref(&done)),
            Err(StorageError::WritesBlocked)
        ));
        assert!(storage.set_tap_history_days(7).is_err());
        storage.set_writes_allowed(true);
        assert_eq!(storage.tap_history(0, Utc::now()).unwrap().attempts, 0);
    }

    #[test]
    fn attempts_round_trip() {
        let storage = Storage::open_in_memory().unwrap();
        let a = attempt(
            Utc.timestamp_millis_opt(1_790_000_000_123).unwrap(),
            10,
            300.0,
            205.0,
        );
        storage.save_tap_attempts(std::slice::from_ref(&a)).unwrap();
        assert_eq!(storage.recent_tap_attempts(5).unwrap(), vec![a]);
    }

    #[test]
    fn history_period_setting() {
        let storage = Storage::open_in_memory().unwrap();
        assert_eq!(storage.tap_history_days().unwrap(), DEFAULT_HISTORY_DAYS);
        storage.set_tap_history_days(90).unwrap();
        assert_eq!(storage.tap_history_days().unwrap(), 90);
        storage.set_tap_history_days(0).unwrap();
        assert_eq!(storage.tap_history_days().unwrap(), 0);
        assert!(storage.set_tap_history_days(100_000).is_err());
    }
}
