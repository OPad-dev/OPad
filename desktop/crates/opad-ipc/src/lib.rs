use opad_layout::{Layout, Screen};
use opad_model::board_test::BoardTestReport;
use opad_model::tap_rate::{AttemptStats, TapHistory};
use opad_model::ui_source::SourceValue;
use opad_model::{
    CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice, JsonBackup,
    LatencyStats, LogEntry, LogLevel, LogSource, RuntimeMode,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

mod transport;
pub use transport::{
    connect, create_listener, get_socket_path, IpcListener, IpcServerStream, IpcStream,
};
#[cfg(windows)]
pub use transport::{create_private_pipe, pipe_security_sddl};

pub const IPC_PROTOCOL_VERSION: u32 = 1;
pub const MAX_REQUEST_FRAME_SIZE: usize = 1024 * 1024; // 1 MiB cap (§P2-6)
pub const MAX_RESPONSE_FRAME_SIZE: usize = 8 * 1024 * 1024; // 8 MiB cap (§P2-6)

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
    #[error("opad-daemon is already running")]
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
    /// Key 3's lifetime count, while a three-key pad is connected
    #[serde(default)]
    pub lifetime_key3: Option<u64>,
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
        /// Only entries at or above this level, filtered by the daemon before
        /// `limit` applies. Optional both ways: an older daemon ignores it
        /// (the client then filters what it gets), and a request from an
        /// older client has none. Omitted from the JSON when unset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<LogLevel>,
        /// Only entries from this source, like `level`
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<LogSource>,
    },
    PrepareFlash,
    FinishFlash,
    /// Hands the serial port back after PrepareFlash without waiting for the
    /// pad to return as the app: after `opadctl bootloader` (the pad stays in
    /// ROM download mode) or a failed flash. Added after IPC v1 shipped, with
    /// no version bump: an older daemon cannot parse it and closes the
    /// connection, so clients fall back to FinishFlash on a new one.
    ResumeDevice,
    /// Send this app image to the pad over its own connection
    /// (HelloAck.ota_chunk_max): it stays a keyboard and shows its update
    /// animation while it writes the slot it is not running, then restarts
    /// into it on trial. Answered with FlashOverUsbFinished once it is back,
    /// UsbFlashUnavailable when this pad has to be flashed through the ROM
    /// bootloader (nothing was done), or an error. Added after IPC v1 shipped,
    /// with no version bump: an older daemon cannot parse it and closes the
    /// connection, and clients then flash through the bootloader.
    FlashOverUsb {
        image: String,
    },
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
    /// For tosu it updates now, even with its automatic updates switched off.
    InstallUpdate {
        component: UpdateComponent,
    },
    /// Check the update feed now rather than at the daily check. Answered at
    /// once with `UpdateCheckStarted`; `GetUpdateStatus` shows the result.
    /// Added after IPC v1 shipped: an older daemon cannot parse it and closes
    /// the connection, which the client shows as "not available".
    CheckForUpdates,
    /// Follow another update channel, `stable` or `beta`
    /// (docs/specs/release-and-updates.md §9.1). Added after IPC v1 shipped,
    /// like `CheckForUpdates`.
    SetUpdateChannel {
        channel: String,
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
    DetectPin {
        key_id: u32,
        #[serde(default)]
        timeout_ms: u32,
        #[serde(default)]
        exclude_gpio: u32,
    },
    TriggerEasterEgg,
    /// The player's tap rate (KPM): the attempt being played, the last one
    /// that ended, and the history over the configured period. Reads only.
    /// Added after IPC v1 shipped: an older daemon cannot parse it and closes
    /// the connection, which the client shows as "not available".
    GetTapStats,
    /// Set the statistics history period in days (0 = all time) and answer
    /// with `TapStats` for it. Kept in memory at once and saved when storage
    /// writes are allowed again (never during gameplay, P1-3).
    SetTapHistoryPeriod {
        days: u32,
    },
    /// Have the pad test the carrier and input module PCBs (answered with
    /// `BoardTest`, or `Error` without a pad or with firmware predating it).
    /// It reconfigures pins for a moment, so the pad refuses it during a map.
    /// Added after IPC v1 shipped: an older daemon cannot parse it and closes
    /// the connection, which the client shows as "not available".
    RunBoardTest,
    /// Play a result animation on the pad now, as the results screen would
    /// (`opad_model::result_anim` ids; answered with `AnimationPlayed`, or
    /// `Error` without a pad, with firmware predating animations, or during
    /// a map). Added after IPC v1 shipped, like `RunBoardTest`.
    PlayAnimation {
        animation: u32,
        grade: String,
    },
    /// Restart the pad, e.g. after swapping its input module (detected only
    /// at boot). Answered with `PadRestarting` once asked; the pad saves its
    /// counters, restarts when idle and reconnects. `Error` without a pad,
    /// with firmware predating it, or during a map. Added after IPC v1
    /// shipped, like `RunBoardTest`.
    RestartPad,
    /// Start interactive Hall Effect calibration on the connected pad
    StartHeCalibration,
    /// Cancel active Hall Effect calibration
    CancelHeCalibration,
    /// Poll current Hall Effect calibration progress
    GetHeCalibrationProgress,
    /// The live key view: keeps the pad streaming (it stops 2 s after the
    /// last request) and answers with `HeLive`. Poll it only while the view
    /// is on screen.
    GetHeLive,
    /// Keep this connection open and be sent the answers to `watches` as they
    /// change, instead of asking again and again. Answered with `Subscribed`;
    /// from then on the daemon sends the answer to each watched request once
    /// at the start and again whenever it differs from the last one it sent,
    /// looking every `every_ms`. Only requests that read can be watched
    /// (`IpcRequest::can_be_watched`); anything else is refused with `Error`
    /// and the connection goes on as before.
    ///
    /// A subscribed connection carries those answers only: the one request
    /// it still takes is another `Subscribe`, which replaces the watches (an
    /// empty list stops them).
    ///
    /// Added after IPC v1 shipped, with no version bump: an older daemon
    /// cannot parse it and closes the connection, and the client goes back to
    /// asking on a timer (`subscribe` reports that as `Ok(false)`).
    Subscribe {
        watches: Vec<Watch>,
    },
}

/// One request the daemon answers again whenever its answer changes
/// (`IpcRequest::Subscribe`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watch {
    pub request: IpcRequest,
    /// How often the daemon looks for a change, in milliseconds. It never
    /// looks more often than every `MIN_WATCH_INTERVAL_MS`.
    pub every_ms: u32,
}

/// The shortest interval a watch is looked at, whatever it asks for
pub const MIN_WATCH_INTERVAL_MS: u32 = 20;
/// The most watches one `Subscribe` may carry
pub const MAX_WATCHES: usize = 16;

impl IpcRequest {
    /// Whether `Subscribe` accepts this request as a watch: the ones that
    /// only read, and so can be repeated without anyone having asked
    pub fn can_be_watched(&self) -> bool {
        matches!(
            self,
            IpcRequest::GetStatus
                | IpcRequest::GetLogEntries { .. }
                | IpcRequest::GetLayouts
                | IpcRequest::GetUiValues
                | IpcRequest::GetUpdateStatus
                | IpcRequest::GetFirmwareUpdate
                | IpcRequest::GetTapStats
                | IpcRequest::GetHeCalibrationProgress
                | IpcRequest::GetHeLive
        )
    }
}

/// The daemon's tap rate statistics (answer to `GetTapStats`)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TapStatsSnapshot {
    /// The attempt being played, live
    pub current: Option<AttemptStats>,
    /// The most recent attempt that ended (this daemon run)
    pub last: Option<AttemptStats>,
    /// Aggregated over `period_days`; None while storage is unavailable or
    /// not yet read
    pub history: Option<TapHistory>,
    /// 0 = all time
    pub period_days: u32,
    /// Ended attempts waiting for gameplay to stop before they are saved
    pub unsaved_attempts: usize,
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
    /// Set when the newer firmware needs a newer app than this one: the app
    /// is updated first (docs/specs/release-and-updates.md §3.2)
    #[serde(default)]
    pub needs_app: Option<String>,
    /// Automatic firmware updates are on: the update installs by itself once
    /// the pad has gone `auto_after_minutes` unused
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub auto_after_minutes: u32,
    /// How long the pad has gone unused, as far as the daemon knows
    #[serde(default)]
    pub idle_minutes: Option<u32>,
}

/// The three independently-updatable things (§U). The firmware updates by
/// itself only while the pad is unused (§U-3, `FirmwareOffer::auto`).
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
    /// manager, so OPad reports it and changes nothing (§U-2a)
    pub notify_only: bool,
    /// An update is downloaded and waiting for the user to press Install
    pub ready_to_install: bool,
    /// The release page of the available version
    #[serde(default)]
    pub notes_url: Option<String>,
    /// tosu on a package install: its files come with OPad's package, so it
    /// is updated together with OPad, never on its own
    #[serde(default)]
    pub with_app: bool,
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
        /// When the daemon last wrote an automatic counter backup, RFC 3339.
        /// `None` means it never has — not that backups are off.
        #[serde(default)]
        last_backup: Option<String>,
        /// The connected pad belongs to another installation and the daemon is
        /// leaving it alone (§W3-3): it refuses syncs and pushes to it, and
        /// clients should grey those actions out. Stays set after the user
        /// answers "leave it alone", when `pending_takeover` is already gone.
        #[serde(default)]
        foreign_pad: bool,
        /// Hall Effect calibration status from the connected pad
        #[serde(default)]
        he_calibration: Option<opad_model::HeCalibrationStatus>,
        /// Key 3's counters, with a pad that has three keys (Hall Effect V3)
        #[serde(default)]
        key3: Option<opad_model::Key3Counters>,
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
    /// Answer to ResumeDevice: the daemon is looking for the pad again
    DeviceResumed,
    /// Answer to FlashOverUsb: the pad is back, and this is what it runs
    FlashOverUsbFinished {
        firmware_version: String,
        running_partition: Option<String>,
        protocol_version: u32,
        compatible: bool,
    },
    /// Answer to FlashOverUsb: this pad is flashed through the ROM bootloader
    /// (firmware predating the update over USB, or none connected). Nothing
    /// was sent.
    UsbFlashUnavailable {
        reason: String,
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
        /// The app was replaced on disk (or, on Windows, its installer waits
        /// for the next start); the running processes are the old ones and
        /// the new version runs from the next start (§U-2)
        #[serde(default)]
        restart_required: bool,
        /// The version installed but not running yet
        #[serde(default)]
        pending_version: Option<String>,
        /// `stable` or `beta`; empty from a daemon predating channels
        #[serde(default)]
        channel: String,
        /// A check is running right now
        #[serde(default)]
        checking: bool,
        /// How the last install went, as a sentence, kept across restarts
        #[serde(default)]
        last_result: Option<String>,
    },
    /// Answer to CheckForUpdates: the check runs in the background
    UpdateCheckStarted,
    /// An update is being applied; the GUI should expect the daemon to go away
    UpdateStarted {
        component: UpdateComponent,
    },
    OperationRejected {
        reason: String,
    },
    PinDetected {
        key_id: u32,
        gpio: u32,
        success: bool,
    },
    EasterEggTriggered,
    TapStats(TapStatsSnapshot),
    BoardTest(BoardTestReport),
    AnimationPlayed,
    /// The pad was asked to restart (answer to `RestartPad`)
    PadRestarting,
    HeCalibrationStarted,
    HeCalibrationCancelled,
    HeCalibrationProgress(Option<opad_model::HeCalibrationProgress>),
    /// Answer to `GetHeLive`. `supported` is false for MX pads, firmware
    /// predating the live view, or no pad; `sample` is `None` while the pad
    /// sends nothing (a map or its cooldown, or the stream just starting).
    HeLive {
        supported: bool,
        sample: Option<opad_model::HeLiveSample>,
    },
    /// Answer to `Subscribe`: the watches are running, and their answers
    /// follow on this connection
    Subscribed,
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
    write_request(stream, req).await?;
    read_response(stream).await
}

/// Asks the daemon to send the answers to `watches` as they change
/// (`IpcRequest::Subscribe`). `Ok(true)`: read them with `read_response`.
/// `Ok(false)`: the daemon closed the connection instead of answering, which
/// is what one that predates `Subscribe` does; ask on a timer instead.
pub async fn subscribe<S: IpcTransport + ?Sized>(
    stream: &mut S,
    watches: Vec<Watch>,
) -> Result<bool, IpcError> {
    match send_request(stream, &IpcRequest::Subscribe { watches }).await {
        Ok(IpcResponse::Subscribed) => Ok(true),
        Ok(IpcResponse::Error(reason)) => Err(IpcError::Protocol(reason)),
        Ok(other) => Err(IpcError::Protocol(format!(
            "Unexpected answer to Subscribe: {:?}",
            other
        ))),
        Err(IpcError::Io(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Writes one request frame without waiting for its answer
pub async fn write_request<S: AsyncWrite + Unpin + ?Sized>(
    stream: &mut S,
    req: &IpcRequest,
) -> Result<(), IpcError> {
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
    Ok(())
}

/// Reads one response frame: the answer to a request, or the next answer a
/// subscribed connection is sent
pub async fn read_response<S: AsyncRead + Unpin + ?Sized>(
    stream: &mut S,
) -> Result<IpcResponse, IpcError> {
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
pub async fn read_request<S: AsyncRead + Unpin + ?Sized>(
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
pub async fn send_response<S: AsyncWrite + Unpin + ?Sized>(
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
