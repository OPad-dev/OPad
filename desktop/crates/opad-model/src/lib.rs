use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod diag;
pub mod log;
pub mod paths;
pub mod tap_rate;
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
    /// Which app partition the running image booted from (§U-3a): `ota_0`,
    /// `ota_1`, or `factory` on a pad still using the pre-OTA single-app
    /// table. `None` for firmware that predates the field, which is the same
    /// thing as "this pad needs a serial reflash before OTA is possible".
    #[serde(default)]
    pub running_partition: Option<String>,
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

/// What a left/right swipe on the pad's touchscreen does. Up/down always set
/// the volume. The wire values are osupad.proto's `SwipeAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwipeAction {
    None,
    PrevTrack,
    NextTrack,
    PlayPause,
    Mute,
    /// The configured keyboard key (`swipe_*_key`). Never sent during a map.
    Key,
}

impl SwipeAction {
    pub const ALL: [SwipeAction; 6] = [
        SwipeAction::None,
        SwipeAction::PrevTrack,
        SwipeAction::NextTrack,
        SwipeAction::PlayPause,
        SwipeAction::Mute,
        SwipeAction::Key,
    ];

    pub fn to_wire(self) -> u32 {
        match self {
            SwipeAction::None => 1,
            SwipeAction::PrevTrack => 2,
            SwipeAction::NextTrack => 3,
            SwipeAction::PlayPause => 4,
            SwipeAction::Mute => 5,
            SwipeAction::Key => 6,
        }
    }

    /// `None` for 0 (keep current, or firmware without swipes) and unknown values
    pub fn from_wire(value: u32) -> Option<SwipeAction> {
        SwipeAction::ALL.into_iter().find(|a| a.to_wire() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            SwipeAction::None => "Nothing",
            SwipeAction::PrevTrack => "Previous track",
            SwipeAction::NextTrack => "Next track",
            SwipeAction::PlayPause => "Play / pause",
            SwipeAction::Mute => "Mute",
            SwipeAction::Key => "Keyboard key",
        }
    }
}

impl std::fmt::Display for SwipeAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A keyboard key a swipe can send ([`SwipeAction::Key`])
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwipeKey {
    pub usage: u32,
    pub name: &'static str,
}

impl std::fmt::Display for SwipeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

const fn swipe_key(usage: u32, name: &'static str) -> SwipeKey {
    SwipeKey { usage, name }
}

/// The keys offered for a swipe, by HID usage. Letters and digits are left
/// out: a swipe that types one is rarely what anyone wants.
pub const SWIPE_KEYS: &[SwipeKey] = &[
    swipe_key(0x29, "Esc"),
    swipe_key(0x28, "Enter"),
    swipe_key(0x2C, "Space"),
    swipe_key(0x2B, "Tab"),
    swipe_key(0x2A, "Backspace"),
    swipe_key(0x4C, "Delete"),
    swipe_key(0x3A, "F1"),
    swipe_key(0x3B, "F2"),
    swipe_key(0x3C, "F3"),
    swipe_key(0x3D, "F4"),
    swipe_key(0x3E, "F5"),
    swipe_key(0x3F, "F6"),
    swipe_key(0x40, "F7"),
    swipe_key(0x41, "F8"),
    swipe_key(0x42, "F9"),
    swipe_key(0x43, "F10"),
    swipe_key(0x44, "F11"),
    swipe_key(0x45, "F12"),
    swipe_key(0x52, "Up"),
    swipe_key(0x51, "Down"),
    swipe_key(0x50, "Left"),
    swipe_key(0x4F, "Right"),
    swipe_key(0x4B, "Page Up"),
    swipe_key(0x4E, "Page Down"),
    swipe_key(0x4A, "Home"),
    swipe_key(0x4D, "End"),
];

pub fn swipe_key_by_usage(usage: u32) -> Option<SwipeKey> {
    SWIPE_KEYS.iter().copied().find(|k| k.usage == usage)
}

pub const DEFAULT_SWIPE_LEFT: SwipeAction = SwipeAction::PrevTrack;
pub const DEFAULT_SWIPE_RIGHT: SwipeAction = SwipeAction::NextTrack;

fn default_swipe_left() -> SwipeAction {
    DEFAULT_SWIPE_LEFT
}

fn default_swipe_right() -> SwipeAction {
    DEFAULT_SWIPE_RIGHT
}

fn validate_swipe(side: &str, action: SwipeAction, key: u32) -> Result<(), String> {
    if key != 0 && !(0x04..=0xE7).contains(&key) {
        return Err(format!("Invalid {} swipe key: 0x{:02X}", side, key));
    }
    if action == SwipeAction::Key && key == 0 {
        return Err(format!("The {} swipe needs a key to send", side));
    }
    Ok(())
}

/// Device configuration parameters (§37)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub key1_hid_usage: u32,        // Default: 0x1D ('Z')
    pub key2_hid_usage: u32,        // Default: 0x1B ('X')
    pub debounce_us: u32,           // Default: 5000 (5ms)
    pub brightness: u32,            // 0..100, default: 100
    pub display_sleep_seconds: u32, // Default: 600 (10 min)
    pub gameplay_display_hz: u32,   // Default: 10
    pub tosu_endpoint: String,      // Default: "ws://127.0.0.1:24050/websocket/v2"
    #[serde(default = "default_key1_gpio")]
    pub key1_gpio: u32, // Default: 14
    #[serde(default = "default_key2_gpio")]
    pub key2_gpio: u32, // Default: 9
    #[serde(default = "default_swipe_left")]
    pub swipe_left_action: SwipeAction,
    #[serde(default = "default_swipe_right")]
    pub swipe_right_action: SwipeAction,
    /// HID usage for [`SwipeAction::Key`], 0 = none
    #[serde(default)]
    pub swipe_left_key: u32,
    #[serde(default)]
    pub swipe_right_key: u32,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            key1_hid_usage: 0x1D, // 'Z'
            key2_hid_usage: 0x1B, // 'X'
            debounce_us: 5000,
            brightness: 100,
            display_sleep_seconds: 600,
            gameplay_display_hz: 10,
            tosu_endpoint: "ws://127.0.0.1:24050/websocket/v2".to_string(),
            key1_gpio: DEFAULT_KEY1_GPIO,
            key2_gpio: DEFAULT_KEY2_GPIO,
            swipe_left_action: DEFAULT_SWIPE_LEFT,
            swipe_right_action: DEFAULT_SWIPE_RIGHT,
            swipe_left_key: 0,
            swipe_right_key: 0,
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
        validate_swipe("left", self.swipe_left_action, self.swipe_left_key)?;
        validate_swipe("right", self.swipe_right_action, self.swipe_right_key)?;
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
    /// What is being played, recorded with each attempt's tap rate
    #[serde(default)]
    pub beatmap: tap_rate::BeatmapRef,
    /// The play has failed: the attempt is over even though osu! is still in
    /// its play state until the retry or the exit
    #[serde(default)]
    pub failed: bool,
    /// Song position and note range, for the song rate. None when tosu does
    /// not say where the notes are.
    #[serde(skip)]
    pub clock: Option<tap_rate::SongClock>,
    /// The play is someone else's: a replay, or spectating. Its presses are
    /// not the player's, so its tap rate is not measured.
    #[serde(default)]
    pub other_player: bool,
}

/// tosu's key counters (`/websocket/v2/precise`), sent when either changes.
/// osu! counts the player's K1/K2 key-downs during a play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCounts {
    pub k1: u32,
    pub k2: u32,
    /// When this daemon received the frame; the tap rate is measured on it
    pub at: std::time::Instant,
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
    // Backups from before swipes have none of these: the defaults
    #[serde(default = "default_swipe_left")]
    pub swipe_left_action: SwipeAction,
    #[serde(default = "default_swipe_right")]
    pub swipe_right_action: SwipeAction,
    #[serde(default)]
    pub swipe_left_key: u32,
    #[serde(default)]
    pub swipe_right_key: u32,
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
                swipe_left_action: config.swipe_left_action,
                swipe_right_action: config.swipe_right_action,
                swipe_left_key: config.swipe_left_key,
                swipe_right_key: config.swipe_right_key,
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
        let key1_hid_usage = char_to_hid_usage(&self.config.key1)
            .ok_or_else(|| format!("Invalid key1 mapping: {}", self.config.key1))?;
        let key2_hid_usage = char_to_hid_usage(&self.config.key2)
            .ok_or_else(|| format!("Invalid key2 mapping: {}", self.config.key2))?;
        // The config the import would apply must pass the same rules as any
        // other config (and the firmware's): one set of ranges, not two
        DeviceConfig {
            key1_hid_usage,
            key2_hid_usage,
            debounce_us: self.config.debounce_us,
            brightness: self.config.brightness,
            display_sleep_seconds: self.config.display_sleep_seconds,
            gameplay_display_hz: self.config.gameplay_display_hz,
            key1_gpio: self.config.key1_gpio,
            key2_gpio: self.config.key2_gpio,
            swipe_left_action: self.config.swipe_left_action,
            swipe_right_action: self.config.swipe_right_action,
            swipe_left_key: self.config.swipe_left_key,
            swipe_right_key: self.config.swipe_right_key,
            ..DeviceConfig::default()
        }
        .validate()
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
                running_partition: None,
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
    fn test_backup_without_swipes_uses_defaults() {
        let mut json = serde_json::to_value(valid_backup()).unwrap();
        let cfg = json["config"].as_object_mut().unwrap();
        for field in [
            "swipe_left_action",
            "swipe_right_action",
            "swipe_left_key",
            "swipe_right_key",
        ] {
            assert!(cfg.remove(field).is_some(), "{field} is written");
        }
        let b: JsonBackup = serde_json::from_value(json).unwrap();
        assert_eq!(b.config.swipe_left_action, SwipeAction::PrevTrack);
        assert_eq!(b.config.swipe_right_action, SwipeAction::NextTrack);
        assert_eq!((b.config.swipe_left_key, b.config.swipe_right_key), (0, 0));
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_swipe_actions() {
        for action in SwipeAction::ALL {
            assert_eq!(SwipeAction::from_wire(action.to_wire()), Some(action));
        }
        // 0 is "keep current" on the wire
        assert_eq!(SwipeAction::from_wire(0), None);
        assert_eq!(SwipeAction::from_wire(7), None);

        let mut c = DeviceConfig {
            swipe_right_action: SwipeAction::Key,
            ..DeviceConfig::default()
        };
        assert!(c
            .validate()
            .unwrap_err()
            .contains("right swipe needs a key"));
        c.swipe_right_key = 0x3B; // F2
        assert!(c.validate().is_ok());
        c.swipe_left_key = 0x03;
        assert!(c.validate().unwrap_err().contains("left swipe key"));
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
    fn test_backup_keys_outside_the_firmware_range_are_rejected() {
        // char_to_hid_usage parses any hex; only 0x04..=0xE7 is a usable key
        for key in ["0x00", "0x03", "0xE8", "0xFFFF"] {
            let mut b = valid_backup();
            b.config.key1 = key.to_string();
            assert!(b.validate().is_err(), "key1 {key} accepted");
            let mut b = valid_backup();
            b.config.key2 = key.to_string();
            assert!(b.validate().is_err(), "key2 {key} accepted");
        }
        let mut b = valid_backup();
        b.config.key1 = "0x04".to_string();
        b.config.key2 = "0xE7".to_string();
        assert!(b.validate().is_ok());

        let mut b = valid_backup();
        b.config.brightness = 101;
        assert!(b.validate().is_err());
    }

    #[test]
    fn test_future_timestamp_rejected() {
        let mut b = valid_backup();
        b.exported_at = chrono::Utc::now() + chrono::Duration::days(2);
        assert!(b.validate().is_err());
    }
}
