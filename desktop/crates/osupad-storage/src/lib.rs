use chrono::Utc;
use osupad_model::{CounterState, DeviceConfig, DeviceInfo};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Migration error: {0}")]
    Migration(String),
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Opens or creates the SQLite database at the specified path and runs pending migrations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Migration(format!("Failed to create storage directory: {}", e))
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    /// Creates an in-memory database instance (primarily for tests)
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> Result<(), StorageError> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
            [],
        )?;

        let current_version: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(version) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();

        let v = current_version.unwrap_or(0);
        if v < 1 {
            self.apply_v1()?;
        }
        Ok(())
    }

    fn apply_v1(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;

            CREATE TABLE IF NOT EXISTS device_state (
                device_id TEXT PRIMARY KEY,
                board_profile TEXT NOT NULL,
                firmware_version TEXT,
                counter_generation INTEGER NOT NULL,
                lifetime_key1 INTEGER NOT NULL,
                lifetime_key2 INTEGER NOT NULL,
                last_seen_at TEXT,
                last_sync_at TEXT
            );

            CREATE TABLE IF NOT EXISTS config (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                key1_hid_usage INTEGER NOT NULL,
                key2_hid_usage INTEGER NOT NULL,
                debounce_us INTEGER NOT NULL,
                brightness INTEGER NOT NULL,
                display_sleep_seconds INTEGER NOT NULL,
                gameplay_display_hz INTEGER NOT NULL,
                tosu_endpoint TEXT NOT NULL
            );

            INSERT OR IGNORE INTO config (
                id, key1_hid_usage, key2_hid_usage, debounce_us,
                brightness, display_sleep_seconds, gameplay_display_hz, tosu_endpoint
            ) VALUES (
                1, 29, 27, 3000, 100, 600, 5, 'ws://127.0.0.1:24050/ws'
            );

            INSERT INTO schema_migrations (version, applied_at) VALUES (1, datetime('now'));

            COMMIT;",
        )?;
        Ok(())
    }

    pub fn load_config(&self) -> Result<DeviceConfig, StorageError> {
        self.conn.query_row(
            "SELECT key1_hid_usage, key2_hid_usage, debounce_us, brightness,
                    display_sleep_seconds, gameplay_display_hz, tosu_endpoint
             FROM config WHERE id = 1",
            [],
            |row| {
                Ok(DeviceConfig {
                    key1_hid_usage: row.get(0)?,
                    key2_hid_usage: row.get(1)?,
                    debounce_us: row.get(2)?,
                    brightness: row.get(3)?,
                    display_sleep_seconds: row.get(4)?,
                    gameplay_display_hz: row.get(5)?,
                    tosu_endpoint: row.get(6)?,
                })
            },
        ).map_err(StorageError::from)
    }

    pub fn save_config(&self, config: &DeviceConfig) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE config SET
                key1_hid_usage = ?1,
                key2_hid_usage = ?2,
                debounce_us = ?3,
                brightness = ?4,
                display_sleep_seconds = ?5,
                gameplay_display_hz = ?6,
                tosu_endpoint = ?7
             WHERE id = 1",
            params![
                config.key1_hid_usage,
                config.key2_hid_usage,
                config.debounce_us,
                config.brightness,
                config.display_sleep_seconds,
                config.gameplay_display_hz,
                config.tosu_endpoint,
            ],
        )?;
        Ok(())
    }

    pub fn load_device_state(&self, device_id: &str) -> Result<Option<CounterState>, StorageError> {
        self.conn
            .query_row(
                "SELECT device_id, counter_generation, lifetime_key1, lifetime_key2
                 FROM device_state WHERE device_id = ?1",
                params![device_id],
                |row| {
                    Ok(CounterState {
                        device_id: row.get(0)?,
                        counter_generation: row.get(1)?,
                        lifetime_key1: row.get(2)?,
                        lifetime_key2: row.get(3)?,
                        map_key1: 0,
                        map_key2: 0,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn save_device_state(
        &self,
        info: &DeviceInfo,
        counters: &CounterState,
    ) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO device_state (
                device_id, board_profile, firmware_version, counter_generation,
                lifetime_key1, lifetime_key2, last_seen_at, last_sync_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            ON CONFLICT(device_id) DO UPDATE SET
                board_profile = excluded.board_profile,
                firmware_version = excluded.firmware_version,
                counter_generation = excluded.counter_generation,
                lifetime_key1 = excluded.lifetime_key1,
                lifetime_key2 = excluded.lifetime_key2,
                last_seen_at = excluded.last_seen_at,
                last_sync_at = excluded.last_sync_at",
            params![
                info.device_id,
                info.board_profile,
                info.firmware_version,
                counters.counter_generation,
                counters.lifetime_key1 as i64,
                counters.lifetime_key2 as i64,
                now,
            ],
        )?;
        Ok(())
    }
}

/// Counter reconciliation algorithm (§13):
/// 1. If generations differ, the newer generation takes precedence.
/// 2. For identical generation, take the maximum valid value for each physical key.
pub fn reconcile_counters(pc: &CounterState, esp: &CounterState) -> CounterState {
    if pc.device_id != esp.device_id && !pc.device_id.is_empty() && !esp.device_id.is_empty() {
        tracing::warn!(
            "Reconciling counters with mismatched device IDs (PC: {}, ESP: {})",
            pc.device_id,
            esp.device_id
        );
    }

    if pc.counter_generation > esp.counter_generation {
        // PC has newer generation (e.g. deliberate reset or imported backup)
        pc.clone()
    } else if esp.counter_generation > pc.counter_generation {
        // ESP has newer generation
        esp.clone()
    } else {
        // Identical generation: maximum valid lifetime counters win
        CounterState {
            device_id: if !pc.device_id.is_empty() {
                pc.device_id.clone()
            } else {
                esp.device_id.clone()
            },
            counter_generation: pc.counter_generation,
            lifetime_key1: pc.lifetime_key1.max(esp.lifetime_key1),
            lifetime_key2: pc.lifetime_key2.max(esp.lifetime_key2),
            map_key1: esp.map_key1,
            map_key2: esp.map_key2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_lifecycle() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let config = storage.load_config().expect("load_config");
        assert_eq!(config.key1_hid_usage, 29); // 'Z'
        assert_eq!(config.key2_hid_usage, 27); // 'X'
        assert_eq!(config.brightness, 100);

        let mut updated = config.clone();
        updated.brightness = 85;
        updated.debounce_us = 2500;
        storage.save_config(&updated).expect("save_config");

        let loaded = storage.load_config().expect("load_config");
        assert_eq!(loaded.brightness, 85);
        assert_eq!(loaded.debounce_us, 2500);

        let info = DeviceInfo {
            device_id: "test-dev-01".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
        };
        let counters = CounterState {
            device_id: "test-dev-01".to_string(),
            counter_generation: 1,
            lifetime_key1: 1000,
            lifetime_key2: 2000,
            map_key1: 10,
            map_key2: 20,
        };

        storage.save_device_state(&info, &counters).expect("save_state");
        let fetched = storage
            .load_device_state("test-dev-01")
            .expect("load_state")
            .expect("found");

        assert_eq!(fetched.lifetime_key1, 1000);
        assert_eq!(fetched.lifetime_key2, 2000);
    }

    #[test]
    fn test_reconciliation_logic() {
        let pc = CounterState {
            device_id: "dev-a".to_string(),
            counter_generation: 2,
            lifetime_key1: 500,
            lifetime_key2: 800,
            map_key1: 0,
            map_key2: 0,
        };

        let esp = CounterState {
            device_id: "dev-a".to_string(),
            counter_generation: 2,
            lifetime_key1: 750,
            lifetime_key2: 600,
            map_key1: 50,
            map_key2: 30,
        };

        let merged = reconcile_counters(&pc, &esp);
        assert_eq!(merged.lifetime_key1, 750);
        assert_eq!(merged.lifetime_key2, 800);
        assert_eq!(merged.map_key1, 50);

        // Newer generation overrides older higher count
        let pc_newer = CounterState {
            counter_generation: 3,
            lifetime_key1: 10,
            lifetime_key2: 10,
            ..pc.clone()
        };
        let merged2 = reconcile_counters(&pc_newer, &esp);
        assert_eq!(merged2.counter_generation, 3);
        assert_eq!(merged2.lifetime_key1, 10);
    }
}
