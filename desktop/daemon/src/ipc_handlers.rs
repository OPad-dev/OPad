use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{info, warn};

use osupad_device::DeviceEvent;
use osupad_ipc::{CurrentBackupState, IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_layout::Screen;
use osupad_model::{
    char_to_hid_usage, CounterState, DeviceConfig, DeviceInfo, JsonBackup, RuntimeMode,
};
use osupad_storage::Storage;

use crate::log_hub::LogHub;
use crate::runtime::{DaemonState, PendingOperations};
use crate::sync::{perform_sync, DeviceLink};

pub async fn handle_ipc_request<D: DeviceLink>(
    req: IpcRequest,
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    device: &D,
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
                s_guard
                    .as_ref()
                    .and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
                    .or_else(|| st.pc_counters.clone())
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
                return IpcResponse::OperationRejected {
                    reason: format!("Invalid layout: {}", e),
                };
            }
            state
                .lock()
                .unwrap()
                .custom_layouts
                .insert(screen, layout.clone());
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                let _ = device.send_layout(screen, &layout).await;
                pending_ops
                    .lock()
                    .unwrap()
                    .pending_layouts
                    .push((screen, Some(layout)));
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
                pending_ops
                    .lock()
                    .unwrap()
                    .pending_layouts
                    .push((screen, None));
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
                return IpcResponse::LayoutApplied {
                    screen,
                    message: "Reset; the pad will use its default".to_string(),
                };
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
                let keys_or_debounce_changed = current_cfg.key1_hid_usage
                    != new_config.key1_hid_usage
                    || current_cfg.key2_hid_usage != new_config.key2_hid_usage
                    || current_cfg.debounce_us != new_config.debounce_us;

                if keys_or_debounce_changed {
                    pending_ops.lock().unwrap().pending_config = Some(new_config);
                    return IpcResponse::OperationDeferred {
                        reason: "Key mapping and debounce changes are deferred until IDLE mode"
                            .to_string(),
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
                    reason:
                        "Database unavailable: counter sync is rejected without persistent storage"
                            .to_string(),
                };
            }
            let _ = perform_sync(state, storage, device, pending_ops).await;
            let counters = { state.lock().unwrap().counters.clone() };
            IpcResponse::SyncCompleted {
                success: true,
                counters,
            }
        }

        IpcRequest::ResetCounters { confirm } => {
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "ResetCounters requires explicit confirmation (--yes or modal confirm)"
                        .to_string(),
                };
            }

            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot reset counters during gameplay or cooldown".to_string(),
                };
            }

            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason:
                        "Database unavailable: counter reset is rejected without persistent storage"
                            .to_string(),
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
                    reason: "Cannot restore counters during active gameplay or cooldown"
                        .to_string(),
                };
            }
            if !confirm {
                return IpcResponse::OperationRejected {
                    reason: "Confirmation required to force-restore ESP counters from PC"
                        .to_string(),
                };
            }
            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason: "Database unavailable: restore from PC is rejected without persistent storage"
                        .to_string(),
                };
            }
            let (info_opt, in_memory_counters, is_connected) = {
                let st = state.lock().unwrap();
                (
                    st.device_info.clone(),
                    st.counters.clone(),
                    st.device_connected,
                )
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
                s_guard
                    .as_ref()
                    .and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
            };
            let Some(pc_counters) = pc_state else {
                return IpcResponse::OperationRejected {
                    reason: "No PC counter state found in database for this device".to_string(),
                };
            };

            let new_gen = std::cmp::max(
                pc_counters.counter_generation,
                in_memory_counters.counter_generation,
            )
            .saturating_add(1);
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
            info!(
                "Force-restored ESP counters from PC (generation: {})",
                new_gen
            );
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
                    reason: "Database unavailable: import from device is rejected without persistent storage"
                        .to_string(),
                };
            }
            let (info_opt, in_memory_counters, is_connected) = {
                let st = state.lock().unwrap();
                (
                    st.device_info.clone(),
                    st.counters.clone(),
                    st.device_connected,
                )
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
                s_guard
                    .as_ref()
                    .and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
            };
            let pc_gen = pc_state.as_ref().map(|c| c.counter_generation).unwrap_or(0);

            let new_gen =
                std::cmp::max(pc_gen, in_memory_counters.counter_generation).saturating_add(1);
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
            info!(
                "Overwrote PC database counters from ESP (generation: {})",
                new_gen
            );
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
                    reason: "Database unavailable: resolving replacement is rejected without persistent storage"
                        .to_string(),
                };
            }

            let (old_device_id_opt, current_info_opt, current_counters) = {
                let mut st = state.lock().unwrap();
                (
                    st.pending_replacement.take(),
                    st.device_info.clone(),
                    st.counters.clone(),
                )
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
                        s_guard
                            .as_ref()
                            .and_then(|s| s.load_device_state(&old_id).unwrap_or(None))
                    };
                    if let Some(old_c) = old_state {
                        let new_gen = std::cmp::max(
                            old_c.counter_generation,
                            current_counters.counter_generation,
                        )
                        .saturating_add(1);
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
                IpcResponse::CountersReset {
                    counters: current_counters,
                }
            }
        }

        IpcRequest::ExportBackup => {
            let st = state.lock().unwrap();
            if !st.device_connected && st.device_info.is_none() && st.counters.device_id.is_empty()
            {
                return IpcResponse::OperationRejected {
                    reason: "No counters known yet: connect the pad once".to_string(),
                };
            }
            let info = st
                .device_info
                .clone()
                .or_else(|| {
                    storage.lock().unwrap().as_ref().and_then(|s| {
                        s.list_device_states()
                            .ok()?
                            .into_iter()
                            .find(|(i, _)| i.device_id == st.counters.device_id)
                            .map(|(i, _)| i)
                    })
                })
                .unwrap_or_else(|| DeviceInfo {
                    device_id: st.counters.device_id.clone(),
                    board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                    firmware_version: String::new(),
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

            let device_id_matches = st.counters.device_id.is_empty()
                || st.counters.device_id == backup.device.device_id;
            let is_counter_rollback = backup.stats.lifetime_key1 < st.counters.lifetime_key1
                || backup.stats.lifetime_key2 < st.counters.lifetime_key2;

            let mut warnings = Vec::new();
            if !device_id_matches {
                warnings.push(format!(
                    "Device ID mismatch: backup is for '{}', current pad is '{}'",
                    backup.device.device_id, st.counters.device_id
                ));
            }
            if is_counter_rollback {
                warnings.push(
                    "Incoming lifetime counters are lower than current counters (counter rollback)"
                        .to_string(),
                );
            }
            if backup.device.counter_generation <= st.counters.counter_generation {
                warnings.push(
                    "Incoming counter generation is not greater than current generation; generation will be bumped"
                        .to_string(),
                );
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
                    reason:
                        "ImportBackup requires explicit confirmation (--yes or user confirmation)"
                            .to_string(),
                };
            }

            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot import backup during gameplay or cooldown".to_string(),
                };
            }

            if storage.lock().unwrap().is_none() {
                return IpcResponse::OperationRejected {
                    reason:
                        "Database unavailable: backup import is rejected without persistent storage"
                            .to_string(),
                };
            }

            if let Err(err) = backup.validate() {
                return IpcResponse::Error(format!("Invalid backup: {}", err));
            }

            let (current_tosu, current_gen) = {
                let st = state.lock().unwrap();
                (
                    st.config.tosu_endpoint.clone(),
                    st.counters.counter_generation,
                )
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
            };

            let new_counters = CounterState {
                device_id: backup.device.device_id.clone(),
                counter_generation: std::cmp::max(current_gen, backup.device.counter_generation)
                    .saturating_add(1),
                lifetime_key1: backup.stats.lifetime_key1,
                lifetime_key2: backup.stats.lifetime_key2,
                map_key1: 0,
                map_key2: 0,
            };

            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_config(&new_config);
                let last_firmware_version = state
                    .lock()
                    .unwrap()
                    .device_info
                    .as_ref()
                    .map(|i| i.firmware_version.clone())
                    .or_else(|| {
                        s.list_device_states()
                            .ok()?
                            .into_iter()
                            .find(|(i, _)| i.device_id == backup.device.device_id)
                            .map(|(i, _)| i.firmware_version)
                    })
                    .unwrap_or_default();
                let info = DeviceInfo {
                    device_id: backup.device.device_id,
                    board_profile: backup.device.board_profile,
                    firmware_version: last_firmware_version,
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
            IpcResponse::LogEntries {
                entries,
                latest_seq,
            }
        }

        IpcRequest::PrepareFlash => {
            if mode == RuntimeMode::Playing || mode == RuntimeMode::Cooldown {
                return IpcResponse::OperationRejected {
                    reason: "Cannot flash firmware during gameplay or cooldown".to_string(),
                };
            }
            info!("Releasing serial port for firmware flash...");
            if !device.pause_and_release(Duration::from_secs(2)).await {
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
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            continue
                        }
                        Err(_) => return Err("Event stream closed".to_string()),
                    }
                }
            })
            .await;

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
                Err(_) => IpcResponse::Error(
                    "Device did not reconnect within 15 seconds after flash".to_string(),
                ),
            }
        }
    }
}

pub async fn wait_for_layout_ack(
    events: &mut tokio::sync::broadcast::Receiver<DeviceEvent>,
    screen: Screen,
) -> IpcResponse {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DeviceEvent::LayoutAck {
                    screen: s,
                    success,
                    message,
                }) if s == screen.to_wire() => {
                    return if success {
                        IpcResponse::LayoutApplied { screen, message }
                    } else {
                        IpcResponse::OperationRejected {
                            reason: format!("The pad rejected the layout: {}", message),
                        }
                    };
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return IpcResponse::Error("Device event stream closed".to_string()),
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(3), wait)
        .await
        .unwrap_or_else(|_| {
            IpcResponse::Error("The pad did not confirm the layout within 3s".to_string())
        })
}
