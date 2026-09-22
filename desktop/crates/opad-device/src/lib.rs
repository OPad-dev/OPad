pub mod flash;

use bytes::BytesMut;
use chrono::{Datelike, Local, Timelike};
use opad_layout::{Layout, Screen};
use opad_model::ui_source::SourceValue;
use opad_model::{CounterState, DeviceConfig, DeviceInfo};
pub use opad_protocol::proto;
use opad_protocol::proto::{host_to_device, DeviceToHost, HostToDevice};
use opad_protocol::{decode_device_message, encode_host_message};
use serialport::SerialPortType;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

pub const ESPRESSIF_VID: u16 = 0x303A;
/// pid.codes open-source VID, the other one an OPad may enumerate under
pub const PIDCODES_VID: u16 = 0x1209;
/// Every pad's USB serial number and HelloAck device_id start with this
pub const DEVICE_ID_PREFIX: &str = "OSUPAD-";
/// USB PID of the OPad application firmware (TinyUSB composite device)
pub const OSUPAD_APP_PID: u16 = 0x4001;
/// USB PID of the ESP32-S3 ROM download bootloader (USB-Serial-JTAG)
pub const ESP_ROM_BOOTLOADER_PID: u16 = 0x1001;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Serial port error: {0}")]
    Serial(#[from] serialport::Error),
    #[error("Protocol error: {0}")]
    Protocol(#[from] opad_protocol::ProtocolError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Device not connected")]
    NotConnected,
    #[error("Device command queue busy")]
    Busy,
}

#[derive(Debug, Clone)]
pub enum DeviceEvent {
    /// Which host install the pad says owns it (§W3-2).
    ///
    /// Emitted from the HelloAck arm *before* `Counters` and `Connected`, and
    /// emitted on every HelloAck even when the pad reports nothing, so a pad
    /// that sends no owner can never be judged against the previous pad's.
    /// That is the R3 failure mode exactly, and it is what §W3-3 warns about.
    Ownership {
        owner_id: Vec<u8>,
    },
    Connected(DeviceInfo, Option<DeviceConfig>),
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
    PinDetected {
        key_id: u32,
        gpio: u32,
        success: bool,
    },
}

/// How often the worker looks for a pad while none is connected (§W1-2).
///
/// Windows has no udev, and `RegisterDeviceNotification` needs an HWND and a
/// message pump — machinery a headless daemon has no other use for, and a
/// Windows-only failure surface next to the serial port. §W1-2 explicitly
/// allows the reconnect poll instead, so this is that poll, fast enough that
/// the "connects within 2 s" acceptance holds on both platforms with margin.
/// Enumeration is a `/sys` read on Linux and a SetupAPI class query on Windows;
/// neither is expensive at this rate, and neither writes storage, so P1-3 is
/// unaffected however often it runs.
const PORT_SCAN_INTERVAL: Duration = Duration::from_millis(400);

/// Backoff after a port exists but will not open. Usually another process is
/// holding it — on Windows serial handles are exclusive — and retrying fast
/// helps nobody.
const PORT_OPEN_RETRY_INTERVAL: Duration = Duration::from_millis(1500);

/// Settle time after a disconnect, before looking again. Long enough for the
/// device node to go away on unplug, short enough that a replug reconnects
/// inside the same acceptance window.
const RECONNECT_SETTLE_INTERVAL: Duration = Duration::from_millis(300);

/// The pad queues each frame whole (usb_cdc_write), so bytes that stay an
/// incomplete frame this long are a false start or a torn stream: drop them
/// and re-handshake rather than wait for a length that will never arrive.
/// Read timeout of the worker's port. The worker alternates between sending
/// queued commands and a blocking read, so this bounds how long a command (the
/// HUD's telemetry) waits behind a quiet line.
const WORKER_READ_TIMEOUT: Duration = Duration::from_millis(10);

const STALE_PARTIAL_FRAME: Duration = Duration::from_millis(500);

/// Read timeout while probing a port that might be a pad (tier 2)
const PROBE_READ_TIMEOUT: Duration = Duration::from_millis(150);
/// How long a probed port gets to answer Hello with a HelloAck
const PROBE_ANSWER_WINDOW: Duration = Duration::from_millis(450);
/// A port with an OPad VID but no identifying strings is probed again this
/// often (the pad may have been booting). Other unknown ports are probed once
/// per appearance: opening a stranger's port can reset it (Arduino-style boards).
const PROBE_RETRY_KNOWN_VID: Duration = Duration::from_secs(5);

pub struct DeviceManager {
    cmd_tx: mpsc::Sender<HostToDevice>,
    event_tx: broadcast::Sender<DeviceEvent>,
    is_connected: Arc<AtomicBool>,
    is_paused: Arc<AtomicBool>,
    is_port_open: Arc<AtomicBool>,
    seq_counter: Arc<AtomicU32>,
    /// Port of the pad last handshaken with, for flashing without re-discovery
    last_port: Arc<Mutex<Option<String>>>,
}

impl DeviceManager {
    pub fn new_dummy() -> (Self, broadcast::Receiver<DeviceEvent>) {
        let (cmd_tx, mut cmd_rx) = mpsc::channel::<HostToDevice>(64);
        let (event_tx, event_rx) = broadcast::channel(64);
        let is_connected = Arc::new(AtomicBool::new(false));
        let is_paused = Arc::new(AtomicBool::new(false));
        let is_port_open = Arc::new(AtomicBool::new(false));
        let seq_counter = Arc::new(AtomicU32::new(1));
        let last_port = Arc::new(Mutex::new(None));
        tokio::spawn(async move { while cmd_rx.recv().await.is_some() {} });
        (
            Self {
                cmd_tx,
                event_tx,
                is_connected,
                is_paused,
                is_port_open,
                seq_counter,
                last_port,
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
        let last_port = Arc::new(Mutex::new(None));
        let last_port_clone = last_port.clone();

        // Spawn background worker managing the serial port lifecycle
        tokio::task::spawn_blocking(move || {
            let mut read_buf = BytesMut::with_capacity(8192);
            // Last open failure, so a port that keeps refusing is reported once
            // rather than on every retry. Cleared when the pad goes away.
            let mut last_open_error: Option<String> = None;
            let mut probes = ProbeHistory::default();

            loop {
                if is_paused_clone.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }

                // Discover target serial port
                let port_path = match discover_port(&mut probes) {
                    Some(p) => p,
                    None => {
                        debug!("Searching for OPad ESP32-S3 USB port...");
                        last_open_error = None;
                        std::thread::sleep(PORT_SCAN_INTERVAL);
                        continue;
                    }
                };

                if last_open_error.is_none() {
                    info!("Opening OPad serial port at {}", port_path);
                }
                let port_builder = serialport::new(&port_path, 115200).timeout(WORKER_READ_TIMEOUT);

                let mut port = match port_builder.open() {
                    Ok(mut p) => {
                        let _ = p.write_data_terminal_ready(true);
                        let _ = p.write_request_to_send(true);
                        last_open_error = None;
                        p
                    }
                    Err(e) => {
                        let message = e.to_string();
                        if last_open_error.as_deref() != Some(message.as_str()) {
                            warn!(
                                "Cannot open OPad serial port {}: {}{} (retrying every {:?})",
                                port_path,
                                message,
                                open_error_hint(&e),
                                PORT_OPEN_RETRY_INTERVAL
                            );
                            last_open_error = Some(message);
                        } else {
                            debug!("Failed to open port {}: {}", port_path, e);
                        }
                        std::thread::sleep(PORT_OPEN_RETRY_INTERVAL);
                        continue;
                    }
                };

                // Connected only once the pad answers Hello with its identity:
                // an open port proves nothing about what is on the other end
                is_open_clone.store(true, Ordering::SeqCst);
                debug!("Opened {}; waiting for HelloAck", port_path);

                let mut raw_buf = [0u8; 1024];
                let mut last_hello = Instant::now() - Duration::from_secs(10);
                let mut has_hello_ack = false;
                let mut partial_since: Option<Instant> = None;

                // Inner communication loop
                loop {
                    if is_paused_clone.load(Ordering::SeqCst) {
                        info!("Pause requested; releasing serial port on {}", port_path);
                        break;
                    }

                    if partial_since.is_some_and(|t| t.elapsed() >= STALE_PARTIAL_FRAME) {
                        warn!(
                            "Dropping {} bytes of an incomplete frame; re-sending Hello",
                            read_buf.len()
                        );
                        read_buf.clear();
                        partial_since = None;
                        has_hello_ack = false;
                        last_hello = Instant::now() - Duration::from_secs(10);
                    }

                    // Periodic Hello retry until HelloAck is received (§6.1)
                    if !has_hello_ack && last_hello.elapsed() >= Duration::from_millis(800) {
                        last_hello = Instant::now();
                        if let Ok(encoded) = encode_host_message(&hello_message()) {
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

                            // Parse all ready frames; a bad one is skipped, not fatal
                            loop {
                                match decode_device_message(&mut read_buf) {
                                    Ok(Some(msg)) => {
                                        if let Some(proto::device_to_host::Payload::HelloAck(ack)) =
                                            &msg.payload
                                        {
                                            if !is_opad_device_id(&ack.device_id) {
                                                warn!(
                                                    "{} answered Hello as {:?}, not an OPad; ignoring it",
                                                    port_path, ack.device_id
                                                );
                                                continue;
                                            }
                                            has_hello_ack = true;
                                            if !is_conn_clone.swap(true, Ordering::SeqCst) {
                                                info!(
                                                    "Connected to OPad {} on {}",
                                                    ack.device_id, port_path
                                                );
                                                *last_port_clone.lock().unwrap() =
                                                    Some(port_path.clone());
                                            }
                                        } else if !has_hello_ack {
                                            // Nothing is trusted before the handshake
                                            continue;
                                        }
                                        handle_device_message(&msg, &event_tx_clone);
                                    }
                                    Ok(None) => break,
                                    Err(e) => debug!("Skipping undecodable frame: {}", e),
                                }
                            }
                            if read_buf.is_empty() {
                                partial_since = None;
                            } else if partial_since.is_none() {
                                partial_since = Some(Instant::now());
                            }
                        }
                        Ok(_) => {}
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            info!("Device disconnected from {}: {}", port_path, e);
                            break;
                        }
                    }
                    // No sleep: the read timeout paces an idle loop, and a busy
                    // line is drained without added delay
                }

                drop(port);
                is_open_clone.store(false, Ordering::SeqCst);
                is_conn_clone.store(false, Ordering::SeqCst);

                // Drain any pending commands on disconnect so they are never replayed to a new connection
                while cmd_rx.try_recv().is_ok() {}
                let _ = event_tx_clone.send(DeviceEvent::Disconnected);
                read_buf.clear();
                std::thread::sleep(RECONNECT_SETTLE_INTERVAL);
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
                last_port,
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

    /// Serial port of the pad this manager last completed a handshake with.
    /// Kept across pause/disconnect so a flasher can find the pad even where
    /// the port carries no identifying strings (tier-2 discovery).
    pub fn port(&self) -> Option<String> {
        self.last_port.lock().unwrap().clone()
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

    /// Records this install as the pad's owner (§W3-2).
    ///
    /// An NVS write on the device, so the firmware honours it only in IDLE.
    /// The host only ever sends it at connect time, which is already one.
    pub async fn claim_ownership(&self, owner_id: &[u8]) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::ClaimOwnership(
                proto::ClaimOwnership {
                    owner_id: owner_id.to_vec(),
                },
            )),
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
                    key1_gpio: config.key1_gpio,
                    key2_gpio: config.key2_gpio,
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
                        label: fit_nanopb_string(&w.label, opad_layout::LABEL_MAX_BYTES + 1),
                        suffix: fit_nanopb_string(&w.suffix, opad_layout::SUFFIX_MAX_BYTES + 1),
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

    pub async fn send_detect_pin(
        &self,
        key_id: u32,
        timeout_ms: u32,
        exclude_gpio: u32,
    ) -> Result<(), DeviceError> {
        let msg = HostToDevice {
            sequence_number: self.next_seq(),
            payload: Some(host_to_device::Payload::DetectPin(
                proto::DetectPinRequest {
                    key_id,
                    timeout_ms,
                    exclude_gpio,
                },
            )),
        };
        self.send_msg(msg)
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
                    // Empty on firmware predating §U-3a; `None` says "unknown"
                    // rather than inventing a slot name for it
                    running_partition: (!ack.running_partition.is_empty())
                        .then(|| ack.running_partition.clone()),
                };
                // Ownership first, then counters: the connect handler decides
                // between known pad, new pad, replacement and takeover from
                // all three, so every one must already be this pad's rather
                // than the last pad's (R3, §W3-3).
                let _ = tx.send(DeviceEvent::Ownership {
                    owner_id: ack.owner_id.clone(),
                });
                let _ = tx.send(DeviceEvent::Counters(CounterState {
                    device_id: ack.device_id.clone(),
                    counter_generation: ack.counter_generation,
                    lifetime_key1: ack.lifetime_key1,
                    lifetime_key2: ack.lifetime_key2,
                    map_key1: 0,
                    map_key2: 0,
                }));
                let cfg_opt = ack.current_config.as_ref().map(|c| DeviceConfig {
                    key1_hid_usage: c.key1_hid_usage,
                    key2_hid_usage: c.key2_hid_usage,
                    debounce_us: c.debounce_us,
                    brightness: c.brightness,
                    display_sleep_seconds: c.display_sleep_seconds,
                    gameplay_display_hz: c.gameplay_display_hz,
                    tosu_endpoint: opad_model::DeviceConfig::default().tosu_endpoint,
                    key1_gpio: if c.key1_gpio == 0 {
                        opad_model::DEFAULT_KEY1_GPIO
                    } else {
                        c.key1_gpio
                    },
                    key2_gpio: if c.key2_gpio == 0 {
                        opad_model::DEFAULT_KEY2_GPIO
                    } else {
                        c.key2_gpio
                    },
                });
                let _ = tx.send(DeviceEvent::Connected(info, cfg_opt));
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
                    tosu_endpoint: opad_model::DeviceConfig::default().tosu_endpoint,
                    // 0 from firmware without configurable pins, which uses the defaults
                    key1_gpio: if c.key1_gpio == 0 {
                        opad_model::DEFAULT_KEY1_GPIO
                    } else {
                        c.key1_gpio
                    },
                    key2_gpio: if c.key2_gpio == 0 {
                        opad_model::DEFAULT_KEY2_GPIO
                    } else {
                        c.key2_gpio
                    },
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
            proto::device_to_host::Payload::DetectPinResp(resp) => {
                let _ = tx.send(DeviceEvent::PinDetected {
                    key_id: resp.key_id,
                    gpio: resp.gpio,
                    success: resp.success,
                });
            }
        }
    }
}

/// What to check when the pad's port exists but will not open.
fn open_error_hint(e: &serialport::Error) -> &'static str {
    match e.kind() {
        serialport::ErrorKind::Io(std::io::ErrorKind::PermissionDenied) => {
            if cfg!(target_os = "linux") {
                ". Permission denied: the udev rule (70-opad.rules) should give the \
                 logged-in user access; check it is installed and `getfacl` on the port \
                 lists your user"
            } else {
                ". Access denied: another program probably has the port open"
            }
        }
        serialport::ErrorKind::NoDevice => ". The port went away while opening it",
        _ => "",
    }
}

pub fn is_opad_device_id(id: &str) -> bool {
    id.starts_with(DEVICE_ID_PREFIX)
}

/// What a port's USB descriptor strings say about it.
#[derive(Debug, PartialEq, Eq)]
enum PortClass {
    /// OPad VID and an OPad product or serial string (tier 1)
    Opad,
    /// Worth a Hello probe: OPad VID without strings, or no USB info at all
    Probe { known_vid: bool },
    /// Something else, identified as such, or a port never to be touched
    Skip,
}

fn is_system_port(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper == "COM1" || upper == "COM2" || name.starts_with("/dev/ttyS")
}

fn classify_port(name: &str, port_type: &SerialPortType) -> PortClass {
    if is_system_port(name) {
        return PortClass::Skip;
    }
    match port_type {
        SerialPortType::UsbPort(info) => {
            // The ROM bootloader: opening it toggles DTR/RTS, which resets the
            // chip out of download mode mid-flash
            if info.vid == ESPRESSIF_VID && info.pid == ESP_ROM_BOOTLOADER_PID {
                return PortClass::Skip;
            }
            let known_vid = info.vid == ESPRESSIF_VID || info.vid == PIDCODES_VID;
            let product_says = info.product.as_deref().map(|p| p.contains("OPad"));
            let serial_says = info.serial_number.as_deref().map(is_opad_device_id);
            if known_vid && (product_says == Some(true) || serial_says == Some(true)) {
                PortClass::Opad
            } else if product_says.is_none() && serial_says.is_none() {
                // Windows often reports neither string for composite devices
                PortClass::Probe { known_vid }
            } else {
                PortClass::Skip
            }
        }
        // Linux ports whose sysfs parent could not be read
        SerialPortType::Unknown => PortClass::Probe { known_vid: false },
        SerialPortType::BluetoothPort | SerialPortType::PciPort => PortClass::Skip,
    }
}

/// Finds the serial port of the OPad running its application firmware from
/// its USB descriptors alone (tier 1). Never opens a port, so it is safe to
/// call while the daemon holds the pad; it can miss a pad whose strings the
/// platform does not report, which [`DeviceManager::port`] covers.
pub fn find_target_port() -> Option<String> {
    serialport::available_ports()
        .ok()?
        .into_iter()
        .find(|p| classify_port(&p.port_name, &p.port_type) == PortClass::Opad)
        .map(|p| p.port_name)
}

/// When each tier-2 candidate was last probed, so ports are not hammered.
#[derive(Default)]
struct ProbeHistory {
    last_probe: HashMap<String, Instant>,
}

impl ProbeHistory {
    fn due(&self, name: &str, known_vid: bool) -> bool {
        match self.last_probe.get(name) {
            None => true,
            Some(t) => known_vid && t.elapsed() >= PROBE_RETRY_KNOWN_VID,
        }
    }
}

/// Tier 1, then tier 2: ports that might be a pad are opened and sent Hello,
/// and only one that answers with an OSUPAD- device_id is returned.
fn discover_port(probes: &mut ProbeHistory) -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    // Forget ports that went away, so a replugged one is probed afresh
    probes
        .last_probe
        .retain(|name, _| ports.iter().any(|p| &p.port_name == name));

    let mut candidates = Vec::new();
    for p in &ports {
        match classify_port(&p.port_name, &p.port_type) {
            PortClass::Opad => return Some(p.port_name.clone()),
            PortClass::Probe { known_vid } if probes.due(&p.port_name, known_vid) => {
                candidates.push(p.port_name.clone())
            }
            _ => {}
        }
    }
    for name in candidates {
        probes.last_probe.insert(name.clone(), Instant::now());
        if probe_port(&name) {
            info!("{} answered Hello as an OPad", name);
            return Some(name);
        }
    }
    None
}

/// Opens `path`, sends a framed Hello and waits briefly for a HelloAck whose
/// device_id marks it as an OPad.
fn probe_port(path: &str) -> bool {
    let Ok(mut port) = serialport::new(path, 115200)
        .timeout(PROBE_READ_TIMEOUT)
        .open()
    else {
        return false;
    };
    let _ = port.write_data_terminal_ready(true);
    let _ = port.write_request_to_send(true);
    let Ok(hello) = encode_host_message(&hello_message()) else {
        return false;
    };
    if port.write_all(&hello).is_err() {
        return false;
    }

    let deadline = Instant::now() + PROBE_ANSWER_WINDOW;
    let mut buf = BytesMut::with_capacity(1024);
    let mut raw = [0u8; 512];
    while Instant::now() < deadline {
        match port.read(&mut raw) {
            Ok(n) if n > 0 => buf.extend_from_slice(&raw[..n]),
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => return false,
        }
        loop {
            match decode_device_message(&mut buf) {
                Ok(Some(DeviceToHost {
                    payload: Some(proto::device_to_host::Payload::HelloAck(ack)),
                    ..
                })) => return is_opad_device_id(&ack.device_id),
                Ok(Some(_)) | Err(_) => continue,
                Ok(None) => break,
            }
        }
    }
    false
}

fn hello_message() -> HostToDevice {
    HostToDevice {
        sequence_number: 1,
        payload: Some(host_to_device::Payload::Hello(proto::Hello {
            protocol_version: 1,
            client_version: "1.0.0".to_string(),
        })),
    }
}

/// Where a pad sits, recorded from its app port before it is rebooted into the
/// ROM bootloader, so the bootloader port can be matched to the same chip.
///
/// 303a:1001 is the USB-Serial-JTAG of every ESP32-S3/C3/C6/H2, so "any
/// bootloader port" can be someone else's dev board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PadLocation {
    /// Chip MAC as 12 uppercase hex digits: the app's serial is OSUPAD-<MAC>,
    /// the ROM's is the same MAC written XX:XX:XX:XX:XX:XX
    pub mac: Option<String>,
    /// Physical USB path (Linux sysfs, e.g. "1-3"); the bootloader re-enumerates
    /// on the same one
    pub usb_path: Option<String>,
}

/// A ROM bootloader port and what identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootloaderPort {
    pub name: String,
    pub location: PadLocation,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BootloaderMatch {
    Found(String),
    None,
    /// Several bootloader ports could be this pad; the caller must name one
    Ambiguous(Vec<String>),
}

fn normalize_mac(s: &str) -> Option<String> {
    let hex: String = s.chars().filter(|c| *c != ':' && *c != '-').collect();
    (hex.len() == 12 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| hex.to_ascii_uppercase())
}

#[cfg(target_os = "linux")]
fn usb_path_of(port_name: &str) -> Option<String> {
    // /sys/class/tty/ttyACM0/device -> .../usb1/1-3/1-3:1.1; the parent of the
    // interface directory is the physical device
    let tty = std::path::Path::new(port_name).file_name()?;
    let iface = std::fs::canonicalize(
        std::path::Path::new("/sys/class/tty")
            .join(tty)
            .join("device"),
    )
    .ok()?;
    let dev = iface.parent()?.file_name()?.to_str()?;
    Some(dev.to_string())
}

#[cfg(not(target_os = "linux"))]
fn usb_path_of(_port_name: &str) -> Option<String> {
    None
}

/// Where the pad behind `app_port` sits. `device_id` (from HelloAck) supplies
/// the MAC when the platform does not report the USB serial string.
pub fn locate_pad(app_port: &str, device_id: Option<&str>) -> PadLocation {
    let serial = serialport::available_ports().ok().and_then(|ports| {
        ports
            .into_iter()
            .find(|p| p.port_name == app_port)
            .and_then(|p| match p.port_type {
                SerialPortType::UsbPort(info) => info.serial_number,
                _ => None,
            })
    });
    let mac = serial
        .as_deref()
        .or(device_id)
        .and_then(|s| s.strip_prefix(DEVICE_ID_PREFIX))
        .and_then(normalize_mac);
    PadLocation {
        mac,
        usb_path: usb_path_of(app_port),
    }
}

/// Every connected ESP ROM bootloader (303a:1001) port.
pub fn bootloader_ports() -> Vec<BootloaderPort> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| match p.port_type {
            SerialPortType::UsbPort(info)
                if info.vid == ESPRESSIF_VID && info.pid == ESP_ROM_BOOTLOADER_PID =>
            {
                Some(BootloaderPort {
                    location: PadLocation {
                        mac: info.serial_number.as_deref().and_then(normalize_mac),
                        usb_path: usb_path_of(&p.port_name),
                    },
                    name: p.port_name,
                })
            }
            _ => None,
        })
        .collect()
}

fn differs(a: &Option<String>, b: &Option<String>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a != b)
}

fn same(a: &Option<String>, b: &Option<String>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a == b)
}

/// Picks the bootloader port that is `pad`.
///
/// A MAC or USB-path match wins. A port that contradicts either is someone
/// else's. What is left — ports that cannot be compared — is accepted only if
/// exactly one of them appeared since `preexisting` was taken, i.e. came up
/// because this pad was just rebooted.
pub fn select_bootloader_port(
    candidates: &[BootloaderPort],
    pad: &PadLocation,
    preexisting: &[String],
) -> BootloaderMatch {
    if let Some(c) = candidates
        .iter()
        .find(|c| same(&c.location.mac, &pad.mac) || same(&c.location.usb_path, &pad.usb_path))
    {
        return BootloaderMatch::Found(c.name.clone());
    }
    let unproven: Vec<&BootloaderPort> = candidates
        .iter()
        .filter(|c| {
            !differs(&c.location.mac, &pad.mac) && !differs(&c.location.usb_path, &pad.usb_path)
        })
        .filter(|c| !preexisting.contains(&c.name))
        .collect();
    match unproven.as_slice() {
        [] => BootloaderMatch::None,
        [one] => BootloaderMatch::Found(one.name.clone()),
        many => BootloaderMatch::Ambiguous(many.iter().map(|c| c.name.clone()).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_port, fit_nanopb_string, handle_device_message, is_opad_device_id, normalize_mac,
        proto, select_bootloader_port, BootloaderMatch, BootloaderPort, DeviceEvent, PadLocation,
        PortClass, ProbeHistory, PORT_SCAN_INTERVAL, PROBE_RETRY_KNOWN_VID,
        RECONNECT_SETTLE_INTERVAL,
    };
    use serialport::SerialPortType;
    use std::time::{Duration, Instant};

    /// §W1-2 acceptance: with the daemon already running, plugging the pad in
    /// connects within 2 s. The worst case is a plug landing just after a scan
    /// while the loop is still serving the settle delay from the unplug, so
    /// these two intervals are the whole budget — pinned here so a future edit
    /// cannot quietly spend it.
    #[test]
    fn hotplug_detection_fits_its_two_second_budget() {
        assert!(
            PORT_SCAN_INTERVAL + RECONNECT_SETTLE_INTERVAL < Duration::from_secs(2),
            "hotplug detection must stay under the 2 s acceptance"
        );
    }

    /// R3, and §W3-3 which inherits it: the connect handler decides known pad
    /// vs new pad vs replacement vs takeover, so the owner and the counters
    /// must both already be *this* pad's when `Connected` arrives.
    #[test]
    fn hello_ack_emits_ownership_and_counters_before_connected() {
        let (tx, mut rx) = tokio::sync::broadcast::channel(8);
        let msg = proto::DeviceToHost {
            sequence_number: 1,
            payload: Some(proto::device_to_host::Payload::HelloAck(proto::HelloAck {
                protocol_version: 1,
                device_id: "OSUPAD-NEW".to_string(),
                counter_generation: 1,
                lifetime_key1: 3,
                lifetime_key2: 4,
                owner_id: vec![7u8; 16],
                ..Default::default()
            })),
        };
        handle_device_message(&msg, &tx);
        assert!(
            matches!(rx.try_recv(), Ok(DeviceEvent::Ownership { owner_id }) if owner_id == vec![7u8; 16])
        );
        assert!(
            matches!(rx.try_recv(), Ok(DeviceEvent::Counters(c)) if c.device_id == "OSUPAD-NEW")
        );
        assert!(matches!(rx.try_recv(), Ok(DeviceEvent::Connected(..))));
    }

    /// Firmware that predates §W3-2 sends no owner at all. The event must still
    /// be emitted, carrying nothing, so the previous pad's owner can never be
    /// left standing for this one to be judged against.
    #[test]
    fn a_hello_ack_with_no_owner_still_reports_ownership() {
        let (tx, mut rx) = tokio::sync::broadcast::channel(8);
        let msg = proto::DeviceToHost {
            sequence_number: 1,
            payload: Some(proto::device_to_host::Payload::HelloAck(proto::HelloAck {
                protocol_version: 1,
                device_id: "OSUPAD-OLD".to_string(),
                ..Default::default()
            })),
        };
        handle_device_message(&msg, &tx);
        assert!(
            matches!(rx.try_recv(), Ok(DeviceEvent::Ownership { owner_id }) if owner_id.is_empty())
        );
    }

    fn usb(vid: u16, pid: u16, product: Option<&str>, serial: Option<&str>) -> SerialPortType {
        SerialPortType::UsbPort(serialport::UsbPortInfo {
            vid,
            pid,
            serial_number: serial.map(str::to_string),
            manufacturer: None,
            product: product.map(str::to_string),
        })
    }

    #[test]
    fn tier_one_needs_an_opad_vid_and_an_opad_string() {
        let by_product = usb(0x303A, 0x4001, Some("OPad ESP32-S3"), None);
        assert_eq!(classify_port("/dev/ttyACM0", &by_product), PortClass::Opad);
        let by_serial = usb(0x1209, 0x1234, None, Some("OSUPAD-A1B2C3D4E5F6"));
        assert_eq!(classify_port("COM7", &by_serial), PortClass::Opad);
        // Another Espressif board that names itself
        let other = usb(0x303A, 0x4001, Some("TinyUSB CDC"), Some("123456"));
        assert_eq!(classify_port("/dev/ttyACM1", &other), PortClass::Skip);
        // Right strings, foreign VID
        let foreign = usb(0x2341, 0x0043, Some("OPad"), None);
        assert_eq!(classify_port("/dev/ttyACM2", &foreign), PortClass::Skip);
    }

    #[test]
    fn ports_without_strings_are_probed_and_system_ports_never() {
        let bare = usb(0x303A, 0x4001, None, None);
        assert_eq!(
            classify_port("COM5", &bare),
            PortClass::Probe { known_vid: true }
        );
        assert_eq!(
            classify_port("/dev/ttyUSB0", &SerialPortType::Unknown),
            PortClass::Probe { known_vid: false }
        );
        for name in ["COM1", "com2", "/dev/ttyS0", "/dev/ttyS12"] {
            assert_eq!(
                classify_port(name, &SerialPortType::Unknown),
                PortClass::Skip
            );
        }
        // The ROM bootloader is never opened by discovery
        let rom = usb(0x303A, 0x1001, None, None);
        assert_eq!(classify_port("/dev/ttyACM0", &rom), PortClass::Skip);
    }

    #[test]
    fn unknown_ports_are_probed_once_known_vid_ports_again_later() {
        let mut h = ProbeHistory::default();
        assert!(h.due("COM9", false));
        h.last_probe.insert("COM9".into(), Instant::now());
        assert!(!h.due("COM9", false));
        h.last_probe
            .insert("COM5".into(), Instant::now() - PROBE_RETRY_KNOWN_VID);
        assert!(h.due("COM5", true));
    }

    #[test]
    fn device_ids_are_recognised_by_prefix() {
        assert!(is_opad_device_id("OSUPAD-0011223344AA"));
        assert!(!is_opad_device_id("osupad-0011"));
        assert!(!is_opad_device_id(""));
    }

    fn boot(name: &str, mac: Option<&str>, path: Option<&str>) -> BootloaderPort {
        BootloaderPort {
            name: name.into(),
            location: PadLocation {
                mac: mac.map(Into::into),
                usb_path: path.map(Into::into),
            },
        }
    }

    #[test]
    fn the_bootloader_port_is_matched_by_mac_or_usb_path() {
        let pad = PadLocation {
            mac: Some("3CDC75701678".into()),
            usb_path: Some("1-3".into()),
        };
        let other = boot("/dev/ttyACM1", Some("AABBCCDDEEFF"), Some("1-4"));
        let ours = boot("/dev/ttyACM2", Some("3CDC75701678"), Some("1-3"));
        // Even though the other board was there first and is listed first
        let all = [other.clone(), ours.clone()];
        let pre = ["/dev/ttyACM1".to_string()];
        assert_eq!(
            select_bootloader_port(&all, &pad, &pre),
            BootloaderMatch::Found("/dev/ttyACM2".into())
        );
        // Only the stranger is in download mode: never it
        assert_eq!(
            select_bootloader_port(&[other], &pad, &[]),
            BootloaderMatch::None
        );
        // USB path alone is enough (no serial string reported)
        let by_path = boot("/dev/ttyACM3", None, Some("1-3"));
        assert_eq!(
            select_bootloader_port(&[by_path], &pad, &[]),
            BootloaderMatch::Found("/dev/ttyACM3".into())
        );
    }

    #[test]
    fn unidentifiable_bootloader_ports_need_to_be_the_only_new_one() {
        let pad = PadLocation::default();
        let a = boot("COM4", None, None);
        let b = boot("COM6", None, None);
        assert_eq!(
            select_bootloader_port(&[a.clone(), b.clone()], &pad, &["COM4".into()]),
            BootloaderMatch::Found("COM6".into())
        );
        assert_eq!(
            select_bootloader_port(&[a, b], &pad, &[]),
            BootloaderMatch::Ambiguous(vec!["COM4".into(), "COM6".into()])
        );
    }

    #[test]
    fn macs_are_normalised_from_both_spellings() {
        assert_eq!(
            normalize_mac("3c:dc:75:70:16:78").as_deref(),
            Some("3CDC75701678")
        );
        assert_eq!(
            normalize_mac("3CDC75701678").as_deref(),
            Some("3CDC75701678")
        );
        assert_eq!(normalize_mac("123456"), None);
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
