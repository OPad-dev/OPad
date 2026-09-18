use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use osupad_daemon::ipc_handlers::handle_ipc_request;
use osupad_daemon::log_hub::LogHub;
use osupad_daemon::runtime::{
    DaemonState, PendingOperations, RuntimeAction, RuntimeController, RuntimeEvent,
    COOLDOWN_DURATION,
};
use osupad_daemon::sync::{perform_sync, DeviceLink};
use osupad_device::{DeviceError, DeviceEvent};
use osupad_ipc::{IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, JsonBackup, LogLevel, LogSource,
    RuntimeMode,
};
use osupad_storage::Storage;

struct MockDeviceLink {
    event_tx: broadcast::Sender<DeviceEvent>,
    connected: Arc<AtomicBool>,
    sent_configs: Arc<Mutex<Vec<DeviceConfig>>>,
    sent_syncs: Arc<Mutex<Vec<(CounterState, bool)>>>,
    sync_response: Arc<Mutex<Option<Result<(), String>>>>,
    sync_seq: Arc<AtomicU32>,
    claimed_owners: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl MockDeviceLink {
    fn new(connected: bool) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            event_tx,
            connected: Arc::new(AtomicBool::new(connected)),
            sent_configs: Arc::new(Mutex::new(Vec::new())),
            sent_syncs: Arc::new(Mutex::new(Vec::new())),
            sync_response: Arc::new(Mutex::new(Some(Ok(())))),
            sync_seq: Arc::new(AtomicU32::new(1)),
            claimed_owners: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl DeviceLink for MockDeviceLink {
    async fn send_time_sync(&self) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn send_config(&self, config: &DeviceConfig) -> Result<(), DeviceError> {
        self.sent_configs.lock().unwrap().push(config.clone());
        Ok(())
    }

    async fn send_layout(&self, _screen: Screen, _layout: &Layout) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn reset_layout(&self, _screen: Screen) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn send_host_status(
        &self,
        _tosu_connected: bool,
        _is_playing: bool,
        _play_id: u32,
    ) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn send_data_update(&self, _values: &[(u8, SourceValue)]) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn send_counter_sync(
        &self,
        counters: &CounterState,
        force_restore: bool,
    ) -> Result<u32, DeviceError> {
        self.sent_syncs
            .lock()
            .unwrap()
            .push((counters.clone(), force_restore));
        let seq = self.sync_seq.fetch_add(1, Ordering::SeqCst);

        let resp_opt = self.sync_response.lock().unwrap().clone();
        if let Some(resp) = resp_opt {
            let (success, message) = match resp {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e),
            };
            let _ = self.event_tx.send(DeviceEvent::CounterSyncResult {
                seq,
                success,
                message,
                state: None,
            });
        }
        Ok(seq)
    }

    async fn claim_ownership(&self, owner_id: &[u8]) -> Result<(), DeviceError> {
        self.claimed_owners.lock().unwrap().push(owner_id.to_vec());
        Ok(())
    }

    async fn request_status(&self) -> Result<(), DeviceError> {
        let _ = self.event_tx.send(DeviceEvent::StatusUpdate(
            osupad_device::proto::DeviceStatus {
                state: osupad_device::proto::DeviceState::Idle as i32,
                ..Default::default()
            },
        ));
        Ok(())
    }

    async fn request_logs(&self) -> Result<(), DeviceError> {
        Ok(())
    }

    async fn reset_latency_stats(&self) -> Result<(), DeviceError> {
        Ok(())
    }

    fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
        self.event_tx.subscribe()
    }

    async fn pause_and_release(&self, _timeout: Duration) -> bool {
        self.connected.store(false, Ordering::SeqCst);
        true
    }

    fn resume(&self) {
        self.connected.store(true, Ordering::SeqCst);
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

// 1. daemon with no ESP; daemon with ESP but no tosu; tosu reconnect
#[tokio::test]
async fn test_daemon_connection_states_and_reconnect() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    );

    // Initial state: no ESP, no tosu
    assert_eq!(controller.state.mode, RuntimeMode::Idle);
    assert!(!controller.state.device_connected);
    assert_eq!(controller.state.counters_source, CounterSource::Pc);
    assert!(!controller.state.tosu_connected);

    // ESP connects
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-TEST01".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(dev_info.clone()), now);
    assert!(controller.state.device_connected);
    assert_eq!(controller.state.counters_source, CounterSource::Device);
    assert!(actions.contains(&RuntimeAction::SendTimeSync));
    assert!(actions.contains(&RuntimeAction::SendConfig(controller.state.config.clone())));
    assert!(actions.contains(&RuntimeAction::SendHostStatus {
        tosu_connected: false,
        is_playing: false,
        play_id: controller.play_id,
    }));

    // tosu connects
    let actions = controller.on_event(RuntimeEvent::TosuConnectionChanged(true), now);
    assert!(controller.state.tosu_connected);
    assert!(actions.contains(&RuntimeAction::SendHostStatus {
        tosu_connected: true,
        is_playing: false,
        play_id: controller.play_id,
    }));

    // tosu disconnects and reconnects
    let actions = controller.on_event(RuntimeEvent::TosuConnectionChanged(false), now);
    assert!(!controller.state.tosu_connected);
    assert!(actions.contains(&RuntimeAction::SendHostStatus {
        tosu_connected: false,
        is_playing: false,
        play_id: controller.play_id,
    }));

    let actions = controller.on_event(RuntimeEvent::TosuConnectionChanged(true), now);
    assert!(controller.state.tosu_connected);
    assert!(actions.contains(&RuntimeAction::SendHostStatus {
        tosu_connected: true,
        is_playing: false,
        play_id: controller.play_id,
    }));
}

// 2. PLAYING → COOLDOWN → PLAYING (no sync, no writes)
#[tokio::test]
async fn test_playing_cooldown_playing_no_sync() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    );

    // 1. Enter playing
    let actions = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "Test Map".to_string(),
            values: Vec::new(),
        },
        now,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Playing);
    assert!(actions.contains(&RuntimeAction::SetStorageWritesAllowed(false)));

    // 2. Map ends -> enters cooldown
    let actions = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "Test Map".to_string(),
            values: Vec::new(),
        },
        now + Duration::from_secs(1),
    );
    assert_eq!(controller.state.mode, RuntimeMode::Cooldown);
    assert!(actions.contains(&RuntimeAction::SetStorageWritesAllowed(false)));
    assert!(!actions.contains(&RuntimeAction::TriggerSync));

    // 3. User restarts or starts another map at 2 seconds into cooldown (< 5s deadline)
    let actions = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 500.0,
            title: "Test Map 2".to_string(),
            values: Vec::new(),
        },
        now + Duration::from_secs(3),
    );
    assert_eq!(controller.state.mode, RuntimeMode::Playing);
    assert_eq!(controller.cooldown_deadline, None);
    assert!(!actions.contains(&RuntimeAction::TriggerSync));
}

// 3. PLAYING → COOLDOWN → SYNC → IDLE (exactly one sync)
#[tokio::test]
async fn test_playing_cooldown_sync_idle_exactly_one_sync() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    );

    // 1. Playing
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "Test Map".to_string(),
            values: Vec::new(),
        },
        now,
    );

    // 2. Left playing -> Cooldown
    let t_cooldown = now + Duration::from_secs(1);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "Test Map".to_string(),
            values: Vec::new(),
        },
        t_cooldown,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Cooldown);

    // 3. Tick before deadline: no sync
    let actions = controller.on_event(
        RuntimeEvent::Tick(t_cooldown + Duration::from_secs(4)),
        t_cooldown + Duration::from_secs(4),
    );
    assert!(!actions.contains(&RuntimeAction::TriggerSync));
    assert_eq!(controller.state.mode, RuntimeMode::Cooldown);

    // 4. Tick after COOLDOWN_DURATION: enters Sync and triggers sync
    let t_expired = t_cooldown + COOLDOWN_DURATION + Duration::from_millis(10);
    let actions = controller.on_event(RuntimeEvent::Tick(t_expired), t_expired);
    assert_eq!(controller.state.mode, RuntimeMode::Sync);
    assert!(actions.contains(&RuntimeAction::TriggerSync));

    // 5. Subsequent ticks during Sync do NOT trigger another sync
    let actions2 = controller.on_event(
        RuntimeEvent::Tick(t_expired + Duration::from_millis(50)),
        t_expired + Duration::from_millis(50),
    );
    assert!(!actions2.contains(&RuntimeAction::TriggerSync));

    // 6. Sync completes -> transitions to Idle
    let synced = CounterState {
        device_id: "OSUPAD-01".to_string(),
        counter_generation: 1,
        lifetime_key1: 100,
        lifetime_key2: 200,
        map_key1: 0,
        map_key2: 0,
    };
    let actions = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: true,
            counters: synced.clone(),
            time_str: Some("2026-09-13T00:00:00Z".to_string()),
            error: None,
        },
        t_expired + Duration::from_millis(100),
    );
    assert_eq!(controller.state.mode, RuntimeMode::Idle);
    assert!(actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)));
    assert_eq!(controller.state.counters.lifetime_key1, 100);
}

// 4. zero Storage writes during PLAYING/COOLDOWN for every IPC operation (P1-3 guard)
#[tokio::test]
async fn test_zero_storage_writes_during_gameplay_and_cooldown() {
    let storage = Storage::open_in_memory().unwrap();
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-SAFE".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };
    let initial_counters = CounterState {
        device_id: "OSUPAD-SAFE".to_string(),
        counter_generation: 1,
        lifetime_key1: 10,
        lifetime_key2: 20,
        map_key1: 0,
        map_key2: 0,
    };
    storage
        .save_device_state(&dev_info, &initial_counters)
        .unwrap();

    let storage = Arc::new(Mutex::new(Some(storage)));
    let device = MockDeviceLink::new(true);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    for test_mode in [RuntimeMode::Playing, RuntimeMode::Cooldown] {
        let daemon_state = Arc::new(Mutex::new(DaemonState {
            mode: test_mode,
            device_connected: true,
            device_info: Some(dev_info.clone()),
            counters: initial_counters.clone(),
            counters_source: CounterSource::Device,
            pc_counters: Some(initial_counters.clone()),
            esp_counters: Some(initial_counters.clone()),
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: None,
            tosu_connected: true,
            latency: None,
            pending_replacement: None,
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
            last_backup: None,
        }));

        // Set storage writes blocked guard
        storage
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .set_writes_allowed(false);

        // 1. ForceSync rejected
        let r = handle_ipc_request(
            IpcRequest::ForceSync,
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 2. ResetCounters rejected
        let r = handle_ipc_request(
            IpcRequest::ResetCounters { confirm: true },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 3. RestoreDeviceFromPc rejected
        let r = handle_ipc_request(
            IpcRequest::RestoreDeviceFromPc { confirm: true },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 4. ImportPcFromDevice rejected
        let r = handle_ipc_request(
            IpcRequest::ImportPcFromDevice { confirm: true },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 5. ResolveReplacement rejected
        let r = handle_ipc_request(
            IpcRequest::ResolveReplacement { restore: true },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 6. ImportBackup rejected
        let backup = JsonBackup::new(&dev_info, &initial_counters, &DeviceConfig::default());
        let r = handle_ipc_request(
            IpcRequest::ImportBackup {
                backup,
                confirm: true,
            },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 7. PrepareFlash rejected
        let r = handle_ipc_request(
            IpcRequest::PrepareFlash,
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationRejected { .. }));

        // 8. UpdateConfig with key change is deferred (no storage write)
        let new_cfg = DeviceConfig {
            debounce_us: 6000,
            ..Default::default()
        };
        let r = handle_ipc_request(
            IpcRequest::UpdateConfig(new_cfg),
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::OperationDeferred { .. }));

        // 9. SetLayout is deferred (no storage write)
        let r = handle_ipc_request(
            IpcRequest::SetLayout {
                screen: Screen::Idle,
                layout: Layout {
                    background: 0,
                    widgets: Vec::new(),
                },
            },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::LayoutApplied { .. }));

        // 10. ResetLayout is deferred (no storage write)
        let r = handle_ipc_request(
            IpcRequest::ResetLayout {
                screen: Screen::Idle,
            },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(matches!(r, IpcResponse::LayoutApplied { .. }));

        // Verify storage counters did NOT change
        let loaded = storage
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .load_device_state("OSUPAD-SAFE")
            .unwrap()
            .unwrap();
        assert_eq!(loaded.lifetime_key1, 10);
        assert_eq!(loaded.lifetime_key2, 20);
    }
}

// 5. reconcile PC→ESP, ESP→PC, stale generation, replacement prompt (P1-2)
#[tokio::test]
async fn test_reconcile_and_replacement_scenarios() {
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-RECON".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };

    // Case A: PC generation > ESP generation -> PC wins and is sent to ESP
    {
        let storage = Storage::open_in_memory().unwrap();
        let pc_state = CounterState {
            device_id: "OSUPAD-RECON".to_string(),
            counter_generation: 5,
            lifetime_key1: 500,
            lifetime_key2: 600,
            map_key1: 0,
            map_key2: 0,
        };
        storage.save_device_state(&dev_info, &pc_state).unwrap();
        let storage = Arc::new(Mutex::new(Some(storage)));
        let device = MockDeviceLink::new(true);
        let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

        let esp_state = CounterState {
            device_id: "OSUPAD-RECON".to_string(),
            counter_generation: 4,
            lifetime_key1: 100,
            lifetime_key2: 200,
            map_key1: 0,
            map_key2: 0,
        };
        let daemon_state = Arc::new(Mutex::new(DaemonState {
            mode: RuntimeMode::Idle,
            device_connected: true,
            device_info: Some(dev_info.clone()),
            counters: esp_state,
            counters_source: CounterSource::Device,
            pc_counters: Some(pc_state.clone()),
            esp_counters: None,
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: None,
            tosu_connected: false,
            latency: None,
            pending_replacement: None,
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
            last_backup: None,
        }));

        let res = perform_sync(&daemon_state, &storage, &device, &pending_ops).await;
        assert!(res.is_ok());
        let synced = res.unwrap();
        assert_eq!(synced.counter_generation, 5);
        assert_eq!(synced.lifetime_key1, 500);
        assert_eq!(synced.lifetime_key2, 600);
    }

    // Case B: ESP generation > PC generation -> ESP wins and is saved to PC
    {
        let storage = Storage::open_in_memory().unwrap();
        let pc_state = CounterState {
            device_id: "OSUPAD-RECON".to_string(),
            counter_generation: 2,
            lifetime_key1: 10,
            lifetime_key2: 20,
            map_key1: 0,
            map_key2: 0,
        };
        storage.save_device_state(&dev_info, &pc_state).unwrap();
        let storage = Arc::new(Mutex::new(Some(storage)));
        let device = MockDeviceLink::new(true);
        let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

        let esp_state = CounterState {
            device_id: "OSUPAD-RECON".to_string(),
            counter_generation: 3,
            lifetime_key1: 300,
            lifetime_key2: 400,
            map_key1: 0,
            map_key2: 0,
        };
        let daemon_state = Arc::new(Mutex::new(DaemonState {
            mode: RuntimeMode::Idle,
            device_connected: true,
            device_info: Some(dev_info.clone()),
            counters: esp_state,
            counters_source: CounterSource::Device,
            pc_counters: Some(pc_state),
            esp_counters: None,
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: None,
            tosu_connected: false,
            latency: None,
            pending_replacement: None,
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
            last_backup: None,
        }));

        let res = perform_sync(&daemon_state, &storage, &device, &pending_ops).await;
        assert!(res.is_ok());
        let saved = storage
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .load_device_state("OSUPAD-RECON")
            .unwrap()
            .unwrap();
        assert_eq!(saved.counter_generation, 3);
        assert_eq!(saved.lifetime_key1, 300);
        assert_eq!(saved.lifetime_key2, 400);
    }

    // Case C: Fresh replacement device prompt detection in RuntimeController
    {
        let now = Instant::now();
        let mut controller = RuntimeController::new(
            DeviceConfig::default(),
            None,
            CounterState::default(),
            HashMap::new(),
            None,
            vec!["OSUPAD-OLD".to_string()],
            now,
        );

        let new_pad_info = DeviceInfo {
            device_id: "OSUPAD-NEW".to_string(),
            board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
            firmware_version: "1.0.0".to_string(),
            protocol_version: 1,
            running_partition: None,
        };
        controller.state.counters.counter_generation = 1;
        controller.state.counters.lifetime_key1 = 5;
        controller.state.counters.lifetime_key2 = 8;

        let _ = controller.on_event(RuntimeEvent::DeviceConnected(new_pad_info), now);
        assert_eq!(
            controller.state.pending_replacement,
            Some("OSUPAD-OLD".to_string())
        );
    }
}

// 6. device rejects sync → retry and error surfaced (P1-1)
#[tokio::test]
async fn test_device_rejects_sync_retries_and_surfaces_error() {
    let storage = Storage::open_in_memory().unwrap();
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-RETRY".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };
    let initial_counters = CounterState {
        device_id: "OSUPAD-RETRY".to_string(),
        counter_generation: 1,
        lifetime_key1: 10,
        lifetime_key2: 20,
        map_key1: 0,
        map_key2: 0,
    };
    storage
        .save_device_state(&dev_info, &initial_counters)
        .unwrap();
    let storage = Arc::new(Mutex::new(Some(storage)));

    let device = MockDeviceLink::new(true);
    // Configure device to reject sync with error message
    *device.sync_response.lock().unwrap() = Some(Err("flash write failure".to_string()));

    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: true,
        device_info: Some(dev_info),
        counters: initial_counters,
        counters_source: CounterSource::Device,
        pc_counters: None,
        esp_counters: None,
        config: DeviceConfig::default(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error: None,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        install_id: None,
        pending_takeover: None,
        foreign_pad: false,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: HashMap::new(),
        last_backup: None,
    }));
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    let res = perform_sync(&daemon_state, &storage, &device, &pending_ops).await;
    assert!(res.is_err());
    // Verify all 3 retry attempts took place
    assert_eq!(device.sent_syncs.lock().unwrap().len(), 3);
    // Verify error is surfaced in daemon state
    let st = daemon_state.lock().unwrap();
    assert!(st
        .last_sync_error
        .as_ref()
        .unwrap()
        .contains("flash write failure"));
}

// 7. JSON validation rules and preview/confirm (P1-5)
#[tokio::test]
async fn test_json_validation_preview_and_confirm() {
    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(true);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    let dev_info = DeviceInfo {
        device_id: "OSUPAD-JSON".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };
    let current_counters = CounterState {
        device_id: "OSUPAD-JSON".to_string(),
        counter_generation: 2,
        lifetime_key1: 1000,
        lifetime_key2: 2000,
        map_key1: 0,
        map_key2: 0,
    };
    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: true,
        device_info: Some(dev_info.clone()),
        counters: current_counters.clone(),
        counters_source: CounterSource::Device,
        pc_counters: Some(current_counters.clone()),
        esp_counters: Some(current_counters.clone()),
        config: DeviceConfig::default(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error: None,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        install_id: None,
        pending_takeover: None,
        foreign_pad: false,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: HashMap::new(),
        last_backup: None,
    }));

    // 1. Preview with counter rollback & generation bump warnings
    let mut rollback_backup =
        JsonBackup::new(&dev_info, &current_counters, &DeviceConfig::default());
    rollback_backup.stats.lifetime_key1 = 500; // lower than 1000
    rollback_backup.device.counter_generation = 1; // lower than current 2

    let resp = handle_ipc_request(
        IpcRequest::PreviewImport(rollback_backup.clone()),
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match resp {
        IpcResponse::ImportPreview {
            device_id_matches,
            is_counter_rollback,
            warnings,
            ..
        } => {
            assert!(device_id_matches);
            assert!(is_counter_rollback);
            assert!(warnings.iter().any(|w| w.contains("rollback")));
            assert!(warnings.iter().any(|w| w.contains("bumped")));
        }
        other => panic!("Expected ImportPreview, got {:?}", other),
    }

    // 2. Import rejected without confirm
    let resp = handle_ipc_request(
        IpcRequest::ImportBackup {
            backup: rollback_backup.clone(),
            confirm: false,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    assert!(matches!(resp, IpcResponse::OperationRejected { .. }));

    // 3. Import accepted with confirm: generation bumped to max(current, incoming) + 1 = 3
    let resp = handle_ipc_request(
        IpcRequest::ImportBackup {
            backup: rollback_backup,
            confirm: true,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    match resp {
        IpcResponse::BackupImported {
            success, counters, ..
        } => {
            assert!(success);
            assert_eq!(counters.counter_generation, 3);
            assert_eq!(counters.lifetime_key1, 500);
        }
        other => panic!("Expected BackupImported, got {:?}", other),
    }
}

// 8. IPC handshake mismatch (P1-7); oversized frame (P2-6); second daemon refused (P2-6)
#[tokio::test]
async fn test_ipc_handshake_mismatch_and_protocol_version() {
    let storage = Arc::new(Mutex::new(None));
    let device = MockDeviceLink::new(false);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: false,
        device_info: None,
        counters: CounterState::default(),
        counters_source: CounterSource::Pc,
        pc_counters: None,
        esp_counters: None,
        config: DeviceConfig::default(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error: None,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        install_id: None,
        pending_takeover: None,
        foreign_pad: false,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: HashMap::new(),
        last_backup: None,
    }));

    // Protocol mismatch rejected
    let resp = handle_ipc_request(
        IpcRequest::Handshake {
            client_version: "0.9.0".to_string(),
            client_protocol: 999, // Mismatched protocol version
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match resp {
        IpcResponse::HandshakeRejected {
            daemon_protocol,
            reason,
        } => {
            assert_eq!(daemon_protocol, IPC_PROTOCOL_VERSION);
            assert!(reason.contains("mismatch"));
        }
        other => panic!("Expected HandshakeRejected, got {:?}", other),
    }

    // §U-2: a client from a different app version is rejected loudly. After an
    // in-place update the files on disk are new while this process is still
    // the old binary, and a new GUI must not talk to it.
    let resp = handle_ipc_request(
        IpcRequest::Handshake {
            client_version: "9.9.9".to_string(),
            client_protocol: IPC_PROTOCOL_VERSION,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match resp {
        IpcResponse::HandshakeRejected { reason, .. } => {
            assert!(reason.contains("9.9.9"), "{reason}");
            assert!(
                reason.contains("restart"),
                "the message must say what to do: {reason}"
            );
        }
        other => panic!("Expected HandshakeRejected, got {:?}", other),
    }

    // Matching protocol and version accepted
    let resp = handle_ipc_request(
        IpcRequest::Handshake {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            client_protocol: IPC_PROTOCOL_VERSION,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    assert!(matches!(resp, IpcResponse::HandshakeAck { .. }));
}

/// §U-2: an update must never be applied while a map is running, and asking
/// for one then must defer rather than fail.
#[tokio::test]
async fn test_install_update_is_refused_outside_idle() {
    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(false);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    for mode in [
        RuntimeMode::Playing,
        RuntimeMode::Cooldown,
        RuntimeMode::Sync,
    ] {
        let daemon_state = Arc::new(Mutex::new(DaemonState {
            mode,
            device_connected: false,
            device_info: None,
            counters: CounterState::default(),
            counters_source: CounterSource::Pc,
            pc_counters: None,
            esp_counters: None,
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: None,
            tosu_connected: false,
            latency: None,
            pending_replacement: None,
            install_id: None,
            pending_takeover: None,
            foreign_pad: false,
            incompatible: None,
            ui_values: Vec::new(),
            custom_layouts: HashMap::new(),
            last_backup: None,
        }));

        let resp = handle_ipc_request(
            IpcRequest::InstallUpdate {
                component: osupad_ipc::UpdateComponent::App,
            },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;

        assert!(
            matches!(resp, IpcResponse::OperationDeferred { .. }),
            "{mode:?} must defer an install, got {resp:?}"
        );
    }
}

/// §U-3: the updater never applies firmware. That path takes explicit consent
/// every time, so there is not even a setting to leave switched on.
#[tokio::test]
async fn test_firmware_is_not_an_enableable_updater() {
    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(false);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
    let daemon_state = Arc::new(Mutex::new(DaemonState {
        mode: RuntimeMode::Idle,
        device_connected: false,
        device_info: None,
        counters: CounterState::default(),
        counters_source: CounterSource::Pc,
        pc_counters: None,
        esp_counters: None,
        config: DeviceConfig::default(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error: None,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        install_id: None,
        pending_takeover: None,
        foreign_pad: false,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: HashMap::new(),
        last_backup: None,
    }));

    let resp = handle_ipc_request(
        IpcRequest::SetUpdateEnabled {
            component: osupad_ipc::UpdateComponent::Firmware,
            enabled: true,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    assert!(
        matches!(resp, IpcResponse::OperationRejected { .. }),
        "{resp:?}"
    );

    // The other two are ordinary settings and persist
    for component in [
        osupad_ipc::UpdateComponent::App,
        osupad_ipc::UpdateComponent::Tosu,
    ] {
        let resp = handle_ipc_request(
            IpcRequest::SetUpdateEnabled {
                component,
                enabled: false,
            },
            &daemon_state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;
        assert!(
            matches!(resp, IpcResponse::ConfigUpdated { .. }),
            "{resp:?}"
        );
    }
    let key = osupad_daemon::updater::APP_ENABLED_KEY;
    assert_eq!(
        storage
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .get_app_state(key)
            .unwrap()
            .as_deref(),
        Some("0")
    );
}

// 9. LogHub since_seq paging (P2-2)
#[tokio::test]
async fn test_log_hub_since_seq_paging_and_ring() {
    let hub = LogHub::new();

    // Push 2100 entries into 2000-sized ring buffer
    for i in 1..=2100 {
        hub.push(
            LogSource::Host,
            LogLevel::Info,
            "test",
            format!("msg {}", i),
        );
    }

    // Paging with limit 50 from seq 0
    let (entries, latest) = hub.get_entries(Some(0), 50);
    assert_eq!(latest, 2100);
    assert_eq!(entries.len(), 50);
    // Oldest available entry in 2000-element buffer is 2100 - 2000 + 1 = 101
    assert_eq!(entries[0].seq, 101);
    assert_eq!(entries[49].seq, 150);

    // Page next 50 using since_seq = 150
    let (next_entries, _) = hub.get_entries(Some(150), 50);
    assert_eq!(next_entries.len(), 50);
    assert_eq!(next_entries[0].seq, 151);
    assert_eq!(next_entries[49].seq, 200);
}

// 10. queue does not block without a device (P1-8)
#[tokio::test]
async fn test_queue_does_not_block_without_device() {
    let (device_manager, _) = osupad_device::DeviceManager::new_dummy();
    // Device is not connected
    assert!(!device_manager.is_connected());

    // Sending commands returns NotConnected immediately without blocking or hanging
    let res = device_manager.send_time_sync().await;
    assert!(matches!(res, Err(osupad_device::DeviceError::NotConnected)));

    let res = device_manager.send_config(&DeviceConfig::default()).await;
    assert!(matches!(res, Err(osupad_device::DeviceError::NotConnected)));

    let res = device_manager.send_host_status(false, false, 1).await;
    assert!(matches!(res, Err(osupad_device::DeviceError::NotConnected)));

    let res = device_manager
        .send_counter_sync(&CounterState::default(), false)
        .await;
    assert!(matches!(res, Err(osupad_device::DeviceError::NotConnected)));
}

fn pad_info(id: &str) -> DeviceInfo {
    DeviceInfo {
        device_id: id.to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    }
}

fn counters(id: &str, generation: u32, k1: u64, k2: u64) -> CounterState {
    CounterState {
        device_id: id.to_string(),
        counter_generation: generation,
        lifetime_key1: k1,
        lifetime_key2: k2,
        map_key1: 0,
        map_key2: 0,
    }
}

// R3: the device layer emits the pad's counters before Connected. A fresh pad connecting to a
// daemon that started with an older pad's counters must trigger the replacement prompt, and
// must not save the old pad's counters under the new id.
#[tokio::test]
async fn test_replacement_detected_with_real_event_order() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        Some(pad_info("OSUPAD-OLD")),
        counters("OSUPAD-OLD", 3, 50_000, 40_000),
        HashMap::new(),
        None,
        vec!["OSUPAD-OLD".to_string()],
        now,
    );

    let _ = controller.on_event(
        RuntimeEvent::DeviceCounters(counters("OSUPAD-NEW", 1, 3, 4)),
        now,
    );
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(pad_info("OSUPAD-NEW")), now);

    assert_eq!(
        controller.state.pending_replacement,
        Some("OSUPAD-OLD".to_string())
    );
    assert!(!actions
        .iter()
        .any(|a| matches!(a, RuntimeAction::SaveInitialDeviceState(..))));
    assert!(!actions.contains(&RuntimeAction::TriggerSync));
}

// R4/R6: syncs finish in the background. A result arriving after a map started must not pull
// the mode out of PLAYING or re-enable storage writes.
#[tokio::test]
async fn test_late_sync_result_does_not_leave_playing() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    );
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "Map".to_string(),
            values: Vec::new(),
        },
        now,
    );

    for success in [true, false] {
        let actions = controller.on_event(
            RuntimeEvent::SyncCompleted {
                success,
                counters: controller.state.counters.clone(),
                time_str: None,
                error: if success {
                    None
                } else {
                    Some("not idle".to_string())
                },
            },
            now,
        );
        assert_eq!(controller.state.mode, RuntimeMode::Playing);
        assert!(!actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)));
    }
}

// A failed post-cooldown sync must return to IDLE with writes allowed (no stuck SYNC mode).
#[tokio::test]
async fn test_failed_sync_returns_to_idle() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        now,
    );
    controller.state.mode = RuntimeMode::Sync;
    let actions = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: false,
            counters: CounterState::default(),
            time_str: None,
            error: Some("Device not in IDLE state within 3s timeout".to_string()),
        },
        now,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Idle);
    assert!(actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)));
    assert!(controller.state.last_sync_error.is_some());
}

// R10: IPC handlers write the shared state. The main loop must not overwrite those writes with
// the controller's older copy, or the next connect pushes the old config/layouts to the pad.
#[tokio::test]
async fn test_apply_event_keeps_ipc_changes() {
    let now = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        vec!["OSUPAD-OLD".to_string()],
        now,
    );
    let shared = Arc::new(Mutex::new(controller.state.clone()));
    let pending = Arc::new(Mutex::new(PendingOperations::default()));

    // UpdateConfig over IPC
    shared.lock().unwrap().config.brightness = 42;
    let _ = osupad_daemon::runtime::apply_event(
        &mut controller,
        &shared,
        &pending,
        RuntimeEvent::Tick(now),
        now,
    );
    assert_eq!(shared.lock().unwrap().config.brightness, 42);

    let actions = osupad_daemon::runtime::apply_event(
        &mut controller,
        &shared,
        &pending,
        RuntimeEvent::DeviceConnected(pad_info("OSUPAD-OLD")),
        now,
    );
    assert!(actions
        .iter()
        .any(|a| matches!(a, RuntimeAction::SendConfig(c) if c.brightness == 42)));

    // Replacement prompt answered over IPC: the pad is remembered and not asked about again
    let _ = osupad_daemon::runtime::apply_event(
        &mut controller,
        &shared,
        &pending,
        RuntimeEvent::DeviceCounters(counters("OSUPAD-NEW", 1, 0, 0)),
        now,
    );
    let _ = osupad_daemon::runtime::apply_event(
        &mut controller,
        &shared,
        &pending,
        RuntimeEvent::DeviceConnected(pad_info("OSUPAD-NEW")),
        now,
    );
    assert!(shared.lock().unwrap().pending_replacement.is_some());
    shared.lock().unwrap().pending_replacement = None;
    let _ = osupad_daemon::runtime::apply_event(
        &mut controller,
        &shared,
        &pending,
        RuntimeEvent::Tick(now),
        now,
    );
    assert!(controller.known_devices.contains("OSUPAD-NEW"));
}

// perform_sync must not lift the storage write guard: if a map started while it ran, writes
// stay blocked until the runtime allows them again.
#[tokio::test]
async fn test_perform_sync_leaves_write_guard_to_runtime() {
    let dev = pad_info("OSUPAD-GUARD");
    let storage = Storage::open_in_memory().unwrap();
    storage
        .save_device_state(&dev, &counters("OSUPAD-GUARD", 1, 10, 10))
        .unwrap();
    let guard = storage.writes_allowed_handle();
    storage.set_writes_allowed(false);
    let storage = Arc::new(Mutex::new(Some(storage)));

    let mut state = RuntimeController::new(
        DeviceConfig::default(),
        Some(dev),
        counters("OSUPAD-GUARD", 1, 20, 20),
        HashMap::new(),
        None,
        vec!["OSUPAD-GUARD".to_string()],
        Instant::now(),
    )
    .state;
    state.device_connected = true;
    state.mode = RuntimeMode::Playing;
    let daemon_state = Arc::new(Mutex::new(state));
    let device = MockDeviceLink::new(true);
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));

    let _ = perform_sync(&daemon_state, &storage, &device, &pending_ops).await;

    assert!(!guard.load(Ordering::SeqCst));
    assert_eq!(daemon_state.lock().unwrap().mode, RuntimeMode::Playing);
}

/// §W1-2 / P1-8: a cable that drops and comes back mid-map must not wedge the
/// state machine or open a storage-write window. Windows detects hotplug by
/// the reconnect poll rather than udev, so a flaky cable produces *more* of
/// these cycles there, not fewer.
#[tokio::test]
async fn test_replug_during_play_keeps_the_state_machine_and_write_guard() {
    let now = Instant::now();
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-HOTPLUG".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    };
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        Some(dev_info.clone()),
        CounterState::default(),
        HashMap::new(),
        None,
        vec![dev_info.device_id.clone()],
        now,
    );

    controller.on_event(RuntimeEvent::DeviceConnected(dev_info.clone()), now);
    let actions = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "Hotplug Map".to_string(),
            values: Vec::new(),
        },
        now,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Playing);
    assert!(actions.contains(&RuntimeAction::SetStorageWritesAllowed(false)));

    // Three unplug/replug cycles inside the same map
    for cycle in 0..3 {
        let actions = controller.on_event(RuntimeEvent::DeviceDisconnected, now);
        assert!(!controller.state.device_connected, "cycle {cycle}");
        assert_eq!(
            controller.state.mode,
            RuntimeMode::Playing,
            "an unplug must not move the mode (cycle {cycle})"
        );
        assert!(
            !actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)),
            "an unplug must not reopen the write guard mid-map (cycle {cycle})"
        );

        let actions = controller.on_event(RuntimeEvent::DeviceConnected(dev_info.clone()), now);
        assert!(controller.state.device_connected, "cycle {cycle}");
        assert_eq!(
            controller.state.mode,
            RuntimeMode::Playing,
            "a replug must not move the mode (cycle {cycle})"
        );
        assert!(
            !actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)),
            "a replug must not reopen the write guard mid-map (cycle {cycle})"
        );
        assert!(
            !actions.contains(&RuntimeAction::TriggerSync),
            "a replug must not sync mid-map (cycle {cycle})"
        );
        // The same pad coming back is not a replacement
        assert!(
            controller.state.pending_replacement.is_none(),
            "cycle {cycle}"
        );
    }

    // The map ends normally afterwards: cooldown, then sync, then idle
    let actions = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: String::new(),
            values: Vec::new(),
        },
        now,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Cooldown);
    assert!(!actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)));
}

// ---------------------------------------------------------------------------
// §W3-3: pad ownership. All four rows of the table, plus the R3 ordering trap.
// ---------------------------------------------------------------------------

fn owner_bytes(install_id: &str) -> Vec<u8> {
    osupad_daemon::identity::parse_owner_id(install_id)
        .expect("a valid install id")
        .to_vec()
}

fn ownership_controller(install_id: Option<&str>, known: Vec<String>) -> RuntimeController {
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        known,
        Instant::now(),
    );
    controller.set_install_id(install_id.map(|s| s.to_string()));
    controller
}

fn pad(device_id: &str) -> DeviceInfo {
    DeviceInfo {
        device_id: device_id.to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: None,
    }
}

/// Row 2: an unclaimed pad is claimed silently. No prompt for the common case.
#[tokio::test]
async fn test_unclaimed_pad_is_claimed_without_a_prompt() {
    let id = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-FRESH");
    let mut controller = ownership_controller(Some(&id), vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(RuntimeEvent::DeviceOwnership(Vec::new()), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(info), now);

    assert!(controller.state.pending_takeover.is_none());
    assert!(!controller.state.foreign_pad);
    assert!(actions.contains(&RuntimeAction::ClaimOwnership));
    assert!(actions.contains(&RuntimeAction::TriggerSync));
}

/// Row 1: our own pad. Silent, and nothing is rewritten.
#[tokio::test]
async fn test_our_own_pad_connects_silently() {
    let id = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-MINE");
    let mut controller = ownership_controller(Some(&id), vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&id)), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(info), now);

    assert!(controller.state.pending_takeover.is_none());
    assert!(!controller.state.foreign_pad);
    assert!(
        !actions.contains(&RuntimeAction::ClaimOwnership),
        "a pad we already own must not be rewritten on every connect"
    );
    assert!(actions.contains(&RuntimeAction::TriggerSync));
}

/// Row 3: another install's pad. Prompt, and block counter sync until answered.
#[tokio::test]
async fn test_a_pad_owned_elsewhere_prompts_and_blocks_sync() {
    let id = uuid::Uuid::new_v4().to_string();
    let other = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-THEIRS");
    let mut controller = ownership_controller(Some(&id), vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(
        RuntimeEvent::DeviceCounters(CounterState {
            device_id: info.device_id.clone(),
            counter_generation: 4,
            lifetime_key1: 1_234_567,
            lifetime_key2: 7_654_321,
            map_key1: 0,
            map_key2: 0,
        }),
        now,
    );
    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(info.clone()), now);

    let pending = controller
        .state
        .pending_takeover
        .clone()
        .expect("a takeover prompt");
    assert_eq!(pending.device_id, info.device_id);
    // The prompt quotes the pad's counters, so the choice is an informed one
    assert_eq!(pending.device_key1, 1_234_567);
    assert_eq!(pending.device_key2, 7_654_321);

    assert!(controller.state.foreign_pad);
    assert!(
        !actions.contains(&RuntimeAction::TriggerSync),
        "no counter may be written while ownership is unresolved"
    );
    assert!(!actions.contains(&RuntimeAction::ClaimOwnership));
    assert!(
        !actions
            .iter()
            .any(|a| matches!(a, RuntimeAction::SendConfig(_))),
        "someone else's pad must not be reconfigured"
    );
}

/// "Leave it alone": still a keyboard, but nothing is pushed or synced.
#[tokio::test]
async fn test_leaving_a_foreign_pad_alone_writes_nothing() {
    let id = uuid::Uuid::new_v4().to_string();
    let other = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-THEIRS");
    let mut controller = ownership_controller(Some(&id), vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    controller.on_event(RuntimeEvent::DeviceConnected(info.clone()), now);

    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(true);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
    let daemon_state = Arc::new(Mutex::new(controller.state.clone()));

    let resp = handle_ipc_request(
        IpcRequest::ResolveTakeover {
            take_over: false,
            keep_device_counters: false,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    assert!(
        matches!(resp, IpcResponse::OperationRejected { .. }),
        "{resp:?}"
    );

    assert!(
        device.claimed_owners.lock().unwrap().is_empty(),
        "declining must not write an owner onto someone else's pad"
    );
    assert!(device.sent_syncs.lock().unwrap().is_empty());
    assert!(device.sent_configs.lock().unwrap().is_empty());

    let st = daemon_state.lock().unwrap().clone();
    assert!(st.pending_takeover.is_none(), "the prompt is answered");
    assert!(st.foreign_pad, "the pad still belongs to someone else");

    // A later tick must not sneak telemetry or a sync onto it
    controller.state = st;
    let actions = controller.on_event(RuntimeEvent::Tick(now + Duration::from_secs(120)), now);
    assert!(!actions.contains(&RuntimeAction::TriggerSync));
    assert!(!actions
        .iter()
        .any(|a| matches!(a, RuntimeAction::SendDataUpdate(_))));
}

/// Taking over must write the new owner onto the pad. Without that write the
/// pad still names the other install and prompts again on the next connect,
/// which §W3-3 says must not happen.
#[tokio::test]
async fn test_taking_over_claims_the_pad_and_resumes() {
    let id = uuid::Uuid::new_v4().to_string();
    let other = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-THEIRS");
    let mut controller = ownership_controller(Some(&id), vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(
        RuntimeEvent::DeviceCounters(CounterState {
            device_id: info.device_id.clone(),
            counter_generation: 3,
            lifetime_key1: 999,
            lifetime_key2: 888,
            map_key1: 0,
            map_key2: 0,
        }),
        now,
    );
    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    controller.on_event(RuntimeEvent::DeviceConnected(info.clone()), now);

    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(true);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
    let daemon_state = Arc::new(Mutex::new(controller.state.clone()));

    let resp = handle_ipc_request(
        IpcRequest::ResolveTakeover {
            take_over: true,
            keep_device_counters: true,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    match resp {
        IpcResponse::CountersRestored { counters } => {
            assert_eq!(counters.lifetime_key1, 999, "the pad's counters were kept");
            assert_eq!(counters.lifetime_key2, 888);
        }
        other => panic!("expected CountersRestored, got {other:?}"),
    }

    assert_eq!(
        device.claimed_owners.lock().unwrap().as_slice(),
        &[owner_bytes(&id)],
        "the pad must be told who owns it now"
    );
    assert!(
        !device.sent_configs.lock().unwrap().is_empty(),
        "config resumes"
    );

    let st = daemon_state.lock().unwrap().clone();
    assert!(!st.foreign_pad);
    assert!(st.pending_takeover.is_none());

    // "prompts exactly once per takeover, and never again on that PC": the pad
    // now reports us as its owner, so the next connect is silent.
    controller.state = st;
    controller.on_event(RuntimeEvent::DeviceDisconnected, now);
    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&id)), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(info), now);
    assert!(controller.state.pending_takeover.is_none());
    assert!(!actions.contains(&RuntimeAction::ClaimOwnership));
}

/// An install with no identity cannot take a pad over, and says so rather than
/// half-doing it: the pad would keep naming the other install.
#[tokio::test]
async fn test_takeover_without_an_identity_is_refused_and_stays_pending() {
    let other = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-THEIRS");
    let mut controller = ownership_controller(
        Some(&uuid::Uuid::new_v4().to_string()),
        vec![info.device_id.clone()],
    );
    let now = Instant::now();
    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    controller.on_event(RuntimeEvent::DeviceConnected(info), now);

    // Storage went away after the prompt appeared
    let mut state_without_identity = controller.state.clone();
    state_without_identity.install_id = None;

    let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
    let device = MockDeviceLink::new(true);
    let log_hub = LogHub::new();
    let pending_ops = Arc::new(Mutex::new(PendingOperations::default()));
    let daemon_state = Arc::new(Mutex::new(state_without_identity));

    let resp = handle_ipc_request(
        IpcRequest::ResolveTakeover {
            take_over: true,
            keep_device_counters: true,
        },
        &daemon_state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;
    assert!(
        matches!(resp, IpcResponse::OperationRejected { .. }),
        "{resp:?}"
    );
    assert!(device.claimed_owners.lock().unwrap().is_empty());
    assert!(
        daemon_state.lock().unwrap().pending_takeover.is_some(),
        "an unanswerable takeover must stay pending, not vanish"
    );
}

/// R3 applies directly here: a second pad's connect must never be judged
/// against the first pad's owner. This is the test §W3-3 asks for by name.
#[tokio::test]
async fn test_a_second_pad_is_never_judged_by_the_first_pads_owner() {
    let id = uuid::Uuid::new_v4().to_string();
    let other = uuid::Uuid::new_v4().to_string();
    let mut controller = ownership_controller(
        Some(&id),
        vec!["OSUPAD-ONE".to_string(), "OSUPAD-TWO".to_string()],
    );
    let now = Instant::now();

    // A pad owned by someone else connects and is left alone
    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    controller.on_event(RuntimeEvent::DeviceConnected(pad("OSUPAD-ONE")), now);
    assert!(controller.state.pending_takeover.is_some());
    controller.on_event(RuntimeEvent::DeviceDisconnected, now);

    // A different, unclaimed pad connects. If the first pad's owner were still
    // standing, this one would be wrongly prompted about — and worse, a pad we
    // do own could be wrongly claimed under someone else's id.
    controller.on_event(RuntimeEvent::DeviceOwnership(Vec::new()), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(pad("OSUPAD-TWO")), now);
    assert!(
        controller.state.pending_takeover.is_none(),
        "the previous pad's owner leaked into this connect"
    );
    assert!(actions.contains(&RuntimeAction::ClaimOwnership));
}

/// No install identity (degraded storage, §P2-12): claim nothing, prompt about
/// nothing, and behave exactly as before this feature existed.
#[tokio::test]
async fn test_an_install_with_no_identity_neither_claims_nor_prompts() {
    let other = uuid::Uuid::new_v4().to_string();
    let info = pad("OSUPAD-THEIRS");
    let mut controller = ownership_controller(None, vec![info.device_id.clone()]);
    let now = Instant::now();

    controller.on_event(RuntimeEvent::DeviceOwnership(owner_bytes(&other)), now);
    let actions = controller.on_event(RuntimeEvent::DeviceConnected(info), now);

    assert!(controller.state.pending_takeover.is_none());
    assert!(!controller.state.foreign_pad);
    assert!(!actions.contains(&RuntimeAction::ClaimOwnership));
    assert!(actions.contains(&RuntimeAction::TriggerSync));
}

// ---------------------------------------------------------------------------
// §U-3b: host-driven firmware flash
// ---------------------------------------------------------------------------

/// State, storage, a mock pad, a log hub and the pending-operations queue:
/// everything `handle_ipc_request` takes.
type IpcFixture = (
    Arc<Mutex<DaemonState>>,
    Arc<Mutex<Option<Storage>>>,
    MockDeviceLink,
    LogHub,
    Arc<Mutex<PendingOperations>>,
);

fn firmware_update_fixture(mode: RuntimeMode, connected: bool) -> IpcFixture {
    let dev_info = DeviceInfo {
        device_id: "OSUPAD-FW".to_string(),
        board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
        firmware_version: "1.0.0".to_string(),
        protocol_version: 1,
        running_partition: Some("ota_0".to_string()),
    };
    let counters = CounterState {
        device_id: "OSUPAD-FW".to_string(),
        counter_generation: 3,
        lifetime_key1: 100,
        lifetime_key2: 200,
        map_key1: 0,
        map_key2: 0,
    };
    let storage = Storage::open_in_memory().unwrap();
    storage.save_device_state(&dev_info, &counters).unwrap();

    let state = Arc::new(Mutex::new(DaemonState {
        mode,
        device_connected: connected,
        device_info: Some(dev_info),
        counters: counters.clone(),
        counters_source: CounterSource::Device,
        pc_counters: Some(counters.clone()),
        esp_counters: Some(counters),
        config: DeviceConfig::default(),
        last_sync_time: None,
        last_sync_error: None,
        storage_error: None,
        tosu_connected: false,
        latency: None,
        pending_replacement: None,
        install_id: None,
        pending_takeover: None,
        foreign_pad: false,
        incompatible: None,
        ui_values: Vec::new(),
        custom_layouts: HashMap::new(),
        last_backup: None,
    }));

    (
        state,
        Arc::new(Mutex::new(Some(storage))),
        MockDeviceLink::new(connected),
        LogHub::new(),
        Arc::new(Mutex::new(PendingOperations::default())),
    )
}

#[tokio::test]
async fn test_firmware_offer_reports_the_pad_without_writing_anything() {
    let (state, storage, device, log_hub, pending_ops) =
        firmware_update_fixture(RuntimeMode::Idle, true);

    let r = handle_ipc_request(
        IpcRequest::GetFirmwareUpdate,
        &state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    let IpcResponse::FirmwareUpdateOffer(offer) = r else {
        panic!("expected an offer, got {r:?}");
    };
    assert_eq!(offer.installed.as_deref(), Some("1.0.0"));
    assert_eq!(offer.running_partition.as_deref(), Some("ota_0"));
    // No update worker in this test, so no verified manifest and nothing to
    // offer — but the question is still answerable and still writes nothing.
    assert!(offer.available.is_none());
    assert!(offer.consent_text.is_none());
    assert!(offer.blockers.is_empty(), "{:?}", offer.blockers);
    assert_eq!(device.sent_syncs.lock().unwrap().len(), 0);
}

#[tokio::test]
async fn test_firmware_flash_without_consent_is_refused_before_anything_happens() {
    // §U-3b: "Explicit consent every time. Never automatic, never silent, not
    // even opt-in." confirm: false must not merely fail late — it must refuse.
    let (state, storage, device, log_hub, pending_ops) =
        firmware_update_fixture(RuntimeMode::Idle, true);

    let r = handle_ipc_request(
        IpcRequest::InstallFirmwareUpdate { confirm: false },
        &state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match r {
        IpcResponse::OperationRejected { reason } => {
            assert!(reason.contains("confirmation"), "{reason}");
        }
        other => panic!("a flash without consent must be rejected, got {other:?}"),
    }
    // Nothing was synced, nothing was released, nothing was written
    assert_eq!(device.sent_syncs.lock().unwrap().len(), 0);
    assert!(state.lock().unwrap().device_connected);
}

#[tokio::test]
async fn test_firmware_flash_is_refused_during_gameplay_and_cooldown() {
    for mode in [RuntimeMode::Playing, RuntimeMode::Cooldown] {
        let (state, storage, device, log_hub, pending_ops) = firmware_update_fixture(mode, true);

        let r = handle_ipc_request(
            IpcRequest::InstallFirmwareUpdate { confirm: true },
            &state,
            &storage,
            &device,
            &log_hub,
            &pending_ops,
            None,
        )
        .await;

        match r {
            IpcResponse::OperationRejected { reason } => {
                assert!(reason.contains("busy"), "{mode:?}: {reason}");
            }
            other => panic!("{mode:?} must refuse a flash, got {other:?}"),
        }
        assert_eq!(device.sent_syncs.lock().unwrap().len(), 0);
        assert!(state.lock().unwrap().device_connected);
    }
}

#[tokio::test]
async fn test_firmware_flash_is_refused_with_no_pad_connected() {
    let (state, storage, device, log_hub, pending_ops) =
        firmware_update_fixture(RuntimeMode::Idle, false);

    let r = handle_ipc_request(
        IpcRequest::InstallFirmwareUpdate { confirm: true },
        &state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match r {
        IpcResponse::OperationRejected { reason } => {
            assert!(reason.contains("not connected"), "{reason}");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[tokio::test]
async fn test_firmware_flash_is_refused_without_a_verified_manifest() {
    // Consent, IDLE and a connected pad are not enough: an image is only ever
    // taken from a manifest this daemon verified (§U-0.3).
    let (state, storage, device, log_hub, pending_ops) =
        firmware_update_fixture(RuntimeMode::Idle, true);

    let r = handle_ipc_request(
        IpcRequest::InstallFirmwareUpdate { confirm: true },
        &state,
        &storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match r {
        IpcResponse::Error(e) => assert!(e.contains("manifest"), "{e}"),
        other => panic!("expected a manifest error, got {other:?}"),
    }
    assert_eq!(device.sent_syncs.lock().unwrap().len(), 0);
    assert!(state.lock().unwrap().device_connected);
}

#[tokio::test]
async fn test_firmware_flash_is_refused_when_the_database_is_unavailable() {
    // Without storage the counters cannot be saved, so flashing could lose
    // them for good (§U-3b).
    let (state, _storage, device, log_hub, pending_ops) =
        firmware_update_fixture(RuntimeMode::Idle, true);
    let no_storage: Arc<Mutex<Option<Storage>>> = Arc::new(Mutex::new(None));

    let r = handle_ipc_request(
        IpcRequest::InstallFirmwareUpdate { confirm: true },
        &state,
        &no_storage,
        &device,
        &log_hub,
        &pending_ops,
        None,
    )
    .await;

    match r {
        IpcResponse::OperationRejected { reason } => {
            assert!(reason.contains("database"), "{reason}");
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Automatic JSON counter backups.
//
// The timing is the whole design: a backup is a storage write, so P1-3 forbids
// it during PLAYING and COOLDOWN, and the delay is what keeps it out of both.
// ---------------------------------------------------------------------------

/// Drives one full play session and returns the controller sitting in IDLE
/// with the backup armed, plus the instant the sync completed.
fn play_a_session(start: Instant) -> (RuntimeController, Instant) {
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        start,
    );

    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        start,
    );
    let ended = start + Duration::from_secs(60);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        ended,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Cooldown);

    // Cooldown expires into SYNC
    let synced = ended + COOLDOWN_DURATION;
    let actions = controller.on_event(RuntimeEvent::Tick(synced), synced);
    assert_eq!(controller.state.mode, RuntimeMode::Sync);
    assert!(actions.contains(&RuntimeAction::TriggerSync));
    // Not yet: SYNC is not IDLE
    assert!(!actions.contains(&RuntimeAction::WriteAutoBackup));

    let _ = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: true,
            counters: CounterState::default(),
            time_str: Some("2026-09-17T00:00:00Z".to_string()),
            error: None,
        },
        synced,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Idle);
    (controller, synced)
}

#[tokio::test]
async fn backup_fires_twenty_seconds_after_cooldown_settles_into_idle() {
    let start = Instant::now();
    let (mut controller, synced) = play_a_session(start);

    // 19 s: still waiting
    let early = synced + Duration::from_secs(19);
    let actions = controller.on_event(RuntimeEvent::Tick(early), early);
    assert!(
        !actions.contains(&RuntimeAction::WriteAutoBackup),
        "the delay must not be shortened"
    );

    // 20 s: written
    let due = synced + Duration::from_secs(20);
    let actions = controller.on_event(RuntimeEvent::Tick(due), due);
    assert!(actions.contains(&RuntimeAction::WriteAutoBackup));
    assert_eq!(controller.state.mode, RuntimeMode::Idle);

    // …exactly once. A backup per tick would be a rotation shredder.
    let later = due + Duration::from_secs(60);
    let actions = controller.on_event(RuntimeEvent::Tick(later), later);
    assert!(!actions.contains(&RuntimeAction::WriteAutoBackup));
}

#[tokio::test]
async fn backup_never_fires_during_a_map_or_a_cooldown() {
    let start = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        start,
    );

    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        start,
    );

    // Five minutes of ticks mid-map: not one backup, and no storage write of
    // any kind is enabled.
    for i in 1..=300 {
        let t = start + Duration::from_secs(i);
        let actions = controller.on_event(RuntimeEvent::Tick(t), t);
        assert!(
            !actions.contains(&RuntimeAction::WriteAutoBackup),
            "a backup during PLAYING violates P1-3 (t = {i}s)"
        );
        assert!(!actions.contains(&RuntimeAction::SetStorageWritesAllowed(true)));
    }

    // And across the cooldown, before the sync re-enables writes
    let ended = start + Duration::from_secs(301);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        ended,
    );
    for i in 1..5 {
        let t = ended + Duration::from_secs(i);
        let actions = controller.on_event(RuntimeEvent::Tick(t), t);
        assert_eq!(controller.state.mode, RuntimeMode::Cooldown);
        assert!(
            !actions.contains(&RuntimeAction::WriteAutoBackup),
            "a backup during COOLDOWN violates P1-3 (t = {i}s)"
        );
    }
}

#[tokio::test]
async fn a_map_starting_inside_the_window_cancels_the_pending_backup() {
    let start = Instant::now();
    let (mut controller, synced) = play_a_session(start);

    // 10 s into the 20 s window the next map starts
    let next_map = synced + Duration::from_secs(10);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 500.0,
            title: "The Next Map".to_string(),
            values: Vec::new(),
        },
        next_map,
    );
    assert_eq!(controller.state.mode, RuntimeMode::Playing);
    assert_eq!(controller.backup_deadline, None, "cancelled, not deferred");

    // Past the original deadline, mid-map: nothing is written
    for i in 11..40 {
        let t = synced + Duration::from_secs(i);
        let actions = controller.on_event(RuntimeEvent::Tick(t), t);
        assert!(!actions.contains(&RuntimeAction::WriteAutoBackup));
    }

    // The new session gets its own backup when *it* settles
    let ended = next_map + Duration::from_secs(60);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "The Next Map".to_string(),
            values: Vec::new(),
        },
        ended,
    );
    let synced2 = ended + COOLDOWN_DURATION;
    let _ = controller.on_event(RuntimeEvent::Tick(synced2), synced2);
    let _ = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: true,
            counters: CounterState::default(),
            time_str: None,
            error: None,
        },
        synced2,
    );
    let due = synced2 + Duration::from_secs(20);
    let actions = controller.on_event(RuntimeEvent::Tick(due), due);
    assert!(actions.contains(&RuntimeAction::WriteAutoBackup));
}

#[tokio::test]
async fn the_periodic_idle_sync_does_not_arm_a_backup() {
    // Only a play session's sync arms one. Otherwise an idle machine would
    // rotate the whole directory away every 50 minutes and lose the real ones.
    let start = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        start,
    );
    let _ = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: true,
            counters: CounterState::default(),
            time_str: None,
            error: None,
        },
        start,
    );
    assert_eq!(controller.backup_deadline, None);

    let due = start + Duration::from_secs(60);
    let actions = controller.on_event(RuntimeEvent::Tick(due), due);
    assert!(!actions.contains(&RuntimeAction::WriteAutoBackup));
}

#[tokio::test]
async fn a_failed_post_play_sync_still_gets_a_backup() {
    // The sync failing is precisely when the counters in memory are the only
    // record, so this is the case the feature exists for.
    let start = Instant::now();
    let mut controller = RuntimeController::new(
        DeviceConfig::default(),
        None,
        CounterState::default(),
        HashMap::new(),
        None,
        Vec::new(),
        start,
    );
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: true,
            live_time_ms: 1000.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        start,
    );
    let ended = start + Duration::from_secs(60);
    let _ = controller.on_event(
        RuntimeEvent::TosuTelemetry {
            is_playing: false,
            live_time_ms: 0.0,
            title: "A Map".to_string(),
            values: Vec::new(),
        },
        ended,
    );
    let synced = ended + COOLDOWN_DURATION;
    let _ = controller.on_event(RuntimeEvent::Tick(synced), synced);
    let _ = controller.on_event(
        RuntimeEvent::SyncCompleted {
            success: false,
            counters: CounterState::default(),
            time_str: None,
            error: Some("the pad went away".to_string()),
        },
        synced,
    );
    let due = synced + Duration::from_secs(20);
    let actions = controller.on_event(RuntimeEvent::Tick(due), due);
    assert!(actions.contains(&RuntimeAction::WriteAutoBackup));
}
