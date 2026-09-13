use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice, LatencyStats,
    LogLevel, RuntimeMode,
};
use osupad_protocol::proto;

use crate::telemetry::DataSync;

pub const COOLDOWN_DURATION: Duration = Duration::from_secs(5);
pub const HOST_STATUS_INTERVAL: Duration = Duration::from_secs(1);
pub const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
pub const RETRY_REWIND_MS: f64 = 2000.0;

#[derive(Clone, Debug)]
pub struct DaemonState {
    pub mode: RuntimeMode,
    pub device_connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub counters: CounterState,
    pub counters_source: CounterSource,
    pub pc_counters: Option<CounterState>,
    pub esp_counters: Option<CounterState>,
    pub config: DeviceConfig,
    pub last_sync_time: Option<String>,
    pub last_sync_error: Option<String>,
    pub storage_error: Option<String>,
    pub tosu_connected: bool,
    pub latency: Option<LatencyStats>,
    pub pending_replacement: Option<String>,
    pub incompatible: Option<IncompatibleDevice>,
    pub ui_values: Vec<(u8, SourceValue)>,
    pub custom_layouts: HashMap<Screen, Layout>,
}

#[derive(Default, Clone, Debug)]
pub struct PendingOperations {
    pub pending_config: Option<DeviceConfig>,
    pub pending_layouts: Vec<(Screen, Option<Layout>)>,
    pub pending_device_push: bool,
    pub pending_last_seen: Option<String>,
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    DeviceConnected(DeviceInfo),
    DeviceDisconnected,
    DeviceCounters(CounterState),
    DeviceLatency(LatencyStats),
    DeviceLayoutAck {
        screen: u8,
        success: bool,
        message: String,
    },
    DeviceLogBatch(proto::LogEventBatch),
    TosuTelemetry {
        is_playing: bool,
        live_time_ms: f64,
        title: String,
        values: Vec<(u8, SourceValue)>,
    },
    TosuConnectionChanged(bool),
    Tick(Instant),
    SyncCompleted {
        success: bool,
        counters: CounterState,
        time_str: Option<String>,
        error: Option<String>,
    },
    SqliteReconnected {
        config: Option<DeviceConfig>,
        layouts: Vec<(Screen, Layout)>,
        stored_device: Option<CounterState>,
    },
    SqliteError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeAction {
    SendTimeSync,
    SendConfig(DeviceConfig),
    SendHostStatus {
        tosu_connected: bool,
        is_playing: bool,
        play_id: u32,
    },
    SendDataUpdate(Vec<(u8, SourceValue)>),
    SendLayout(Screen, Layout),
    TriggerSync,
    RequestDeviceStatus,
    SetStorageWritesAllowed(bool),
    SaveInitialDeviceState(DeviceInfo, CounterState),
    TouchDeviceLastSeen(String),
    PushEspLog {
        level: LogLevel,
        tag: String,
        message: String,
    },
}

pub struct RuntimeController {
    pub state: DaemonState,
    pub pending_ops: PendingOperations,
    pub cooldown_deadline: Option<Instant>,
    pub last_host_status: Instant,
    pub last_status_poll: Instant,
    pub last_periodic_sync: Instant,
    pub last_periodic_time_sync: Instant,
    pub last_synced_counters: Option<CounterState>,
    pub play_id: u32,
    pub last_live_ms: Option<f64>,
    pub data_sync: DataSync,
    pub known_devices: HashSet<String>,
    pub has_initial_db_entry: bool,
}

impl RuntimeController {
    pub fn new(
        initial_config: DeviceConfig,
        initial_device: Option<DeviceInfo>,
        initial_counters: CounterState,
        initial_layouts: HashMap<Screen, Layout>,
        storage_error: Option<String>,
        existing_device_ids: Vec<String>,
        now: Instant,
    ) -> Self {
        let mut known = HashSet::new();
        for id in &existing_device_ids {
            known.insert(id.clone());
        }
        let has_initial_db_entry = !existing_device_ids.is_empty() || initial_device.is_some();
        if let Some(d) = &initial_device {
            known.insert(d.device_id.clone());
        }

        let state = DaemonState {
            mode: RuntimeMode::Idle,
            device_connected: false,
            device_info: initial_device,
            counters: initial_counters.clone(),
            counters_source: CounterSource::Pc,
            pc_counters: if initial_counters.device_id.is_empty() {
                None
            } else {
                Some(initial_counters.clone())
            },
            esp_counters: None,
            config: initial_config,
            last_sync_time: None,
            last_sync_error: None,
            storage_error,
            tosu_connected: false,
            latency: None,
            pending_replacement: None,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: initial_layouts,
        };

        Self {
            state,
            pending_ops: PendingOperations::default(),
            cooldown_deadline: None,
            last_host_status: now - HOST_STATUS_INTERVAL,
            last_status_poll: now,
            last_periodic_sync: now,
            last_periodic_time_sync: now,
            last_synced_counters: None,
            play_id: chrono::Utc::now().timestamp() as u32,
            last_live_ms: None,
            data_sync: DataSync::default(),
            known_devices: known,
            has_initial_db_entry,
        }
    }

    pub fn on_event(&mut self, event: RuntimeEvent, now: Instant) -> Vec<RuntimeAction> {
        let mut actions = Vec::new();

        match event {
            RuntimeEvent::DeviceConnected(info) => {
                self.state.device_connected = true;
                self.state.counters_source = CounterSource::Device;

                if info.protocol_version != 1 {
                    self.state.incompatible = Some(IncompatibleDevice {
                        firmware_version: info.firmware_version.clone(),
                        protocol_version: info.protocol_version,
                    });
                    return actions;
                }
                self.state.incompatible = None;
                self.state.device_info = Some(info.clone());
                self.state.esp_counters = Some(self.state.counters.clone());

                let is_known = self.known_devices.contains(&info.device_id);
                if !is_known {
                    if self.known_devices.is_empty() && !self.has_initial_db_entry {
                        self.has_initial_db_entry = true;
                        self.known_devices.insert(info.device_id.clone());
                        if self.state.mode == RuntimeMode::Idle {
                            actions.push(RuntimeAction::SaveInitialDeviceState(
                                info.clone(),
                                self.state.counters.clone(),
                            ));
                            actions
                                .push(RuntimeAction::TouchDeviceLastSeen(info.device_id.clone()));
                        } else {
                            self.pending_ops.pending_last_seen = Some(info.device_id.clone());
                        }
                    } else if self.state.counters.counter_generation <= 1
                        && self.state.counters.lifetime_key1 < 1000
                        && self.state.counters.lifetime_key2 < 1000
                    {
                        let prev_id = self
                            .known_devices
                            .iter()
                            .next()
                            .cloned()
                            .unwrap_or_else(|| "previous".to_string());
                        self.state.pending_replacement = Some(prev_id);
                    } else {
                        self.known_devices.insert(info.device_id.clone());
                        if self.state.mode == RuntimeMode::Idle {
                            actions.push(RuntimeAction::SaveInitialDeviceState(
                                info.clone(),
                                self.state.counters.clone(),
                            ));
                            actions
                                .push(RuntimeAction::TouchDeviceLastSeen(info.device_id.clone()));
                        } else {
                            self.pending_ops.pending_last_seen = Some(info.device_id.clone());
                        }
                    }
                } else if self.state.mode == RuntimeMode::Idle {
                    actions.push(RuntimeAction::TouchDeviceLastSeen(info.device_id.clone()));
                } else {
                    self.pending_ops.pending_last_seen = Some(info.device_id.clone());
                }

                actions.push(RuntimeAction::SendTimeSync);
                actions.push(RuntimeAction::SendConfig(self.state.config.clone()));
                actions.push(RuntimeAction::SendHostStatus {
                    tosu_connected: self.state.tosu_connected,
                    is_playing: self.state.mode == RuntimeMode::Playing,
                    play_id: self.play_id,
                });
                self.data_sync.reset_sent();

                for (screen, layout) in &self.state.custom_layouts {
                    actions.push(RuntimeAction::SendLayout(*screen, layout.clone()));
                }

                if self.state.storage_error.is_none()
                    && self.state.pending_replacement.is_none()
                    && self.state.mode == RuntimeMode::Idle
                {
                    actions.push(RuntimeAction::TriggerSync);
                }

                self.last_periodic_sync = now;
                self.last_periodic_time_sync = now;
                self.last_synced_counters = Some(self.state.counters.clone());
            }

            RuntimeEvent::DeviceDisconnected => {
                self.state.device_connected = false;
                self.state.counters_source = CounterSource::Pc;
                self.state.esp_counters = None;
            }

            RuntimeEvent::DeviceCounters(c) => {
                self.state.counters.lifetime_key1 = c.lifetime_key1;
                self.state.counters.lifetime_key2 = c.lifetime_key2;
                if !c.device_id.is_empty() {
                    self.state.counters.device_id = c.device_id;
                    self.state.counters.counter_generation = c.counter_generation;
                } else {
                    self.state.counters.map_key1 = c.map_key1;
                    self.state.counters.map_key2 = c.map_key2;
                }
                self.state.esp_counters = Some(self.state.counters.clone());
            }

            RuntimeEvent::DeviceLatency(stats) => {
                self.state.latency = Some(stats);
            }

            RuntimeEvent::DeviceLayoutAck { .. } => {}

            RuntimeEvent::DeviceLogBatch(batch) => {
                for ev in batch.events {
                    let level = match ev.level {
                        0 => LogLevel::Debug,
                        1 => LogLevel::Info,
                        2 => LogLevel::Warn,
                        3 => LogLevel::Error,
                        _ => LogLevel::Info,
                    };
                    let msg = if ev.event_id != 0 {
                        osupad_model::diag::format_diag_event(ev.event_id, ev.arg0, ev.arg1)
                    } else if !ev.message.is_empty() {
                        ev.message
                    } else {
                        format!("ESP event {}", ev.event_id)
                    };
                    actions.push(RuntimeAction::PushEspLog {
                        level,
                        tag: "firmware".to_string(),
                        message: msg,
                    });
                }
            }

            RuntimeEvent::TosuTelemetry {
                is_playing,
                live_time_ms,
                title: _,
                values,
            } => {
                let current_mode = self.state.mode;
                self.data_sync.ingest(values.iter().cloned());
                self.state.ui_values = values;

                if is_playing {
                    let rewound = self
                        .last_live_ms
                        .is_some_and(|prev| live_time_ms < prev - RETRY_REWIND_MS);
                    let new_attempt = current_mode != RuntimeMode::Playing || rewound;
                    self.last_live_ms = Some(live_time_ms);
                    if new_attempt {
                        self.play_id = self.play_id.wrapping_add(1);
                    }

                    if current_mode != RuntimeMode::Playing {
                        self.state.mode = RuntimeMode::Playing;
                        self.cooldown_deadline = None;
                        actions.push(RuntimeAction::SetStorageWritesAllowed(false));
                    }

                    if new_attempt {
                        actions.push(RuntimeAction::SendHostStatus {
                            tosu_connected: true,
                            is_playing: true,
                            play_id: self.play_id,
                        });
                        self.last_host_status = now;
                        let playing_hz = self.state.config.gameplay_display_hz;
                        let changes = self.data_sync.take_changes(true, playing_hz, true);
                        if !changes.is_empty() {
                            actions.push(RuntimeAction::SendDataUpdate(changes));
                        }
                    }
                } else if current_mode == RuntimeMode::Playing {
                    self.last_live_ms = None;
                    self.state.mode = RuntimeMode::Cooldown;
                    self.cooldown_deadline = Some(now + COOLDOWN_DURATION);
                    actions.push(RuntimeAction::SetStorageWritesAllowed(false));
                    actions.push(RuntimeAction::SendHostStatus {
                        tosu_connected: true,
                        is_playing: false,
                        play_id: self.play_id,
                    });
                }
            }

            RuntimeEvent::TosuConnectionChanged(connected) => {
                let was_playing = self.state.mode == RuntimeMode::Playing;
                self.state.tosu_connected = connected;

                if !connected && was_playing {
                    self.state.mode = RuntimeMode::Cooldown;
                    self.cooldown_deadline = Some(now + COOLDOWN_DURATION);
                    actions.push(RuntimeAction::SetStorageWritesAllowed(false));
                    self.last_live_ms = None;
                }
                if !connected {
                    self.data_sync.clear();
                }
                actions.push(RuntimeAction::SendHostStatus {
                    tosu_connected: connected,
                    is_playing: false,
                    play_id: self.play_id,
                });
                self.last_host_status = now;
            }

            RuntimeEvent::Tick(now) => {
                // 1. Cooldown expiration -> SYNC
                if self.state.mode == RuntimeMode::Cooldown {
                    if let Some(deadline) = self.cooldown_deadline {
                        if now >= deadline {
                            self.state.mode = RuntimeMode::Sync;
                            self.cooldown_deadline = None;
                            actions.push(RuntimeAction::TriggerSync);
                        }
                    }
                }

                // 2. Status poll
                if now.duration_since(self.last_status_poll) >= STATUS_POLL_INTERVAL {
                    self.last_status_poll = now;
                    if self.state.device_connected {
                        actions.push(RuntimeAction::RequestDeviceStatus);
                    }
                }

                // 3. Host status heartbeat
                let is_playing = self.state.mode == RuntimeMode::Playing;
                if self.state.device_connected
                    && now.duration_since(self.last_host_status) >= HOST_STATUS_INTERVAL
                {
                    self.last_host_status = now;
                    actions.push(RuntimeAction::SendHostStatus {
                        tosu_connected: self.state.tosu_connected,
                        is_playing,
                        play_id: self.play_id,
                    });
                }

                // 4. UI data updates
                if self.state.device_connected {
                    let playing_hz = self.state.config.gameplay_display_hz;
                    let changes = self.data_sync.take_changes(is_playing, playing_hz, false);
                    if !changes.is_empty() {
                        actions.push(RuntimeAction::SendDataUpdate(changes));
                    }
                }

                // 5. Periodic idle sync and time sync
                if self.state.device_connected && self.state.mode == RuntimeMode::Idle {
                    if now.duration_since(self.last_periodic_time_sync) >= Duration::from_secs(600)
                    {
                        self.last_periodic_time_sync = now;
                        actions.push(RuntimeAction::SendTimeSync);
                    }

                    if now.duration_since(self.last_periodic_sync) >= Duration::from_secs(300) {
                        self.last_periodic_sync = now;
                        let current = &self.state.counters;
                        let changed = match &self.last_synced_counters {
                            Some(prev) => {
                                prev.lifetime_key1 != current.lifetime_key1
                                    || prev.lifetime_key2 != current.lifetime_key2
                            }
                            None => true,
                        };
                        if changed && self.state.storage_error.is_none() {
                            actions.push(RuntimeAction::TriggerSync);
                        }
                    }
                }
            }

            RuntimeEvent::SyncCompleted {
                success,
                counters,
                time_str,
                error,
            } => {
                if success {
                    self.state.counters = counters.clone();
                    self.state.pc_counters = Some(counters.clone());
                    self.state.esp_counters = Some(counters.clone());
                    self.state.last_sync_time = time_str;
                    self.state.last_sync_error = None;
                    self.state.mode = RuntimeMode::Idle;
                    self.last_synced_counters = Some(counters);
                    self.last_periodic_sync = now;
                    self.last_periodic_time_sync = now;
                } else {
                    self.state.last_sync_error = error;
                    self.state.mode = RuntimeMode::Idle;
                }
                actions.push(RuntimeAction::SetStorageWritesAllowed(true));
            }

            RuntimeEvent::SqliteReconnected {
                config,
                layouts,
                stored_device,
            } => {
                self.state.storage_error = None;
                if let Some(cfg) = config {
                    self.state.config = cfg;
                }
                for (s, l) in layouts {
                    self.state.custom_layouts.insert(s, l);
                }
                if let Some(st) = stored_device {
                    self.state.pc_counters = Some(st);
                }
            }

            RuntimeEvent::SqliteError(err) => {
                self.state.storage_error = Some(err);
            }
        }

        actions
    }
}
