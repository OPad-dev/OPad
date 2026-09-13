use chrono::Utc;
use osupad_model::{CounterState, DeviceConfig, DeviceInfo};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("Storage writes blocked during active gameplay/cooldown")]
    WritesBlocked,
}

pub struct Storage {
    conn: Connection,
    writes_allowed: Arc<AtomicBool>,
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

        let storage = Self {
            conn,
            writes_allowed: Arc::new(AtomicBool::new(true)),
        };
        storage.migrate()?;
        Ok(storage)
    }

    /// Creates an in-memory database instance (primarily for tests)
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn,
            writes_allowed: Arc::new(AtomicBool::new(true)),
        };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn set_writes_allowed(&self, allowed: bool) {
        self.writes_allowed.store(allowed, Ordering::SeqCst);
    }

    pub fn writes_allowed_handle(&self) -> Arc<AtomicBool> {
        self.writes_allowed.clone()
    }

    fn check_writes_allowed(&self) -> Result<(), StorageError> {
        if !self.writes_allowed.load(Ordering::SeqCst) {
            tracing::warn!("Blocked storage write while writes_allowed is false");
            return Err(StorageError::WritesBlocked);
        }
        Ok(())
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
        if v < 2 {
            self.apply_v2()?;
        }
        if v < 3 {
            self.apply_v3()?;
        }
        if v < 4 {
            self.apply_v4()?;
        }
        Ok(())
    }

    /// v2: configurable key press highlight color for the gameplay screen
    fn apply_v2(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            ALTER TABLE config ADD COLUMN press_color_rgb INTEGER NOT NULL DEFAULT 16711680;
            INSERT INTO schema_migrations (version, applied_at) VALUES (2, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v3: custom screen layouts from the PC designer
    fn apply_v3(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS layouts (
                screen INTEGER PRIMARY KEY,
                json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO schema_migrations (version, applied_at) VALUES (3, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v4: update default gameplay_display_hz from 5 to 10 (§P2-7)
    fn apply_v4(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            UPDATE config SET gameplay_display_hz = 10 WHERE gameplay_display_hz = 5;
            INSERT INTO schema_migrations (version, applied_at) VALUES (4, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// Custom layout JSON for a screen, if one was saved
    pub fn load_layout(&self, screen: u8) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row("SELECT json FROM layouts WHERE screen = ?1", params![screen], |row| row.get(0))
            .optional()
            .map_err(StorageError::from)
    }

    pub fn save_layout(&self, screen: u8, json: &str) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
        self.conn.execute(
            "INSERT INTO layouts (screen, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(screen) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![screen, json, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn delete_layout(&self, screen: u8) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
        self.conn.execute("DELETE FROM layouts WHERE screen = ?1", params![screen])?;
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
                    display_sleep_seconds, gameplay_display_hz, tosu_endpoint, press_color_rgb
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
                    press_color_rgb: row.get(7)?,
                })
            },
        ).map_err(StorageError::from)
    }

    pub fn save_config(&self, config: &DeviceConfig) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
        self.conn.execute(
            "UPDATE config SET
                key1_hid_usage = ?1,
                key2_hid_usage = ?2,
                debounce_us = ?3,
                brightness = ?4,
                display_sleep_seconds = ?5,
                gameplay_display_hz = ?6,
                tosu_endpoint = ?7,
                press_color_rgb = ?8
             WHERE id = 1",
            params![
                config.key1_hid_usage,
                config.key2_hid_usage,
                config.debounce_us,
                config.brightness,
                config.display_sleep_seconds,
                config.gameplay_display_hz,
                config.tosu_endpoint,
                config.press_color_rgb,
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
                    let k1: i64 = row.get(2)?;
                    let k2: i64 = row.get(3)?;
                    Ok(CounterState {
                        device_id: row.get(0)?,
                        counter_generation: row.get(1)?,
                        lifetime_key1: k1 as u64,
                        lifetime_key2: k2 as u64,
                        map_key1: 0,
                        map_key2: 0,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_device_states(&self) -> Result<Vec<(DeviceInfo, CounterState)>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, board_profile, firmware_version, counter_generation,
                    lifetime_key1, lifetime_key2, last_seen_at, last_sync_at
             FROM device_state ORDER BY last_seen_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let device_id: String = row.get(0)?;
            let board_profile: String = row.get(1)?;
            let firmware_version: Option<String> = row.get(2)?;
            let counter_generation: u32 = row.get(3)?;
            let lifetime_key1: i64 = row.get(4)?;
            let lifetime_key2: i64 = row.get(5)?;

            let info = DeviceInfo {
                device_id: device_id.clone(),
                board_profile,
                firmware_version: firmware_version.unwrap_or_else(|| "1.0.0".to_string()),
                protocol_version: 1,
            };
            let counters = CounterState {
                device_id,
                counter_generation,
                lifetime_key1: lifetime_key1 as u64,
                lifetime_key2: lifetime_key2 as u64,
                map_key1: 0,
                map_key2: 0,
            };
            Ok((info, counters))
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn load_latest_device_state(&self) -> Result<Option<(DeviceInfo, CounterState)>, StorageError> {
        let states = self.list_device_states()?;
        Ok(states.into_iter().next())
    }

    pub fn touch_device_last_seen(&self, device_id: &str) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE device_state SET last_seen_at = ?1 WHERE device_id = ?2",
            params![now, device_id],
        )?;
        Ok(())
    }

    pub fn save_device_state(
        &self,
        info: &DeviceInfo,
        counters: &CounterState,
    ) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
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
    fn layouts_round_trip() {
        let storage = Storage::open_in_memory().expect("open");
        assert_eq!(storage.load_layout(1).unwrap(), None);
        storage.save_layout(1, "{\"a\":1}").unwrap();
        storage.save_layout(1, "{\"a\":2}").unwrap();
        assert_eq!(storage.load_layout(1).unwrap().as_deref(), Some("{\"a\":2}"));
        storage.delete_layout(1).unwrap();
        assert_eq!(storage.load_layout(1).unwrap(), None);
    }

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

    #[test]
    fn test_writes_blocked_guard() {
        let storage = Storage::open_in_memory().expect("open");
        storage.set_writes_allowed(false);

        let config = storage.load_config().expect("read is allowed");
        assert!(matches!(
            storage.save_config(&config),
            Err(StorageError::WritesBlocked)
        ));
        assert!(matches!(
            storage.save_layout(0, "{}"),
            Err(StorageError::WritesBlocked)
        ));
        assert!(matches!(
            storage.delete_layout(0),
            Err(StorageError::WritesBlocked)
        ));

        let info = DeviceInfo {
            device_id: "dev-test".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
        };
        let counters = CounterState::default();
        assert!(matches!(
            storage.save_device_state(&info, &counters),
            Err(StorageError::WritesBlocked)
        ));

        // Re-enable and verify it works
        storage.set_writes_allowed(true);
        storage.save_config(&config).expect("save should succeed");
    }

    #[test]
    fn test_list_and_load_latest_device_state() {
        let storage = Storage::open_in_memory().expect("open");
        assert!(storage.load_latest_device_state().unwrap().is_none());

        let info1 = DeviceInfo {
            device_id: "dev-01".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
        };
        let counters1 = CounterState {
            device_id: "dev-01".to_string(),
            counter_generation: 1,
            lifetime_key1: 100,
            lifetime_key2: 200,
            map_key1: 0,
            map_key2: 0,
        };
        storage.save_device_state(&info1, &counters1).unwrap();

        let latest = storage.load_latest_device_state().unwrap().expect("latest");
        assert_eq!(latest.0.device_id, "dev-01");
        assert_eq!(latest.1.lifetime_key1, 100);

        let states = storage.list_device_states().unwrap();
        assert_eq!(states.len(), 1);
    }

    #[test]
    fn test_migration_v4_updates_gameplay_display_hz() {
        let storage = Storage::open_in_memory().expect("open");
        let cfg = storage.load_config().unwrap();
        assert_eq!(cfg.gameplay_display_hz, 10);

        // Manually update to 5, reset migration version to 3, and run migrate()
        storage.conn.execute("UPDATE config SET gameplay_display_hz = 5 WHERE id = 1", []).unwrap();
        storage.conn.execute("DELETE FROM schema_migrations WHERE version = 4", []).unwrap();
        storage.migrate().unwrap();

        let migrated = storage.load_config().unwrap();
        assert_eq!(migrated.gameplay_display_hz, 10);

        // A custom value (e.g. 20) should not be altered by v4
        storage.conn.execute("UPDATE config SET gameplay_display_hz = 20 WHERE id = 1", []).unwrap();
        storage.conn.execute("DELETE FROM schema_migrations WHERE version = 4", []).unwrap();
        storage.migrate().unwrap();

        let custom = storage.load_config().unwrap();
        assert_eq!(custom.gameplay_display_hz, 20);
    }
}
