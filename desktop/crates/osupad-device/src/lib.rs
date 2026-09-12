use bytes::BytesMut;
use chrono::{Datelike, Local, Timelike};
use osupad_model::{CounterState, DeviceConfig, DeviceInfo, GameplayTelemetry};
use osupad_protocol::proto::{self, host_to_device, DeviceToHost, HostToDevice};
use osupad_protocol::{decode_device_message, encode_host_message};
use serialport::SerialPortType;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

pub const ESPRESSIF_VID: u16 = 0x303A;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Serial port error: {0}")]
    Serial(#[from] serialport::Error),
    #[error("Protocol error: {0}")]
    Protocol(#[from] osupad_protocol::ProtocolError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Device not connected")]
    NotConnected,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected(DeviceInfo),
    Disconnected,
    StatusUpdate(proto::DeviceStatus),
    Counters(CounterState),
    LogBatch(proto::LogEventBatch),
}

pub struct DeviceManager {
    cmd_tx: mpsc::Sender<HostToDevice>,
    event_tx: broadcast::Sender<DeviceEvent>,
    is_connected: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    seq_counter: Arc<AtomicU32>,
}

impl DeviceManager {
    pub fn new() -> (Self, broadcast::Receiver<DeviceEvent>) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<HostToDevice>(64);
        let (event_tx, event_rx) = broadcast::channel(64);
        let is_connected = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let seq_counter = Arc::new(AtomicU32::new(1));

        let is_conn_clone = is_connected.clone();
        let is_paused_clone = is_paused.clone();
        let event_tx_clone = event_tx.clone();

        // Spawn background worker managing the serial port lifecycle
        tokio::task::spawn_blocking(move || {
            let mut read_buf = BytesMut::with_capacity(8192);

            loop {
                if is_paused_clone.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }

                // Discover target serial port
                let port_path = match find_target_port() {
                    Some(p) => p,
                    None => {
                        debug!("Searching for osu!pad ESP32-S3 USB port...");
                        std::thread::sleep(Duration::from_millis(1500));
                        continue;
                    }
                };

                info!("Opening osu!pad serial port at {}", port_path);
                let port_builder = serialport::new(&port_path, 115200)
                    .timeout(Duration::from_millis(100));

                let mut port = match port_builder.open() {
                    Ok(p) => p,
                    Err(e) => {
                        debug!("Failed to open port {}: {}", port_path, e);
                        std::thread::sleep(Duration::from_millis(1500));
                        continue;
                    }
                };

                is_conn_clone.store(true, Ordering::SeqCst);
                info!("Connected to osu!pad on {}", port_path);

                // Send initial Hello
                let hello_msg = HostToDevice {
                    sequence_number: 1,
                    payload: Some(host_to_device::Payload::Hello(proto::Hello {
                        protocol_version: 1,
                        client_version: "1.0.0".to_string(),
                    })),
                };
                if let Ok(encoded) = encode_host_message(&hello_msg) {
                    let _ = port.write_all(&encoded);
                }

                let mut raw_buf = [0u8; 1024];

                // Inner communication loop
                loop {
                    if is_paused_clone.load(Ordering::SeqCst) {
                        info!("Pause requested; releasing serial port on {}", port_path);
                        break;
                    }

                    // 1. Drain incoming command queue to transmit to device
                    while let Ok(cmd) = cmd_rx.try_recv() {
                        if let Ok(encoded) = encode_host_message(&cmd) {
                            if let Err(e) = port.write_all(&encoded) {
                                warn!("Failed to write to device: {}", e);
                                break;
                            }
                        }
                    }

                    // 2. Read incoming serial bytes
                    match port.read(&mut raw_buf) {
                        Ok(n) if n > 0 => {
                            read_buf.extend_from_slice(&raw_buf[..n]);

                            // Parse all ready frames
                            while let Ok(Some(msg)) = decode_device_message(&mut read_buf) {
                                handle_device_message(&msg, &event_tx_clone);
                            }
                        }
                        Ok(_) => {}
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            info!("Device disconnected from {}: {}", port_path, e);
                            break;
                        }
                    }

                    std::thread::sleep(Duration::from_millis(5));
                }

                is_conn_clone.store(false, Ordering::SeqCst);
                let _ = event_tx_clone.send(DeviceEvent::Disconnected);
                read_buf.clear();
                std::thread::sleep(Duration::from_millis(1500));
            }
        });

        (
            Self {
                cmd_tx,
                event_tx,
                is_connected,
                is_paused,
                seq_counter,
            },
            event_rx,
        )
    }

    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::SeqCst);
        self.is_connected.store(false, Ordering::SeqCst);
        let _ = self.event_tx.send(DeviceEvent::Disconnected);
    }

    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::SeqCst);
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::SeqCst)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
        self.event_tx.subscribe()
    }

    fn next_seq(&self) -> u32 {
        self.seq_counter.fetch_add(1, Ordering::SeqCst)
    }

    pub async fn send_time_sync(&self) -> Result<(), DeviceError> {
        let now = Local::now();
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::TimeSync(proto::TimeSync {
                year: now.year() as u32,
                month: now.month(),
                day: now.day(),
                hour: now.hour(),
                minute: now.minute(),
                second: now.second(),
            })),
        };
        self.cmd_tx.send(msg).await.map_err(|_| DeviceError::NotConnected)?;
        Ok(())
    }

    pub async fn send_config(&self, config: &DeviceConfig) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::SetConfig(proto::SetConfig {
                config: Some(proto::ConfigPayload {
                    key1_hid_usage: config.key1_hid_usage,
                    key2_hid_usage: config.key2_hid_usage,
                    debounce_us: config.debounce_us,
                    brightness: config.brightness,
                    display_sleep_seconds: config.display_sleep_seconds,
                    gameplay_display_hz: config.gameplay_display_hz,
                }),
            })),
        };
        self.cmd_tx.send(msg).await.map_err(|_| DeviceError::NotConnected)?;
        Ok(())
    }

    pub async fn send_gameplay_state(
        &self,
        telemetry: &GameplayTelemetry,
        k1_count: u32,
        k2_count: u32,
    ) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::GameplayState(
                proto::GameplayDisplayState {
                    title: telemetry.title.clone(),
                    artist: telemetry.artist.clone(),
                    current_pp: telemetry.current_pp,
                    progress_ratio: telemetry.progress_ratio,
                    current_map_presses_k1: k1_count,
                    current_map_presses_k2: k2_count,
                },
            )),
        };
        self.cmd_tx.send(msg).await.map_err(|_| DeviceError::NotConnected)?;
        Ok(())
    }

    pub async fn send_counter_sync(
        &self,
        counters: &CounterState,
        force_restore: bool,
    ) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::CounterSync(
                proto::CounterSyncRequest {
                    target_state: Some(proto::CounterState {
                        device_id: counters.device_id.clone(),
                        counter_generation: counters.counter_generation,
                        lifetime_key1: counters.lifetime_key1,
                        lifetime_key2: counters.lifetime_key2,
                    }),
                    force_restore,
                },
            )),
        };
        self.cmd_tx.send(msg).await.map_err(|_| DeviceError::NotConnected)?;
        Ok(())
    }
}

fn handle_device_message(msg: &DeviceToHost, tx: &broadcast::Sender<DeviceEvent>) {
    if let Some(payload) = &msg.payload {
        match payload {
            proto::device_to_host::Payload::HelloAck(ack) => {
                let info = DeviceInfo {
                    device_id: ack.device_id.clone(),
                    board_profile: ack.board_profile.clone(),
                    firmware_version: ack.firmware_version.clone(),
                    protocol_version: ack.protocol_version,
                };
                let _ = tx.send(DeviceEvent::Connected(info));
                let _ = tx.send(DeviceEvent::Counters(CounterState {
                    device_id: ack.device_id.clone(),
                    counter_generation: ack.counter_generation,
                    lifetime_key1: ack.lifetime_key1,
                    lifetime_key2: ack.lifetime_key2,
                    map_key1: 0,
                    map_key2: 0,
                }));
            }
            proto::device_to_host::Payload::Status(st) => {
                let _ = tx.send(DeviceEvent::StatusUpdate(st.clone()));
                let _ = tx.send(DeviceEvent::Counters(CounterState {
                    device_id: "".to_string(),
                    counter_generation: 0,
                    lifetime_key1: st.lifetime_key1,
                    lifetime_key2: st.lifetime_key2,
                    map_key1: st.map_key1,
                    map_key2: st.map_key2,
                }));
            }
            proto::device_to_host::Payload::CounterSyncResp(resp) => {
                if let Some(st) = &resp.synchronized_state {
                    let _ = tx.send(DeviceEvent::Counters(CounterState {
                        device_id: st.device_id.clone(),
                        counter_generation: st.counter_generation,
                        lifetime_key1: st.lifetime_key1,
                        lifetime_key2: st.lifetime_key2,
                        map_key1: 0,
                        map_key2: 0,
                    }));
                }
            }
            proto::device_to_host::Payload::LogBatch(batch) => {
                let _ = tx.send(DeviceEvent::LogBatch(batch.clone()));
            }
            _ => {}
        }
    }
}

/// Finds the most likely serial port path for the ESP32-S3
pub fn find_target_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for p in ports {
        if let SerialPortType::UsbPort(info) = p.port_type {
            if info.vid == ESPRESSIF_VID {
                return Some(p.port_name);
            }
        }
    }
    // Fallback: search for /dev/ttyACM0
    if std::path::Path::new("/dev/ttyACM0").exists() {
        return Some("/dev/ttyACM0".to_string());
    }
    None
}
