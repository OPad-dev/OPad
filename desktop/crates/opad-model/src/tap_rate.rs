//! Player tap rate, in PPM (presses per minute).
//!
//! PPM is how fast the player is physically pressing K1 and K2. It is not the
//! beatmap's BPM (`map.bpm`), which is song metadata: a 200 BPM map can be
//! played at any PPM. Three channels are tracked independently: K1, K2 and
//! Combined, where Combined counts every press of either key. Alternating
//! K1/K2 with one press every 300 ms is therefore 200 PPM combined (and 100
//! on each key), not 400.
//!
//! Only key-down events count. The rules, all measured between consecutive
//! presses on one channel:
//!
//! - An interval longer than [`SEQUENCE_BREAK_MS`] ends the tapping sequence.
//!   It is not an interval at all: breaks, sliders and pauses never reach the
//!   average.
//! - Every press counts, however close to the one before: tosu's counts are
//!   presses osu! registered (the pad debounces its switches itself). Mashing
//!   both keys puts presses a few ms apart, and dropping those halved the
//!   measured rate. Presses landing in one telemetry sample share its time
//!   and add a 0 ms interval: the count is right, and so is the rate over the
//!   window.
//! - Current PPM is the live rate: presses over time across the last
//!   [`WINDOW_INTERVALS`] intervals of the sequence. Once the player
//!   has clearly stopped (a gap over twice their rhythm and over
//!   [`STOP_GAP_MS`]) it is the gap itself, so it falls (an 800 ms pause
//!   reads 75 PPM), and it reads 0 once the sequence has ended. A normal 1/1
//!   note after a burst does not count as stopping, or the value would swing
//!   on every beat. It always has a value while an attempt is played, so the
//!   pad never hides it mid-map.
//! - Peak PPM is the highest Current PPM over a *full* window: the fastest
//!   the player tapped, over 7 presses in a row. That catches a short burst
//!   at its real speed (a 1/4 burst reads its 1/4 rate), while one quick
//!   double tap is too short to set it.
//! - Average PPM is presses over tapping time: the sum of the sequences'
//!   intervals, not the length of the map.
//!
//! The song rate is the other way of looking at it: every press over the song
//! time played, from the first note to the last, breaks included. It runs on
//! the song's own clock, so the game paused (Esc) does not count, and it is
//! converted to real time under speed mods. It is not split by key. Its peak
//! is the most presses in any [`SONG_PEAK_WINDOW_MS`] of song time, counted
//! the same way: a stretch long enough that one burst cannot make it, so it
//! points at the hardest section. (The per-channel Peak above is the fastest
//! tapping: a short burst at its real speed.)

use crate::ui_source::{self as src, SourceValue};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

/// Valid intervals in the rolling window behind Current and Peak PPM
pub const WINDOW_INTERVALS: usize = 6;
/// A gap must be at least this long, and twice the player's rhythm, before
/// the live rate treats it as having stopped (100 PPM; a 1/1 note at 100 BPM)
pub const STOP_GAP_MS: f64 = 600.0;
/// A gap longer than this ends the tapping sequence (60 PPM on one channel)
pub const SEQUENCE_BREAK_MS: f64 = 1000.0;
/// Presses this long (song time) before the first note or after the last one
/// still count: an early or late hit on a note is still a hit on it
pub const SONG_HIT_MARGIN_MS: f64 = 200.0;
/// The song rate is shown once this much song time (real ms) has been played.
/// Over the first seconds it divides by very little time and swings wildly.
pub const MIN_SONG_MS: f64 = 5000.0;
/// The song peak is the most presses in any stretch of song this long (real ms)
pub const SONG_PEAK_WINDOW_MS: f64 = 10_000.0;
/// How far a press's song time is carried past the last song clock: tosu
/// sends one every few dozen ms, and while the game is paused it does not move
const SONG_CLOCK_CARRY_MS: f64 = 250.0;
/// Attempts with fewer combined presses are not written to the history
pub const MIN_RECORDED_PRESSES: u32 = 10;

/// The default statistics history period, in days
pub const DEFAULT_HISTORY_DAYS: u32 = 30;
/// The periods the app offers; 0 = all time
pub const HISTORY_PERIOD_CHOICES: [u32; 4] = [7, 30, 90, 0];
/// The longest period accepted, in days (0 = all time is always accepted)
pub const MAX_HISTORY_DAYS: u32 = 3650;

pub fn ppm_from_interval_ms(interval_ms: f64) -> f64 {
    60_000.0 / interval_ms
}

/// "Last 30 days", "All time"
pub fn history_period_label(days: u32) -> String {
    match days {
        0 => "All time".into(),
        1 => "Last day".into(),
        d => format!("Last {d} days"),
    }
}

pub fn validate_history_days(days: u32) -> Result<(), String> {
    if days > MAX_HISTORY_DAYS {
        Err(format!(
            "History period must be at most {MAX_HISTORY_DAYS} days (or 0 for all time)"
        ))
    } else {
        Ok(())
    }
}

/// One channel's statistics for an attempt
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelStats {
    /// None outside a tapping sequence
    pub current_ppm: Option<f64>,
    pub average_ppm: Option<f64>,
    pub peak_ppm: Option<f64>,
    /// Every key-down, including those whose interval was dropped
    pub presses: u32,
    /// Intervals that count towards the average
    pub valid_intervals: u32,
    /// Their total length, ms. With `valid_intervals` this rebuilds the
    /// average exactly, which is what makes the history weighting right.
    pub interval_ms_sum: f64,
}

/// What an attempt was played on, as far as tosu says
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BeatmapRef {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub beatmap_id: Option<i64>,
    /// The .osu file's MD5, stable across renames and resubmissions
    #[serde(default)]
    pub checksum: Option<String>,
    /// "osu", "taiko", "fruits", "mania"
    #[serde(default)]
    pub ruleset: Option<String>,
    #[serde(default)]
    pub mods: Option<String>,
}

/// Where the song is, from tosu's state frame
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SongClock {
    /// Song position, ms; stops while the game is paused
    pub live_ms: f64,
    pub first_object_ms: f64,
    pub last_object_ms: f64,
    /// Playback rate: 1.5 under DT, 0.75 under HT
    pub rate: f64,
}

/// Presses over song time played, breaks included
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct SongRate {
    /// Presses from the first note to the last
    pub presses: u32,
    /// Song time played between them, in real ms
    pub ms: f64,
    /// The most presses in any [`SONG_PEAK_WINDOW_MS`] of song, per minute.
    /// None until that much song has been played.
    #[serde(default)]
    pub peak_ppm: Option<f64>,
}

impl SongRate {
    pub fn ppm(&self) -> Option<f64> {
        (self.ms >= MIN_SONG_MS).then(|| 60_000.0 * f64::from(self.presses) / self.ms)
    }
}

/// One attempt: live while it is being played, final once it ended
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttemptStats {
    pub started_at: DateTime<Utc>,
    /// None while the attempt is still being played
    pub ended_at: Option<DateTime<Utc>>,
    pub beatmap: BeatmapRef,
    pub k1: ChannelStats,
    pub k2: ChannelStats,
    pub combined: ChannelStats,
    #[serde(default)]
    pub song: SongRate,
}

impl AttemptStats {
    pub fn channel(&self, channel: TapChannel) -> &ChannelStats {
        match channel {
            TapChannel::K1 => &self.k1,
            TapChannel::K2 => &self.k2,
            TapChannel::Combined => &self.combined,
        }
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.ended_at
            .map(|end| (end - self.started_at).num_milliseconds().max(0))
    }

    /// Worth keeping in the history: enough presses to say anything
    pub fn is_recordable(&self) -> bool {
        self.combined.presses >= MIN_RECORDED_PRESSES && self.combined.valid_intervals > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapChannel {
    K1,
    K2,
    Combined,
}

impl TapChannel {
    pub const ALL: [TapChannel; 3] = [TapChannel::K1, TapChannel::K2, TapChannel::Combined];

    pub fn label(self) -> &'static str {
        match self {
            TapChannel::K1 => "K1",
            TapChannel::K2 => "K2",
            TapChannel::Combined => "Combined",
        }
    }
}

/// One channel over the history period
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelHistory {
    /// Weighted by tapping time: all valid intervals over their total length
    pub average_ppm: Option<f64>,
    /// Mean of the attempts' peaks
    pub average_peak_ppm: Option<f64>,
    /// The highest peak recorded
    pub best_peak_ppm: Option<f64>,
    pub total_presses: u64,
}

/// Aggregated attempts over the history period
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TapHistory {
    /// 0 = all time
    pub period_days: u32,
    pub attempts: u64,
    pub k1: ChannelHistory,
    pub k2: ChannelHistory,
    pub combined: ChannelHistory,
    /// All the period's song presses over all its song time
    #[serde(default)]
    pub song_ppm: Option<f64>,
    /// The best song peak of the period
    #[serde(default)]
    pub best_song_peak_ppm: Option<f64>,
}

impl TapHistory {
    pub fn channel(&self, channel: TapChannel) -> &ChannelHistory {
        match channel {
            TapChannel::K1 => &self.k1,
            TapChannel::K2 => &self.k2,
            TapChannel::Combined => &self.combined,
        }
    }
}

// ---- Calculation -------------------------------------------------------------------

/// The rolling calculation for one channel. Times are milliseconds on any
/// monotonic clock.
#[derive(Debug, Clone, Default)]
pub struct ChannelTracker {
    last_press_ms: Option<f64>,
    /// The latest time seen, from a press or [`Self::advance`]
    now_ms: f64,
    window: VecDeque<f64>,
    peak_ppm: Option<f64>,
    presses: u32,
    valid_intervals: u32,
    interval_ms_sum: f64,
}

impl ChannelTracker {
    pub fn press(&mut self, at_ms: f64) {
        self.now_ms = self.now_ms.max(at_ms);
        self.presses = self.presses.saturating_add(1);
        let Some(prev) = self.last_press_ms else {
            self.last_press_ms = Some(at_ms);
            return;
        };
        let interval = (at_ms - prev).max(0.0);
        self.last_press_ms = Some(at_ms);
        if interval > SEQUENCE_BREAK_MS {
            self.end_sequence();
            return;
        }
        self.valid_intervals += 1;
        self.interval_ms_sum += interval;
        if self.window.len() == WINDOW_INTERVALS {
            self.window.pop_front();
        }
        self.window.push_back(interval);
        if self.window.len() == WINDOW_INTERVALS {
            if let Some(ppm) = window_interval_ms(&self.window).map(ppm_from_interval_ms) {
                self.peak_ppm = Some(self.peak_ppm.map_or(ppm, |p| p.max(ppm)));
            }
        }
    }

    /// Moves the clock on without a press: the live rate falls with the gap,
    /// and the sequence ends once it passes [`SEQUENCE_BREAK_MS`]
    pub fn advance(&mut self, now_ms: f64) {
        self.now_ms = self.now_ms.max(now_ms);
        if self
            .last_press_ms
            .is_some_and(|last| self.now_ms - last > SEQUENCE_BREAK_MS)
        {
            self.end_sequence();
        }
    }

    fn end_sequence(&mut self) {
        self.window.clear();
    }

    /// The live rate: the window's rate, or the gap once the player has
    /// clearly stopped, and 0 outside a sequence
    pub fn current_ppm(&self) -> f64 {
        let (Some(last), Some(interval)) = (self.last_press_ms, window_interval_ms(&self.window))
        else {
            return 0.0;
        };
        let gap = self.now_ms - last;
        let stopped = gap > (2.0 * interval).max(STOP_GAP_MS);
        ppm_from_interval_ms(if stopped { gap } else { interval })
    }

    pub fn stats(&self) -> ChannelStats {
        ChannelStats {
            current_ppm: Some(self.current_ppm()),
            average_ppm: (self.valid_intervals > 0 && self.interval_ms_sum > 0.0).then(|| {
                ppm_from_interval_ms(self.interval_ms_sum / f64::from(self.valid_intervals))
            }),
            peak_ppm: self.peak_ppm,
            presses: self.presses,
            valid_intervals: self.valid_intervals,
            interval_ms_sum: self.interval_ms_sum,
        }
    }
}

/// The window's mean interval: its presses over its time. None while it is
/// empty, or while all its presses landed at the same instant.
fn window_interval_ms(window: &VecDeque<f64>) -> Option<f64> {
    let sum: f64 = window.iter().sum();
    (!window.is_empty() && sum > 0.0).then(|| sum / window.len() as f64)
}

/// One attempt being played: feed it key-down counts as they arrive
#[derive(Debug, Clone)]
pub struct AttemptTracker {
    origin: Instant,
    started_at: DateTime<Utc>,
    beatmap: BeatmapRef,
    k1: ChannelTracker,
    k2: ChannelTracker,
    combined: ChannelTracker,
    song: SongRate,
    /// Between the first note and the last, by the latest song clock
    in_song: bool,
    /// The latest song clock: song time played (real ms) and when it came
    song_at: Option<(f64, Instant)>,
    /// Song times (real ms) of the presses in the last [`SONG_PEAK_WINDOW_MS`]
    song_window: VecDeque<f64>,
}

impl AttemptTracker {
    pub fn new(beatmap: BeatmapRef, origin: Instant, started_at: DateTime<Utc>) -> Self {
        Self {
            origin,
            started_at,
            beatmap,
            k1: ChannelTracker::default(),
            k2: ChannelTracker::default(),
            combined: ChannelTracker::default(),
            song: SongRate::default(),
            in_song: false,
            song_at: None,
            song_window: VecDeque::new(),
        }
    }

    /// The song clock moved. Presses count towards the song rate from just
    /// before the first note to the last; the time does from the first note
    /// to the last, so an intro or an outro does not water it down.
    pub fn song_clock(&mut self, clock: SongClock, at: Instant) {
        let SongClock {
            live_ms,
            first_object_ms: first,
            last_object_ms: last,
            rate,
        } = clock;
        if last <= first || last.is_nan() || first.is_nan() || !rate.is_finite() || rate <= 0.0 {
            return;
        }
        self.in_song =
            live_ms >= first - SONG_HIT_MARGIN_MS && live_ms <= last + SONG_HIT_MARGIN_MS;
        self.song.ms = (live_ms.clamp(first, last) - first) / rate;
        self.song_at = Some((self.song.ms, at));
    }

    /// A press's place in the song (real ms since the first note): the latest
    /// clock, carried forward a little for the time since it came
    fn song_time(&self, at: Instant) -> f64 {
        match self.song_at {
            Some((ms, clock_at)) => {
                let since = at.saturating_duration_since(clock_at).as_secs_f64() * 1000.0;
                ms + since.min(SONG_CLOCK_CARRY_MS)
            }
            None => self.song.ms,
        }
    }

    fn song_press(&mut self, at: Instant) {
        self.song.presses = self.song.presses.saturating_add(1);
        let t = self.song_time(at);
        self.song_window.push_back(t);
        while self
            .song_window
            .front()
            .is_some_and(|first| *first <= t - SONG_PEAK_WINDOW_MS)
        {
            self.song_window.pop_front();
        }
        if t >= SONG_PEAK_WINDOW_MS {
            let ppm = 60_000.0 * self.song_window.len() as f64 / SONG_PEAK_WINDOW_MS;
            self.song.peak_ppm = Some(self.song.peak_ppm.map_or(ppm, |p| p.max(ppm)));
        }
    }

    fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.origin).as_secs_f64() * 1000.0
    }

    /// `k1` and `k2` new key-downs seen at `at`. Several in one sample share
    /// its timestamp, so all but the first are counted without an interval.
    pub fn presses(&mut self, k1: u32, k2: u32, at: Instant) {
        let t = self.ms(at);
        if self.in_song {
            for _ in 0..(k1 + k2).min(128) {
                self.song_press(at);
            }
        }
        for _ in 0..k1.min(64) {
            self.k1.press(t);
            self.combined.press(t);
        }
        for _ in 0..k2.min(64) {
            self.k2.press(t);
            self.combined.press(t);
        }
    }

    /// Moves the clock on, so the live rate falls while nothing is pressed
    pub fn advance(&mut self, now: Instant) {
        let t = self.ms(now);
        self.k1.advance(t);
        self.k2.advance(t);
        self.combined.advance(t);
    }

    /// Mods are only known once the play frame says so; keep the latest
    pub fn update_beatmap(&mut self, beatmap: BeatmapRef) {
        self.beatmap = beatmap;
    }

    pub fn snapshot(&self) -> AttemptStats {
        AttemptStats {
            started_at: self.started_at,
            ended_at: None,
            beatmap: self.beatmap.clone(),
            k1: self.k1.stats(),
            k2: self.k2.stats(),
            combined: self.combined.stats(),
            song: self.song,
        }
    }

    /// The final record, with an end time. Nobody is tapping any more, so
    /// the live rate is 0.
    pub fn finish(&self, ended_at: DateTime<Utc>) -> AttemptStats {
        let mut stats = self.snapshot();
        stats.ended_at = Some(ended_at);
        for c in [&mut stats.k1, &mut stats.k2, &mut stats.combined] {
            c.current_ppm = Some(0.0);
        }
        stats
    }
}

// ---- Pad data sources --------------------------------------------------------------

fn ppm_value(v: Option<f64>) -> SourceValue {
    v.filter(|p| p.is_finite())
        .map_or(SourceValue::Clear, |p| SourceValue::Number(p.round()))
}

/// The `play.*ppm*` sources for an attempt (live or just ended), or all
/// cleared for None
pub fn attempt_ui_values(stats: Option<&AttemptStats>) -> Vec<(u8, SourceValue)> {
    let c = |ch: TapChannel| stats.map(|s| s.channel(ch).clone()).unwrap_or_default();
    let (k1, k2, all) = (
        c(TapChannel::K1),
        c(TapChannel::K2),
        c(TapChannel::Combined),
    );
    vec![
        (src::PLAY_PPM, ppm_value(all.current_ppm)),
        (src::PLAY_PPM_AVG, ppm_value(all.average_ppm)),
        (src::PLAY_PPM_PEAK, ppm_value(all.peak_ppm)),
        (src::PLAY_K1_PPM, ppm_value(k1.current_ppm)),
        (src::PLAY_K1_PPM_AVG, ppm_value(k1.average_ppm)),
        (src::PLAY_K1_PPM_PEAK, ppm_value(k1.peak_ppm)),
        (src::PLAY_K2_PPM, ppm_value(k2.current_ppm)),
        (src::PLAY_K2_PPM_AVG, ppm_value(k2.average_ppm)),
        (src::PLAY_K2_PPM_PEAK, ppm_value(k2.peak_ppm)),
        (
            src::PLAY_PPM_SONG,
            match stats {
                // 0 until there is enough song to say anything
                Some(s) => ppm_value(Some(s.song.ppm().unwrap_or(0.0))),
                None => SourceValue::Clear,
            },
        ),
        // Empty ("-" on the pad) until 10 s of song have been played
        (
            src::PLAY_PPM_SONG_PEAK,
            ppm_value(stats.and_then(|s| s.song.peak_ppm)),
        ),
    ]
}

/// The `history.*` sources. The pad shows the best peak as "peak".
pub fn history_ui_values(history: Option<&TapHistory>) -> Vec<(u8, SourceValue)> {
    let c = |ch: TapChannel| history.map(|h| h.channel(ch).clone()).unwrap_or_default();
    let (k1, k2, all) = (
        c(TapChannel::K1),
        c(TapChannel::K2),
        c(TapChannel::Combined),
    );
    vec![
        (src::HISTORY_PPM_AVG, ppm_value(all.average_ppm)),
        (src::HISTORY_PPM_PEAK, ppm_value(all.best_peak_ppm)),
        (src::HISTORY_K1_PPM_AVG, ppm_value(k1.average_ppm)),
        (src::HISTORY_K1_PPM_PEAK, ppm_value(k1.best_peak_ppm)),
        (src::HISTORY_K2_PPM_AVG, ppm_value(k2.average_ppm)),
        (src::HISTORY_K2_PPM_PEAK, ppm_value(k2.best_peak_ppm)),
        (
            src::HISTORY_PPM_SONG,
            ppm_value(history.and_then(|h| h.song_ppm)),
        ),
        (
            src::HISTORY_PPM_SONG_PEAK,
            ppm_value(history.and_then(|h| h.best_song_peak_ppm)),
        ),
        (
            src::HISTORY_PERIOD,
            match history {
                Some(h) => SourceValue::Text(history_period_label(h.period_days).to_uppercase()),
                None => SourceValue::Clear,
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn approx(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < 0.5)
    }

    fn tapped(intervals: &[f64]) -> ChannelTracker {
        let mut t = ChannelTracker::default();
        let mut at = 0.0;
        t.press(at);
        for i in intervals {
            at += i;
            t.press(at);
        }
        t
    }

    #[test]
    fn single_tapping_every_300_ms_is_200_ppm() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        for i in 0..20u64 {
            a.presses(1, 0, origin + Duration::from_millis(300 * i));
        }
        let s = a.snapshot();
        assert!(approx(s.k1.current_ppm, 200.0));
        assert!(approx(s.k1.average_ppm, 200.0));
        assert!(approx(s.k1.peak_ppm, 200.0));
        assert!(approx(s.combined.current_ppm, 200.0));
        assert_eq!(s.k2.presses, 0);
        // A key nobody pressed is at 0, not missing
        assert_eq!(s.k2.current_ppm, Some(0.0));
    }

    #[test]
    fn alternating_is_not_doubled() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        for i in 0..20u64 {
            let at = origin + Duration::from_millis(300 * i);
            if i % 2 == 0 {
                a.presses(1, 0, at);
            } else {
                a.presses(0, 1, at);
            }
        }
        let s = a.snapshot();
        assert!(approx(s.combined.current_ppm, 200.0), "{:?}", s.combined);
        assert!(approx(s.combined.average_ppm, 200.0));
        assert!(approx(s.combined.peak_ppm, 200.0));
        // Each key alone presses every 600 ms
        assert!(approx(s.k1.average_ppm, 100.0));
        assert!(approx(s.k2.average_ppm, 100.0));
        assert_eq!(s.combined.presses, 20);
    }

    #[test]
    fn keys_are_measured_independently() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        // K1 every 200 ms, K2 every 400 ms, offset so they never coincide
        for i in 0..30u64 {
            a.presses(1, 0, origin + Duration::from_millis(200 * i));
        }
        for i in 0..15u64 {
            a.presses(0, 1, origin + Duration::from_millis(400 * i + 100));
        }
        let s = a.snapshot();
        assert!(approx(s.k1.average_ppm, 300.0));
        assert!(approx(s.k2.average_ppm, 150.0));
    }

    #[test]
    fn a_long_pause_ends_the_sequence_and_stays_out_of_the_average() {
        let mut t = tapped(&[300.0; 10]);
        assert!(approx(t.stats().current_ppm, 200.0));
        // Silence: the rate is 0 once the break has passed
        t.advance(3000.0 + SEQUENCE_BREAK_MS + 1.0);
        assert_eq!(t.stats().current_ppm, Some(0.0));
        // Then a new sequence at the same speed, 2 s after the last press
        let mut at = 3000.0 + 2000.0;
        for _ in 0..10 {
            t.press(at);
            at += 300.0;
        }
        let s = t.stats();
        assert!(approx(s.average_ppm, 200.0), "{:?}", s);
        assert_eq!(s.valid_intervals, 19);
        assert_eq!(s.presses, 21);
    }

    #[test]
    fn uneven_mashing_is_measured_in_full() {
        // Both keys mashed: K1 and K2 20 ms apart, a pair every 115 ms, so
        // 2 presses per 115 ms: 1043 PPM. Dropping the 20 ms gaps read 522.
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        for i in 0..60u64 {
            a.presses(1, 0, origin + Duration::from_millis(115 * i));
            a.presses(0, 1, origin + Duration::from_millis(115 * i + 20));
        }
        let s = a.snapshot();
        assert!(approx(s.combined.peak_ppm, 1043.5), "{:?}", s.combined);
        assert!((s.combined.average_ppm.unwrap() - 1043.5).abs() < 10.0);
        assert!(approx(s.combined.current_ppm, 1043.5));
        // Each key on its own presses every 115 ms
        assert!(approx(s.k1.peak_ppm, 521.7));
    }

    #[test]
    fn one_stray_press_moves_the_peak_only_a_little() {
        // A press splitting one 300 ms interval into 40 + 260 ms: one more
        // press over the window's 1.5 s (240 PPM), not the 1500 PPM that 40 ms
        // alone would mean
        let t = tapped(&[300.0, 300.0, 300.0, 300.0, 300.0, 40.0, 260.0, 300.0, 300.0]);
        let peak = t.stats().peak_ppm.unwrap();
        assert!(peak <= 240.01, "peak {peak}");
    }

    #[test]
    fn the_peak_needs_a_sustained_window() {
        // Three fast intervals are shown as current but are not a peak
        let t = tapped(&[100.0, 100.0, 100.0]);
        assert!(approx(t.stats().current_ppm, 600.0));
        assert_eq!(t.stats().peak_ppm, None);
    }

    #[test]
    fn the_live_rate_falls_as_soon_as_tapping_slows() {
        // 162 ms taps: 370 PPM
        let mut t = tapped(&[162.0; 8]);
        let last = 162.0 * 8.0;
        assert!(approx(t.stats().current_ppm, 370.4));
        // A gap like a 1/1 note is not stopping: the value holds steady
        t.advance(last + 500.0);
        assert!(approx(t.stats().current_ppm, 370.4));
        // An 800 ms pause is: it reads what the gap means
        t.advance(last + 800.0);
        assert!(approx(t.stats().current_ppm, 75.0));
        // The peak is what was sustained, not touched by the pause
        assert!(approx(t.stats().peak_ppm, 370.4));
        // Tapping again: the pause is one of the window's intervals until
        // enough presses push it out, so the rate climbs back
        let mut at = last + 900.0;
        t.press(at);
        assert!(approx(t.stats().current_ppm, 210.5));
        for _ in 0..6 {
            at += 162.0;
            t.press(at);
        }
        assert!(approx(t.stats().current_ppm, 370.4));
    }

    #[test]
    fn the_live_rate_is_shown_from_the_second_press() {
        let mut t = ChannelTracker::default();
        assert_eq!(t.stats().current_ppm, Some(0.0));
        t.press(0.0);
        assert_eq!(t.stats().current_ppm, Some(0.0));
        t.press(300.0);
        assert!(approx(t.stats().current_ppm, 200.0));
    }

    #[test]
    fn simultaneous_presses_all_count() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        a.presses(1, 1, origin);
        a.presses(1, 1, origin + Duration::from_millis(300));
        let s = a.snapshot();
        // 4 presses in 300 ms: 3 intervals (two of them 0 ms) over 300 ms
        assert_eq!(s.combined.presses, 4);
        assert_eq!(s.combined.valid_intervals, 3);
        assert!(approx(s.combined.average_ppm, 600.0));
        // K1 alone: 2 presses 300 ms apart
        assert!(approx(s.k1.average_ppm, 200.0));
    }

    #[test]
    fn finished_attempts_read_zero_now() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        for i in 0..20u64 {
            a.presses(1, 0, origin + Duration::from_millis(300 * i));
        }
        let f = a.finish(Utc::now());
        assert_eq!(f.combined.current_ppm, Some(0.0));
        assert!(f.ended_at.is_some());
        assert!(f.is_recordable());
    }

    fn clock(live_ms: f64, rate: f64) -> SongClock {
        SongClock {
            live_ms,
            first_object_ms: 10_000.0,
            last_object_ms: 100_000.0,
            rate,
        }
    }

    #[test]
    fn the_song_rate_counts_breaks_but_not_the_intro() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        // A press during the intro does not count towards the song rate
        a.song_clock(clock(2000.0, 1.0), origin);
        a.presses(1, 0, origin);
        // 10 s of song from the first note: 20 presses in 5 s, then a 5 s break
        a.song_clock(clock(10_000.0, 1.0), origin);
        for i in 0..20u64 {
            a.presses(1, 0, origin + Duration::from_millis(2000 + 250 * i));
        }
        a.song_clock(clock(20_000.0, 1.0), origin);
        let s = a.snapshot();
        assert_eq!(s.song.presses, 20);
        // 20 presses over 10 s, not over the 5 s of tapping
        assert!(approx(s.song.ppm(), 120.0), "{:?}", s.song);
        // The tapping average ignores the break
        assert!(approx(s.combined.average_ppm, 240.0));
    }

    #[test]
    fn the_song_rate_is_in_real_time_and_stops_at_the_last_note() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        // DT: 30 s of song are 20 s of real time
        a.song_clock(clock(10_000.0, 1.5), origin);
        a.presses(40, 0, origin);
        a.song_clock(clock(40_000.0, 1.5), origin);
        assert!(approx(a.snapshot().song.ppm(), 120.0));
        // After the last note neither time nor presses count
        a.song_clock(clock(105_000.0, 1.5), origin); // well past the last note
        a.presses(10, 0, origin);
        let s = a.snapshot();
        assert_eq!(s.song.presses, 40);
        assert!((s.song.ms - 60_000.0).abs() < 1.0);
    }

    #[test]
    fn the_song_rate_reads_zero_until_there_is_enough_song() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        a.song_clock(clock(10_100.0, 1.0), origin);
        a.presses(1, 0, origin);
        assert_eq!(a.snapshot().song.ppm(), None);
        let values = attempt_ui_values(Some(&a.snapshot()));
        assert!(values.contains(&(src::PLAY_PPM_SONG, SourceValue::Number(0.0))));
    }

    /// Plays `presses` presses `gap_ms` apart from song time `from_ms`, with the
    /// song clock moving along (one clock per press, as tosu's frames would)
    fn play_song(a: &mut AttemptTracker, origin: Instant, from_ms: f64, gap_ms: f64, presses: u32) {
        for i in 0..presses {
            let song_ms = from_ms + gap_ms * f64::from(i);
            let at = origin + Duration::from_secs_f64(song_ms / 1000.0);
            a.song_clock(clock(10_000.0 + song_ms, 1.0), at);
            a.presses(1, 0, at);
        }
    }

    #[test]
    fn the_song_peak_is_the_best_ten_seconds_not_one_burst() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        // A 0.5 s burst at 1200 PPM right at the start, then 300 PPM
        play_song(&mut a, origin, 0.0, 50.0, 10);
        play_song(&mut a, origin, 500.0, 200.0, 50);
        // then the hard part: 15 s at 600 PPM
        play_song(&mut a, origin, 10_500.0, 100.0, 150);
        // and an easy end: 10 s at 150 PPM
        play_song(&mut a, origin, 25_500.0, 400.0, 25);
        let s = a.snapshot();
        let peak = s.song.peak_ppm.unwrap();
        assert!((peak - 600.0).abs() <= 6.0, "peak {peak}");
        // The burst set the per-channel (6-press) peak, not the song peak
        assert!(s.combined.peak_ppm.unwrap() > 1000.0);
        // and the song peak is never below the song rate
        assert!(peak >= s.song.ppm().unwrap());
    }

    #[test]
    fn peak_is_the_fastest_burst_of_an_easy_song() {
        // 300 PPM all song long, with one 8-press burst at 720 PPM in the middle
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        play_song(&mut a, origin, 0.0, 200.0, 50);
        play_song(&mut a, origin, 10_000.0, 1000.0 / 12.0, 8);
        play_song(
            &mut a,
            origin,
            10_000.0 + 7.0 * 1000.0 / 12.0 + 200.0,
            200.0,
            50,
        );
        let s = a.snapshot();
        let peak = s.combined.peak_ppm.unwrap();
        assert!((peak - 720.0).abs() < 1.0, "peak {peak}");
        // The song average and the best 10 s barely notice it
        assert!(s.song.ppm().unwrap() < 330.0);
        assert!(s.song.peak_ppm.unwrap() < 340.0);
    }

    #[test]
    fn there_is_no_song_peak_before_ten_seconds_of_song() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        play_song(&mut a, origin, 0.0, 100.0, 90);
        assert_eq!(a.snapshot().song.peak_ppm, None);
        assert!(attempt_ui_values(Some(&a.snapshot()))
            .contains(&(src::PLAY_PPM_SONG_PEAK, SourceValue::Clear)));
        play_song(&mut a, origin, 9_000.0, 100.0, 20);
        assert!(a.snapshot().song.peak_ppm.is_some());
    }

    #[test]
    fn the_song_rate_waits_five_seconds_before_it_shows() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        play_song(&mut a, origin, 0.0, 100.0, 40);
        assert_eq!(a.snapshot().song.ppm(), None);
        play_song(&mut a, origin, 4_000.0, 100.0, 20);
        assert!(a.snapshot().song.ppm().is_some());
    }

    #[test]
    fn ui_values_round_and_clear() {
        let values = attempt_ui_values(None);
        assert_eq!(values.len(), 11);
        assert!(values.iter().all(|(_, v)| *v == SourceValue::Clear));
        let h = TapHistory {
            period_days: 30,
            combined: ChannelHistory {
                average_ppm: Some(193.6),
                best_peak_ppm: Some(251.2),
                ..Default::default()
            },
            ..Default::default()
        };
        let values = history_ui_values(Some(&h));
        assert!(values.contains(&(src::HISTORY_PPM_AVG, SourceValue::Number(194.0))));
        assert!(values.contains(&(src::HISTORY_PPM_PEAK, SourceValue::Number(251.0))));
        assert!(values.contains(&(
            src::HISTORY_PERIOD,
            SourceValue::Text("LAST 30 DAYS".into())
        )));
        assert!(values.contains(&(src::HISTORY_K1_PPM_AVG, SourceValue::Clear)));
    }
}
