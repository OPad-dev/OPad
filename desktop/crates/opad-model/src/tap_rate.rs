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
//! - An interval shorter than [`MIN_INTERVAL_MS`] is a duplicate event, switch
//!   bounce or two presses landing in one telemetry sample. The second press
//!   still counts as a press, but the interval is dropped and the next one is
//!   measured from the first press.
//! - Current PPM is the rolling rate over the last [`WINDOW_INTERVALS`] valid
//!   intervals of the sequence. It is shown from [`MIN_DISPLAY_INTERVALS`] on.
//! - Peak PPM is the highest Current PPM over a *full* window, and a full
//!   window drops its shortest and longest interval before averaging, so one
//!   stray interval can move neither the peak nor the displayed value much.
//! - Average PPM is valid presses over valid tapping time: the sum of valid
//!   intervals, not the length of the map.

use crate::ui_source::{self as src, SourceValue};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

/// Valid intervals in the rolling window behind Current and Peak PPM
pub const WINDOW_INTERVALS: usize = 6;
/// Current PPM is shown once a sequence has this many valid intervals
pub const MIN_DISPLAY_INTERVALS: usize = 3;
/// A gap longer than this ends the tapping sequence (60 PPM on one channel)
pub const SEQUENCE_BREAK_MS: f64 = 1000.0;
/// Shorter intervals are treated as duplicates or bounce (2400 PPM). The
/// fastest human bursts on one key are well above this.
pub const MIN_INTERVAL_MS: f64 = 25.0;
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
    window: VecDeque<f64>,
    current_ppm: Option<f64>,
    peak_ppm: Option<f64>,
    presses: u32,
    valid_intervals: u32,
    interval_ms_sum: f64,
}

impl ChannelTracker {
    pub fn press(&mut self, at_ms: f64) {
        self.presses = self.presses.saturating_add(1);
        let Some(prev) = self.last_press_ms else {
            self.last_press_ms = Some(at_ms);
            return;
        };
        let interval = at_ms - prev;
        if interval < MIN_INTERVAL_MS {
            // Measured from the first of the pair instead
            return;
        }
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
        self.current_ppm = window_ppm(&self.window);
        if self.window.len() == WINDOW_INTERVALS {
            if let Some(ppm) = self.current_ppm {
                self.peak_ppm = Some(self.peak_ppm.map_or(ppm, |p| p.max(ppm)));
            }
        }
    }

    /// Ends the sequence once no press has come for [`SEQUENCE_BREAK_MS`]
    pub fn expire(&mut self, now_ms: f64) {
        if self
            .last_press_ms
            .is_some_and(|last| now_ms - last > SEQUENCE_BREAK_MS)
        {
            self.end_sequence();
        }
    }

    fn end_sequence(&mut self) {
        self.window.clear();
        self.current_ppm = None;
    }

    pub fn stats(&self) -> ChannelStats {
        ChannelStats {
            current_ppm: self.current_ppm,
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

/// Current PPM from the window: the plain mean interval until it is full,
/// then the mean without its shortest and longest interval
fn window_ppm(window: &VecDeque<f64>) -> Option<f64> {
    if window.len() < MIN_DISPLAY_INTERVALS {
        return None;
    }
    let sum: f64 = window.iter().sum();
    let mean = if window.len() == WINDOW_INTERVALS {
        let min = window.iter().copied().fold(f64::INFINITY, f64::min);
        let max = window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (sum - min - max) / (window.len() - 2) as f64
    } else {
        sum / window.len() as f64
    };
    (mean > 0.0).then(|| ppm_from_interval_ms(mean))
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
        }
    }

    fn ms(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.origin).as_secs_f64() * 1000.0
    }

    /// `k1` and `k2` new key-downs seen at `at`. Several in one sample share
    /// its timestamp, so all but the first are counted without an interval.
    pub fn presses(&mut self, k1: u32, k2: u32, at: Instant) {
        let t = self.ms(at);
        for _ in 0..k1.min(64) {
            self.k1.press(t);
            self.combined.press(t);
        }
        for _ in 0..k2.min(64) {
            self.k2.press(t);
            self.combined.press(t);
        }
    }

    pub fn expire(&mut self, now: Instant) {
        let t = self.ms(now);
        self.k1.expire(t);
        self.k2.expire(t);
        self.combined.expire(t);
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
        }
    }

    /// The final record: no current rate, an end time
    pub fn finish(&self, ended_at: DateTime<Utc>) -> AttemptStats {
        let mut stats = self.snapshot();
        stats.ended_at = Some(ended_at);
        for c in [&mut stats.k1, &mut stats.k2, &mut stats.combined] {
            c.current_ppm = None;
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
        assert_eq!(s.k2.current_ppm, None);
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
        // Silence: the rate is gone once the break has passed
        t.expire(3000.0 + SEQUENCE_BREAK_MS + 1.0);
        assert_eq!(t.stats().current_ppm, None);
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
    fn one_bounce_cannot_make_a_peak() {
        // A duplicate event 5 ms after a press
        let mut t = tapped(&[300.0; 8]);
        let last = 300.0 * 8.0;
        t.press(last + 5.0);
        let mut at = last + 300.0;
        for _ in 0..8 {
            t.press(at);
            at += 300.0;
        }
        assert!(approx(t.stats().peak_ppm, 200.0), "{:?}", t.stats());
        // A stray press splitting one 300 ms interval into 40 + 260 ms: the
        // 40 ms is trimmed from the full window, and the peak moves by a few
        // percent rather than jumping to the 1500 PPM that 40 ms would mean
        let mut t = tapped(&[300.0, 300.0, 300.0, 300.0, 300.0, 40.0, 260.0, 300.0, 300.0]);
        let peak = t.stats().peak_ppm.unwrap();
        assert!(peak < 210.0, "peak {peak}");
        t.press(10_000.0);
        assert!(t.stats().peak_ppm.unwrap() < 210.0);
    }

    #[test]
    fn the_peak_needs_a_sustained_window() {
        // Three fast intervals are shown as current but are not a peak
        let t = tapped(&[100.0, 100.0, 100.0]);
        assert!(approx(t.stats().current_ppm, 600.0));
        assert_eq!(t.stats().peak_ppm, None);
    }

    #[test]
    fn simultaneous_presses_count_but_add_no_interval() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        a.presses(1, 1, origin);
        a.presses(1, 1, origin + Duration::from_millis(300));
        let s = a.snapshot();
        assert_eq!(s.combined.presses, 4);
        assert_eq!(s.combined.valid_intervals, 1);
        assert!(approx(s.combined.average_ppm, 200.0));
    }

    #[test]
    fn finished_attempts_have_no_current_rate() {
        let origin = Instant::now();
        let mut a = AttemptTracker::new(BeatmapRef::default(), origin, Utc::now());
        for i in 0..20u64 {
            a.presses(1, 0, origin + Duration::from_millis(300 * i));
        }
        let f = a.finish(Utc::now());
        assert_eq!(f.combined.current_ppm, None);
        assert!(f.ended_at.is_some());
        assert!(f.is_recordable());
    }

    #[test]
    fn ui_values_round_and_clear() {
        let values = attempt_ui_values(None);
        assert_eq!(values.len(), 9);
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
