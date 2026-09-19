//! Check scheduling (§U-0.6).
//!
//! Once a day, with ETag caching. The unauthenticated GitHub API allows 60
//! requests an hour per IP; a naive poll multiplied across users looks like
//! abuse and gets the whole product rate-limited. Failures back off
//! exponentially so an outage does not turn into a retry storm either.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const FIRST_BACKOFF: Duration = Duration::from_secs(15 * 60);
const MAX_BACKOFF: Duration = Duration::from_secs(6 * 60 * 60);

/// Serialisable so the daily cadence survives a restart. A daemon that is
/// restarted often must not check on every launch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckSchedule {
    #[serde(default)]
    pub last_check: Option<SystemTime>,
    /// Last *successful* check, which is what the GUI shows (§U-0.4)
    #[serde(default)]
    pub last_success: Option<SystemTime>,
    /// Sent as `If-None-Match`; a 304 costs no rate-limit budget
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl CheckSchedule {
    /// How long to wait after the last attempt before trying again
    pub fn interval(&self) -> Duration {
        if self.consecutive_failures == 0 {
            return DEFAULT_INTERVAL;
        }
        let shift = (self.consecutive_failures - 1).min(8);
        FIRST_BACKOFF.saturating_mul(1u32 << shift).min(MAX_BACKOFF)
    }

    /// A check that has never run is due immediately. A clock that moved
    /// backwards — a VM snapshot, a timezone fix — also makes it due, rather
    /// than blocking updates until the clock catches up.
    pub fn due_at(&self, now: SystemTime) -> bool {
        match self.last_check {
            None => true,
            Some(last) => match now.duration_since(last) {
                Ok(elapsed) => elapsed >= self.interval(),
                Err(_) => true,
            },
        }
    }

    pub fn record_success(&mut self, now: SystemTime, etag: Option<String>) {
        self.last_check = Some(now);
        self.last_success = Some(now);
        self.consecutive_failures = 0;
        self.last_error = None;
        // A 304 carries no ETag; keep the one we already have.
        if etag.is_some() {
            self.etag = etag;
        }
    }

    pub fn record_failure(&mut self, now: SystemTime, error: impl Into<String>) {
        self.last_check = Some(now);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_error = Some(error.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn a_first_run_checks_immediately_then_waits_a_day() {
        let mut s = CheckSchedule::default();
        assert!(s.due_at(at(0)));

        s.record_success(at(0), Some("\"abc\"".into()));
        assert!(!s.due_at(at(60)));
        assert!(!s.due_at(at(23 * 3600)));
        assert!(s.due_at(at(24 * 3600)));
    }

    #[test]
    fn failures_back_off_and_stay_bounded() {
        let mut s = CheckSchedule::default();
        s.record_failure(at(0), "offline");
        assert_eq!(s.interval(), FIRST_BACKOFF);
        s.record_failure(at(0), "offline");
        assert_eq!(s.interval(), FIRST_BACKOFF * 2);

        for _ in 0..50 {
            s.record_failure(at(0), "offline");
        }
        assert_eq!(s.interval(), MAX_BACKOFF, "backoff must not grow forever");
        assert!(
            s.interval() < DEFAULT_INTERVAL,
            "retrying sooner than a day"
        );

        // One success clears the penalty and the recorded error
        s.record_success(at(0), None);
        assert_eq!(s.interval(), DEFAULT_INTERVAL);
        assert_eq!(s.consecutive_failures, 0);
        assert!(s.last_error.is_none());
    }

    #[test]
    fn a_304_keeps_the_etag_it_was_matched_against() {
        let mut s = CheckSchedule::default();
        s.record_success(at(0), Some("\"v1\"".into()));
        s.record_success(at(1), None);
        assert_eq!(s.etag.as_deref(), Some("\"v1\""));
    }

    #[test]
    fn a_failed_check_still_counts_as_an_attempt() {
        // Otherwise an offline machine retries in a tight loop forever.
        let mut s = CheckSchedule::default();
        s.record_failure(at(1000), "dns");
        assert!(!s.due_at(at(1001)));
        assert!(s.due_at(at(1000 + FIRST_BACKOFF.as_secs())));
    }

    #[test]
    fn a_clock_that_moved_backwards_does_not_block_updates_forever() {
        let mut s = CheckSchedule::default();
        s.record_success(at(10_000), None);
        assert!(s.due_at(at(5_000)));
    }

    #[test]
    fn the_schedule_survives_a_restart() {
        let mut s = CheckSchedule::default();
        s.record_success(at(1000), Some("\"e\"".into()));
        let round_tripped: CheckSchedule =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(round_tripped, s);
        assert!(!round_tripped.due_at(at(2000)));
    }
}
