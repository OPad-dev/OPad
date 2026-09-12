use osupad_model::{CounterState, DeviceConfig, DeviceInfo, JsonBackup, RuntimeMode};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub const IPC_PROTOCOL_VERSION: u32 = 1;

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
    ResetCounters,
    ExportBackup,
    ImportBackup(JsonBackup),
    GetLogEntries {
        limit: usize,
    },
    PrepareFlash,
    FinishFlash,
}

/// Daemon responses to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    HandshakeAck {
        daemon_version: String,
        daemon_protocol: u32,
        device_connected: bool,
    },
    Status {
        mode: RuntimeMode,
        device_connected: bool,
        device_info: Option<DeviceInfo>,
        counters: CounterState,
        config: DeviceConfig,
        last_sync_time: Option<String>,
        tosu_connected: bool,
    },
    ConfigUpdated {
        config: DeviceConfig,
    },
    SyncCompleted {
        success: bool,
        counters: CounterState,
    },
    CountersReset {
        counters: CounterState,
    },
    BackupExported(JsonBackup),
    BackupImported {
        success: bool,
        counters: CounterState,
        config: DeviceConfig,
    },
    ReadyForFlash {
        port: Option<String>,
    },
    LogEntries(Vec<String>),
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

/// Creates and binds a UnixListener at the specified socket path
pub fn create_listener<P: AsRef<Path>>(path: P) -> Result<UnixListener, IpcError> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.as_ref().exists() {
        let _ = std::fs::remove_file(path.as_ref());
    }
    let listener = UnixListener::bind(path)?;
    Ok(listener)
}
