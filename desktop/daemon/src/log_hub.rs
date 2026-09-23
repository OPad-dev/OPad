use opad_model::{LogEntry, LogLevel, LogSource};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const LOG_HUB_CAPACITY: usize = 2000;

#[derive(Debug)]
struct LogHubInner {
    entries: VecDeque<LogEntry>,
    next_seq: u64,
}

#[derive(Debug, Clone)]
pub struct LogHub {
    inner: Arc<Mutex<LogHubInner>>,
}

impl Default for LogHub {
    fn default() -> Self {
        Self::new()
    }
}

impl LogHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogHubInner {
                entries: VecDeque::with_capacity(LOG_HUB_CAPACITY),
                next_seq: 1,
            })),
        }
    }

    pub fn push(
        &self,
        source: LogSource,
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
    ) {
        let mut inner = self.inner.lock();
        let seq = inner.next_seq;
        inner.next_seq = inner.next_seq.wrapping_add(1);

        if inner.entries.len() >= LOG_HUB_CAPACITY {
            inner.entries.pop_front();
        }

        let cutoff = chrono::Local::now() - chrono::Duration::hours(24);
        inner.entries.retain(|e| e.ts >= cutoff);

        let entry = LogEntry::new(seq, source, level, target, message);
        inner.entries.push_back(entry);
    }

    pub fn get_entries(&self, since_seq: Option<u64>, limit: usize) -> (Vec<LogEntry>, u64) {
        self.get_matching_entries(since_seq, limit, None, None)
    }

    /// [`Self::get_entries`] restricted to `level` and above and to `source`.
    /// The filter applies before `limit`, so a filtered request still gets up
    /// to `limit` matching entries rather than whatever matched among the
    /// last `limit`.
    pub fn get_matching_entries(
        &self,
        since_seq: Option<u64>,
        limit: usize,
        level: Option<LogLevel>,
        source: Option<LogSource>,
    ) -> (Vec<LogEntry>, u64) {
        let inner = self.inner.lock();
        let latest_seq = inner.next_seq.saturating_sub(1);
        let matches = |e: &&LogEntry| {
            level.is_none_or(|l| e.level >= l) && source.is_none_or(|s| e.source == s)
        };

        let entries = match since_seq {
            Some(since) => inner
                .entries
                .iter()
                .filter(|e| e.seq > since)
                .filter(matches)
                .take(limit)
                .cloned()
                .collect(),
            None => {
                let matching: Vec<&LogEntry> = inner.entries.iter().filter(matches).collect();
                let start = matching.len().saturating_sub(limit);
                matching[start..].iter().map(|e| (*e).clone()).collect()
            }
        };

        (entries, latest_seq)
    }
}

pub struct LogHubLayer {
    hub: LogHub,
}

impl LogHubLayer {
    pub fn new(hub: LogHub) -> Self {
        Self { hub }
    }
}

impl<S> Layer<S> for LogHubLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let level = match *meta.level() {
            tracing::Level::ERROR => LogLevel::Error,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::TRACE => return,
        };

        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);

        self.hub
            .push(LogSource::Host, level, meta.target(), visitor.message);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let s = format!("{:?}", value);
            self.message = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                s[1..s.len() - 1].to_string()
            } else {
                s
            };
        } else if !self.message.is_empty() {
            use std::fmt::Write;
            let _ = write!(self.message, " {}={:?}", field.name(), value);
        } else {
            self.message = format!("{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else if !self.message.is_empty() {
            use std::fmt::Write;
            let _ = write!(self.message, " {}={}", field.name(), value);
        } else {
            self.message = format!("{}={}", field.name(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_hub_ring_buffer_and_since_seq() {
        let hub = LogHub::new();
        for i in 1..=10 {
            hub.push(
                LogSource::Host,
                LogLevel::Info,
                "test",
                format!("msg {}", i),
            );
        }

        let (all, latest) = hub.get_entries(None, 100);
        assert_eq!(all.len(), 10);
        assert_eq!(latest, 10);
        assert_eq!(all[0].seq, 1);
        assert_eq!(all[9].seq, 10);

        let (recent, _) = hub.get_entries(Some(7), 100);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].seq, 8);
        assert_eq!(recent[2].seq, 10);

        let (limited, _) = hub.get_entries(None, 5);
        assert_eq!(limited.len(), 5);
        assert_eq!(limited[0].seq, 6);
        assert_eq!(limited[4].seq, 10);
    }

    /// DC#6: the filter applies before `limit`, not after it
    #[test]
    fn filters_apply_before_the_limit() {
        let hub = LogHub::new();
        for i in 1..=100 {
            let (source, level) = if i % 25 == 0 {
                (LogSource::Esp, LogLevel::Error)
            } else {
                (LogSource::Host, LogLevel::Info)
            };
            hub.push(source, level, "test", format!("msg {}", i));
        }

        // The four errors are all older than the last 10 entries' window
        let (errors, latest) = hub.get_matching_entries(None, 10, Some(LogLevel::Error), None);
        assert_eq!(latest, 100);
        let seqs: Vec<u64> = errors.iter().map(|e| e.seq).collect();
        assert_eq!(seqs, [25, 50, 75, 100]);

        // Newest matching entries when capped, oldest first after a cursor
        let (last_two, _) = hub.get_matching_entries(None, 2, None, Some(LogSource::Esp));
        assert_eq!(
            last_two.iter().map(|e| e.seq).collect::<Vec<_>>(),
            [75, 100]
        );
        let (after, _) = hub.get_matching_entries(Some(30), 2, Some(LogLevel::Warn), None);
        assert_eq!(after.iter().map(|e| e.seq).collect::<Vec<_>>(), [50, 75]);

        // Level is a floor
        let (info_up, _) = hub.get_matching_entries(Some(95), 100, Some(LogLevel::Info), None);
        assert_eq!(info_up.len(), 5);
    }
}
