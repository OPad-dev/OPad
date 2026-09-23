use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use opad_layout::{Layout, Screen};
use opad_model::ui_source::SourceValue;
use opad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice, LatencyStats,
    LogLevel, RuntimeMode,
};
use opad_protocol::proto;

use crate::telemetry::DataSync;
use tracing::warn;

pub const COOLDOWN_DURATION: Duration = Duration::from_secs(5);
pub const HOST_STATUS_INTERVAL: Duration = Duration::from_secs(1);
pub const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
pub const RETRY_REWIND_MS: f64 = 2000.0;
/// How long after a play session has settled the automatic backup is written.
///
/// A backup is a storage write, so P1-3 forbids it during PLAYING and
/// COOLDOWN; this delay is measured from IDLE, not from the end of the map,
/// and the write re-checks IDLE when it fires. It is also long enough that
/// starting the next map immediately cancels the write instead of racing it.
/// Do not shorten it — the delay *is* the mechanism.
pub const AUTO_BACKUP_DELAY: Duration = Duration::from_secs(20);

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
    /// This install's identity (§W3-1). `None` when storage is unavailable, in
    /// which case ownership is neither claimed nor checked. Lives here rather
    /// than on the controller so the IPC handler resolving a takeover can
    /// reach it without a second copy to keep in step.
    pub install_id: Option<String>,
    /// Set when the connected pad says a different install owns it (§W3-3).
    /// Counter sync is blocked until the user answers.
    pub pending_takeover: Option<PendingTakeover>,
    /// This pad belongs to another installation and we are leaving it alone —
    /// while the prompt is open, and afterwards if the user said so. Nothing
    /// is pushed to it and nothing is synced from it. It keeps working as a
    /// keyboard throughout, which was never in question (§A.6.1).
    pub foreign_pad: bool,
    pub incompatible: Option<IncompatibleDevice>,
    pub ui_values: Vec<(u8, SourceValue)>,
    pub custom_layouts: HashMap<Screen, Layout>,
    /// When the last automatic counter backup was written, RFC 3339. Seeded
    /// at startup from the backup directory, so a restart does not read as
    /// "never".
    pub last_backup: Option<String>,
    /// The connected pad is one we know but came back blank (NVS erased:
    /// generation 1, zero counters, or DIAG_EVENT_NVS_ERASED). The next sync
    /// restores its counters from this PC; its default config is not adopted.
    pub nvs_restore_pending: bool,
}

impl DaemonState {
    /// Why nothing may be pushed to or synced from the connected pad right now,
    /// or `None` when it may. Every sync trigger and every IPC handler that
    /// writes counters, config or layouts to the pad asks this one question, so
    /// a pad owned by another install (§W3-3), or one waiting on the user's
    /// replacement answer, is never written behind the prompt's back.
    pub fn pad_guard(&self) -> Option<&'static str> {
        if self.foreign_pad || self.pending_takeover.is_some() {
            Some("Pad belongs to another install; resolve the takeover prompt first")
        } else if self.pending_replacement.is_some() {
            Some("A replacement pad is waiting on your answer; resolve that prompt first")
        } else {
            None
        }
    }

    pub fn may_touch_pad(&self) -> bool {
        self.pad_guard().is_none()
    }
}

/// DIAG_EVENT_NVS_ERASED in firmware/main/diag/diag.h
const DIAG_EVENT_NVS_ERASED: u32 = 8;

/// The four cases in the §W3-3 table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ownership {
    /// This install has no identity, so it claims nothing and prompts about
    /// nothing (§W3-1, degraded storage)
    NoIdentity,
    /// All-zero or absent: claim it silently
    Unclaimed,
    Ours,
    /// A different install: prompt, and block counter sync until answered
    Someone,
}

/// A pad that belongs to another installation, waiting on the user (§W3-3)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingTakeover {
    pub device_id: String,
    /// The pad's lifetime counters, which the prompt shows so the choice
    /// between keeping them and keeping this PC's is an informed one
    pub device_key1: u64,
    pub device_key2: u64,
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
    DeviceConnected(DeviceInfo, Option<DeviceConfig>),
    /// The pad's recorded owner, from the same HelloAck as the connect (§W3-3)
    DeviceOwnership(Vec<u8>),
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
    /// The pad names another install as its owner. Take it over without a
    /// prompt if that loses no presses (see [`pc_is_ahead`]); otherwise the
    /// prompt stays up for the user.
    TakeOverIfPadIsAhead,
    SendTimeSync,
    SendConfig(DeviceConfig),
    SaveDeviceConfig(DeviceConfig),
    SendHostStatus {
        tosu_connected: bool,
        is_playing: bool,
        play_id: u32,
    },
    SendDataUpdate(Vec<(u8, SourceValue)>),
    SendLayout(Screen, Layout),
    TriggerSync,
    /// Write this install's id into the pad's NVS (§W3-2). Only ever emitted
    /// for an unclaimed pad, or after the user chose to take one over.
    ClaimOwnership,
    RequestDeviceStatus,
    SetStorageWritesAllowed(bool),
    SaveInitialDeviceState(DeviceInfo, CounterState),
    TouchDeviceLastSeen(String),
    PushEspLog {
        level: LogLevel,
        tag: String,
        message: String,
    },
    /// Write a JSON counter backup to `<data>/backups` (§P1-3-safe: only ever
    /// emitted in IDLE, `AUTO_BACKUP_DELAY` after the post-play sync settled).
    WriteAutoBackup,
}

pub struct RuntimeController {
    pub state: DaemonState,
    pub pending_ops: PendingOperations,
    pub cooldown_deadline: Option<Instant>,
    /// Armed when a play session's sync settles into IDLE; fires one
    /// `WriteAutoBackup` and disarms. Cleared the moment the mode leaves IDLE,
    /// so a new map cancels the pending write rather than deferring it.
    pub backup_deadline: Option<Instant>,
    /// Set on the COOLDOWN -> SYNC transition and consumed by the matching
    /// `SyncCompleted`, so only a *post-play* sync arms a backup. The periodic
    /// five-minute idle sync must not: it would turn the rotation into a
    /// clock rather than a record of play sessions.
    backup_after_sync: bool,
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
    /// The owner the *currently connecting* pad reported. Overwritten by every
    /// HelloAck and cleared on disconnect, so it can never be the last pad's.
    reported_owner: Option<Vec<u8>>,
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
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            nvs_restore_pending: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: initial_layouts,
            last_backup: None,
        };

        Self {
            state,
            pending_ops: PendingOperations::default(),
            cooldown_deadline: None,
            backup_deadline: None,
            backup_after_sync: false,
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
            reported_owner: None,
        }
    }

    /// Supplies this install's identity (§W3-1). Set once at startup, before
    /// any pad can connect.
    pub fn set_install_id(&mut self, install_id: Option<String>) {
        self.state.install_id = install_id;
    }

    fn pc_is_ahead_of_pad(&self) -> bool {
        pc_is_ahead(self.state.pc_counters.as_ref(), &self.state.counters)
    }

    fn ownership_of(&self, owner_id: &[u8]) -> Ownership {
        if self.state.install_id.is_none() {
            // No identity to compare against and none to write. Behave exactly
            // as before this feature existed rather than prompting about
            // something the user cannot resolve.
            return Ownership::NoIdentity;
        }
        if crate::identity::is_unclaimed(owner_id) {
            return Ownership::Unclaimed;
        }
        if crate::identity::owns(self.state.install_id.as_deref(), owner_id) {
            return Ownership::Ours;
        }
        Ownership::Someone
    }

    pub fn on_event(&mut self, event: RuntimeEvent, now: Instant) -> Vec<RuntimeAction> {
        let mut actions = Vec::new();

        match event {
            RuntimeEvent::DeviceConnected(info, dev_cfg) => {
                self.state.device_connected = true;
                self.state.counters_source = CounterSource::Device;

                // A pad we have counters for, reporting a factory-fresh NVS:
                // its flash was erased. Its config is the firmware default, not
                // a choice anyone made, so ours is pushed rather than replaced.
                let c = &self.state.counters;
                let nvs_wiped = self.known_devices.contains(&info.device_id)
                    && c.counter_generation <= 1
                    && c.lifetime_key1 == 0
                    && c.lifetime_key2 == 0
                    && self.state.pc_counters.as_ref().is_some_and(|pc| {
                        pc.device_id == info.device_id
                            && (pc.lifetime_key1 > 0 || pc.lifetime_key2 > 0)
                    });
                self.state.nvs_restore_pending = nvs_wiped;
                if nvs_wiped {
                    warn!(
                        "{} came back with blank counters (NVS erased?); restoring them from this PC",
                        info.device_id
                    );
                }

                if let Some(cfg) = dev_cfg.as_ref().filter(|_| !nvs_wiped) {
                    let mut adopted = cfg.clone();
                    if adopted.tosu_endpoint.trim().is_empty() {
                        adopted.tosu_endpoint = if self.state.config.tosu_endpoint.trim().is_empty()
                        {
                            opad_model::DeviceConfig::default().tosu_endpoint
                        } else {
                            self.state.config.tosu_endpoint.clone()
                        };
                    }
                    self.state.config = adopted.clone();
                    actions.push(RuntimeAction::SaveDeviceConfig(adopted));
                }

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

                // Ownership is decided here, in the same HelloAck that brought
                // the counters, and *before* anything reconciles them. R3 was
                // exactly this bug in its replacement form: the connect handler
                // saw the previous pad's counters, the prompt never fired, and
                // old counters were saved under the new device_id (§W3-3).
                let owner = self.reported_owner.take().unwrap_or_default();
                match self.ownership_of(&owner) {
                    Ownership::NoIdentity | Ownership::Ours => {
                        self.state.pending_takeover = None;
                        self.state.foreign_pad = false;
                    }
                    Ownership::Unclaimed => {
                        // The common case, and it is silent: a pad nobody owns
                        // is claimed on first connect with no prompt.
                        self.state.pending_takeover = None;
                        self.state.foreign_pad = false;
                        actions.push(RuntimeAction::ClaimOwnership);
                    }
                    Ownership::Someone => {
                        self.state.pending_takeover = Some(PendingTakeover {
                            device_id: info.device_id.clone(),
                            device_key1: self.state.counters.lifetime_key1,
                            device_key2: self.state.counters.lifetime_key2,
                        });
                        self.state.foreign_pad = true;
                        actions.push(RuntimeAction::TakeOverIfPadIsAhead);
                    }
                }

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
                        // Restoring only makes sense if it gives back presses
                        && self.pc_is_ahead_of_pad()
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
                if (dev_cfg.is_none() || nvs_wiped) && !self.state.foreign_pad {
                    actions.push(RuntimeAction::SendConfig(self.state.config.clone()));
                }
                actions.push(RuntimeAction::SendHostStatus {
                    tosu_connected: self.state.tosu_connected,
                    is_playing: self.state.mode == RuntimeMode::Playing,
                    play_id: self.play_id,
                });
                self.data_sync.reset_sent();

                if !self.state.foreign_pad {
                    for (screen, layout) in &self.state.custom_layouts {
                        actions.push(RuntimeAction::SendLayout(*screen, layout.clone()));
                    }
                }

                if self.state.storage_error.is_none()
                    // A pad owned by another install syncs nothing until the
                    // user answers: no counter may ever be written under the
                    // wrong owner (§W3-3).
                    && self.state.may_touch_pad()
                    && self.state.mode == RuntimeMode::Idle
                {
                    actions.push(RuntimeAction::TriggerSync);
                }

                self.last_periodic_sync = now;
                self.last_periodic_time_sync = now;
                self.last_synced_counters = Some(self.state.counters.clone());
            }

            RuntimeEvent::DeviceOwnership(owner_id) => {
                // Recorded, not judged. The decision belongs in the connect
                // arm, where the pad's identity and counters are also known.
                self.reported_owner = Some(owner_id);
            }

            RuntimeEvent::DeviceDisconnected => {
                self.state.device_connected = false;
                self.state.counters_source = CounterSource::Pc;
                self.state.esp_counters = None;
                // Never carry one pad's owner into the next pad's connect
                self.reported_owner = None;
                self.state.pending_takeover = None;
                self.state.foreign_pad = false;
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
                    if ev.event_id == DIAG_EVENT_NVS_ERASED
                        && self.state.device_connected
                        && !self.state.foreign_pad
                        && !self.state.nvs_restore_pending
                    {
                        warn!(
                            "The pad reports its NVS was erased; restoring counters from this PC"
                        );
                        self.state.nvs_restore_pending = true;
                        if self.state.mode == RuntimeMode::Idle && self.state.may_touch_pad() {
                            actions.push(RuntimeAction::TriggerSync);
                        }
                    }
                    let level = match ev.level {
                        0 => LogLevel::Debug,
                        1 => LogLevel::Info,
                        2 => LogLevel::Warn,
                        3 => LogLevel::Error,
                        _ => LogLevel::Info,
                    };
                    let msg = if ev.event_id != 0 {
                        opad_model::diag::format_diag_event(ev.event_id, ev.arg0, ev.arg1)
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
                        // A map started inside the backup window: cancel the
                        // pending write outright rather than deferring it. The
                        // next session's sync arms a fresh one.
                        self.backup_deadline = None;
                        self.backup_after_sync = false;
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
                        if !changes.is_empty() && !self.state.foreign_pad {
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
                        if now >= deadline && !self.state.may_touch_pad() {
                            // Nothing is synced from a pad we are leaving
                            // alone (§W3-3), and its counters are not ours to
                            // back up.
                            self.state.mode = RuntimeMode::Idle;
                            self.cooldown_deadline = None;
                            actions.push(RuntimeAction::SetStorageWritesAllowed(true));
                        } else if now >= deadline {
                            self.state.mode = RuntimeMode::Sync;
                            self.cooldown_deadline = None;
                            // A play session just ended, so the sync that
                            // follows is the one that arms a backup.
                            self.backup_after_sync = true;
                            // SYNC is the phase where persistence is allowed again (§11.3)
                            actions.push(RuntimeAction::SetStorageWritesAllowed(true));
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
                if self.state.device_connected && !self.state.foreign_pad {
                    let playing_hz = self.state.config.gameplay_display_hz;
                    let changes = self.data_sync.take_changes(is_playing, playing_hz, false);
                    if !changes.is_empty() {
                        actions.push(RuntimeAction::SendDataUpdate(changes));
                    }
                }

                // 5. Periodic idle sync and time sync
                if self.state.device_connected
                    && !self.state.foreign_pad
                    && self.state.mode == RuntimeMode::Idle
                {
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
                        if changed
                            && self.state.storage_error.is_none()
                            && self.state.may_touch_pad()
                        {
                            actions.push(RuntimeAction::TriggerSync);
                        }
                    }
                }

                // 6. The automatic counter backup, once the post-play sync has
                //    settled and nothing has started since. The IDLE check is
                //    repeated here rather than trusted from arming time: it is
                //    a storage write, and P1-3 is absolute.
                if let Some(deadline) = self.backup_deadline {
                    if self.state.mode != RuntimeMode::Idle {
                        self.backup_deadline = None;
                    } else if now >= deadline {
                        self.backup_deadline = None;
                        actions.push(RuntimeAction::WriteAutoBackup);
                    }
                }
            }

            RuntimeEvent::SyncCompleted {
                success,
                counters,
                time_str,
                error,
            } => {
                // Syncs run in the background, so a map may have started meanwhile:
                // never pull the mode out of PLAYING/COOLDOWN or re-enable writes there
                if success {
                    self.state.counters = counters.clone();
                    self.state.pc_counters = Some(counters.clone());
                    self.state.esp_counters = Some(counters.clone());
                    self.state.last_sync_time = time_str;
                    self.state.last_sync_error = None;
                    self.state.nvs_restore_pending = false;
                    self.last_synced_counters = Some(counters);
                    self.last_periodic_sync = now;
                    self.last_periodic_time_sync = now;
                    if let Some(info) = &self.state.device_info {
                        self.known_devices.insert(info.device_id.clone());
                    }
                } else {
                    self.state.last_sync_error = error;
                }
                if self.state.mode == RuntimeMode::Sync {
                    self.state.mode = RuntimeMode::Idle;
                }
                if self.state.mode == RuntimeMode::Idle {
                    actions.push(RuntimeAction::SetStorageWritesAllowed(true));
                }

                // Arm the automatic backup, but only for the sync that a play
                // session triggered, and only if a new map has not already
                // pulled us back out of IDLE. Armed even when the sync failed:
                // the counters we hold are then the best record there is, and
                // "the pad might die" is exactly the case this exists for.
                if std::mem::take(&mut self.backup_after_sync)
                    && self.state.mode == RuntimeMode::Idle
                {
                    self.backup_deadline = Some(now + AUTO_BACKUP_DELAY);
                }
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

/// Runs one event through the controller against the shared state.
///
/// IPC handlers and `perform_sync` write to the shared `DaemonState` directly (config, layouts,
/// counters, replacement choice). Pulling it in before the event and publishing the result under
/// the same lock keeps those writes instead of overwriting them with a stale controller copy.
/// Whether this PC holds more presses than the pad on either key, i.e. whether
/// siding with the pad would lose any. Only then is the user asked; when the
/// pad has at least as many on both keys its counters are simply kept.
pub fn pc_is_ahead(pc: Option<&CounterState>, pad: &CounterState) -> bool {
    pc.is_some_and(|pc| {
        pc.lifetime_key1 > pad.lifetime_key1 || pc.lifetime_key2 > pad.lifetime_key2
    })
}

pub fn apply_event(
    controller: &mut RuntimeController,
    shared: &Arc<Mutex<DaemonState>>,
    pending_ops: &Arc<Mutex<PendingOperations>>,
    event: RuntimeEvent,
    now: Instant,
) -> Vec<RuntimeAction> {
    let actions = {
        let mut ds = shared.lock();
        // A replacement prompt resolved over IPC: remember the pad so it is not asked again
        if controller.state.pending_replacement.is_some() && ds.pending_replacement.is_none() {
            if let Some(info) = &ds.device_info {
                controller.known_devices.insert(info.device_id.clone());
            }
        }
        controller.state = ds.clone();
        let actions = controller.on_event(event, now);
        *ds = controller.state.clone();
        actions
    };
    // perform_sync drains the shared queue, so hand over what the controller deferred
    if let Some(id) = controller.pending_ops.pending_last_seen.take() {
        pending_ops.lock().pending_last_seen = Some(id);
    }
    actions
}
