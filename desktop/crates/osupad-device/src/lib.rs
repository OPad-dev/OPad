use bytes::BytesMut;
use chrono::{Datelike, Local, Timelike};
use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{CounterState, DeviceConfig, DeviceInfo};
pub use osupad_protocol::proto;
use osupad_protocol::proto::{host_to_device, DeviceToHost, HostToDevice};
use osupad_protocol::{decode_device_message, encode_host_message};
use serialport::SerialPortType;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

pub const ESPRESSIF_VID: u16 = 0x303A;
/// USB PID of the osu!pad application firmware (TinyUSB composite device)
pub const OSUPAD_APP_PID: u16 = 0x4001;
/// USB PID of the ESP32-S3 ROM download bootloader (USB-Serial-JTAG)
pub const ESP_ROM_BOOTLOADER_PID: u16 = 0x1001;

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
    #[error("Device command queue busy")]
    Busy,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    Connected(DeviceInfo),
    Disconnected,
    StatusUpdate(proto::DeviceStatus),
    Counters(CounterState),
    CounterSyncResult {
        seq: u32,
        success: bool,
        message: String,
        state: Option<CounterState>,
    },
    ConfigAck {
        seq: u32,
        success: bool,
        message: String,
        current_config: Option<DeviceConfig>,
    },
    LogBatch(proto::LogEventBatch),
    LayoutAck {
        screen: u8,
        success: bool,
        message: String,
    },
}

pub struct DeviceManager {
    cmd_tx: mpsc::Sender<HostToDevice>,
    event_tx: broadcast::Sender<DeviceEvent>,
    is_connected: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    is_port_open: Arc<AtomicBool>,
    seq_counter: Arc<AtomicU32>,
}

impl DeviceManager {
    pub fn new_dummy() -> (Self, broadcast::Receiver<DeviceEvent>) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<HostToDevice>(64);
        let (event_tx, event_rx) = broadcast::channel(64);
        let is_connected = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let is_port_open = Arc::new(AtomicBool::new(false));
        let seq_counter = Arc::new(AtomicU32::new(1));
        tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });
        (
            Self {
                cmd_tx,
                event_tx,
                is_connected,
                is_paused,
                is_port_open,
                seq_counter,
            },
            event_rx,
        )
    }

    pub fn new() -> (Self, broadcast::Receiver<DeviceEvent>) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<HostToDevice>(64);
        let (event_tx, event_rx) = broadcast::channel(64);
        let is_connected = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let seq_counter = Arc::new(AtomicU32::new(1));

        let is_conn_clone = is_connected.clone();
        let is_paused_clone = is_paused.clone();
        let is_port_open = Arc::new(AtomicBool::new(false));
        let is_open_clone = is_port_open.clone();
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
                let port_builder =
                    serialport::new(&port_path, 115200).timeout(Duration::from_millis(100));

                let mut port = match port_builder.open() {
                    Ok(mut p) => {
                        let _ = p.write_data_terminal_ready(true);
                        let _ = p.write_request_to_send(true);
                        p
                    }
                    Err(e) => {
                        debug!("Failed to open port {}: {}", port_path, e);
                        std::thread::sleep(Duration::from_millis(1500));
                        continue;
                    }
                };

                is_open_clone.store(true, Ordering::SeqCst);
                is_conn_clone.store(true, Ordering::SeqCst);
                info!("Connected to osu!pad on {}", port_path);

                let mut raw_buf = [0u8; 1024];
                let mut last_hello = Instant::now() - Duration::from_secs(10);
                let mut has_hello_ack = false;

                // Inner communication loop
                loop {
                    if is_paused_clone.load(Ordering::SeqCst) {
                        info!("Pause requested; releasing serial port on {}", port_path);
                        break;
                    }

                    // Periodic Hello retry until HelloAck is received (§6.1)
                    if !has_hello_ack && last_hello.elapsed() >= Duration::from_millis(800) {
                        last_hello = Instant::now();
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
                                if let Some(proto::device_to_host::Payload::HelloAck(_)) =
                                    &msg.payload
                                {
                                    has_hello_ack = true;
                                }
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

                drop(port);
                is_open_clone.store(false, Ordering::SeqCst);
                is_conn_clone.store(false, Ordering::SeqCst);
                // Drain any pending commands on disconnect so they are never replayed to a new connection
                while cmd_rx.try_recv().is_ok() {}
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
                is_port_open,
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

    /// Pause and wait until the worker has actually closed the serial port,
    /// so a flasher can open it without hitting EBUSY. Returns false on timeout.
    pub async fn pause_and_release(&self, timeout: Duration) -> bool {
        self.pause();
        let deadline = Instant::now() + timeout;
        while self.is_port_open.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        true
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

    fn send_msg(&self, msg: HostToDevice) -> Result<(), DeviceError> {
        if !self.is_connected.load(Ordering::SeqCst) {
            return Err(DeviceError::NotConnected);
        }
        self.cmd_tx.try_send(msg).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => DeviceError::Busy,
            mpsc::error::TrySendError::Closed(_) => DeviceError::NotConnected,
        })
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
        self.send_msg(msg)
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
                    #[allow(deprecated)]
                    press_color_rgb: 0,
                }),
            })),
        };
        self.send_msg(msg)
    }

    /// Send changed UI data source values, split to fit the device's 32-value messages
    pub async fn send_data_update(&self, values: &[(u8, SourceValue)]) -> Result<(), DeviceError> {
        const MAX_VALUES: usize = 32;
        const TEXT_MAX: usize = 64; // nanopb max_size, including NUL
        for chunk in values.chunks(MAX_VALUES) {
            let msg = HostToDevice {
                sequence_number: self.next_seq(),
                payload: Some(host_to_device::Payload::DataUpdate(proto::DataUpdate {
                    values: chunk
                        .iter()
                        .map(|(source, value)| proto::DataValue {
                            source: *source as u32,
                            value: Some(match value {
                                SourceValue::Number(n) => proto::data_value::Value::Number(*n),
                                SourceValue::Text(t) => {
                                    proto::data_value::Value::Text(fit_nanopb_string(t, TEXT_MAX))
                                }
                                SourceValue::Clear => proto::data_value::Value::Clear(true),
                            }),
                        })
                        .collect(),
                })),
            };
            self.send_msg(msg)?;
        }
        Ok(())
    }

    pub async fn reset_latency_stats(&self) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::ResetLatencyStats(true)),
        };
        self.send_msg(msg)
    }

    /// Apply a screen layout on the device (it also stores it, unless playing)
    pub async fn send_layout(&self, screen: Screen, layout: &Layout) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::SetLayout(proto::SetLayout {
                screen: screen.to_wire() as u32,
                background: layout.background,
                widgets: layout
                    .widgets
                    .iter()
                    .map(|w| proto::UiWidget {
                        kind: w.kind.to_wire() as u32,
                        source: w.source as u32,
                        font: w.font.to_wire() as u32,
                        align: w.align.to_wire() as u32,
                        x: w.x as i32,
                        y: w.y as i32,
                        w: w.w.max(0) as u32,
                        h: w.h.max(0) as u32,
                        fg: w.fg,
                        bg: w.bg,
                        accent: w.accent,
                        radius: w.radius as u32,
                        decimals: w.decimals as u32,
                        flags: w.flags as u32,
                        label: fit_nanopb_string(&w.label, osupad_layout::LABEL_MAX_BYTES + 1),
                        suffix: fit_nanopb_string(&w.suffix, osupad_layout::SUFFIX_MAX_BYTES + 1),
                    })
                    .collect(),
            })),
        };
        self.send_msg(msg)
    }

    /// Return a screen to the device's built-in layout
    pub async fn reset_layout(&self, screen: Screen) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::ResetLayout(screen.to_wire() as u32)),
        };
        self.send_msg(msg)
    }

    /// Ask the device for a DeviceStatus (lifetime and current-map counters)
    pub async fn request_status(&self) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::RequestStatus(true)),
        };
        self.send_msg(msg)
    }

    pub async fn request_logs(&self) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::RequestLogs(true)),
        };
        self.send_msg(msg)
    }

    pub async fn send_host_status(
        &self,
        tosu_connected: bool,
        playing: bool,
        play_id: u32,
    ) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::HostStatus(proto::HostStatus {
                tosu_connected,
                playing,
                play_id,
            })),
        };
        self.send_msg(msg)
    }

    pub async fn send_counter_sync(
        &self,
        counters: &CounterState,
        force_restore: bool,
    ) -> Result<u32, DeviceError> {
        let seq = self.next_seq();
        let msg = HostToDevice {
            sequence_number: seq,
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
        self.send_msg(msg)?;
        Ok(seq)
    }
}

/// Truncate to fit a nanopb fixed `char[max_size]` field (max_size - 1 bytes + NUL).
/// nanopb rejects the whole message if a string is too long, which would freeze the display.
fn fit_nanopb_string(s: &str, max_size: usize) -> String {
    let limit = max_size - 1;
    if s.len() <= limit {
        return s.to_string();
    }
    let mut end = limit;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
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
                // Counters first: the connect handler decides between known pad, new pad and
                // replacement from them, so they must already be the pad's, not the last pad's
                let _ = tx.send(DeviceEvent::Counters(CounterState {
                    device_id: ack.device_id.clone(),
                    counter_generation: ack.counter_generation,
                    lifetime_key1: ack.lifetime_key1,
                    lifetime_key2: ack.lifetime_key2,
                    map_key1: 0,
                    map_key2: 0,
                }));
                let _ = tx.send(DeviceEvent::Connected(info));
            }
            proto::device_to_host::Payload::Status(st) => {
                let _ = tx.send(DeviceEvent::StatusUpdate(*st));
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
                let st_opt = resp.synchronized_state.as_ref().map(|st| CounterState {
                    device_id: st.device_id.clone(),
                    counter_generation: st.counter_generation,
                    lifetime_key1: st.lifetime_key1,
                    lifetime_key2: st.lifetime_key2,
                    map_key1: 0,
                    map_key2: 0,
                });
                let _ = tx.send(DeviceEvent::CounterSyncResult {
                    seq: msg.sequence_number,
                    success: resp.success,
                    message: resp.message.clone(),
                    state: st_opt.clone(),
                });
                if let Some(st) = st_opt {
                    let _ = tx.send(DeviceEvent::Counters(st));
                }
            }
            proto::device_to_host::Payload::ConfigAck(ack) => {
                let cfg_opt = ack.current_config.as_ref().map(|c| DeviceConfig {
                    key1_hid_usage: c.key1_hid_usage,
                    key2_hid_usage: c.key2_hid_usage,
                    debounce_us: c.debounce_us,
                    brightness: c.brightness,
                    display_sleep_seconds: c.display_sleep_seconds,
                    gameplay_display_hz: c.gameplay_display_hz,
                    tosu_endpoint: "".to_string(),
                });
                let _ = tx.send(DeviceEvent::ConfigAck {
                    seq: msg.sequence_number,
                    success: ack.success,
                    message: ack.message.clone(),
                    current_config: cfg_opt,
                });
            }
            proto::device_to_host::Payload::LogBatch(batch) => {
                let _ = tx.send(DeviceEvent::LogBatch(batch.clone()));
            }
            proto::device_to_host::Payload::LayoutAck(ack) => {
                let _ = tx.send(DeviceEvent::LayoutAck {
                    screen: ack.screen.min(u8::MAX as u32) as u8,
                    success: ack.success,
                    message: ack.message.clone(),
                });
            }
        }
    }
}

/// Finds the serial port of the osu!pad running its application firmware.
///
/// Deliberately ignores the ROM bootloader (303a:1001): opening that port
/// toggles DTR/RTS, which resets the chip out of download mode mid-flash.
pub fn find_target_port() -> Option<String> {
    find_usb_port(ESPRESSIF_VID, OSUPAD_APP_PID)
}

/// Finds the serial port of the ESP32-S3 ROM download bootloader.
pub fn find_bootloader_port() -> Option<String> {
    find_usb_port(ESPRESSIF_VID, ESP_ROM_BOOTLOADER_PID)
}

fn find_usb_port(vid: u16, pid: u16) -> Option<String> {
    serialport::available_ports()
        .ok()?
        .into_iter()
        .find(|p| matches!(&p.port_type, SerialPortType::UsbPort(info) if info.vid == vid && info.pid == pid))
        .map(|p| p.port_name)
}

#[cfg(test)]
mod tests {
    use super::{fit_nanopb_string, handle_device_message, proto, DeviceEvent};

    #[test]
    fn hello_ack_emits_counters_before_connected() {
        let (tx, mut rx) = tokio::sync::broadcast::channel(8);
        let msg = proto::DeviceToHost {
            sequence_number: 1,
            payload: Some(proto::device_to_host::Payload::HelloAck(proto::HelloAck {
                protocol_version: 1,
                device_id: "OSUPAD-NEW".to_string(),
                counter_generation: 1,
                lifetime_key1: 3,
                lifetime_key2: 4,
                ..Default::default()
            })),
        };
        handle_device_message(&msg, &tx);
        assert!(
            matches!(rx.try_recv(), Ok(DeviceEvent::Counters(c)) if c.device_id == "OSUPAD-NEW")
        );
        assert!(matches!(rx.try_recv(), Ok(DeviceEvent::Connected(_))));
    }

    #[test]
    fn test_fit_nanopb_string() {
        assert_eq!(fit_nanopb_string("Lunatic", 32), "Lunatic");
        assert_eq!(fit_nanopb_string(&"a".repeat(100), 64).len(), 63);
        // Never splits a multi-byte UTF-8 character
        let jp = "灰".repeat(30); // 3 bytes each
        let fitted = fit_nanopb_string(&jp, 64);
        assert!(fitted.len() <= 63 && fitted.chars().all(|c| c == '灰'));
    }
}
