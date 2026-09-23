use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[serde(rename_all = "UPPERCASE")]
pub enum LogSource {
    #[cfg_attr(feature = "clap", value(alias = "daemon"))]
    Host,
    #[cfg_attr(feature = "clap", value(alias = "device"))]
    Esp,
    #[cfg_attr(feature = "clap", value(alias = "app", alias = "gui"))]
    Program,
    Tosu,
}

impl std::fmt::Display for LogSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Host => "DAEMON",
            Self::Esp => "DEVICE",
            Self::Program => "PROGRAM",
            Self::Tosu => "TOSU",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    Debug,
    Info,
    #[cfg_attr(feature = "clap", value(alias = "warning"))]
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub ts: DateTime<Local>,
    pub source: LogSource,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
}

impl LogEntry {
    pub fn new(
        seq: u64,
        source: LogSource,
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            seq,
            ts: Local::now(),
            source,
            level,
            target: target.into(),
            message: message.into(),
        }
    }

    /// Formats line per spec §24.3: HH:MM:SS SOURCE LEVEL message
    pub fn format_line(&self) -> String {
        format!(
            "{} {:<7} {:<5} {}",
            self.ts.format("%H:%M:%S"),
            self.source,
            self.level,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_source_padding() {
        assert_eq!(format!("[{:<7}]", LogSource::Tosu), "[TOSU   ]");
        assert_eq!(format!("[{:<7}]", LogSource::Host), "[DAEMON ]");
        assert_eq!(format!("[{:<7}]", LogSource::Esp), "[DEVICE ]");
        assert_eq!(format!("[{:<7}]", LogSource::Program), "[PROGRAM]");
    }

    #[test]
    fn test_log_level_padding() {
        assert_eq!(format!("[{:<5}]", LogLevel::Debug), "[DEBUG]");
        assert_eq!(format!("[{:<5}]", LogLevel::Info), "[INFO ]");
        assert_eq!(format!("[{:<5}]", LogLevel::Warn), "[WARN ]");
        assert_eq!(format!("[{:<5}]", LogLevel::Error), "[ERROR]");
    }
}
