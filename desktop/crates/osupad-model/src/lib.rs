use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod diag;
pub mod log;
pub mod paths;
pub mod ui_source;

pub use log::{LogEntry, LogLevel, LogSource};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeviceInfo {
    pub device_id: String,
    pub board_profile: String,
    pub firmware_version: String,
    pub protocol_version: u32,
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

/// A header GPIO a key switch can be wired to (switch to GND, internal pull-up)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPin {
    pub gpio: u32,
    /// Waveshare header (P1 / P2) and pin number
    pub header: &'static str,
    pub pin: u8,
}

impl std::fmt::Display for KeyPin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GPIO{} ({} pin {})", self.gpio, self.header, self.pin)
    }
}

/// Key switch pins supported on the Waveshare ESP32-S3-Touch-LCD-2, from its schematic.
/// Mirrors KEY_GPIO_ALLOWED in firmware/main/config/config_validate.c.
/// Left out: GPIO19/20 (USB D-/D+), GPIO43/44 (UART0 console), GPIO47/48 (touch and IMU I2C),
/// GPIO17 (CAM_PWDN, pulled down on the board). The rest are camera pins, free with no camera.
pub const KEY_PINS: &[KeyPin] = &[
    KeyPin {
        gpio: 2,
        header: "P1",
        pin: 1,
    },
    KeyPin {
        gpio: 4,
        header: "P1",
        pin: 2,
    },
    KeyPin {
        gpio: 6,
        header: "P1",
        pin: 3,
    },
    KeyPin {
        gpio: 7,
        header: "P1",
        pin: 9,
    },
    KeyPin {
        gpio: 8,
        header: "P1",
        pin: 8,
    },
    KeyPin {
        gpio: 9,
        header: "P2",
        pin: 12,
    },
    KeyPin {
        gpio: 10,
        header: "P1",
        pin: 10,
    },
    KeyPin {
        gpio: 11,
        header: "P2",
        pin: 9,
    },
    KeyPin {
        gpio: 12,
        header: "P2",
        pin: 10,
    },
    KeyPin {
        gpio: 13,
        header: "P2",
        pin: 8,
    },
    KeyPin {
        gpio: 14,
        header: "P2",
        pin: 11,
    },
    KeyPin {
        gpio: 15,
        header: "P2",
        pin: 7,
    },
    KeyPin {
        gpio: 16,
        header: "P1",
        pin: 4,
    },
    KeyPin {
        gpio: 18,
        header: "P1",
        pin: 6,
    },
    KeyPin {
        gpio: 21,
        header: "P1",
        pin: 7,
    },
];

pub const DEFAULT_KEY1_GPIO: u32 = 14;
pub const DEFAULT_KEY2_GPIO: u32 = 9;

pub fn key_pin(gpio: u32) -> Option<KeyPin> {
    KEY_PINS.iter().copied().find(|p| p.gpio == gpio)
}

fn default_key1_gpio() -> u32 {
    DEFAULT_KEY1_GPIO
}

fn default_key2_gpio() -> u32 {
    DEFAULT_KEY2_GPIO
}

fn validate_key_gpios(key1_gpio: u32, key2_gpio: u32) -> Result<(), String> {
    if key_pin(key1_gpio).is_none() {
        return Err(format!("Unsupported key 1 pin: GPIO{}", key1_gpio));
    }
    if key_pin(key2_gpio).is_none() {
        return Err(format!("Unsupported key 2 pin: GPIO{}", key2_gpio));
    }
    if key1_gpio == key2_gpio {
        return Err(format!("Key 1 and key 2 cannot share GPIO{}", key1_gpio));
    }
    Ok(())
}

/// Device configuration parameters (§37)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub key1_hid_usage: u32,        // Default: 0x1D ('Z')
    pub key2_hid_usage: u32,        // Default: 0x1B ('X')
    pub debounce_us: u32,           // Default: 3000 (3ms)
    pub brightness: u32,            // 0..100, default: 100
    pub display_sleep_seconds: u32, // Default: 600 (10 min)
    pub gameplay_display_hz: u32,   // Default: 10
    pub tosu_endpoint: String,      // Default: "ws://127.0.0.1:24050/websocket/v2"
    #[serde(default = "default_key1_gpio")]
    pub key1_gpio: u32, // Default: 14
    #[serde(default = "default_key2_gpio")]
    pub key2_gpio: u32, // Default: 9
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            key1_hid_usage: 0x1D, // 'Z'
            key2_hid_usage: 0x1B, // 'X'
            debounce_us: 3000,
            brightness: 100,
            display_sleep_seconds: 600,
            gameplay_display_hz: 10,
            tosu_endpoint: "ws://127.0.0.1:24050/websocket/v2".to_string(),
            key1_gpio: DEFAULT_KEY1_GPIO,
            key2_gpio: DEFAULT_KEY2_GPIO,
        }
    }
}

impl DeviceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.key1_hid_usage < 0x04 || self.key1_hid_usage > 0xE7 {
            return Err(format!(
                "Invalid key1 HID usage: 0x{:02X}",
                self.key1_hid_usage
            ));
        }
        if self.key2_hid_usage < 0x04 || self.key2_hid_usage > 0xE7 {
            return Err(format!(
                "Invalid key2 HID usage: 0x{:02X}",
                self.key2_hid_usage
            ));
        }
        if self.debounce_us < 500 || self.debounce_us > 20000 {
            return Err(format!(
                "Debounce lockout must be between 500 and 20000 µs (got {})",
                self.debounce_us
            ));
        }
        if self.brightness > 100 {
            return Err(format!(
                "Brightness must be between 0 and 100 (got {})",
                self.brightness
            ));
        }
        if self.display_sleep_seconds != 0
            && (self.display_sleep_seconds < 10 || self.display_sleep_seconds > 86400)
        {
            return Err(format!(
                "Display sleep must be 0 or 10..=86400 seconds (got {})",
                self.display_sleep_seconds
            ));
        }
        if self.gameplay_display_hz < 1 || self.gameplay_display_hz > 30 {
            return Err(format!(
                "Gameplay display Hz must be between 1 and 30 (got {})",
                self.gameplay_display_hz
            ));
        }
        validate_key_gpios(self.key1_gpio, self.key2_gpio)
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
    if let Some(rest) = s.strip_prefix("0X") {
        return u32::from_str_radix(rest, 16).ok();
    }
    None
}

/// Live gameplay telemetry from tosu (§15)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GameplayTelemetry {
    pub is_playing: bool,
    pub title: String,
    #[serde(default)]
    pub live_time_ms: f64, // Song position; jumps back on retry
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
    #[serde(default = "default_key1_gpio")]
    pub key1_gpio: u32,
    #[serde(default = "default_key2_gpio")]
    pub key2_gpio: u32,
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
                key1_gpio: config.key1_gpio,
                key2_gpio: config.key2_gpio,
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format_version != 1 {
            return Err(format!(
                "Unsupported backup format version: {}",
                self.format_version
            ));
        }
        if self.device.board_profile != "waveshare_esp32s3_touch_lcd_2" {
            return Err(format!(
                "Unsupported board profile: {}",
                self.device.board_profile
            ));
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
            return Err(format!(
                "Debounce lockout must be between 500 and 20000 µs (got {})",
                self.config.debounce_us
            ));
        }
        if self.config.brightness > 100 {
            return Err("Brightness must be between 0 and 100".to_string());
        }
        if self.config.display_sleep_seconds != 0
            && (self.config.display_sleep_seconds < 10 || self.config.display_sleep_seconds > 86400)
        {
            return Err(format!(
                "Display sleep must be 0 or 10..=86400 seconds (got {})",
                self.config.display_sleep_seconds
            ));
        }
        if self.config.gameplay_display_hz < 1 || self.config.gameplay_display_hz > 30 {
            return Err(format!(
                "Gameplay display Hz must be between 1 and 30 (got {})",
                self.config.gameplay_display_hz
            ));
        }
        if char_to_hid_usage(&self.config.key1).is_none() {
            return Err(format!("Invalid key1 mapping: {}", self.config.key1));
        }
        if char_to_hid_usage(&self.config.key2).is_none() {
            return Err(format!("Invalid key2 mapping: {}", self.config.key2));
        }
        validate_key_gpios(self.config.key1_gpio, self.config.key2_gpio)
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
    fn test_key_gpio_validation() {
        let mut b = valid_backup();
        b.config.key1_gpio = 19; // USB D-
        assert!(b.validate().is_err());

        b.config.key1_gpio = 9; // Same pin as key 2
        assert!(b.validate().is_err());

        b.config.key1_gpio = 21;
        b.config.key2_gpio = 2;
        assert!(b.validate().is_ok());

        let mut c = DeviceConfig::default();
        assert!(c.validate().is_ok());
        for gpio in [0, 17, 20, 43, 44, 47, 48] {
            c.key2_gpio = gpio;
            assert!(c.validate().is_err(), "GPIO{} accepted", gpio);
        }
    }

    #[test]
    fn test_key_pins_match_firmware_list() {
        let gpios: Vec<u32> = KEY_PINS.iter().map(|p| p.gpio).collect();
        assert_eq!(
            gpios,
            [2, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 21]
        );
        assert!(key_pin(DEFAULT_KEY1_GPIO).is_some() && key_pin(DEFAULT_KEY2_GPIO).is_some());
    }

    #[test]
    fn test_backup_without_pins_uses_defaults() {
        let mut json = serde_json::to_value(valid_backup()).unwrap();
        let cfg = json["config"].as_object_mut().unwrap();
        cfg.remove("key1_gpio");
        cfg.remove("key2_gpio");
        let b: JsonBackup = serde_json::from_value(json).unwrap();
        assert_eq!((b.config.key1_gpio, b.config.key2_gpio), (14, 9));
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_future_timestamp_rejected() {
        let mut b = valid_backup();
        b.exported_at = chrono::Utc::now() + chrono::Duration::days(2);
        assert!(b.validate().is_err());
    }
}
