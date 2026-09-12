use anyhow::Result;
use chrono::Utc;
use osupad_device::{DeviceEvent, DeviceManager};
use osupad_ipc::{
    create_listener, get_socket_path, read_request, send_response, IpcRequest, IpcResponse,
    IPC_PROTOCOL_VERSION,
};
use osupad_model::{
    char_to_hid_usage, CounterState, DeviceConfig, DeviceInfo, JsonBackup,
    RuntimeMode,
};
use osupad_storage::{reconcile_counters, Storage};
use osupad_tosu::TosuManager;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

const COOLDOWN_DURATION: Duration = Duration::from_secs(5);
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
    }));

    // Start tosu WebSocket manager
    let (tosu_manager, mut tosu_rx) = TosuManager::new(initial_config.tosu_endpoint.clone());
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
    let mut last_gameplay_update = Instant::now();
    let gameplay_interval = Duration::from_millis(1000 / initial_config.gameplay_display_hz.max(1) as u64);

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

                        // Send time sync immediately (§14.3)
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.send_time_sync().await;
                        });
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
                        }
                    }
                    DeviceEvent::StatusUpdate(_) => {}
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

                if telemetry.is_playing {
                    // Enter PLAYING mode
                    if current_mode != RuntimeMode::Playing {
                        info!("State transition -> PLAYING (osu! map active)");
                        log_info(&log_hub, &format!("State -> PLAYING ({})", telemetry.title));
                        let mut st = daemon_state.lock().unwrap();
                        st.mode = RuntimeMode::Playing;
                        cooldown_deadline = None;
                    }

                    // Rate-limited gameplay display stream (§14.2)
                    if last_gameplay_update.elapsed() >= gameplay_interval {
                        last_gameplay_update = Instant::now();
                        let (k1, k2) = {
                            let st = daemon_state.lock().unwrap();
                            (st.counters.map_key1, st.counters.map_key2)
                        };
                        let _ = device_manager.send_gameplay_state(&telemetry, k1, k2).await;
                    }
                } else if current_mode == RuntimeMode::Playing {
                    // Left playing mode -> start COOLDOWN timer (§11.2)
                    info!("State transition -> COOLDOWN (5s window started)");
                    log_info(&log_hub, "State -> COOLDOWN (5s)");
                    let mut st = daemon_state.lock().unwrap();
                    st.mode = RuntimeMode::Cooldown;
                    cooldown_deadline = Some(Instant::now() + COOLDOWN_DURATION);
                }
            }

            // 3. Periodic tick for cooldown expiration and background time maintenance
            _ = tokio::time::sleep(Duration::from_millis(200)) => {
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
                tosu_endpoint: "ws://127.0.0.1:24050/ws".to_string(),
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
            device.pause();
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

fn log_info(hub: &Arc<Mutex<VecDeque<String>>>, msg: &str) {
    let now = Utc::now().format("%H:%M:%S").to_string();
    let mut h = hub.lock().unwrap();
    if h.len() >= MAX_LOG_ENTRIES {
        h.pop_front();
    }
    h.push_back(format!("{} HOST {}", now, msg));
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
