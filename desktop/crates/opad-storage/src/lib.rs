use chrono::Utc;
use opad_model::{
    CounterState, DeviceConfig, DeviceInfo, SwipeAction, DEFAULT_SWIPE_LEFT, DEFAULT_SWIPE_RIGHT,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

mod tap_stats;
pub use tap_stats::HISTORY_DAYS_KEY;

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
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
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
        if v < 5 {
            self.apply_v5()?;
        }
        if v < 6 {
            self.apply_v6()?;
        }
        if v < 7 {
            self.apply_v7()?;
        }
        if v < 8 {
            self.apply_v8()?;
        }
        if v < 9 {
            self.apply_v9()?;
        }
        if v < 10 {
            self.apply_v10()?;
        }
        if v < 11 {
            self.apply_v11()?;
        }
        if v < 12 {
            self.apply_v12()?;
        }
        if v < 13 {
            self.apply_v13()?;
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

    /// v5: normalise legacy tosu_endpoint '/ws' to '/websocket/v2' (§P3-5)
    fn apply_v5(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            UPDATE config SET tosu_endpoint = 'ws://127.0.0.1:24050/websocket/v2' WHERE tosu_endpoint = 'ws://127.0.0.1:24050/ws';
            INSERT INTO schema_migrations (version, applied_at) VALUES (5, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v6: configurable key switch GPIOs (defaults: the original fixed pins)
    fn apply_v6(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            ALTER TABLE config ADD COLUMN key1_gpio INTEGER NOT NULL DEFAULT 14;
            ALTER TABLE config ADD COLUMN key2_gpio INTEGER NOT NULL DEFAULT 9;
            INSERT INTO schema_migrations (version, applied_at) VALUES (6, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v7: host-side state that is neither device config nor counters — the
    /// install identity (§W3-1) and the updaters' settings and check schedules
    /// (§U-0.4, §U-0.6). One key/value table rather than a column per setting,
    /// so adding an updater is not a migration.
    fn apply_v7(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            CREATE TABLE IF NOT EXISTS app_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO schema_migrations (version, applied_at) VALUES (7, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v8: update default debounce_us from 3000 to 5000 µs to eliminate switch chatter
    fn apply_v8(&self) -> Result<(), StorageError> {
        self.conn.execute_batch(
            "BEGIN TRANSACTION;
            UPDATE config SET debounce_us = 5000 WHERE debounce_us = 3000;
            INSERT INTO schema_migrations (version, applied_at) VALUES (8, datetime('now'));
            COMMIT;",
        )?;
        Ok(())
    }

    /// v12: left/right touchscreen swipe actions, as `SwipeAction` wire values
    /// (defaults: previous / next track) and their keyboard keys
    fn apply_v12(&self) -> Result<(), StorageError> {
        // Re-run safe, like v11: the columns may already be there
        let has_swipes: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('config') WHERE name = 'swipe_left_action'",
            [],
            |row| row.get(0),
        )?;
        self.conn.execute_batch(&format!(
            "BEGIN TRANSACTION;
            {}
            INSERT INTO schema_migrations (version, applied_at) VALUES (12, datetime('now'));
            COMMIT;",
            if has_swipes {
                ""
            } else {
                "ALTER TABLE config ADD COLUMN swipe_left_action INTEGER NOT NULL DEFAULT 2;
                ALTER TABLE config ADD COLUMN swipe_right_action INTEGER NOT NULL DEFAULT 3;
                ALTER TABLE config ADD COLUMN swipe_left_key INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE config ADD COLUMN swipe_right_key INTEGER NOT NULL DEFAULT 0;"
            }
        ))?;
        Ok(())
    }

    /// v13: up/down swipes inverted (0 = swipe up is volume up)
    fn apply_v13(&self) -> Result<(), StorageError> {
        // Re-run safe, like v11
        let has_column: bool = self.conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('config') WHERE name = 'swipe_invert_vertical'",
            [],
            |row| row.get(0),
        )?;
        self.conn.execute_batch(&format!(
            "BEGIN TRANSACTION;
            {}
            INSERT INTO schema_migrations (version, applied_at) VALUES (13, datetime('now'));
            COMMIT;",
            if has_column {
                ""
            } else {
                "ALTER TABLE config ADD COLUMN swipe_invert_vertical INTEGER NOT NULL DEFAULT 0;"
            }
        ))?;
        Ok(())
    }

    /// Reads a host-side value written by [`Self::set_app_state`]
    pub fn get_app_state(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(Into::into)
    }

    /// Writes a host-side value.
    ///
    /// Goes through the same P1-3 write guard as everything else: an updater
    /// recording a check time mid-map is still a storage write.
    pub fn set_app_state(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.check_writes_allowed()?;
        self.conn.execute(
            "INSERT INTO app_state (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value],
        )?;
        Ok(())
    }

    /// Custom layout JSON for a screen, if one was saved
    pub fn load_layout(&self, screen: u8) -> Result<Option<String>, StorageError> {
        self.conn
            .query_row(
                "SELECT json FROM layouts WHERE screen = ?1",
                params![screen],
                |row| row.get(0),
            )
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
        self.conn
            .execute("DELETE FROM layouts WHERE screen = ?1", params![screen])?;
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
                1, 29, 27, 5000, 100, 600, 10, 'ws://127.0.0.1:24050/websocket/v2'
            );

            INSERT INTO schema_migrations (version, applied_at) VALUES (1, datetime('now'));

            COMMIT;",
        )?;
        Ok(())
    }

    pub fn load_config(&self) -> Result<DeviceConfig, StorageError> {
        self.conn
            .query_row(
                "SELECT key1_hid_usage, key2_hid_usage, debounce_us, brightness,
                    display_sleep_seconds, gameplay_display_hz, tosu_endpoint,
                    key1_gpio, key2_gpio, swipe_left_action, swipe_right_action,
                    swipe_left_key, swipe_right_key, swipe_invert_vertical
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
                        tosu_endpoint: {
                            let ep: String = row.get(6)?;
                            if ep.trim().is_empty() {
                                "ws://127.0.0.1:24050/websocket/v2".to_string()
                            } else {
                                ep
                            }
                        },
                        key1_gpio: row.get(7)?,
                        key2_gpio: row.get(8)?,
                        swipe_left_action: SwipeAction::from_wire(row.get(9)?)
                            .unwrap_or(DEFAULT_SWIPE_LEFT),
                        swipe_right_action: SwipeAction::from_wire(row.get(10)?)
                            .unwrap_or(DEFAULT_SWIPE_RIGHT),
                        swipe_left_key: row.get(11)?,
                        swipe_right_key: row.get(12)?,
                        swipe_invert_vertical: row.get(13)?,
                    })
                },
            )
            .map_err(StorageError::from)
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
                key1_gpio = ?8,
                key2_gpio = ?9,
                swipe_left_action = ?10,
                swipe_right_action = ?11,
                swipe_left_key = ?12,
                swipe_right_key = ?13,
                swipe_invert_vertical = ?14
             WHERE id = 1",
            params![
                config.key1_hid_usage,
                config.key2_hid_usage,
                config.debounce_us,
                config.brightness,
                config.display_sleep_seconds,
                config.gameplay_display_hz,
                config.tosu_endpoint,
                config.key1_gpio,
                config.key2_gpio,
                config.swipe_left_action.to_wire(),
                config.swipe_right_action.to_wire(),
                config.swipe_left_key,
                config.swipe_right_key,
                config.swipe_invert_vertical,
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
                firmware_version: firmware_version.unwrap_or_default(),
                protocol_version: 1,
                running_partition: None,
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

    pub fn load_latest_device_state(
        &self,
    ) -> Result<Option<(DeviceInfo, CounterState)>, StorageError> {
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
        assert_eq!(
            storage.load_layout(1).unwrap().as_deref(),
            Some("{\"a\":2}")
        );
        storage.delete_layout(1).unwrap();
        assert_eq!(storage.load_layout(1).unwrap(), None);
    }

    #[test]
    fn test_storage_lifecycle() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let config = storage.load_config().expect("load_config");
        assert_eq!(config.key1_hid_usage, 29); // 'Z'
        assert_eq!(config.key2_hid_usage, 27); // 'X'
        assert_eq!(config.debounce_us, 5000);
        assert_eq!(config.brightness, 100);
        assert_eq!((config.key1_gpio, config.key2_gpio), (14, 9));

        let mut updated = config.clone();
        updated.brightness = 85;
        updated.debounce_us = 2500;
        updated.key1_gpio = 21;
        updated.key2_gpio = 2;
        storage.save_config(&updated).expect("save_config");

        let loaded = storage.load_config().expect("load_config");
        assert_eq!(loaded.brightness, 85);
        assert_eq!(loaded.debounce_us, 2500);
        assert_eq!((loaded.key1_gpio, loaded.key2_gpio), (21, 2));

        let info = DeviceInfo {
            device_id: "test-dev-01".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
            running_partition: None,
        };
        let counters = CounterState {
            device_id: "test-dev-01".to_string(),
            counter_generation: 1,
            lifetime_key1: 1000,
            lifetime_key2: 2000,
            map_key1: 10,
            map_key2: 20,
        };

        storage
            .save_device_state(&info, &counters)
            .expect("save_state");
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
            running_partition: None,
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
            running_partition: None,
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

        // Manually update to 5, then re-run v4
        storage
            .conn
            .execute("UPDATE config SET gameplay_display_hz = 5 WHERE id = 1", [])
            .unwrap();
        storage
            .conn
            .execute("DELETE FROM schema_migrations WHERE version = 4", [])
            .unwrap();
        storage.apply_v4().unwrap();

        let migrated = storage.load_config().unwrap();
        assert_eq!(migrated.gameplay_display_hz, 10);

        // A custom value (e.g. 20) should not be altered by v4
        storage
            .conn
            .execute(
                "UPDATE config SET gameplay_display_hz = 20 WHERE id = 1",
                [],
            )
            .unwrap();
        storage
            .conn
            .execute("DELETE FROM schema_migrations WHERE version = 4", [])
            .unwrap();
        storage.apply_v4().unwrap();

        let custom = storage.load_config().unwrap();
        assert_eq!(custom.gameplay_display_hz, 20);
    }

    #[test]
    fn test_migration_v5_normalizes_tosu_endpoint() {
        let storage = Storage::open_in_memory().expect("open");
        let cfg = storage.load_config().unwrap();
        assert_eq!(cfg.tosu_endpoint, "ws://127.0.0.1:24050/websocket/v2");

        // Manually update to legacy '/ws', then re-run v5
        storage
            .conn
            .execute(
                "UPDATE config SET tosu_endpoint = 'ws://127.0.0.1:24050/ws' WHERE id = 1",
                [],
            )
            .unwrap();
        storage
            .conn
            .execute("DELETE FROM schema_migrations WHERE version = 5", [])
            .unwrap();
        storage.apply_v5().unwrap();

        let migrated = storage.load_config().unwrap();
        assert_eq!(migrated.tosu_endpoint, "ws://127.0.0.1:24050/websocket/v2");
    }

    #[test]
    fn test_swipe_actions_round_trip_and_migrate() {
        let storage = Storage::open_in_memory().expect("open");
        let config = storage.load_config().unwrap();
        assert_eq!(
            (config.swipe_left_action, config.swipe_right_action),
            (SwipeAction::PrevTrack, SwipeAction::NextTrack)
        );

        let mut updated = config.clone();
        updated.swipe_left_action = SwipeAction::Key;
        updated.swipe_left_key = 0x3B;
        updated.swipe_right_action = SwipeAction::Mute;
        updated.swipe_invert_vertical = true;
        storage.save_config(&updated).unwrap();
        assert_eq!(storage.load_config().unwrap(), updated);

        // A database from before swipes gets the defaults
        storage
            .conn
            .execute_batch(
                "ALTER TABLE config DROP COLUMN swipe_left_action;
                 ALTER TABLE config DROP COLUMN swipe_right_action;
                 ALTER TABLE config DROP COLUMN swipe_left_key;
                 ALTER TABLE config DROP COLUMN swipe_right_key;
                 ALTER TABLE config DROP COLUMN swipe_invert_vertical;
                 DELETE FROM schema_migrations WHERE version >= 12;",
            )
            .unwrap();
        storage.migrate().unwrap();
        let migrated = storage.load_config().unwrap();
        assert_eq!(migrated.swipe_left_action, SwipeAction::PrevTrack);
        assert_eq!(migrated.swipe_left_key, 0);
        assert!(!migrated.swipe_invert_vertical);
    }

    #[test]
    fn test_migration_v6_adds_default_key_gpios() {
        let storage = Storage::open_in_memory().expect("open");
        storage
            .conn
            .execute_batch(
                "UPDATE config SET debounce_us = 4000 WHERE id = 1;
                 ALTER TABLE config DROP COLUMN key1_gpio;
                 ALTER TABLE config DROP COLUMN key2_gpio;
                 DELETE FROM schema_migrations WHERE version >= 6;",
            )
            .unwrap();
        storage.migrate().unwrap();

        let migrated = storage.load_config().unwrap();
        assert_eq!((migrated.key1_gpio, migrated.key2_gpio), (14, 9));
        assert_eq!(migrated.debounce_us, 4000);
    }

    #[test]
    fn test_migration_v8_updates_default_debounce() {
        let storage = Storage::open_in_memory().expect("open");
        storage
            .conn
            .execute_batch(
                "UPDATE config SET debounce_us = 3000 WHERE id = 1;
                 DELETE FROM schema_migrations WHERE version >= 8;",
            )
            .unwrap();
        storage.migrate().unwrap();

        let migrated = storage.load_config().unwrap();
        assert_eq!(migrated.debounce_us, 5000);

        // Custom debounce value (e.g. 1500) should not be overwritten
        storage
            .conn
            .execute_batch(
                "UPDATE config SET debounce_us = 1500 WHERE id = 1;
                 DELETE FROM schema_migrations WHERE version >= 8;",
            )
            .unwrap();
        storage.migrate().unwrap();

        let preserved = storage.load_config().unwrap();
        assert_eq!(preserved.debounce_us, 1500);
    }

    #[test]
    fn test_app_state_round_trip_and_write_guard() {
        let storage = Storage::open_in_memory().expect("open");
        assert_eq!(storage.get_app_state("update.schedule").unwrap(), None);

        storage.set_app_state("update.schedule", "{}").unwrap();
        storage
            .set_app_state("update.schedule", "{\"etag\":\"v1\"}")
            .unwrap();
        assert_eq!(
            storage.get_app_state("update.schedule").unwrap().as_deref(),
            Some("{\"etag\":\"v1\"}"),
            "a second write must replace, not duplicate"
        );

        // P1-3 covers host-side state too: recording an update check is still
        // a storage write.
        storage.set_writes_allowed(false);
        assert!(matches!(
            storage.set_app_state("update.schedule", "{}"),
            Err(StorageError::WritesBlocked)
        ));
        assert!(
            storage.get_app_state("update.schedule").is_ok(),
            "reads stay allowed"
        );
    }
}
