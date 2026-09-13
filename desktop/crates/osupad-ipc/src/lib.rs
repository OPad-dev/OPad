use osupad_layout::{Layout, Screen};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice, JsonBackup,
    LatencyStats, LogEntry, RuntimeMode,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub const IPC_PROTOCOL_VERSION: u32 = 1;
pub const MAX_IPC_FRAME_SIZE: usize = 1024 * 1024; // 1 MB cap (§P2-6)

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
    SetLayout { screen: Screen, layout: Layout },
    /// Forget a custom layout and return the device to its default
    ResetLayout { screen: Screen },
    /// Latest UI data values the daemon knows (tosu data), for a live designer preview
    GetUiValues,
}

/// Daemon responses to clients
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
        tosu_connected: bool,
        #[serde(default)]
        latency: Option<LatencyStats>,
        #[serde(default)]
        pending_replacement: Option<String>,
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
    OperationRejected {
        reason: String,
    },
    Error(String),
}

/// Resolves standard socket path in a cross-platform/Linux-friendly manner
pub fn get_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("osupad").join("daemon.sock")
    } else {
        PathBuf::from(format!("/tmp/osupad-{}.sock", std::process::id()))
    }
}

/// Connects to the daemon socket and performs the mandatory handshake (§P1-7)
pub async fn connect_and_handshake() -> Result<(UnixStream, IpcResponse), IpcError> {
    connect_and_handshake_at(get_socket_path()).await
}

/// Connects to a specific socket path and performs the handshake
pub async fn connect_and_handshake_at<P: AsRef<Path>>(path: P) -> Result<(UnixStream, IpcResponse), IpcError> {
    let mut stream = UnixStream::connect(path.as_ref()).await.map_err(|e| {
        IpcError::NotConnected(format!("Failed to connect to {}: {}", path.as_ref().display(), e))
    })?;

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

/// Sends a request over a UnixStream and waits for the typed response
pub async fn send_request(stream: &mut UnixStream, req: &IpcRequest) -> Result<IpcResponse, IpcError> {
    let payload = serde_json::to_vec(req)?;
    let header = (payload.len() as u32).to_le_bytes();

    stream.write_all(&header).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;

    let mut resp_header = [0u8; 4];
    stream.read_exact(&mut resp_header).await?;
    let resp_len = u32::from_le_bytes(resp_header) as usize;
    if resp_len > MAX_IPC_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Response frame size {} exceeds {} byte limit",
            resp_len, MAX_IPC_FRAME_SIZE
        )));
    }

    let mut resp_buf = vec![0u8; resp_len];
    stream.read_exact(&mut resp_buf).await?;

    let resp: IpcResponse = serde_json::from_slice(&resp_buf)?;
    Ok(resp)
}

/// Reads a request from an active client stream
pub async fn read_request(stream: &mut UnixStream) -> Result<IpcRequest, IpcError> {
    let mut header = [0u8; 4];
    stream.read_exact(&mut header).await?;
    let len = u32::from_le_bytes(header) as usize;
    if len > MAX_IPC_FRAME_SIZE {
        return Err(IpcError::Protocol(format!(
            "Request frame size {} exceeds {} byte limit",
            len, MAX_IPC_FRAME_SIZE
        )));
    }

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;

    let req: IpcRequest = serde_json::from_slice(&buf)?;
    Ok(req)
}

/// Sends a response to a client stream
pub async fn send_response(stream: &mut UnixStream, resp: &IpcResponse) -> Result<(), IpcError> {
    let payload = serde_json::to_vec(resp)?;
    let header = (payload.len() as u32).to_le_bytes();

    stream.write_all(&header).await?;
    stream.write_all(&payload).await?;
    stream.flush().await?;
    Ok(())
}

/// Creates and binds a UnixListener at the specified socket path with hardened permissions (§P2-6)
pub fn create_listener<P: AsRef<Path>>(path: P) -> Result<UnixListener, IpcError> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    if path.as_ref().exists() {
        let _ = std::fs::remove_file(path.as_ref());
    }
    let listener = UnixListener::bind(&path)?;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    Ok(listener)
}
