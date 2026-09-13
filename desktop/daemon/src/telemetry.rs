//! Keeps the pad's UI data in sync with tosu while sending as little as possible.

use osupad_model::ui_source::SourceValue;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Live values change constantly while playing; cap how often they go over USB
#[allow(dead_code)]
pub const DEFAULT_FLUSH_INTERVAL_PLAYING: Duration = Duration::from_millis(100);
#[allow(dead_code)]
pub const FLUSH_INTERVAL_PLAYING: Duration = DEFAULT_FLUSH_INTERVAL_PLAYING;
pub const FLUSH_INTERVAL_IDLE: Duration = Duration::from_millis(500);

/// Calculates interval between flushes based on configured Hz (clamped to 1..=30 Hz, §P2-7)
pub fn flush_interval_playing(hz: u32) -> Duration {
    let clamped_hz = hz.clamp(1, 30);
    Duration::from_millis(1000 / clamped_hz as u64)
}

#[derive(Default)]
pub struct DataSync {
    latest: HashMap<u8, SourceValue>,
    sent: HashMap<u8, SourceValue>,
    last_flush: Option<Instant>,
}

impl DataSync {
    /// Record the newest values from a tosu frame
    pub fn ingest(&mut self, values: impl IntoIterator<Item = (u8, SourceValue)>) {
        self.latest.extend(values);
    }

    /// Changed values if the flush interval elapsed (or `force`); marks them as sent
    pub fn take_changes(
        &mut self,
        playing: bool,
        playing_hz: u32,
        force: bool,
    ) -> Vec<(u8, SourceValue)> {
        let interval = if playing {
            flush_interval_playing(playing_hz)
        } else {
            FLUSH_INTERVAL_IDLE
        };
        if !force && self.last_flush.is_some_and(|t| t.elapsed() < interval) {
            return Vec::new();
        }
        self.last_flush = Some(Instant::now());

        let mut changes: Vec<(u8, SourceValue)> = self
            .latest
            .iter()
            .filter(|(source, value)| self.sent.get(*source) != Some(*value))
            .map(|(source, value)| (*source, value.clone()))
            .collect();
        changes.sort_by_key(|(source, _)| *source);
        for (source, value) in &changes {
            self.sent.insert(*source, value.clone());
        }
        changes
    }

    /// The device lost its state (reconnect/reboot): resend everything on the next flush
    pub fn reset_sent(&mut self) {
        self.sent.clear();
    }

    /// tosu went away: the device clears its tosu values itself
    pub fn clear(&mut self) {
        self.latest.clear();
        self.sent.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flush_interval_calculation() {
        assert_eq!(flush_interval_playing(10), Duration::from_millis(100));
        assert_eq!(flush_interval_playing(5), Duration::from_millis(200));
        assert_eq!(flush_interval_playing(2), Duration::from_millis(500));
        assert_eq!(flush_interval_playing(20), Duration::from_millis(50));
        assert_eq!(flush_interval_playing(0), Duration::from_millis(1000)); // clamped to 1
        assert_eq!(flush_interval_playing(60), Duration::from_millis(33)); // clamped to 30
    }

    #[test]
    fn only_changes_are_sent() {
        let mut sync = DataSync::default();
        sync.ingest([
            (1, SourceValue::Text("a".into())),
            (20, SourceValue::Number(1.0)),
        ]);
        assert_eq!(sync.take_changes(true, 10, true).len(), 2);

        sync.ingest([
            (1, SourceValue::Text("a".into())),
            (20, SourceValue::Number(2.0)),
        ]);
        assert_eq!(
            sync.take_changes(true, 10, true),
            vec![(20, SourceValue::Number(2.0))]
        );

        sync.reset_sent();
        assert_eq!(sync.take_changes(true, 10, true).len(), 2);
    }

    #[test]
    fn flushes_are_rate_limited() {
        let mut sync = DataSync::default();
        sync.ingest([(20, SourceValue::Number(1.0))]);
        assert_eq!(sync.take_changes(true, 10, false).len(), 1);
        sync.ingest([(20, SourceValue::Number(2.0))]);
        assert!(sync.take_changes(true, 10, false).is_empty());
    }
}
