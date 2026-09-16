use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use osupad_device::{DeviceEvent, DeviceManager};
use osupad_ipc::{create_listener, get_socket_path, read_request, send_response};
use osupad_layout::{Layout, Screen};
use osupad_model::{CounterState, DeviceConfig, LogSource, RuntimeMode};
use osupad_storage::Storage;
use osupad_tosu::{spawn_tosu_supervisor, TosuManager};

pub mod identity;
pub mod ipc_handlers;
pub mod log_hub;
pub mod runtime;
pub mod sync;
pub mod telemetry;
pub mod updater;

use ipc_handlers::handle_ipc_request;
use log_hub::{LogHub, LogHubLayer};
use runtime::{apply_event, PendingOperations, RuntimeAction, RuntimeController, RuntimeEvent};
use sync::perform_sync;

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
    if osupad_ipc::connect(&socket_path).await.is_ok() {
        anyhow::bail!(
            "Another osupad-daemon instance is already running at {}",
            socket_path.display()
        );
    }

    let db_path = osupad_model::paths::database_path()
        .context("Cannot resolve where to keep the osu!pad database")?;
    info!("Using SQLite database at {}", db_path.display());
    let (
        storage,
        storage_error,
        initial_config,
        initial_device,
        initial_counters,
        initial_layouts,
        existing_ids,
    ) = match Storage::open(&db_path) {
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
            let existing = s
                .list_device_states()
                .unwrap_or_default()
                .into_iter()
                .map(|(info, _)| info.device_id)
                .collect();
            info!("osupad-daemon initialized, storage loaded successfully");
            (
                Arc::new(Mutex::new(Some(s))),
                None,
                cfg,
                init_dev,
                init_cnt,
                layouts,
                existing,
            )
        }
        Err(e) => {
            let err_msg = format!("Database error ({}): {}", db_path.display(), e);
            error!(
                "Failed to open/migrate SQLite database: {}. Starting in degraded mode (§P2-12)",
                err_msg
            );
            (
                Arc::new(Mutex::new(None)),
                Some(err_msg),
                DeviceConfig::default(),
                None,
                CounterState::default(),
                HashMap::new(),
                Vec::new(),
            )
        }
    };

    // This install's identity (§W3-1), needed before the first pad connects
    // so an unclaimed pad can be claimed silently on that first HelloAck.
    let install_id = identity::load_or_create(&storage);
    if install_id.is_none() {
        warn!("Running with no install identity; pad ownership will not be recorded (§W3-1)");
    }

    let start_now = Instant::now();
    let mut controller = RuntimeController::new(
        initial_config.clone(),
        initial_device,
        initial_counters,
        initial_layouts,
        storage_error,
        existing_ids,
        start_now,
    );

    controller.set_install_id(install_id.clone());

    let daemon_state = Arc::new(Mutex::new(controller.state.clone()));
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    // Launch and supervise tosu, then follow its WebSocket
    let tosu_log_path = osupad_model::paths::tosu_log_path()
        .context("Cannot resolve where to keep the tosu log")?;
    let tosu_supervisor =
        spawn_tosu_supervisor(initial_config.tosu_endpoint.clone(), tosu_log_path);
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

    // Updates: tosu (§U-1) and the app (§U-2). Idle-only, daily, and inert
    // until a signing key is compiled in (§U-0.3).
    let update_service =
        updater::spawn_update_worker(daemon_state.clone(), storage.clone(), tosu_supervisor);

    // Spawn IPC request handling task
    {
        let daemon_state = daemon_state.clone();
        let storage = storage.clone();
        let device_manager = device_manager.clone();
        let log_hub = log_hub.clone();
        let pending_ops = pending_ops.clone();
        let update_service = update_service.clone();

        tokio::spawn(async move {
            loop {
                match ipc_listener.accept().await {
                    Ok(mut stream) => {
                        let daemon_state = daemon_state.clone();
                        let storage = storage.clone();
                        let device_manager = device_manager.clone();
                        let log_hub = log_hub.clone();
                        let pending_ops = pending_ops.clone();
                        let update_service = update_service.clone();

                        tokio::spawn(async move {
                            while let Ok(req) = read_request(&mut stream).await {
                                let resp = handle_ipc_request(
                                    req,
                                    &daemon_state,
                                    &storage,
                                    &*device_manager,
                                    &log_hub,
                                    &pending_ops,
                                    Some(&update_service),
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
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    let mut last_db_retry = Instant::now();
    let mut last_instant = Instant::now();
    let mut last_system_time = std::time::SystemTime::now();
    // Syncs run in their own task (they can take ~10 s) and report back here
    let (sync_done_tx, mut sync_done_rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<CounterState, String>>();

    loop {
        let mut event_opt = None;

        tokio::select! {
            // 1. Device hardware events
            Ok(dev_event) = device_rx.recv() => {
                match dev_event {
                    DeviceEvent::Connected(info) => {
                        info!(
                            "ESP32 Device Connected: ID={}, Board={}, Firmware={}, Slot={}",
                            info.device_id,
                            info.board_profile,
                            info.firmware_version,
                            info.running_partition.as_deref().unwrap_or("unknown"),
                        );
                        event_opt = Some(RuntimeEvent::DeviceConnected(info));
                    }
                    DeviceEvent::Ownership { owner_id } => {
                        event_opt = Some(RuntimeEvent::DeviceOwnership(owner_id));
                    }
                    DeviceEvent::Disconnected => {
                        warn!("ESP32 Device Disconnected");
                        event_opt = Some(RuntimeEvent::DeviceDisconnected);
                    }
                    DeviceEvent::Counters(c) => {
                        event_opt = Some(RuntimeEvent::DeviceCounters(c));
                    }
                    DeviceEvent::StatusUpdate(status) => {
                        event_opt = Some(RuntimeEvent::DeviceLatency(osupad_model::LatencyStats {
                            samples: status.latency_samples,
                            p50_us: status.latency_p50_us,
                            p99_us: status.latency_p99_us,
                            p999_us: status.latency_p999_us,
                            max_us: status.latency_max_us,
                            deferred_reports: status.hid_deferred_reports,
                        }));
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
                        event_opt = Some(RuntimeEvent::DeviceLayoutAck { screen, success, message });
                    }
                    DeviceEvent::LogBatch(batch) => {
                        event_opt = Some(RuntimeEvent::DeviceLogBatch(batch));
                    }
                }
            }

            // 2. Telemetry events from tosu
            Ok(telemetry) = tosu_rx.recv() => {
                event_opt = Some(RuntimeEvent::TosuTelemetry {
                    is_playing: telemetry.is_playing,
                    live_time_ms: telemetry.live_time_ms,
                    title: telemetry.title,
                    values: telemetry.values,
                });
            }

            // 3. tosu WebSocket connected / disconnected
            Ok(()) = tosu_connected_rx.changed() => {
                let connected = *tosu_connected_rx.borrow_and_update();
                info!("{}", if connected { "tosu connected" } else { "tosu disconnected" });
                event_opt = Some(RuntimeEvent::TosuConnectionChanged(connected));
            }

            // 4. A background sync finished
            Some(res) = sync_done_rx.recv() => {
                event_opt = Some(match res {
                    Ok(counters) => RuntimeEvent::SyncCompleted {
                        success: true,
                        counters,
                        time_str: Some(chrono::Utc::now().to_rfc3339()),
                        error: None,
                    },
                    Err(e) => RuntimeEvent::SyncCompleted {
                        success: false,
                        counters: controller.state.counters.clone(),
                        time_str: None,
                        error: Some(e),
                    },
                });
            }

            // 5. Periodic tick
            _ = tick.tick() => {
                let now = Instant::now();

                // Periodic clock jump check
                let now_system = std::time::SystemTime::now();
                let elapsed_instant = now.saturating_duration_since(last_instant);
                let clock_jump = match now_system.duration_since(last_system_time) {
                    Ok(sys_elapsed) => sys_elapsed.abs_diff(elapsed_instant) > Duration::from_secs(2),
                    Err(_) => true,
                };
                last_instant = now;
                last_system_time = now_system;

                if clock_jump && controller.state.device_connected && controller.state.mode == RuntimeMode::Idle {
                    let dm = device_manager.clone();
                    tokio::spawn(async move {
                        let _ = dm.send_time_sync().await;
                    });
                }

                // Retry opening SQLite every 60s if unavailable (§P2-12)
                if storage.lock().unwrap().is_none() && last_db_retry.elapsed() >= Duration::from_secs(60) {
                    last_db_retry = Instant::now();
                    info!("Retrying SQLite database connection at {}", db_path.display());
                    match Storage::open(&db_path) {
                        Ok(s) => {
                            info!("Successfully reconnected to SQLite database at {}", db_path.display());
                            let cfg = s.load_config().ok();
                            let mut layouts = Vec::new();
                            for screen in Screen::ALL {
                                if let Ok(Some(json)) = s.load_layout(screen.to_wire()) {
                                    if let Ok(l) = Layout::from_json(&json) {
                                        layouts.push((*screen, l));
                                    }
                                }
                            }
                            let stored_dev = if let Some(info) = &controller.state.device_info {
                                s.load_device_state(&info.device_id).unwrap_or(None)
                            } else {
                                None
                            };
                            *storage.lock().unwrap() = Some(s);
                            let _ = apply_event(&mut controller, &daemon_state, &pending_ops, RuntimeEvent::SqliteReconnected {
                                config: cfg,
                                layouts,
                                stored_device: stored_dev,
                            }, now);
                        }
                        Err(e) => {
                            warn!("SQLite database reconnection attempt failed: {}", e);
                            let _ = apply_event(&mut controller, &daemon_state, &pending_ops, RuntimeEvent::SqliteError(format!("Database error ({}): {}", db_path.display(), e)), now);
                        }
                    }
                }

                event_opt = Some(RuntimeEvent::Tick(now));
            }
        }

        if let Some(event) = event_opt {
            let now = Instant::now();
            let actions = apply_event(&mut controller, &daemon_state, &pending_ops, event, now);

            for action in actions {
                match action {
                    RuntimeAction::SendTimeSync => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.send_time_sync().await;
                        });
                    }
                    RuntimeAction::ClaimOwnership => {
                        // §W3-2: an NVS write, which the firmware honours only
                        // in IDLE. Connect time already is one.
                        if let Some(owner) =
                            install_id.as_deref().and_then(identity::parse_owner_id)
                        {
                            let dm = device_manager.clone();
                            tokio::spawn(async move {
                                if let Err(e) = dm.claim_ownership(&owner).await {
                                    warn!("Could not record ownership on the pad: {}", e);
                                }
                            });
                        }
                    }
                    RuntimeAction::SendConfig(cfg) => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.send_config(&cfg).await;
                        });
                    }
                    RuntimeAction::SendHostStatus {
                        tosu_connected,
                        is_playing,
                        play_id,
                    } => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm
                                .send_host_status(tosu_connected, is_playing, play_id)
                                .await;
                        });
                    }
                    RuntimeAction::SendDataUpdate(changes) => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.send_data_update(&changes).await;
                        });
                    }
                    RuntimeAction::SendLayout(screen, layout) => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.send_layout(screen, &layout).await;
                        });
                    }
                    RuntimeAction::TriggerSync => {
                        let ds = daemon_state.clone();
                        let stg = storage.clone();
                        let dm = device_manager.clone();
                        let po = pending_ops.clone();
                        let done = sync_done_tx.clone();
                        tokio::spawn(async move {
                            let res = perform_sync(&ds, &stg, &*dm, &po).await;
                            let _ = done.send(res);
                        });
                    }
                    RuntimeAction::RequestDeviceStatus => {
                        let dm = device_manager.clone();
                        tokio::spawn(async move {
                            let _ = dm.request_status().await;
                        });
                    }
                    RuntimeAction::SetStorageWritesAllowed(allowed) => {
                        if let Some(s) = storage.lock().unwrap().as_mut() {
                            s.set_writes_allowed(allowed);
                        }
                    }
                    RuntimeAction::SaveInitialDeviceState(info, counters) => {
                        if let Some(s) = storage.lock().unwrap().as_ref() {
                            let _ = s.save_device_state(&info, &counters);
                        }
                    }
                    RuntimeAction::TouchDeviceLastSeen(device_id) => {
                        if let Some(s) = storage.lock().unwrap().as_ref() {
                            let _ = s.touch_device_last_seen(&device_id);
                        }
                    }
                    RuntimeAction::PushEspLog {
                        level,
                        tag,
                        message,
                    } => {
                        log_hub.push(LogSource::Esp, level, &tag, message);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::DaemonState;
    use osupad_ipc::{IpcRequest, IpcResponse};
    use osupad_model::{CounterSource, DeviceInfo};

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
                running_partition: None,
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
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
        }));

        // 1. GetStatus surfaces storage_error
        let resp = handle_ipc_request(
            IpcRequest::GetStatus,
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        match resp {
            IpcResponse::Status { storage_error, .. } => {
                assert_eq!(
                    storage_error,
                    Some("Database permission denied".to_string())
                );
            }
            other => panic!("Expected Status response, got {:?}", other),
        }

        // 2. Destructive/reconciliation operations rejected without SQLite
        let resp = handle_ipc_request(
            IpcRequest::ForceSync,
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::ResetCounters { confirm: true },
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::RestoreDeviceFromPc { confirm: true },
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        let resp = handle_ipc_request(
            IpcRequest::ImportPcFromDevice { confirm: true },
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

        // 3. Layouts still work in-memory
        let resp = handle_ipc_request(
            IpcRequest::GetLayouts,
            &daemon_state,
            &storage,
            &*device_manager,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(
            resp,
            IpcResponse::Layouts {
                idle: None,
                playing: None
            }
        ));

        // 4. perform_sync does not modify ESP counters when storage is None
        let res = perform_sync(&daemon_state, &storage, &*device_manager, &pending_ops).await;
        assert!(res.is_err());
        let st = daemon_state.lock().unwrap();
        assert_eq!(st.counters.lifetime_key1, 50);
        assert_eq!(st.counters.lifetime_key2, 60);
        assert!(st.last_sync_error.is_some());
    }
}
