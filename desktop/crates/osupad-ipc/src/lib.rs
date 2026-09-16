use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice, JsonBackup,
    LatencyStats, LogEntry, RuntimeMode,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

mod transport;
#[cfg(windows)]
pub use transport::pipe_security_sddl;
pub use transport::{
    connect, create_listener, get_socket_path, IpcListener, IpcServerStream, IpcStream,
};

pub const IPC_PROTOCOL_VERSION: u32 = 1;
pub const MAX_REQUEST_FRAME_SIZE: usize = 1024 * 1024; // 1 MiB cap (§P2-6)
pub const MAX_RESPONSE_FRAME_SIZE: usize = 8 * 1024 * 1024; // 8 MiB cap (§P2-6)
pub const MAX_IPC_FRAME_SIZE: usize = MAX_REQUEST_FRAME_SIZE;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Daemon offline / connection refused: {0}")]
    NotConnected(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("osupad-daemon is already running")]
    AlreadyRunning,
}

/// What the §W3-3 takeover prompt shows.
///
/// The counters are in here because the choice between keeping the pad's and
/// keeping this PC's is meaningless without both numbers in front of the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TakeoverPrompt {
    pub device_id: String,
    pub device_key1: u64,
    pub device_key2: u64,
    pub pc_key1: u64,
    pub pc_key2: u64,
}

/// Snapshot of current PC state for backup comparison/preview (§21, §P1-5)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrentBackupState {
    pub device_id: String,
    pub counter_generation: u32,
    pub lifetime_key1: u64,
    pub lifetime_key2: u64,
    pub config: DeviceConfig,
}

/// Requests that GUI or CLI can send to the Daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    Handshake {
        client_version: String,
        client_protocol: u32,
    },
    GetStatus,
    UpdateConfig(DeviceConfig),
    ForceSync,
    ResetCounters {
        #[serde(default)]
        confirm: bool,
    },
    ExportBackup,
    PreviewImport(JsonBackup),
    ImportBackup {
        backup: JsonBackup,
        #[serde(default)]
        confirm: bool,
    },
    ResolveReplacement {
        restore: bool,
    },
    /// Answers the §W3-3 takeover prompt.
    ///
    /// `take_over: false` is "leave it alone": nothing is written to the pad
    /// and nothing is synced from it. It keeps working as a keyboard.
    /// `keep_device_counters` only matters when taking over — true adopts the
    /// pad's lifetime counters, false pushes this PC's onto it.
    ResolveTakeover {
        take_over: bool,
        #[serde(default)]
        keep_device_counters: bool,
    },
    RestoreDeviceFromPc {
        #[serde(default)]
        confirm: bool,
    },
    ImportPcFromDevice {
        #[serde(default)]
        confirm: bool,
    },
    GetLogEntries {
        #[serde(default)]
        since_seq: Option<u64>,
        limit: usize,
    },
    PrepareFlash,
    FinishFlash,
    ResetLatencyStats,
    /// Custom layouts saved in the daemon (None = the device's built-in default)
    GetLayouts,
    /// Validate, save and push a layout to the device
    SetLayout {
        screen: Screen,
        layout: Layout,
    },
    /// Forget a custom layout and return the device to its default
    ResetLayout {
        screen: Screen,
    },
    /// Latest UI data values the daemon knows (tosu data), for a live designer preview
    GetUiValues,
    /// What each updater knows: versions, last check, whether one is pending (§U-0.4)
    GetUpdateStatus,
    /// Turn one updater on or off. Per updater, never global (§U-0.4).
    SetUpdateEnabled {
        component: UpdateComponent,
        enabled: bool,
    },
    /// Apply a pending update. Only ever sent because a person pressed
    /// Install: the daemon never applies an app update on its own (§U-2).
    InstallUpdate {
        component: UpdateComponent,
    },
    /// What a firmware update would do, and everything currently stopping it
    /// (§U-3b). Reads state and writes nothing, so it is safe to poll.
    GetFirmwareUpdate,
    /// Flash the pad (§U-3b).
    ///
    /// Deliberately not `InstallUpdate { Firmware }`: this is the one
    /// operation that can stop the pad being a keyboard, so it carries its own
    /// consent flag, is unreachable from any updater setting, and is refused
    /// outright with `confirm: false`. The daemon syncs the counters to this PC
    /// before it writes anything.
    InstallFirmwareUpdate {
        #[serde(default)]
        confirm: bool,
    },
}

/// The firmware update offer, and what is in the way (§U-3b)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirmwareOffer {
    /// What the pad reports. `None` means no pad, or firmware too old to say.
    pub installed: Option<String>,
    /// `None` when there is nothing newer, or no verified manifest yet.
    pub available: Option<String>,
    pub notes: Option<String>,
    /// Which OTA slot the pad is running from (§U-3a)
    pub running_partition: Option<String>,
    /// Empty when the flash would start as soon as someone consents. Each
    /// entry is a whole sentence, ready to show.
    pub blockers: Vec<String>,
    /// The exact wording a person has to agree to. `None` when there is
    /// nothing to offer.
    pub consent_text: Option<String>,
    /// Roughly how long the pad stops being a keyboard
    pub outage_seconds: u32,
}

/// The three independently-updatable things (§U). The firmware is listed here
/// because the GUI shows all three together, but §U-3 firmware updates take
/// explicit consent every time and are never started by an updater setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateComponent {
    App,
    Tosu,
    Firmware,
}

/// One updater's state, as the GUI shows it (§U-0.4)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentUpdate {
    pub installed: Option<String>,
    pub available: Option<String>,
    pub notes: Option<String>,
    pub enabled: bool,
    /// A newer version exists but this install's files belong to a package
    /// manager, so osu!pad reports it and changes nothing (§U-2a)
    pub notify_only: bool,
    /// An update is downloaded and waiting for the user to press Install
    pub ready_to_install: bool,
}

/// Daemon responses to clients
// `Status` is far larger than the other variants and clippy would rather it
// were boxed. It is built at most a few times a second, immediately serialised
// to JSON and dropped, so the stack size buys nothing, while boxing it would
// churn every `IpcResponse::Status { .. }` pattern in the daemon, GUI and CLI.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    HandshakeAck {
        daemon_version: String,
        daemon_protocol: u32,
        device_connected: bool,
    },
    HandshakeRejected {
        daemon_protocol: u32,
        reason: String,
    },
    Status {
        mode: RuntimeMode,
        device_connected: bool,
        device_info: Option<DeviceInfo>,
        counters: CounterState,
        #[serde(default)]
        counters_source: CounterSource,
        #[serde(default)]
        pc_counters: Option<CounterState>,
        #[serde(default)]
        esp_counters: Option<CounterState>,
        config: DeviceConfig,
        last_sync_time: Option<String>,
        #[serde(default)]
        last_sync_error: Option<String>,
        #[serde(default)]
        storage_error: Option<String>,
        tosu_connected: bool,
        #[serde(default)]
        latency: Option<LatencyStats>,
        #[serde(default)]
        pending_replacement: Option<String>,
        /// A pad owned by another installation, waiting on the user (§W3-3).
        /// Boxed because it is rarely set and `Status` is already the largest
        /// variant of this enum by a wide margin.
        #[serde(default)]
        pending_takeover: Option<Box<TakeoverPrompt>>,
        #[serde(default)]
        incompatible: Option<IncompatibleDevice>,
    },
    ConfigUpdated {
        config: DeviceConfig,
        #[serde(default)]
        deferred_persist: bool,
    },
    OperationDeferred {
        reason: String,
    },
    SyncCompleted {
        success: bool,
        counters: CounterState,
    },
    CountersReset {
        counters: CounterState,
    },
    CountersRestored {
        counters: CounterState,
    },
    BackupExported(JsonBackup),
    ImportPreview {
        current: Option<CurrentBackupState>,
        incoming: JsonBackup,
        device_id_matches: bool,
        is_counter_rollback: bool,
        warnings: Vec<String>,
    },
    BackupImported {
        success: bool,
        counters: CounterState,
        config: DeviceConfig,
    },
    ReadyForFlash {
        port: Option<String>,
    },
    FlashFinished {
        firmware_version: String,
        protocol_version: u32,
        compatible: bool,
    },
    FirmwareUpdateOffer(FirmwareOffer),
    /// The pad came back, and this is what it came back as (§U-3b)
    FirmwareUpdateFinished {
        from: String,
        to: String,
        firmware_version: String,
        running_partition: Option<String>,
        protocol_version: u32,
        compatible: bool,
    },
    LogEntries {
        entries: Vec<LogEntry>,
        latest_seq: u64,
    },
    Layouts {
        idle: Option<Layout>,
        playing: Option<Layout>,
    },
    /// The device applied the layout; `message` carries notes such as "not saved while playing"
    LayoutApplied {
        screen: Screen,
        message: String,
    },
    UiValues(Vec<(u8, SourceValue)>),
    UpdateStatus {
        app: ComponentUpdate,
        tosu: ComponentUpdate,
        /// RFC 3339, or None if no check has ever completed
        last_check: Option<String>,
        #[serde(default)]
        last_error: Option<String>,
        /// The app was replaced on disk; the running processes are the old
        /// ones and must be restarted (§U-2)
        #[serde(default)]
        restart_required: bool,
    },
    /// An update is being applied; the GUI should expect the daemon to go away
    UpdateStarted {
        component: UpdateComponent,
    },
    OperationRejected {
        reason: String,
    },
    Error(String),
}

/// Either end of an IPC connection.
///
/// `IpcStream` and `IpcServerStream` are the same type on Unix and different
/// types on Windows, so the framing helpers are generic over this rather than
/// over one concrete transport (§W0-1).
pub trait IpcTransport: AsyncRead + AsyncWrite + Unpin {}
impl<T: AsyncRead + AsyncWrite + Unpin + ?Sized> IpcTransport for T {}

/// Connects to the daemon and performs the mandatory handshake (§P1-7)
pub async fn connect_and_handshake() -> Result<(IpcStream, IpcResponse), IpcError> {
    connect_and_handshake_at(get_socket_path()).await
}

/// Connects to a specific socket path and performs the handshake
pub async fn connect_and_handshake_at<P: AsRef<Path>>(
    path: P,
) -> Result<(IpcStream, IpcResponse), IpcError> {
    let mut stream = connect(path).await?;

    let handshake = IpcRequest::Handshake {
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        client_protocol: IPC_PROTOCOL_VERSION,
    };
    let resp = send_request(&mut stream, &handshake).await?;
    match &resp {
        IpcResponse::HandshakeAck { .. } => Ok((stream, resp)),
        IpcResponse::HandshakeRejected {
            daemon_protocol,
            reason,
        } => Err(IpcError::Protocol(format!(
            "Handshake rejected (daemon protocol v{}, client v{}): {}",
            daemon_protocol, IPC_PROTOCOL_VERSION, reason
        ))),
        other => Err(IpcError::Protocol(format!(
            "Unexpected handshake response: {:?}",
            other
        ))),
    }
}

/// Sends a request over an IPC connection and waits for the typed response
pub async fn send_request<S: IpcTransport + ?Sized>(
    stream: &mut S,
    req: &IpcRequest,
) -> Result<IpcResponse, IpcError> {
    let payload = serde_json::to_vec(req)?;
    if payload.len() > MAX_REQUEST_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Request frame size {} exceeds {} byte limit",
            payload.len(),
            MAX_REQUEST_FRAME_SIZE
        )));
    }
    let header = (payload.len() as u32).to_le_bytes();

    stream.write_all(&header).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut resp_header = [0u8; 4];
    stream.read_exact(&mut resp_header).await?;
    let resp_len = u32::from_le_bytes(resp_header) as usize;
    if resp_len > MAX_RESPONSE_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Response frame size {} exceeds {} byte limit",
            resp_len, MAX_RESPONSE_FRAME_SIZE
        )));
    }

    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await?;

    let resp: IpcResponse = serde_json::from_slice(&resp_buf)?;
    Ok(resp)
}

/// Reads a request from an active client stream
pub async fn read_request<S: IpcTransport + ?Sized>(
    stream: &mut S,
) -> Result<IpcRequest, IpcError> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_REQUEST_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Request frame size {} exceeds {} byte limit",
            len, MAX_REQUEST_FRAME_SIZE
        )));
    }

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;

    let req: IpcRequest = serde_json::from_slice(&buf)?;
    Ok(req)
}

/// Sends a response to a client stream
pub async fn send_response<S: IpcTransport + ?Sized>(
    stream: &mut S,
    resp: &IpcResponse,
) -> Result<(), IpcError> {
    let payload = serde_json::to_vec(resp)?;
    if payload.len() > MAX_RESPONSE_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Response frame size {} exceeds {} byte limit",
            payload.len(),
            MAX_RESPONSE_FRAME_SIZE
        )));
    }
    let header = (payload.len() as u32).to_le_bytes();

    stream.write_all(&header).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}
