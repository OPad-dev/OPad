use anyhow::Result;
use chrono::Utc;
use osupad_device::{DeviceEvent, DeviceManager};
use osupad_ipc::{
    create_listener, get_socket_path, read_request, send_response, CurrentBackupState, IpcRequest,
    IpcResponse, IPC_PROTOCOL_VERSION,
};
use osupad_model::{
    char_to_hid_usage, CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice,
    JsonBackup, LatencyStats, RuntimeMode,
};
use osupad_storage::{reconcile_counters, Storage};
use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{LogLevel, LogSource};
use osupad_tosu::{spawn_tosu_supervisor, TosuManager};
mod log_hub;
mod telemetry;

use log_hub::{LogHub, LogHubLayer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const COOLDOWN_DURATION: Duration = Duration::from_secs(5);
/// Firmware drops out of PLAYING after 3s without a playing HostStatus, and treats
/// status older than 10s as unknown
const HOST_STATUS_INTERVAL: Duration = Duration::from_secs(1);
/// Poll DeviceStatus for live lifetime / current-map counters
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// A song position jump backwards larger than this is a retry of the same map
const RETRY_REWIND_MS: f64 = 2000.0;

#[derive(Clone)]
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
    /// Latest tosu-derived UI values (for the designer's live preview)
    pub ui_values: Vec<(u8, SourceValue)>,
    pub custom_layouts: HashMap<Screen, Layout>,
}

#[derive(Default)]
pub struct PendingOperations {
    pub pending_config: Option<DeviceConfig>,
    pub pending_layouts: Vec<(Screen, Option<Layout>)>,
    pub pending_device_push: bool,
    pub pending_last_seen: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let log_hub = LogHub::new();
    let fmt_layer = tracing_subscriber::fmt::layer();
    let hub_layer = LogHubLayer::new(log_hub.clone());
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(tracing::Level::INFO.into());

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(hub_layer)
        .init();

    info!("Starting osupad-daemon v1.0.0");

    let socket_path = get_socket_path();
    if socket_path.exists() {
        if tokio::net::UnixStream::connect(&socket_path).await.is_ok() {
            anyhow::bail!(
                "Another osupad-daemon instance is already running on socket {}",
                socket_path.display()
            );
        }
    }

    let db_path = get_database_path();
    info!("Using SQLite database at {}", db_path.display());
    let (storage, storage_error, initial_config, initial_device, initial_counters, initial_layouts) = match Storage::open(&db_path) {
        Ok(s) => {
            let cfg = s.load_config().unwrap_or_default();
            let latest = s.load_latest_device_state().unwrap_or(None);
            let mut layouts = HashMap::new();
            for screen in Screen::ALL {
                if let Ok(Some(json)) = s.load_layout(screen.to_wire()) {
                    if let Ok(layout) = Layout::from_json(&json) {
                        layouts.insert(*screen, layout);
                    }
                }
            }
            let (init_dev, init_cnt) = match latest {
                Some((info, counters)) => (Some(info), counters),
                None => (None, CounterState::default()),
            };
            info!("osupad-daemon initialized, storage loaded successfully");
            (Arc::new(Mutex::new(Some(s))), None, cfg, init_dev, init_cnt, layouts)
        }
        Err(e) => {
            let err_msg = format!("Database error ({}): {}", db_path.display(), e);
            error!("Failed to open/migrate SQLite database: {}. Starting in degraded mode (§P2-12)", err_msg);
            (
                Arc::new(Mutex::new(None)),
                Some(err_msg),
                DeviceConfig::default(),
                None,
                CounterState::default(),
                HashMap::new(),
            )
        }
    };

    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: false,
        device_info: initial_device,
        counters: initial_counters.clone(),
        counters_source: CounterSource::Pc,
        pc_counters: if initial_counters.device_id.is_empty() { None } else { Some(initial_counters) },
        esp_counters: None,
        config: initial_config.clone(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: initial_layouts,
    }));

    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    // Launch and supervise tosu, then follow its WebSocket
    spawn_tosu_supervisor(initial_config.tosu_endpoint.clone(), get_tosu_log_path());
    let (tosu_manager, mut tosu_rx) = TosuManager::new(initial_config.tosu_endpoint.clone());
    let mut tosu_connected_rx = tosu_manager.subscribe_connected();
    tosu_manager.start();

    // Start Device CDC manager
    let (device_manager, mut device_rx) = DeviceManager::new();
    let device_manager = Arc::new(device_manager);

    // Setup Local IPC Server
    let socket_path = get_socket_path();
    let ipc_listener = match create_listener(&socket_path) {
        Ok(l) => l,
        Err(osupad_ipc::IpcError::AlreadyRunning) => {
            eprintln!("osupad-daemon is already running");
            std::process::exit(0);
        }
        Err(e) => return Err(e.into()),
    };

    // Spawn IPC request handling task
    {
        let daemon_state = daemon_state.clone();
        let storage = storage.clone();
        let device_manager = device_manager.clone();
        let log_hub = log_hub.clone();
        let pending_ops = pending_ops.clone();

        tokio::spawn(async move {
            loop {
                match ipc_listener.accept().await {
                    Ok((mut stream, _)) => {
                        let daemon_state = daemon_state.clone();
                        let storage = storage.clone();
                        let device_manager = device_manager.clone();
                        let log_hub = log_hub.clone();
                        let pending_ops = pending_ops.clone();

                        tokio::spawn(async move {
                            while let Ok(req) = read_request(&mut stream).await {
                                let resp = handle_ipc_request(
                                    req,
                                    &daemon_state,
                                    &storage,
                                    &device_manager,
                                    &log_hub,
                                    &pending_ops,
                                )
                                .await;
                                if send_response(&mut stream, &resp).await.is_err() {
                                    break;
                                }
                            }
                        });
                    }
                    Err(e) => {
                        warn!("IPC accept error: {}", e);
                    }
                }
            }
        });
    }

    // Main Runtime Coordinator loop (§11: IDLE <-> PLAYING <-> COOLDOWN <-> SYNC)
    let mut cooldown_deadline: Option<Instant> = None;
    let mut data_sync = telemetry::DataSync::default();
    // An interval, not a fresh sleep per select! iteration: tosu frames arrive every
    // ~150ms and would otherwise starve this branch (cooldown would never expire)
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    let mut last_host_status = Instant::now() - HOST_STATUS_INTERVAL;
    let mut last_status_poll = Instant::now();
    // Identifies the current osu! attempt; the device zeroes its map counters when it changes.
    // Seeded from the clock so a daemon restart never reuses the device's last id.
    let mut play_id = Utc::now().timestamp() as u32;
    let mut last_live_ms: Option<f64> = None;

    let mut last_periodic_sync = Instant::now();
    let mut last_synced_counters: Option<CounterState> = None;
    let mut last_periodic_time_sync = Instant::now();
    let mut last_instant = Instant::now();
    let mut last_system_time = std::time::SystemTime::now();
    let mut last_db_retry = Instant::now();

    loop {
        tokio::select! {
            // 1. Device hardware events
            Ok(dev_event) = device_rx.recv() => {
                match dev_event {
                    DeviceEvent::Connected(info) => {
                        info!("ESP32 Device Connected: ID={}, Board={}", info.device_id, info.board_profile);

                        let mut st = daemon_state.lock().unwrap();
                        st.device_connected = true;
                        st.counters_source = CounterSource::Device;

                        // Protocol version check (§P1-7)
                        if info.protocol_version != 1 {
                            warn!("Incompatible ESP32 protocol version: {}", info.protocol_version);
                            st.incompatible = Some(IncompatibleDevice {
                                firmware_version: info.firmware_version.clone(),
                                protocol_version: info.protocol_version,
                            });
                            continue;
                        }
                        st.incompatible = None;
                        st.device_info = Some(info.clone());

                        // Handle device registration & replacement (§P1-2)
                        let stored_row = {
                            let s_guard = storage.lock().unwrap();
                            s_guard.as_ref().and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
                        };
                        st.pc_counters = stored_row.clone();
                        st.esp_counters = Some(st.counters.clone());

                        if let Some(s) = storage.lock().unwrap().as_ref() {
                            if stored_row.is_none() {
                                let existing_states = s.list_device_states().unwrap_or_default();

                                if existing_states.is_empty() {
                                    // First device ever seen: import into SQLite
                                    if st.mode == RuntimeMode::Idle {
                                        let _ = s.save_device_state(&info, &st.counters);
                                        let _ = s.touch_device_last_seen(&info.device_id);
                                    } else {
                                        pending_ops.lock().unwrap().pending_last_seen = Some(info.device_id.clone());
                                    }
                                } else if st.counters.counter_generation <= 1 && st.counters.lifetime_key1 < 1000 && st.counters.lifetime_key2 < 1000 {
                                    // Fresh replacement device
                                    st.pending_replacement = Some(existing_states[0].0.device_id.clone());
                                    warn!("Detected potential ESP replacement with device_id: {}", info.device_id);
                                } else {
                                    // Distinct device with real counters: register it
                                    if st.mode == RuntimeMode::Idle {
                                        let _ = s.save_device_state(&info, &st.counters);
                                        let _ = s.touch_device_last_seen(&info.device_id);
                                    } else {
                                        pending_ops.lock().unwrap().pending_last_seen = Some(info.device_id.clone());
                                    }
                                }
                            } else if st.mode == RuntimeMode::Idle {
                                let _ = s.touch_device_last_seen(&info.device_id);
                            } else {
                                pending_ops.lock().unwrap().pending_last_seen = Some(info.device_id.clone());
                            }
                        }

                        // Send time sync, config and host status immediately (§14.3).
                        let dm = device_manager.clone();
                        let (tosu, playing) = (st.tosu_connected, st.mode == RuntimeMode::Playing);
                        let config = st.config.clone();
                        let current_play = play_id;
                        tokio::spawn(async move {
                            let _ = dm.send_time_sync().await;
                            let _ = dm.send_config(&config).await;
                            let _ = dm.send_host_status(tosu, playing, current_play).await;
                        });
                        // The device starts with empty data: resend every value
                        data_sync.reset_sent();

                        // Re-push saved layouts (the device skips the flash write when unchanged)
                        let layouts: Vec<(Screen, Layout)> = st.custom_layouts.iter().map(|(s, l)| (*s, l.clone())).collect();
                        if !layouts.is_empty() {
                            let dm = device_manager.clone();
                            tokio::spawn(async move {
                                for (screen, layout) in layouts {
                                    let _ = dm.send_layout(screen, &layout).await;
                                }
                            });
                        }

                        // P1-2: Reconcile on connect if Idle, storage is available, and not pending replacement
                        if storage.lock().unwrap().is_some() && st.pending_replacement.is_none() && st.mode == RuntimeMode::Idle {
                            let ds = daemon_state.clone();
                            let stg = storage.clone();
                            let dm_clone = device_manager.clone();
                            let po = pending_ops.clone();
                            tokio::spawn(async move {
                                perform_sync(&ds, &stg, &dm_clone, &po).await;
                            });
                        }
                        last_periodic_sync = Instant::now();
                        last_periodic_time_sync = Instant::now();
                        last_synced_counters = Some(st.counters.clone());
                    }
                    DeviceEvent::Disconnected => {
                        warn!("ESP32 Device Disconnected");
                        let mut st = daemon_state.lock().unwrap();
                        st.device_connected = false;
                        st.counters_source = CounterSource::Pc;
                        st.esp_counters = None;
                    }
                    DeviceEvent::Counters(c) => {
                        let mut st = daemon_state.lock().unwrap();
                        // Update in-memory counters
                        st.counters.lifetime_key1 = c.lifetime_key1;
                        st.counters.lifetime_key2 = c.lifetime_key2;
                        if !c.device_id.is_empty() {
                            st.counters.device_id = c.device_id;
                            st.counters.counter_generation = c.counter_generation;
                        } else {
                            // DeviceStatus: carries the device-side current-map counters
                            st.counters.map_key1 = c.map_key1;
                            st.counters.map_key2 = c.map_key2;
                        }
                        st.esp_counters = Some(st.counters.clone());
                    }
                    DeviceEvent::StatusUpdate(status) => {
                        daemon_state.lock().unwrap().latency = Some(LatencyStats {
                            samples: status.latency_samples,
                            p50_us: status.latency_p50_us,
                            p99_us: status.latency_p99_us,
                            p999_us: status.latency_p999_us,
                            max_us: status.latency_max_us,
                            deferred_reports: status.hid_deferred_reports,
                        });
                    }
                    DeviceEvent::CounterSyncResult { .. } => {}
                    DeviceEvent::ConfigAck { .. } => {}
                    DeviceEvent::LayoutAck { screen, success, message } => {
                        let note = if message.is_empty() { String::new() } else { format!(" ({})", message) };
                        info!(
                            "Layout {} {}{}",
                            Screen::from_wire(screen).map_or("?", |s| s.label()),
                            if success { "applied" } else { "rejected" },
                            note
                        );
                    }
                    DeviceEvent::LogBatch(batch) => {
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
                            log_hub.push(LogSource::Esp, level, "firmware", msg);
                        }
                    }
                }
            }

            // 2. Telemetry events from tosu
            Ok(telemetry) = tosu_rx.recv() => {
                let current_mode = { daemon_state.lock().unwrap().mode };
                data_sync.ingest(telemetry.values.iter().cloned());
                daemon_state.lock().unwrap().ui_values = telemetry.values.clone();

                if telemetry.is_playing {
                    let rewound = last_live_ms.is_some_and(|prev| telemetry.live_time_ms < prev - RETRY_REWIND_MS);
                    let new_attempt = current_mode != RuntimeMode::Playing || rewound;
                    last_live_ms = Some(telemetry.live_time_ms);
                    if new_attempt {
                        play_id = play_id.wrapping_add(1);
                        if rewound {
                            info!("Retry detected ({})", telemetry.title);
                        }
                    }

                    // Enter PLAYING mode
                    if current_mode != RuntimeMode::Playing {
                        info!("State transition -> PLAYING (osu! map active: {})", telemetry.title);
                        if let Some(s) = storage.lock().unwrap().as_mut() {
                            s.set_writes_allowed(false);
                        }
                        {
                            let mut st = daemon_state.lock().unwrap();
                            st.mode = RuntimeMode::Playing;
                            cooldown_deadline = None;
                        }
                    }
                    if new_attempt {
                        // Right away, so the device zeroes its map counters immediately
                        let _ = device_manager.send_host_status(true, true, play_id).await;
                        last_host_status = Instant::now();
                        let playing_hz = daemon_state.lock().unwrap().config.gameplay_display_hz;
                        let changes = data_sync.take_changes(true, playing_hz, true);
                        let _ = device_manager.send_data_update(&changes).await;
                    }
                } else if current_mode == RuntimeMode::Playing {
                    last_live_ms = None;
                    // Left playing mode -> start COOLDOWN timer (§11.2)
                    enter_cooldown(&daemon_state, &storage, &mut cooldown_deadline);
                    let _ = device_manager.send_host_status(true, false, play_id).await;
                }
            }

            // 3. tosu WebSocket connected / disconnected
            Ok(()) = tosu_connected_rx.changed() => {
                let connected = *tosu_connected_rx.borrow_and_update();
                info!("{}", if connected { "tosu connected" } else { "tosu disconnected" });
                let was_playing = {
                    let mut st = daemon_state.lock().unwrap();
                    st.tosu_connected = connected;
                    st.mode == RuntimeMode::Playing
                };
                // Without tosu there is no way to see the map end; don't stay stuck in PLAYING
                if !connected && was_playing {
                    enter_cooldown(&daemon_state, &storage, &mut cooldown_deadline);
                    last_live_ms = None;
                }
                if !connected {
                    data_sync.clear();
                }
                let _ = device_manager.send_host_status(connected, false, play_id).await;
                last_host_status = Instant::now();
            }

            // 4. Periodic tick for cooldown expiration, host status and time maintenance
            _ = tick.tick() => {
                // Retry opening SQLite every 60s if unavailable (§P2-12)
                if storage.lock().unwrap().is_none() && last_db_retry.elapsed() >= Duration::from_secs(60) {
                    last_db_retry = Instant::now();
                    info!("Retrying SQLite database connection at {}", db_path.display());
                    match Storage::open(&db_path) {
                        Ok(s) => {
                            info!("Successfully reconnected to SQLite database at {}", db_path.display());
                            if let Ok(cfg) = s.load_config() {
                                daemon_state.lock().unwrap().config = cfg;
                            }
                            for screen in Screen::ALL {
                                if let Ok(Some(json)) = s.load_layout(screen.to_wire()) {
                                    if let Ok(layout) = Layout::from_json(&json) {
                                        daemon_state.lock().unwrap().custom_layouts.insert(*screen, layout);
                                    }
                                }
                            }
                            if let Some(info) = &daemon_state.lock().unwrap().device_info {
                                let stored = s.load_device_state(&info.device_id).unwrap_or(None);
                                daemon_state.lock().unwrap().pc_counters = stored;
                            }
                            daemon_state.lock().unwrap().storage_error = None;
                            *storage.lock().unwrap() = Some(s);
                        }
                        Err(e) => {
                            warn!("SQLite database reconnection attempt failed: {}", e);
                            daemon_state.lock().unwrap().storage_error = Some(format!("Database error ({}): {}", db_path.display(), e));
                        }
                    }
                }

                if last_status_poll.elapsed() >= STATUS_POLL_INTERVAL {
                    last_status_poll = Instant::now();
                    if daemon_state.lock().unwrap().device_connected {
                        let _ = device_manager.request_status().await;
                    }
                }

                let (device_connected, tosu, playing, playing_hz) = {
                    let st = daemon_state.lock().unwrap();
                    (st.device_connected, st.tosu_connected, st.mode == RuntimeMode::Playing, st.config.gameplay_display_hz)
                };
                if device_connected && last_host_status.elapsed() >= HOST_STATUS_INTERVAL {
                    last_host_status = Instant::now();
                    let _ = device_manager.send_host_status(tosu, playing, play_id).await;
                }
                if device_connected {
                    let changes = data_sync.take_changes(playing, playing_hz, false);
                    if !changes.is_empty() {
                        let _ = device_manager.send_data_update(&changes).await;
                    }
                }

                let now_instant = Instant::now();
                let now_system = std::time::SystemTime::now();
                let elapsed_instant = now_instant.saturating_duration_since(last_instant);
                let clock_jump = match now_system.duration_since(last_system_time) {
                    Ok(system_elapsed) => {
                        let diff = if system_elapsed > elapsed_instant {
                            system_elapsed - elapsed_instant
                        } else {
                            elapsed_instant - system_elapsed
                        };
                        diff > Duration::from_secs(2)
                    }
                    Err(_) => true,
                };
                last_instant = now_instant;
                last_system_time = now_system;

                if device_connected && daemon_state.lock().unwrap().mode == RuntimeMode::Idle {
                    if clock_jump || last_periodic_time_sync.elapsed() >= Duration::from_secs(600) {
                        last_periodic_time_sync = Instant::now();
                        let _ = device_manager.send_time_sync().await;
                    }

                    if last_periodic_sync.elapsed() >= Duration::from_secs(300) {
                        last_periodic_sync = Instant::now();
                        let current_counters = daemon_state.lock().unwrap().counters.clone();
                        let changed = match &last_synced_counters {
                            Some(prev) => prev.lifetime_key1 != current_counters.lifetime_key1 || prev.lifetime_key2 != current_counters.lifetime_key2,
                            None => true,
                        };
                        if changed {
                            perform_sync(&daemon_state, &storage, &device_manager, &pending_ops).await;
                            last_synced_counters = Some(daemon_state.lock().unwrap().counters.clone());
                        }
                    }
                }

                let mut should_sync = false;
                {
                    let mut st = daemon_state.lock().unwrap();
                    if st.mode == RuntimeMode::Cooldown {
                        if let Some(deadline) = cooldown_deadline {
                            if Instant::now() >= deadline {
                                info!("Cooldown expired -> Entering SYNC/IDLE");
                                st.mode = RuntimeMode::Sync;
                                should_sync = true;
                                cooldown_deadline = None;
                            }
                        }
                    }
                }

                if should_sync {
                    perform_sync(&daemon_state, &storage, &device_manager, &pending_ops).await;
                    last_synced_counters = Some(daemon_state.lock().unwrap().counters.clone());
                    last_periodic_sync = Instant::now();
                    last_periodic_time_sync = Instant::now();
                }
            }
        }
    }
}

fn enter_cooldown(
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    cooldown_deadline: &mut Option<Instant>,
) {
    info!("State transition -> COOLDOWN (5s window started)");
    if let Some(s) = storage.lock().unwrap().as_mut() {
        s.set_writes_allowed(false);
    }
    state.lock().unwrap().mode = RuntimeMode::Cooldown;
    *cooldown_deadline = Some(Instant::now() + COOLDOWN_DURATION);
}

async fn perform_sync(
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    device: &Arc<DeviceManager>,
    pending_ops: &Arc<Mutex<PendingOperations>>,
) {
    info!("Performing atomic state synchronization (§11.3, §13)...");

    let is_storage_available = storage.lock().unwrap().is_some();
    if !is_storage_available {
        warn!("perform_sync skipped: SQLite storage unavailable (P2-12). ESP counters will not be modified.");
        let mut st = state.lock().unwrap();
        st.mode = RuntimeMode::Idle;
        st.last_sync_error = Some("Sync skipped: persistent storage unavailable".to_string());
        return;
    }

    let (info_opt, in_memory_counters, is_connected) = {
        let st = state.lock().unwrap();
        (st.device_info.clone(), st.counters.clone(), st.device_connected)
    };

    let Some(info) = info_opt else {
        let mut st = state.lock().unwrap();
        st.mode = RuntimeMode::Idle;
        if let Some(s) = storage.lock().unwrap().as_mut() {
            s.set_writes_allowed(true);
        }
        return;
    };

    if !is_connected {
        let mut st = state.lock().unwrap();
        st.mode = RuntimeMode::Idle;
        if let Some(s) = storage.lock().unwrap().as_mut() {
            s.set_writes_allowed(true);
        }
        return;
    }

    // Step 1: Wait up to 3s for device to reach IDLE (§P1-1)
    let mut idle_reached = false;
    let mut events = device.subscribe();
    let poll_deadline = Instant::now() + Duration::from_secs(3);

    while Instant::now() < poll_deadline {
        let _ = device.request_status().await;
        let wait_res = tokio::time::timeout(Duration::from_millis(100), async {
            loop {
                match events.recv().await {
                    Ok(DeviceEvent::StatusUpdate(s)) => {
                        return s.state == osupad_device::proto::DeviceState::Idle as i32;
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return false,
                }
            }
        }).await;

        if let Ok(is_idle) = wait_res {
            if is_idle {
                idle_reached = true;
                break;
            }
        }
    }

    if !idle_reached {
        warn!("Timed out waiting for device to report IDLE state before counter sync");
        let mut st = state.lock().unwrap();
        st.last_sync_error = Some("Device not in IDLE state within 3s timeout".to_string());
        return;
    }

    let stored_counters = {
        let s_guard = storage.lock().unwrap();
        s_guard.as_ref().and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None)).unwrap_or_else(|| CounterState {
            device_id: info.device_id.clone(),
            counter_generation: in_memory_counters.counter_generation,
            lifetime_key1: 0,
            lifetime_key2: 0,
            map_key1: 0,
            map_key2: 0,
        })
    };

    // Reconcile (§13)
    let reconciled = reconcile_counters(&stored_counters, &in_memory_counters);

    // Save reconciled state to SQLite (writes allowed in sync)
    {
        if let Some(s) = storage.lock().unwrap().as_mut() {
            s.set_writes_allowed(true);
            if let Err(e) = s.save_device_state(&info, &reconciled) {
                error!("Failed to save reconciled device state to SQLite: {}", e);
            }
        }
    }

    // Step 2 & 3: Send CounterSync and await matching CounterSyncResult (retry up to 3 times) (§P1-1)
    let mut sync_success = false;
    let mut last_error_msg = String::new();

    for attempt in 0..3 {
        let mut resp_events = device.subscribe();
        match device.send_counter_sync(&reconciled, false).await {
            Ok(seq) => {
                let wait_resp = tokio::time::timeout(Duration::from_secs(2), async {
                    loop {
                        match resp_events.recv().await {
                            Ok(DeviceEvent::CounterSyncResult { seq: r_seq, success, message, state: _ }) if r_seq == seq => {
                                return Ok((success, message));
                            }
                            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => return Err("Event channel closed".to_string()),
                        }
                    }
                }).await;

                match wait_resp {
                    Ok(Ok((true, _))) => {
                        sync_success = true;
                        break;
                    }
                    Ok(Ok((false, msg))) => {
                        last_error_msg = msg;
                        warn!("Device rejected CounterSync (attempt {}): {}", attempt + 1, last_error_msg);
                    }
                    Ok(Err(e)) => {
                        last_error_msg = e;
                        warn!("Error waiting for CounterSync response (attempt {}): {}", attempt + 1, last_error_msg);
                    }
                    Err(_) => {
                        last_error_msg = "Timeout waiting for CounterSyncResponse (2s)".to_string();
                        warn!("Timeout waiting for CounterSync response (attempt {})", attempt + 1);
                    }
                }
            }
            Err(e) => {
                last_error_msg = format!("Failed to send CounterSync: {}", e);
                warn!("{}", last_error_msg);
            }
        }
        tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await;
    }

    if !sync_success {
        error!("State synchronization failed after 3 attempts: {}", last_error_msg);
        let mut st = state.lock().unwrap();
        st.last_sync_error = Some(format!("Counter sync failed: {}", last_error_msg));
        st.counters = reconciled; // Keep SQLite as reconciled (§P1-1)
        return;
    }

    // Step 4: Success path
    let _ = device.send_time_sync().await;
    let now_str = Utc::now().to_rfc3339();

    {
        if let Some(s) = storage.lock().unwrap().as_ref() {
            let _ = s.save_device_state(&info, &reconciled);
            let _ = s.touch_device_last_seen(&info.device_id);
        }
    }

    // Drain pending operations (§P1-3)
    let (pending_cfg, pending_layouts, pending_seen) = {
        let mut p = pending_ops.lock().unwrap();
        (p.pending_config.take(), std::mem::take(&mut p.pending_layouts), p.pending_last_seen.take())
    };

    if let Some(cfg) = pending_cfg {
        {
            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_config(&cfg);
            }
        }
        let _ = device.send_config(&cfg).await;
    }

    for (screen, layout_opt) in pending_layouts {
        if let Some(layout) = layout_opt {
            {
                if let Some(s) = storage.lock().unwrap().as_ref() {
                    let _ = s.save_layout(screen.to_wire(), &layout.to_json());
                }
            }
            let _ = device.send_layout(screen, &layout).await;
        } else {
            {
                if let Some(s) = storage.lock().unwrap().as_ref() {
                    let _ = s.delete_layout(screen.to_wire());
                }
            }
            let _ = device.reset_layout(screen).await;
        }
    }

    if let Some(seen_id) = pending_seen {
        if let Some(s) = storage.lock().unwrap().as_ref() {
            let _ = s.touch_device_last_seen(&seen_id);
        }
    }

    {
        let mut st = state.lock().unwrap();
        st.counters = reconciled.clone();
        st.pc_counters = Some(reconciled.clone());
        st.esp_counters = Some(reconciled);
        st.last_sync_time = Some(now_str);
        st.last_sync_error = None;
        st.mode = RuntimeMode::Idle;
    }

    if let Some(s) = storage.lock().unwrap().as_mut() {
        s.set_writes_allowed(true);
    }
    info!("Synchronization completed successfully");
}

async fn handle_ipc_request(
    req: IpcRequest,
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    device: &Arc<DeviceManager>,
    log_hub: &LogHub,
    pending_ops: &Arc<Mutex<PendingOperations>>,
) -> IpcResponse {
    let mode = { state.lock().unwrap().mode };

    match req {
        IpcRequest::Handshake {
            client_protocol, ..
        } => {
            if client_protocol != IPC_PROTOCOL_VERSION {
                return IpcResponse::HandshakeRejected {
                    daemon_protocol: IPC_PROTOCOL_VERSION,
                    reason: format!(
                        "IPC protocol version mismatch: client is {}, daemon is {}",
                        client_protocol, IPC_PROTOCOL_VERSION
                    ),
                };
            }
            let st = state.lock().unwrap();
            IpcResponse::HandshakeAck {
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                daemon_protocol: IPC_PROTOCOL_VERSION,
                device_connected: st.device_connected,
            }
        }

        IpcRequest::GetStatus => {
            let st = state.lock().unwrap();
            let pc_counters = if let Some(info) = &st.device_info {
                let s_guard = storage.lock().unwrap();
                s_guard.as_ref().and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None)).or_else(|| st.pc_counters.clone())
            } else {
                st.pc_counters.clone()
            };
            IpcResponse::Status {
                mode: st.mode,
                device_connected: st.device_connected,
                device_info: st.device_info.clone(),
                counters: st.counters.clone(),
                counters_source: st.counters_source,
                pc_counters,
                esp_counters: st.esp_counters.clone(),
                config: st.config.clone(),
                last_sync_time: st.last_sync_time.clone(),
                last_sync_error: st.last_sync_error.clone(),
                storage_error: st.storage_error.clone(),
                tosu_connected: st.tosu_connected,
                latency: st.latency,
                pending_replacement: st.pending_replacement.clone(),
                incompatible: st.incompatible.clone(),
            }
        }

        IpcRequest::GetLayouts => {
            let st = state.lock().unwrap();
            IpcResponse::Layouts {
                idle: st.custom_layouts.get(&Screen::Idle).cloned(),
                playing: st.custom_layouts.get(&Screen::Playing).cloned(),
            }
        }

        IpcRequest::SetLayout { screen, layout } => {
            if let Err(e) = layout.validate() {
                return IpcResponse::OperationRejected { reason: format!("Invalid layout: {}", e) };
            }
            state.lock().unwrap().custom_layouts.insert(screen, layout.clone());
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                let _ = device.send_layout(screen, &layout).await;
                pending_ops.lock().unwrap().pending_layouts.push((screen, Some(layout)));
                return IpcResponse::LayoutApplied {
                    screen,
                    message: "Layout applied to pad RAM; will be saved after gameplay".to_string(),
                };
            }
            if let Some(s) = storage.lock().unwrap().as_ref() {
                if let Err(e) = s.save_layout(screen.to_wire(), &layout.to_json()) {
                    warn!("Failed to save layout: {}", e);
                }
            }
            info!("Layout {} saved", screen.label());
            if !state.lock().unwrap().device_connected {
                return IpcResponse::LayoutApplied {
                    screen,
                    message: "Saved; it will be applied when the pad connects".to_string(),
                };
            }
            let mut events = device.subscribe();
            if let Err(e) = device.send_layout(screen, &layout).await {
                return IpcResponse::Error(format!("Failed to send layout: {}", e));
            }
            wait_for_layout_ack(&mut events, screen).await
        }

        IpcRequest::ResetLayout { screen } => {
            state.lock().unwrap().custom_layouts.remove(&screen);
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                let _ = device.reset_layout(screen).await;
                pending_ops.lock().unwrap().pending_layouts.push((screen, None));
                return IpcResponse::LayoutApplied {
                    screen,
                    message: "Layout reset in pad RAM; will be saved after gameplay".to_string(),
                };
            }
            if let Some(s) = storage.lock().unwrap().as_ref() {
                if let Err(e) = s.delete_layout(screen.to_wire()) {
                    warn!("Failed to delete layout: {}", e);
                }
            }
            info!("Layout {} reset to default", screen.label());
            if !state.lock().unwrap().device_connected {
                return IpcResponse::LayoutApplied { screen, message: "Reset; the pad will use its default".to_string() };
            }
            let mut events = device.subscribe();
            if let Err(e) = device.reset_layout(screen).await {
                return IpcResponse::Error(format!("Failed to reset layout: {}", e));
            }
            wait_for_layout_ack(&mut events, screen).await
        }

        IpcRequest::GetUiValues => IpcResponse::UiValues(state.lock().unwrap().ui_values.clone()),

        IpcRequest::ResetLatencyStats => {
            if let Err(e) = device.reset_latency_stats().await {
                return IpcResponse::Error(format!("Failed to reset latency stats: {}", e));
            }
            state.lock().unwrap().latency = None;
            info!("Latency statistics reset");
            IpcResponse::HandshakeAck {
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                daemon_protocol: IPC_PROTOCOL_VERSION,
                device_connected: state.lock().unwrap().device_connected,
            }
        }

        IpcRequest::UpdateConfig(new_config) => {
            if let Err(e) = new_config.validate() {
                return IpcResponse::OperationRejected {
                    reason: format!("Invalid configuration: {}", e),
                };
            }

            let current_cfg = { state.lock().unwrap().config.clone() };

            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                let keys_or_debounce_changed = current_cfg.key1_hid_usage != new_config.key1_hid_usage
                    || current_cfg.key2_hid_usage != new_config.key2_hid_usage
                    || current_cfg.debounce_us != new_config.debounce_us;

                if keys_or_debounce_changed {
                    pending_ops.lock().unwrap().pending_config = Some(new_config);
                    return IpcResponse::OperationDeferred {
                        reason: "Key mapping and debounce changes are deferred until IDLE mode".to_string(),
                    };
                }

                // Brightness / sleep / hz changes are applied to RAM immediately (§P1-3)
                let _ = device.send_config(&new_config).await;
                {
                    let mut st = state.lock().unwrap();
                    st.config = new_config.clone();
                }
                pending_ops.lock().unwrap().pending_config = Some(new_config.clone());
                info!("Configuration applied to RAM (persistence queued for SYNC)");
                return IpcResponse::ConfigUpdated {
                    config: new_config,
                    deferred_persist: true,
                };
            }

            if let Some(s) = storage.lock().unwrap().as_ref() {
                if let Err(e) = s.save_config(&new_config) {
                    return IpcResponse::Error(format!("Failed to save config: {}", e));
                }
            }

            {
                let mut st = state.lock().unwrap();
                st.config = new_config.clone();
            }

            let _ = device.send_config(&new_config).await;
            info!("Configuration updated");
            IpcResponse::ConfigUpdated {
                config: new_config,
                deferred_persist: false,
            }
        }

        IpcRequest::ForceSync => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot force synchronization during gameplay or cooldown".to_string(),
                };
            }
            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: counter sync is rejected without persistent storage".to_string(),
                };
            }
            perform_sync(state, storage, device, pending_ops).await;
            let counters = { state.lock().unwrap().counters.clone() };
            IpcResponse::SyncCompleted {
                success: true,
                counters,
            }
        }

        IpcRequest::ResetCounters { confirm } => {
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "ResetCounters requires explicit confirmation (--yes or modal confirm)".to_string(),
                };
            }

            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot reset counters during gameplay or cooldown".to_string(),
                };
            }

            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: counter reset is rejected without persistent storage".to_string(),
                };
            }

            let updated = {
                let mut st = state.lock().unwrap();
                st.counters.counter_generation = st.counters.counter_generation.saturating_add(1);
                st.counters.lifetime_key1 = 0;
                st.counters.lifetime_key2 = 0;
                let updated = st.counters.clone();

                if let Some(info) = &st.device_info {
                    if let Some(s) = storage.lock().unwrap().as_ref() {
                        let _ = s.save_device_state(info, &updated);
                    }
                } else {
                    pending_ops.lock().unwrap().pending_device_push = true;
                }
                updated
            };

            if state.lock().unwrap().device_connected {
                let _ = device.send_counter_sync(&updated, true).await;
            } else {
                pending_ops.lock().unwrap().pending_device_push = true;
            }
            info!("Lifetime counters reset with incremented generation");
            IpcResponse::CountersReset { counters: updated }
        }

        IpcRequest::RestoreDeviceFromPc { confirm } => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot restore counters during active gameplay or cooldown".to_string(),
                };
            }
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "Confirmation required to force-restore ESP counters from PC".to_string(),
                };
            }
            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: restore from PC is rejected without persistent storage".to_string(),
                };
            }
            let (info_opt, in_memory_counters, is_connected) = {
                let st = state.lock().unwrap();
                (st.device_info.clone(), st.counters.clone(), st.device_connected)
            };
            let Some(info) = info_opt else {
                return IpcResponse::OperationRejected {
                    reason: "No device currently connected".to_string(),
                };
            };
            if !is_connected {
                return IpcResponse::OperationRejected {
                    reason: "Device is disconnected".to_string(),
                };
            }

            let pc_state = {
                let s_guard = storage.lock().unwrap();
                s_guard.as_ref().and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
            };
            let Some(pc_counters) = pc_state else {
                return IpcResponse::OperationRejected {
                    reason: "No PC counter state found in database for this device".to_string(),
                };
            };

            let new_gen = std::cmp::max(pc_counters.counter_generation, in_memory_counters.counter_generation).saturating_add(1);
            let target = CounterState {
                device_id: info.device_id.clone(),
                counter_generation: new_gen,
                lifetime_key1: pc_counters.lifetime_key1,
                lifetime_key2: pc_counters.lifetime_key2,
                map_key1: 0,
                map_key2: 0,
            };

            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_device_state(&info, &target);
                let _ = s.touch_device_last_seen(&info.device_id);
            }

            let _ = device.send_counter_sync(&target, true).await;
            {
                let mut st = state.lock().unwrap();
                st.counters = target.clone();
                st.pc_counters = Some(target.clone());
                st.esp_counters = Some(target.clone());
            }
            info!("Force-restored ESP counters from PC (generation: {})", new_gen);
            IpcResponse::CountersRestored { counters: target }
        }

        IpcRequest::ImportPcFromDevice { confirm } => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot import counters during active gameplay or cooldown".to_string(),
                };
            }
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "Confirmation required to overwrite PC counters from ESP".to_string(),
                };
            }
            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: import from device is rejected without persistent storage".to_string(),
                };
            }
            let (info_opt, in_memory_counters, is_connected) = {
                let st = state.lock().unwrap();
                (st.device_info.clone(), st.counters.clone(), st.device_connected)
            };
            let Some(info) = info_opt else {
                return IpcResponse::OperationRejected {
                    reason: "No device currently connected".to_string(),
                };
            };
            if !is_connected {
                return IpcResponse::OperationRejected {
                    reason: "Device is disconnected".to_string(),
                };
            }

            let pc_state = {
                let s_guard = storage.lock().unwrap();
                s_guard.as_ref().and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
            };
            let pc_gen = pc_state.as_ref().map(|c| c.counter_generation).unwrap_or(0);

            let new_gen = std::cmp::max(pc_gen, in_memory_counters.counter_generation).saturating_add(1);
            let target = CounterState {
                device_id: info.device_id.clone(),
                counter_generation: new_gen,
                lifetime_key1: in_memory_counters.lifetime_key1,
                lifetime_key2: in_memory_counters.lifetime_key2,
                map_key1: 0,
                map_key2: 0,
            };

            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_device_state(&info, &target);
                let _ = s.touch_device_last_seen(&info.device_id);
            }

            let _ = device.send_counter_sync(&target, true).await;
            {
                let mut st = state.lock().unwrap();
                st.counters = target.clone();
                st.pc_counters = Some(target.clone());
                st.esp_counters = Some(target.clone());
            }
            info!("Overwrote PC database counters from ESP (generation: {})", new_gen);
            IpcResponse::CountersRestored { counters: target }
        }

        IpcRequest::ResolveReplacement { restore } => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot resolve replacement during gameplay or cooldown".to_string(),
                };
            }
            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: resolving replacement is rejected without persistent storage".to_string(),
                };
            }

            let (old_device_id_opt, current_info_opt, current_counters) = {
                let mut st = state.lock().unwrap();
                (st.pending_replacement.take(), st.device_info.clone(), st.counters.clone())
            };

            let Some(current_info) = current_info_opt else {
                return IpcResponse::OperationRejected {
                    reason: "No device currently connected to resolve replacement for".to_string(),
                };
            };

            if restore {
                if let Some(old_id) = old_device_id_opt {
                    let old_state = {
                        let s_guard = storage.lock().unwrap();
                        s_guard.as_ref().and_then(|s| s.load_device_state(&old_id).unwrap_or(None))
                    };
                    if let Some(old_c) = old_state {
                        let new_gen = std::cmp::max(old_c.counter_generation, current_counters.counter_generation).saturating_add(1);
                        let restored = CounterState {
                            device_id: current_info.device_id.clone(),
                            counter_generation: new_gen,
                            lifetime_key1: old_c.lifetime_key1,
                            lifetime_key2: old_c.lifetime_key2,
                            map_key1: 0,
                            map_key2: 0,
                        };
                        if let Some(s) = storage.lock().unwrap().as_ref() {
                            let _ = s.save_device_state(&current_info, &restored);
                            let _ = s.touch_device_last_seen(&current_info.device_id);
                        }
                        let _ = device.send_counter_sync(&restored, true).await;
                        state.lock().unwrap().counters = restored.clone();
                        info!("Restored lifetime counters from previous pad");
                        return IpcResponse::CountersReset { counters: restored };
                    }
                }
                IpcResponse::OperationRejected {
                    reason: "No previous device state found to restore from".to_string(),
                }
            } else {
                // Treat as new pad: register it in SQLite
                if let Some(s) = storage.lock().unwrap().as_ref() {
                    let _ = s.save_device_state(&current_info, &current_counters);
                    let _ = s.touch_device_last_seen(&current_info.device_id);
                }
                info!("Adopted new pad as separate device");
                IpcResponse::CountersReset { counters: current_counters }
            }
        }

        IpcRequest::ExportBackup => {
            let st = state.lock().unwrap();
            if !st.device_connected && st.device_info.is_none() && st.counters.device_id.is_empty() {
                return IpcResponse::OperationRejected {
                    reason: "No counters known yet: connect the pad once".to_string(),
                };
            }
            let info = st.device_info.clone().unwrap_or_else(|| DeviceInfo {
                device_id: st.counters.device_id.clone(),
                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                firmware_version: env!("CARGO_PKG_VERSION").to_string(),
                protocol_version: 1,
            });
            let backup = JsonBackup::new(&info, &st.counters, &st.config);
            IpcResponse::BackupExported(backup)
        }

        IpcRequest::PreviewImport(backup) => {
            if let Err(err) = backup.validate() {
                return IpcResponse::Error(format!("Invalid backup: {}", err));
            }

            let st = state.lock().unwrap();
            let current = Some(CurrentBackupState {
                device_id: st.counters.device_id.clone(),
                counter_generation: st.counters.counter_generation,
                lifetime_key1: st.counters.lifetime_key1,
                lifetime_key2: st.counters.lifetime_key2,
                config: st.config.clone(),
            });

            let device_id_matches = st.counters.device_id.is_empty() || st.counters.device_id == backup.device.device_id;
            let is_counter_rollback = backup.stats.lifetime_key1 < st.counters.lifetime_key1 || backup.stats.lifetime_key2 < st.counters.lifetime_key2;

            let mut warnings = Vec::new();
            if !device_id_matches {
                warnings.push(format!(
                    "Device ID mismatch: backup is for '{}', current pad is '{}'",
                    backup.device.device_id, st.counters.device_id
                ));
            }
            if is_counter_rollback {
                warnings.push("Incoming lifetime counters are lower than current counters (counter rollback)".to_string());
            }
            if backup.device.counter_generation <= st.counters.counter_generation {
                warnings.push("Incoming counter generation is not greater than current generation; generation will be bumped".to_string());
            }

            IpcResponse::ImportPreview {
                current,
                incoming: backup,
                device_id_matches,
                is_counter_rollback,
                warnings,
            }
        }

        IpcRequest::ImportBackup { backup, confirm } => {
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "ImportBackup requires explicit confirmation (--yes or user confirmation)".to_string(),
                };
            }

            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot import backup during gameplay or cooldown".to_string(),
                };
            }

            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: backup import is rejected without persistent storage".to_string(),
                };
            }

            if let Err(err) = backup.validate() {
                return IpcResponse::Error(format!("Invalid backup: {}", err));
            }

            let (current_tosu, current_color, current_gen) = {
                let st = state.lock().unwrap();
                (st.config.tosu_endpoint.clone(), st.config.press_color_rgb, st.counters.counter_generation)
            };

            let k1_usage = char_to_hid_usage(&backup.config.key1).unwrap_or(0x1D);
            let k2_usage = char_to_hid_usage(&backup.config.key2).unwrap_or(0x1B);

            let new_config = DeviceConfig {
                key1_hid_usage: k1_usage,
                key2_hid_usage: k2_usage,
                debounce_us: backup.config.debounce_us,
                brightness: backup.config.brightness,
                display_sleep_seconds: backup.config.display_sleep_seconds,
                gameplay_display_hz: backup.config.gameplay_display_hz,
                tosu_endpoint: current_tosu,
                press_color_rgb: current_color,
            };

            let new_counters = CounterState {
                device_id: backup.device.device_id.clone(),
                counter_generation: std::cmp::max(current_gen, backup.device.counter_generation).saturating_add(1),
                lifetime_key1: backup.stats.lifetime_key1,
                lifetime_key2: backup.stats.lifetime_key2,
                map_key1: 0,
                map_key2: 0,
            };

            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_config(&new_config);
                let info = DeviceInfo {
                    device_id: backup.device.device_id,
                    board_profile: backup.device.board_profile,
                    firmware_version: env!("CARGO_PKG_VERSION").to_string(),
                    protocol_version: 1,
                };
                let _ = s.save_device_state(&info, &new_counters);
            }

            {
                let mut st = state.lock().unwrap();
                st.config = new_config.clone();
                st.counters = new_counters.clone();
            }

            let _ = device.send_config(&new_config).await;
            let _ = device.send_counter_sync(&new_counters, true).await;
            info!("JSON Backup imported and applied");

            IpcResponse::BackupImported {
                success: true,
                counters: new_counters,
                config: new_config,
            }
        }

        IpcRequest::GetLogEntries { since_seq, limit } => {
            let (entries, latest_seq) = log_hub.get_entries(since_seq, limit);
            IpcResponse::LogEntries { entries, latest_seq }
        }

        IpcRequest::PrepareFlash => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot flash firmware during gameplay or cooldown".to_string(),
                };
            }
            info!("Releasing serial port for firmware flash...");
            if !device.pause_and_release(std::time::Duration::from_secs(2)).await {
                device.resume();
                return IpcResponse::OperationRejected {
                    reason: "Timed out waiting for the serial port to be released".to_string(),
                };
            }
            {
                let mut st = state.lock().unwrap();
                st.device_connected = false;
            }
            let port = osupad_device::find_target_port();
            IpcResponse::ReadyForFlash { port }
        }

        IpcRequest::FinishFlash => {
            info!("Flash finished, resuming device discovery and waiting for reconnect...");
            let mut events = device.subscribe();
            device.resume();

            let wait_res = tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    match events.recv().await {
                        Ok(DeviceEvent::Connected(info)) => return Ok(info),
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => return Err("Event stream closed".to_string()),
                    }
                }
            }).await;

            match wait_res {
                Ok(Ok(info)) => {
                    let compatible = info.protocol_version == 1;
                    IpcResponse::FlashFinished {
                        firmware_version: info.firmware_version,
                        protocol_version: info.protocol_version,
                        compatible,
                    }
                }
                Ok(Err(e)) => IpcResponse::Error(format!("Flash verification failed: {}", e)),
                Err(_) => IpcResponse::Error("Device did not reconnect within 15 seconds after flash".to_string()),
            }
        }
    }
}

async fn wait_for_layout_ack(
    events: &mut tokio::sync::broadcast::Receiver<DeviceEvent>,
    screen: Screen,
) -> IpcResponse {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DeviceEvent::LayoutAck { screen: s, success, message }) if s == screen.to_wire() => {
                    return if success {
                        IpcResponse::LayoutApplied { screen, message }
                    } else {
                        IpcResponse::OperationRejected { reason: format!("The pad rejected the layout: {}", message) }
                    };
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return IpcResponse::Error("Device event stream closed".to_string()),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(3), wait)
        .await
        .unwrap_or_else(|_| IpcResponse::Error("The pad did not confirm the layout within 3s".to_string()))
}

fn get_tosu_log_path() -> PathBuf {
    let state_dir = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(|| PathBuf::from("."));
    state_dir.join("osupad").join("tosu.log")
}

fn get_database_path() -> PathBuf {
    if let Ok(data_dir) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(data_dir).join("osupad").join("osupad.db")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("osupad")
            .join("osupad.db")
    } else {
        PathBuf::from("osupad.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_failure_isolation() {
        let (device_manager, _) = DeviceManager::new_dummy();
        let device_manager = Arc::new(device_manager);
        let log_hub = LogHub::new();
        let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
        let storage: Arc<Mutex<Option<Storage>>> = Arc::new(Mutex::new(None));

        let daemon_state = Arc::new(Mutex::new(DaemonState {
            mode: RuntimeMode::Idle,
            device_connected: true,
            device_info: Some(DeviceInfo {
                device_id: "OSUPAD-TEST".to_string(),
                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                firmware_version: "1.0.0".to_string(),
                protocol_version: 1,
            }),
            counters: CounterState {
                device_id: "OSUPAD-TEST".to_string(),
                counter_generation: 1,
                lifetime_key1: 50,
                lifetime_key2: 60,
                map_key1: 0,
                map_key2: 0,
            },
            counters_source: CounterSource::Device,
            pc_counters: None,
            esp_counters: Some(CounterState {
                device_id: "OSUPAD-TEST".to_string(),
                counter_generation: 1,
                lifetime_key1: 50,
                lifetime_key2: 60,
                map_key1: 0,
                map_key2: 0,
            }),
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: Some("Database permission denied".to_string()),
            tosu_connected: false,
            latency: None,
            pending_replacement: None,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
        }));

        // 1. GetStatus surfaces storage_error
        let resp = handle_ipc_request(
            IpcRequest::GetStatus,
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        match resp {
            IpcResponse::Status { storage_error, .. } => {
                assert_eq!(storage_error, Some("Database permission denied".to_string()));
            }
            other => panic!("Expected Status response, got {:?}", other),
        }

        // 2. Destructive/reconciliation operations rejected without SQLite
        let resp = handle_ipc_request(
            IpcRequest::ForceSync,
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::ResetCounters { confirm: true },
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::RestoreDeviceFromPc { confirm: true },
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::ImportPcFromDevice { confirm: true },
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        // 3. Layouts still work in-memory
        let resp = handle_ipc_request(
            IpcRequest::GetLayouts,
            &daemon_state,
            &storage,
            &device_manager,
            &log_hub,
            &pending_ops,
        )
        .await;
        assert!(matches!(resp, IpcResponse::Layouts { idle: None, playing: None }));

        // 4. perform_sync does not modify ESP counters when storage is None
        perform_sync(&daemon_state, &storage, &device_manager, &pending_ops).await;
        let st = daemon_state.lock().unwrap();
        assert_eq!(st.counters.lifetime_key1, 50);
        assert_eq!(st.counters.lifetime_key2, 60);
        assert!(st.last_sync_error.is_some());
    }
}
