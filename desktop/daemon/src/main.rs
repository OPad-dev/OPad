use anyhow::Result;
use chrono::Utc;
use osupad_device::{DeviceEvent, DeviceManager};
use osupad_ipc::{
    create_listener, get_socket_path, read_request, send_response, IpcRequest, IpcResponse,
    IPC_PROTOCOL_VERSION,
};
use osupad_model::{
    char_to_hid_usage, CounterState, DeviceConfig, DeviceInfo, JsonBackup, LatencyStats,
    RuntimeMode,
};
use osupad_storage::{reconcile_counters, Storage};
use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_tosu::{spawn_tosu_supervisor, TosuManager};
mod telemetry;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const COOLDOWN_DURATION: Duration = Duration::from_secs(5);
/// Firmware drops out of PLAYING after 3s without a playing HostStatus, and treats
/// status older than 10s as unknown
const HOST_STATUS_INTERVAL: Duration = Duration::from_secs(1);
/// Poll DeviceStatus for live lifetime / current-map counters
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// A song position jump backwards larger than this is a retry of the same map
const RETRY_REWIND_MS: f64 = 2000.0;
const MAX_LOG_ENTRIES: usize = 500;

#[derive(Clone)]
pub struct DaemonState {
    pub mode: RuntimeMode,
    pub device_connected: bool,
    pub device_info: Option<DeviceInfo>,
    pub counters: CounterState,
    pub config: DeviceConfig,
    pub last_sync_time: Option<String>,
    pub tosu_connected: bool,
    pub latency: Option<LatencyStats>,
    /// Latest tosu-derived UI values (for the designer's live preview)
    pub ui_values: Vec<(u8, SourceValue)>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting osupad-daemon v1.0.0");

    let db_path = get_database_path();
    info!("Using SQLite database at {}", db_path.display());
    let storage = Arc::new(Mutex::new(Storage::open(&db_path)?));

    // Load initial configuration
    let initial_config = {
        let s = storage.lock().unwrap();
        s.load_config().unwrap_or_default()
    };

    let log_hub = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(MAX_LOG_ENTRIES)));
    log_info(
        &log_hub,
        "osupad-daemon initialized, storage loaded successfully",
    );

    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: false,
        device_info: None,
        counters: CounterState::default(),
        config: initial_config.clone(),
        last_sync_time: None,
        tosu_connected: false,
        latency: None,
        ui_values: Vec::new(),
    }));

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
    info!("Starting local IPC listener at {}", socket_path.display());
    let ipc_listener = create_listener(&socket_path)?;

    // Spawn IPC request handling task
    {
        let daemon_state = daemon_state.clone();
        let storage = storage.clone();
        let device_manager = device_manager.clone();
        let log_hub = log_hub.clone();

        tokio::spawn(async move {
            loop {
                match ipc_listener.accept().await {
                    Ok((mut stream, _)) => {
                        let daemon_state = daemon_state.clone();
                        let storage = storage.clone();
                        let device_manager = device_manager.clone();
                        let log_hub = log_hub.clone();

                        tokio::spawn(async move {
                            while let Ok(req) = read_request(&mut stream).await {
                                let resp = handle_ipc_request(
                                    req,
                                    &daemon_state,
                                    &storage,
                                    &device_manager,
                                    &log_hub,
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

    loop {
        tokio::select! {
            // 1. Device hardware events
            Ok(dev_event) = device_rx.recv() => {
                match dev_event {
                    DeviceEvent::Connected(info) => {
                        info!("ESP32 Device Connected: ID={}, Board={}", info.device_id, info.board_profile);
                        log_info(&log_hub, &format!("Device connected: {}", info.device_id));

                        let mut st = daemon_state.lock().unwrap();
                        st.device_connected = true;
                        st.device_info = Some(info.clone());

                        // Send time sync, config and host status immediately (§14.3).
                        // The device does not persist its config, so push it on every connect.
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
                        let layouts: Vec<(Screen, Layout)> = Screen::ALL
                            .iter()
                            .filter_map(|screen| {
                                let json = storage.lock().unwrap().load_layout(screen.to_wire()).ok().flatten()?;
                                Some((*screen, Layout::from_json(&json).ok()?))
                            })
                            .collect();
                        if !layouts.is_empty() {
                            let dm = device_manager.clone();
                            tokio::spawn(async move {
                                for (screen, layout) in layouts {
                                    let _ = dm.send_layout(screen, &layout).await;
                                }
                            });
                        }
                    }
                    DeviceEvent::Disconnected => {
                        warn!("ESP32 Device Disconnected");
                        log_info(&log_hub, "Device disconnected");
                        let mut st = daemon_state.lock().unwrap();
                        st.device_connected = false;
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
                    DeviceEvent::LayoutAck { screen, success, message } => {
                        let note = if message.is_empty() { String::new() } else { format!(" ({})", message) };
                        log_info(&log_hub, &format!(
                            "Layout {} {}{}",
                            Screen::from_wire(screen).map_or("?", |s| s.label()),
                            if success { "applied" } else { "rejected" },
                            note
                        ));
                    }
                    DeviceEvent::LogBatch(batch) => {
                        for ev in batch.events {
                            let entry = format!("ESP [{}]: {}", ev.tag, ev.message);
                            log_info(&log_hub, &entry);
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
                            log_info(&log_hub, &format!("Retry detected ({})", telemetry.title));
                        }
                    }

                    // Enter PLAYING mode
                    if current_mode != RuntimeMode::Playing {
                        info!("State transition -> PLAYING (osu! map active)");
                        log_info(&log_hub, &format!("State -> PLAYING ({})", telemetry.title));
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
                        let changes = data_sync.take_changes(true, true);
                        let _ = device_manager.send_data_update(&changes).await;
                    }
                } else if current_mode == RuntimeMode::Playing {
                    last_live_ms = None;
                    // Left playing mode -> start COOLDOWN timer (§11.2)
                    enter_cooldown(&daemon_state, &log_hub, &mut cooldown_deadline);
                    let _ = device_manager.send_host_status(true, false, play_id).await;
                }
            }

            // 3. tosu WebSocket connected / disconnected
            Ok(()) = tosu_connected_rx.changed() => {
                let connected = *tosu_connected_rx.borrow_and_update();
                log_info(&log_hub, if connected { "tosu connected" } else { "tosu disconnected" });
                let was_playing = {
                    let mut st = daemon_state.lock().unwrap();
                    st.tosu_connected = connected;
                    st.mode == RuntimeMode::Playing
                };
                // Without tosu there is no way to see the map end; don't stay stuck in PLAYING
                if !connected && was_playing {
                    enter_cooldown(&daemon_state, &log_hub, &mut cooldown_deadline);
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
                if last_status_poll.elapsed() >= STATUS_POLL_INTERVAL {
                    last_status_poll = Instant::now();
                    if daemon_state.lock().unwrap().device_connected {
                        let _ = device_manager.request_status().await;
                    }
                }

                let (device_connected, tosu, playing) = {
                    let st = daemon_state.lock().unwrap();
                    (st.device_connected, st.tosu_connected, st.mode == RuntimeMode::Playing)
                };
                if device_connected && last_host_status.elapsed() >= HOST_STATUS_INTERVAL {
                    last_host_status = Instant::now();
                    let _ = device_manager.send_host_status(tosu, playing, play_id).await;
                }
                if device_connected {
                    let changes = data_sync.take_changes(playing, false);
                    if !changes.is_empty() {
                        let _ = device_manager.send_data_update(&changes).await;
                    }
                }

                let mut should_sync = false;
                {
                    let mut st = daemon_state.lock().unwrap();
                    if st.mode == RuntimeMode::Cooldown {
                        if let Some(deadline) = cooldown_deadline {
                            if Instant::now() >= deadline {
                                info!("Cooldown expired -> Entering SYNC/IDLE");
                                log_info(&log_hub, "Cooldown expired -> SYNC");
                                st.mode = RuntimeMode::Sync;
                                should_sync = true;
                                cooldown_deadline = None;
                            }
                        }
                    }
                }

                if should_sync {
                    perform_sync(&daemon_state, &storage, &device_manager, &log_hub).await;
                }
            }
        }
    }
}

fn enter_cooldown(
    state: &Arc<Mutex<DaemonState>>,
    log_hub: &Arc<Mutex<VecDeque<String>>>,
    cooldown_deadline: &mut Option<Instant>,
) {
    info!("State transition -> COOLDOWN (5s window started)");
    log_info(log_hub, "State -> COOLDOWN (5s)");
    state.lock().unwrap().mode = RuntimeMode::Cooldown;
    *cooldown_deadline = Some(Instant::now() + COOLDOWN_DURATION);
}

async fn perform_sync(
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Storage>>,
    device: &Arc<DeviceManager>,
    log_hub: &Arc<Mutex<VecDeque<String>>>,
) {
    info!("Performing atomic state synchronization (§11.3, §13)...");
    log_info(log_hub, "Performing synchronization...");

    let (info_opt, in_memory_counters) = {
        let st = state.lock().unwrap();
        (st.device_info.clone(), st.counters.clone())
    };

    if let Some(info) = info_opt {
        let stored_counters = {
            let s = storage.lock().unwrap();
            s.load_device_state(&info.device_id).unwrap_or(None).unwrap_or_else(|| CounterState {
                device_id: info.device_id.clone(),
                counter_generation: in_memory_counters.counter_generation,
                lifetime_key1: 0,
                lifetime_key2: 0,
                map_key1: 0,
                map_key2: 0,
            })
        };

        // Reconcile
        let reconciled = reconcile_counters(&stored_counters, &in_memory_counters);

        // Commit to SQLite
        {
            let s = storage.lock().unwrap();
            if let Err(e) = s.save_device_state(&info, &reconciled) {
                error!("Failed to save reconciled device state to SQLite: {}", e);
            }
        }

        // Send reconciled counters to ESP device
        let _ = device.send_counter_sync(&reconciled, false).await;

        // Sync Clock
        let _ = device.send_time_sync().await;

        let now_str = Utc::now().to_rfc3339();
        {
            let mut st = state.lock().unwrap();
            st.counters = reconciled;
            st.last_sync_time = Some(now_str.clone());
            st.mode = RuntimeMode::Idle;
        }

        log_info(log_hub, "Synchronization completed successfully");
    } else {
        let mut st = state.lock().unwrap();
        st.mode = RuntimeMode::Idle;
    }
}

async fn handle_ipc_request(
    req: IpcRequest,
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Storage>>,
    device: &Arc<DeviceManager>,
    log_hub: &Arc<Mutex<VecDeque<String>>>,
) -> IpcResponse {
    let mode = { state.lock().unwrap().mode };

    match req {
        IpcRequest::Handshake {
            client_protocol: _, ..
        } => {
            let st = state.lock().unwrap();
            IpcResponse::HandshakeAck {
                daemon_version: "1.0.0".to_string(),
                daemon_protocol: IPC_PROTOCOL_VERSION,
                device_connected: st.device_connected,
            }
        }

        IpcRequest::GetStatus => {
            let st = state.lock().unwrap();
            IpcResponse::Status {
                mode: st.mode,
                device_connected: st.device_connected,
                device_info: st.device_info.clone(),
                counters: st.counters.clone(),
                config: st.config.clone(),
                last_sync_time: st.last_sync_time.clone(),
                tosu_connected: st.tosu_connected,
                latency: st.latency,
            }
        }

        IpcRequest::GetLayouts => {
            let load = |screen: Screen| {
                let json = storage.lock().unwrap().load_layout(screen.to_wire()).ok().flatten()?;
                Layout::from_json(&json).ok()
            };
            IpcResponse::Layouts { idle: load(Screen::Idle), playing: load(Screen::Playing) }
        }

        IpcRequest::SetLayout { screen, layout } => {
            if let Err(e) = layout.validate() {
                return IpcResponse::OperationRejected { reason: format!("Invalid layout: {}", e) };
            }
            if let Err(e) = storage.lock().unwrap().save_layout(screen.to_wire(), &layout.to_json()) {
                return IpcResponse::Error(format!("Failed to save layout: {}", e));
            }
            log_info(log_hub, &format!("Layout {} saved", screen.label()));
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
            if let Err(e) = storage.lock().unwrap().delete_layout(screen.to_wire()) {
                return IpcResponse::Error(format!("Failed to delete layout: {}", e));
            }
            log_info(log_hub, &format!("Layout {} reset to default", screen.label()));
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
            log_info(log_hub, "Latency statistics reset");
            IpcResponse::HandshakeAck {
                daemon_version: "1.0.0".to_string(),
                daemon_protocol: IPC_PROTOCOL_VERSION,
                device_connected: state.lock().unwrap().device_connected,
            }
        }

        IpcRequest::UpdateConfig(new_config) => {
            // Check §30 policy: Defer or reject config updates while playing
            if mode == RuntimeMode::Playing {
                return IpcResponse::OperationRejected {
                    reason: "Cannot change hardware configuration during PLAYING mode".to_string(),
                };
            }

            {
                let s = storage.lock().unwrap();
                if let Err(e) = s.save_config(&new_config) {
                    return IpcResponse::Error(format!("Failed to save config: {}", e));
                }
            }

            {
                let mut st = state.lock().unwrap();
                st.config = new_config.clone();
            }

            let _ = device.send_config(&new_config).await;
            log_info(log_hub, "Configuration updated");
            IpcResponse::ConfigUpdated { config: new_config }
        }

        IpcRequest::ForceSync => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot force synchronization during gameplay or cooldown".to_string(),
                };
            }
            perform_sync(state, storage, device, log_hub).await;
            let counters = { state.lock().unwrap().counters.clone() };
            IpcResponse::SyncCompleted {
                success: true,
                counters,
            }
        }

        IpcRequest::ResetCounters => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot reset counters during gameplay".to_string(),
                };
            }

            let updated = {
                let mut st = state.lock().unwrap();
                st.counters.counter_generation = st.counters.counter_generation.saturating_add(1);
                st.counters.lifetime_key1 = 0;
                st.counters.lifetime_key2 = 0;
                let updated = st.counters.clone();

                if let Some(info) = &st.device_info {
                    let s = storage.lock().unwrap();
                    let _ = s.save_device_state(info, &updated);
                }
                updated
            };

            let _ = device.send_counter_sync(&updated, true).await;
            log_info(log_hub, "Lifetime counters reset with incremented generation");
            IpcResponse::CountersReset { counters: updated }
        }

        IpcRequest::ExportBackup => {
            let st = state.lock().unwrap();
            let info = st.device_info.clone().unwrap_or_default();
            let backup = JsonBackup::new(&info, &st.counters, &st.config);
            IpcResponse::BackupExported(backup)
        }

        IpcRequest::ImportBackup(backup) => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot import backup during gameplay".to_string(),
                };
            }

            if let Err(err) = backup.validate() {
                return IpcResponse::Error(format!("Invalid backup: {}", err));
            }

            let k1_usage = char_to_hid_usage(&backup.config.key1).unwrap_or(0x1D);
            let k2_usage = char_to_hid_usage(&backup.config.key2).unwrap_or(0x1B);

            let new_config = DeviceConfig {
                key1_hid_usage: k1_usage,
                key2_hid_usage: k2_usage,
                debounce_us: backup.config.debounce_us,
                brightness: backup.config.brightness,
                display_sleep_seconds: backup.config.display_sleep_seconds,
                gameplay_display_hz: backup.config.gameplay_display_hz,
                tosu_endpoint: "ws://127.0.0.1:24050/websocket/v2".to_string(),
                press_color_rgb: state.lock().unwrap().config.press_color_rgb,
            };

            let new_counters = CounterState {
                device_id: backup.device.device_id.clone(),
                counter_generation: backup.device.counter_generation.saturating_add(1),
                lifetime_key1: backup.stats.lifetime_key1,
                lifetime_key2: backup.stats.lifetime_key2,
                map_key1: 0,
                map_key2: 0,
            };

            {
                let s = storage.lock().unwrap();
                let _ = s.save_config(&new_config);
                let info = DeviceInfo {
                    device_id: backup.device.device_id,
                    board_profile: backup.device.board_profile,
                    firmware_version: "1.0.0".to_string(),
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
            log_info(log_hub, "JSON Backup imported and applied");

            IpcResponse::BackupImported {
                success: true,
                counters: new_counters,
                config: new_config,
            }
        }

        IpcRequest::GetLogEntries { limit } => {
            let logs = {
                let hub = log_hub.lock().unwrap();
                hub.iter().rev().take(limit).cloned().collect()
            };
            IpcResponse::LogEntries(logs)
        }

        IpcRequest::PrepareFlash => {
            if mode == RuntimeMode::Playing {
                return IpcResponse::OperationRejected {
                    reason: "Cannot flash firmware during gameplay".to_string(),
                };
            }
            log_info(log_hub, "Releasing serial port for firmware flash...");
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
            log_info(log_hub, "Flash finished, resuming device discovery...");
            device.resume();
            IpcResponse::HandshakeAck {
                daemon_version: "1.0.0".to_string(),
                daemon_protocol: IPC_PROTOCOL_VERSION,
                device_connected: state.lock().unwrap().device_connected,
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

fn log_info(hub: &Arc<Mutex<VecDeque<String>>>, msg: &str) {
    let now = Utc::now().format("%H:%M:%S").to_string();
    let mut h = hub.lock().unwrap();
    if h.len() >= MAX_LOG_ENTRIES {
        h.pop_front();
    }
    h.push_back(format!("{} HOST {}", now, msg));
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
