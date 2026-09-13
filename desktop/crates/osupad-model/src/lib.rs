use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod ui_source;
pub mod diag;

/// Key edge to HID report submit latency, measured on the device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LatencyStats {
    pub samples: u32,
    pub p50_us: u32,
    pub p99_us: u32,
    pub p999_us: u32,
    pub max_us: u32,
    /// Key changes that waited for the next USB poll (endpoint busy) and were resent
    #[serde(alias = "dropped_reports")]
    pub deferred_reports: u32,
}

/// Operational mode of the system (§11)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    #[default]
    Idle,
    Playing,
    Cooldown,
    Sync,
}

/// Source of active counters (§31)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CounterSource {
    #[default]
    Device,
    Pc,
}

/// Information when a connected device has an incompatible protocol version
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncompatibleDevice {
    pub firmware_version: String,
    pub protocol_version: u32,
}

/// Device hardware & firmware metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub board_profile: String,
    pub firmware_version: String,
    pub protocol_version: u32,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_id: "unknown".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
        }
    }
}

/// Press counters maintained on both device and host (§12, §13)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CounterState {
    pub device_id: String,
    pub counter_generation: u32,
    pub lifetime_key1: u64,
    pub lifetime_key2: u64,
    #[serde(default)]
    pub map_key1: u32,
    #[serde(default)]
    pub map_key2: u32,
}

impl CounterState {
    pub fn total_lifetime_presses(&self) -> u64 {
        self.lifetime_key1.saturating_add(self.lifetime_key2)
    }
}

/// Device configuration parameters (§37)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub key1_hid_usage: u32,       // Default: 0x1D ('Z')
    pub key2_hid_usage: u32,       // Default: 0x1B ('X')
    pub debounce_us: u32,          // Default: 3000 (3ms)
    pub brightness: u32,           // 0..100, default: 100
    pub display_sleep_seconds: u32,// Default: 600 (10 min)
    pub gameplay_display_hz: u32,  // Default: 5
    pub tosu_endpoint: String,     // Default: "ws://127.0.0.1:24050/websocket/v2"
    #[serde(default = "default_press_color_rgb")]
    pub press_color_rgb: u32,      // Gameplay screen key press highlight 0xRRGGBB, default: red
}

pub const DEFAULT_PRESS_COLOR_RGB: u32 = 0xFF0000;

fn default_press_color_rgb() -> u32 {
    DEFAULT_PRESS_COLOR_RGB
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            key1_hid_usage: 0x1D, // 'Z'
            key2_hid_usage: 0x1B, // 'X'
            debounce_us: 3000,
            brightness: 100,
            display_sleep_seconds: 600,
            gameplay_display_hz: 5,
            tosu_endpoint: "ws://127.0.0.1:24050/websocket/v2".to_string(),
            press_color_rgb: DEFAULT_PRESS_COLOR_RGB,
        }
    }
}

impl DeviceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.key1_hid_usage < 0x04 || self.key1_hid_usage > 0xE7 {
            return Err(format!("Invalid key1 HID usage: 0x{:02X}", self.key1_hid_usage));
        }
        if self.key2_hid_usage < 0x04 || self.key2_hid_usage > 0xE7 {
            return Err(format!("Invalid key2 HID usage: 0x{:02X}", self.key2_hid_usage));
        }
        if self.debounce_us < 500 || self.debounce_us > 20000 {
            return Err(format!("Debounce lockout must be between 500 and 20000 µs (got {})", self.debounce_us));
        }
        if self.brightness > 100 {
            return Err(format!("Brightness must be between 0 and 100 (got {})", self.brightness));
        }
        if self.display_sleep_seconds != 0 && (self.display_sleep_seconds < 10 || self.display_sleep_seconds > 86400) {
            return Err(format!("Display sleep must be 0 or 10..=86400 seconds (got {})", self.display_sleep_seconds));
        }
        if self.gameplay_display_hz < 1 || self.gameplay_display_hz > 30 {
            return Err(format!("Gameplay display Hz must be between 1 and 30 (got {})", self.gameplay_display_hz));
        }
        Ok(())
    }

    pub fn key1_char(&self) -> String {
        hid_usage_to_char(self.key1_hid_usage)
    }

    pub fn key2_char(&self) -> String {
        hid_usage_to_char(self.key2_hid_usage)
    }
}

/// Helper converting standard HID usage to character representation
pub fn hid_usage_to_char(usage: u32) -> String {
    match usage {
        0x04..=0x1D => {
            let ch = (b'A' + (usage - 0x04) as u8) as char;
            ch.to_string()
        }
        0x1E..=0x27 => {
            let num = (usage - 0x1E + 1) % 10;
            num.to_string()
        }
        _ => format!("0x{:02X}", usage),
    }
}

/// Helper converting string/character to HID keyboard usage
pub fn char_to_hid_usage(ch: &str) -> Option<u32> {
    let s = ch.trim().to_uppercase();
    if s.len() == 1 {
        let c = s.chars().next()?;
        if c.is_ascii_alphabetic() {
            return Some(0x04 + (c as u32 - 'A' as u32));
        }
        if c.is_ascii_digit() {
            if c == '0' {
                return Some(0x27);
            }
            return Some(0x1E + (c as u32 - '1' as u32));
        }
    }
    if s.starts_with("0X") {
        return u32::from_str_radix(&s[2..], 16).ok();
    }
    None
}

/// Live gameplay telemetry from tosu (§15)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameplayTelemetry {
    pub is_playing: bool,
    pub title: String,
    #[serde(default)]
    pub live_time_ms: f64,   // Song position; jumps back on retry
    /// Every UI data source this frame provides (see `ui_source`)
    #[serde(default)]
    pub values: Vec<(u8, ui_source::SourceValue)>,
}

/// Portable JSON Backup Format (§21)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonBackup {
    pub format_version: u32,
    pub exported_at: DateTime<Utc>,
    pub device: JsonBackupDevice,
    pub stats: JsonBackupStats,
    pub config: JsonBackupConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonBackupDevice {
    pub device_id: String,
    pub board_profile: String,
    pub counter_generation: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonBackupStats {
    pub lifetime_key1: u64,
    pub lifetime_key2: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonBackupConfig {
    pub key1: String,
    pub key2: String,
    pub debounce_us: u32,
    pub brightness: u32,
    pub display_sleep_seconds: u32,
    pub gameplay_display_hz: u32,
}

impl JsonBackup {
    pub fn new(info: &DeviceInfo, counters: &CounterState, config: &DeviceConfig) -> Self {
        Self {
            format_version: 1,
            exported_at: Utc::now(),
            device: JsonBackupDevice {
                device_id: info.device_id.clone(),
                board_profile: info.board_profile.clone(),
                counter_generation: counters.counter_generation,
            },
            stats: JsonBackupStats {
                lifetime_key1: counters.lifetime_key1,
                lifetime_key2: counters.lifetime_key2,
            },
            config: JsonBackupConfig {
                key1: config.key1_char(),
                key2: config.key2_char(),
                debounce_us: config.debounce_us,
                brightness: config.brightness,
                display_sleep_seconds: config.display_sleep_seconds,
                gameplay_display_hz: config.gameplay_display_hz,
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err(format!("Unsupported backup format version: {}", self.format_version));
        }
        if self.device.board_profile != "waveshare_esp32s3_touch_lcd_2" {
            return Err(format!("Unsupported board profile: {}", self.device.board_profile));
        }
        if self.stats.lifetime_key1 > i64::MAX as u64 {
            return Err("lifetime_key1 exceeds maximum integer size".to_string());
        }
        if self.stats.lifetime_key2 > i64::MAX as u64 {
            return Err("lifetime_key2 exceeds maximum integer size".to_string());
        }
        if self.exported_at > chrono::Utc::now() + chrono::Duration::days(1) {
            return Err("Backup timestamp is in the future".to_string());
        }
        if self.config.debounce_us < 500 || self.config.debounce_us > 20000 {
            return Err(format!("Debounce lockout must be between 500 and 20000 µs (got {})", self.config.debounce_us));
        }
        if self.config.brightness > 100 {
            return Err("Brightness must be between 0 and 100".to_string());
        }
        if self.config.display_sleep_seconds != 0 && (self.config.display_sleep_seconds < 10 || self.config.display_sleep_seconds > 86400) {
            return Err(format!("Display sleep must be 0 or 10..=86400 seconds (got {})", self.config.display_sleep_seconds));
        }
        if self.config.gameplay_display_hz < 1 || self.config.gameplay_display_hz > 30 {
            return Err(format!("Gameplay display Hz must be between 1 and 30 (got {})", self.config.gameplay_display_hz));
        }
        if char_to_hid_usage(&self.config.key1).is_none() {
            return Err(format!("Invalid key1 mapping: {}", self.config.key1));
        }
        if char_to_hid_usage(&self.config.key2).is_none() {
            return Err(format!("Invalid key2 mapping: {}", self.config.key2));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_backup() -> JsonBackup {
        JsonBackup::new(
            &DeviceInfo {
                device_id: "OSUPAD-TEST".to_string(),
                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                firmware_version: "1.0.0".to_string(),
                protocol_version: 1,
            },
            &CounterState {
                device_id: "OSUPAD-TEST".to_string(),
                counter_generation: 1,
                lifetime_key1: 500,
                lifetime_key2: 600,
                map_key1: 0,
                map_key2: 0,
            },
            &DeviceConfig::default(),
        )
    }

    #[test]
    fn test_valid_backup_passes() {
        let b = valid_backup();
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_invalid_board_profile() {
        let mut b = valid_backup();
        b.device.board_profile = "custom_board".to_string();
        assert!(b.validate().is_err());
    }

    #[test]
    fn test_debounce_range_validation() {
        let mut b = valid_backup();
        b.config.debounce_us = 499;
        assert!(b.validate().is_err());

        b.config.debounce_us = 20001;
        assert!(b.validate().is_err());

        b.config.debounce_us = 1000;
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_sleep_seconds_validation() {
        let mut b = valid_backup();
        b.config.display_sleep_seconds = 5;
        assert!(b.validate().is_err());

        b.config.display_sleep_seconds = 0;
        assert!(b.validate().is_ok());

        b.config.display_sleep_seconds = 600;
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_display_hz_validation() {
        let mut b = valid_backup();
        b.config.gameplay_display_hz = 0;
        assert!(b.validate().is_err());

        b.config.gameplay_display_hz = 31;
        assert!(b.validate().is_err());

        b.config.gameplay_display_hz = 15;
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_future_timestamp_rejected() {
        let mut b = valid_backup();
        b.exported_at = chrono::Utc::now() + chrono::Duration::days(2);
        assert!(b.validate().is_err());
    }
}
