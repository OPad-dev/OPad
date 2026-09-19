use chrono::Utc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use opad_device::{DeviceError, DeviceEvent, DeviceManager};
use opad_layout::{Layout, Screen};
use opad_model::ui_source::SourceValue;
use opad_model::{CounterState, DeviceConfig};
use opad_storage::{reconcile_counters, Storage};

use crate::runtime::{DaemonState, PendingOperations};

#[allow(async_fn_in_trait)]
pub trait DeviceLink: Send + Sync {
    async fn send_time_sync(&self) -> Result<(), DeviceError>;
    async fn send_config(&self, config: &DeviceConfig) -> Result<(), DeviceError>;
    async fn send_layout(&self, screen: Screen, layout: &Layout) -> Result<(), DeviceError>;
    async fn reset_layout(&self, screen: Screen) -> Result<(), DeviceError>;
    async fn send_host_status(
        &self,
        tosu_connected: bool,
        is_playing: bool,
        play_id: u32,
    ) -> Result<(), DeviceError>;
    async fn send_data_update(&self, values: &[(u8, SourceValue)]) -> Result<(), DeviceError>;
    async fn send_counter_sync(
        &self,
        counters: &CounterState,
        force_restore: bool,
    ) -> Result<u32, DeviceError>;
    /// Records this install as the pad's owner (§W3-2)
    async fn claim_ownership(&self, owner_id: &[u8]) -> Result<(), DeviceError>;
    async fn request_status(&self) -> Result<(), DeviceError>;
    async fn request_logs(&self) -> Result<(), DeviceError>;
    async fn reset_latency_stats(&self) -> Result<(), DeviceError>;
    async fn send_detect_pin(
        &self,
        key_id: u32,
        timeout_ms: u32,
        exclude_gpio: u32,
    ) -> Result<(), DeviceError>;
    fn subscribe(&self) -> broadcast::Receiver<DeviceEvent>;
    async fn pause_and_release(&self, timeout: Duration) -> bool;
    fn resume(&self);
    fn is_connected(&self) -> bool;
}

impl DeviceLink for DeviceManager {
    async fn send_time_sync(&self) -> Result<(), DeviceError> {
        self.send_time_sync().await
    }

    async fn claim_ownership(&self, owner_id: &[u8]) -> Result<(), DeviceError> {
        self.claim_ownership(owner_id).await
    }

    async fn send_config(&self, config: &DeviceConfig) -> Result<(), DeviceError> {
        self.send_config(config).await
    }

    async fn send_layout(&self, screen: Screen, layout: &Layout) -> Result<(), DeviceError> {
        self.send_layout(screen, layout).await
    }

    async fn reset_layout(&self, screen: Screen) -> Result<(), DeviceError> {
        self.reset_layout(screen).await
    }

    async fn send_host_status(
        &self,
        tosu_connected: bool,
        is_playing: bool,
        play_id: u32,
    ) -> Result<(), DeviceError> {
        self.send_host_status(tosu_connected, is_playing, play_id)
            .await
    }

    async fn send_data_update(&self, values: &[(u8, SourceValue)]) -> Result<(), DeviceError> {
        self.send_data_update(values).await
    }

    async fn send_counter_sync(
        &self,
        counters: &CounterState,
        force_restore: bool,
    ) -> Result<u32, DeviceError> {
        self.send_counter_sync(counters, force_restore).await
    }

    async fn request_status(&self) -> Result<(), DeviceError> {
        self.request_status().await
    }

    async fn request_logs(&self) -> Result<(), DeviceError> {
        self.request_logs().await
    }

    async fn reset_latency_stats(&self) -> Result<(), DeviceError> {
        self.reset_latency_stats().await
    }

    async fn send_detect_pin(
        &self,
        key_id: u32,
        timeout_ms: u32,
        exclude_gpio: u32,
    ) -> Result<(), DeviceError> {
        self.send_detect_pin(key_id, timeout_ms, exclude_gpio).await
    }

    fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
        self.subscribe()
    }

    async fn pause_and_release(&self, timeout: Duration) -> bool {
        self.pause_and_release(timeout).await
    }

    fn resume(&self) {
        self.resume()
    }

    fn is_connected(&self) -> bool {
        self.is_connected()
    }
}

pub async fn perform_sync<D: DeviceLink>(
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    device: &D,
    pending_ops: &Arc<Mutex<PendingOperations>>,
) -> Result<CounterState, String> {
    // The runtime (post-cooldown, connect, periodic) and ForceSync can overlap; run one at a time
    static SYNC_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    let _sync_guard = SYNC_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    // Mode and the storage write guard belong to the RuntimeController: a map can start
    // while this runs, and storage writes must then stay blocked
    info!("Performing atomic state synchronization (§11.3, §13)...");

    let is_storage_available = storage.lock().unwrap().is_some();
    if !is_storage_available {
        let err_msg = "Sync skipped: persistent storage unavailable".to_string();
        warn!("perform_sync skipped: SQLite storage unavailable (P2-12). ESP counters will not be modified.");
        let mut st = state.lock().unwrap();
        st.last_sync_error = Some(err_msg.clone());
        return Err(err_msg);
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
        return Ok(in_memory_counters);
    };

    if !is_connected {
        return Ok(in_memory_counters);
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
                        return s.state == opad_device::proto::DeviceState::Idle as i32;
                    }
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return false,
                }
            }
        })
        .await;

        if let Ok(is_idle) = wait_res {
            if is_idle {
                idle_reached = true;
                break;
            }
        }
    }

    if !idle_reached {
        let err_msg = "Device not in IDLE state within 3s timeout".to_string();
        warn!("Timed out waiting for device to report IDLE state before counter sync");
        let mut st = state.lock().unwrap();
        st.last_sync_error = Some(err_msg.clone());
        return Err(err_msg);
    }

    let stored_counters = {
        let s_guard = storage.lock().unwrap();
        s_guard
            .as_ref()
            .and_then(|s| s.load_device_state(&info.device_id).unwrap_or(None))
            .unwrap_or_else(|| CounterState {
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

    // Save reconciled state to SQLite (blocked by the write guard if a map started)
    {
        if let Some(s) = storage.lock().unwrap().as_ref() {
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
                            Ok(DeviceEvent::CounterSyncResult {
                                seq: r_seq,
                                success,
                                message,
                                state: _,
                            }) if r_seq == seq => {
                                return Ok((success, message));
                            }
                            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                continue
                            }
                            Err(_) => return Err("Event channel closed".to_string()),
                        }
                    }
                })
                .await;

                match wait_resp {
                    Ok(Ok((true, _))) => {
                        sync_success = true;
                        break;
                    }
                    Ok(Ok((false, msg))) => {
                        last_error_msg = msg;
                        warn!(
                            "Device rejected CounterSync (attempt {}): {}",
                            attempt + 1,
                            last_error_msg
                        );
                    }
                    Ok(Err(e)) => {
                        last_error_msg = e;
                        warn!(
                            "Error waiting for CounterSync response (attempt {}): {}",
                            attempt + 1,
                            last_error_msg
                        );
                    }
                    Err(_) => {
                        last_error_msg = "Timeout waiting for CounterSyncResponse (2s)".to_string();
                        warn!(
                            "Timeout waiting for CounterSync response (attempt {})",
                            attempt + 1
                        );
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
        error!(
            "State synchronization failed after 3 attempts: {}",
            last_error_msg
        );
        let mut st = state.lock().unwrap();
        st.last_sync_error = Some(format!("Counter sync failed: {}", last_error_msg));
        st.counters = reconciled; // Keep SQLite as reconciled (§P1-1)
        return Err(last_error_msg);
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
        (
            p.pending_config.take(),
            std::mem::take(&mut p.pending_layouts),
            p.pending_last_seen.take(),
        )
    };

    if let Some(cfg) = pending_cfg {
        {
            if let Some(s) = storage.lock().unwrap().as_ref() {
                let _ = s.save_config(&cfg);
            }
        }
        let _ = device.send_config(&cfg).await;
        // Otherwise the next connect would push the old config back to the pad
        state.lock().unwrap().config = cfg;
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
        st.esp_counters = Some(reconciled.clone());
        st.last_sync_time = Some(now_str);
        st.last_sync_error = None;
    }

    info!("Synchronization completed successfully");
    Ok(reconciled)
}
