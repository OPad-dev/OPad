mod chrome;
mod designer;
mod ipc;
mod pages;
#[cfg(target_os = "linux")]
mod platform_linux;
#[cfg(windows)]
mod platform_windows;
mod single_instance;
mod theme;
mod tray;

use iced::widget::{
    button, checkbox, column, container, row, scrollable, stack, text, text_input, Space,
};
use iced::{window, Alignment, Element, Length, Size, Subscription, Task};
use osupad_ipc::{CurrentBackupState, IpcRequest, IpcResponse};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    char_to_hid_usage, CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice,
    JsonBackup, KeyPin, LatencyStats, LogEntry, LogLevel, LogSource, RuntimeMode,
};
use std::collections::HashMap;
use std::time::Duration;

pub fn parse_page_arg() -> Option<Page> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--page" || args[i] == "-p") && i + 1 < args.len() {
            return match args[i + 1].to_lowercase().as_str() {
                "dashboard" => Some(Page::Dashboard),
                "designer" => Some(Page::Designer),
                "settings" => Some(Page::Settings),
                "device" => Some(Page::Device),
                "monitor" => Some(Page::Monitor),
                _ => None,
            };
        }
    }
    None
}

pub fn main() -> iced::Result {
    let target_page = parse_page_arg();
    let page_str = target_page.map(|p| match p {
        Page::Dashboard => "dashboard",
        Page::Designer => "designer",
        Page::Settings => "settings",
        Page::Device => "device",
        Page::Monitor => "monitor",
    });

    if !single_instance::claim(page_str) {
        // Another instance was asked to show its window
        return Ok(());
    }
    let start_hidden = std::env::args().any(|a| a == "--tray");

    // A daemon keeps running with no window open, living in the tray (Discord-style)
    iced::daemon(
        move || App::new(start_hidden, target_page),
        App::update,
        App::view,
    )
    .title(App::title)
    .theme(|_: &App, _| theme::theme())
    .subscription(App::subscription)
    .font(iced_aw::ICED_AW_FONT_BYTES)
    .font(theme::FONT_MEDIUM_BYTES)
    .font(theme::FONT_BOLD_BYTES)
    .default_font(theme::FONT)
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Designer,
    Settings,
    Device,
    Monitor,
}

impl Page {
    const ALL: [(Page, &'static str); 5] = [
        (Page::Dashboard, "Dashboard"),
        (Page::Designer, "Designer"),
        (Page::Settings, "Settings"),
        (Page::Device, "Device"),
        (Page::Monitor, "Monitor"),
    ];
}

#[derive(Debug, Clone)]
pub struct ImportModalState {
    pub current: Option<CurrentBackupState>,
    pub incoming: JsonBackup,
    pub device_id_matches: bool,
    pub is_counter_rollback: bool,
    pub warnings: Vec<String>,
    pub rollback_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    RestoreDeviceFromPc,
    ImportPcFromDevice,
}

#[derive(Debug, Clone)]
pub enum FirmwareModalState {
    Consent {
        consent_text: String,
        from_version: String,
        to_version: String,
    },
    Flashing {
        from_version: String,
        to_version: String,
    },
    Success {
        from: String,
        to: String,
        firmware_version: String,
        running_partition: Option<String>,
    },
    Failed {
        error: String,
    },
}

/// The updater state the Settings page and the update banner render (§U-0.4)
#[derive(Debug, Clone, Default)]
pub struct UpdateView {
    pub app: osupad_ipc::ComponentUpdate,
    pub tosu: osupad_ipc::ComponentUpdate,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub restart_required: bool,
}

pub struct App {
    pub page: Page,
    window: Option<window::Id>,
    pub maximized: bool,
    tray: Option<tray::TrayHandle>,
    tray_available: Option<bool>,

    // Daemon state
    pub daemon_online: bool,
    pub device_connected: bool,
    pub tosu_connected: bool,
    pub mode: RuntimeMode,
    pub device_info: Option<DeviceInfo>,
    pub counters: CounterState,
    pub counters_source: CounterSource,
    pub pc_counters: Option<CounterState>,
    pub esp_counters: Option<CounterState>,
    pub config: DeviceConfig,
    pub last_sync_time: Option<String>,
    pub last_sync_error: Option<String>,
    pub storage_error: Option<String>,
    pub latency: Option<LatencyStats>,
    pub pending_replacement: Option<String>,
    /// A pad paired with another installation (§W3-3)
    pub pending_takeover: Option<osupad_ipc::TakeoverPrompt>,
    /// What each updater knows (§U-0.4)
    pub updates: Option<UpdateView>,
    pub incompatible: Option<IncompatibleDevice>,
    pub reset_modal: Option<String>,
    pub import_modal: Option<ImportModalState>,
    pub recovery_modal: Option<RecoveryAction>,
    pub firmware_offer: Option<osupad_ipc::FirmwareOffer>,
    pub firmware_modal: Option<FirmwareModalState>,
    pub ui_values: HashMap<u8, SourceValue>,

    // Settings form
    pub k1_input: String,
    pub k2_input: String,
    pub k1_gpio: u32,
    pub k2_gpio: u32,
    pub debounce: u32,
    pub brightness: u32,
    pub sleep_seconds: u32,
    pub gameplay_display_hz: u32,
    pub autostart_tray: bool,
    config_loaded: bool,

    pub logs: Vec<LogEntry>,
    pub latest_log_seq: u64,
    pub log_filter_level: Option<LogLevel>,
    pub log_filter_source: Option<LogSource>,
    pub log_cleared_seq: u64,
    pub log_auto_scroll: bool,
    pub banner: Option<String>,
    pub tosu_override_path: Option<std::path::PathBuf>,
    pub designer: designer::Designer,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    Poll,
    Status(Result<IpcResponse, String>),
    UiValues(Result<IpcResponse, String>),
    Logs(Result<IpcResponse, String>),
    FilterLogLevel(Option<LogLevel>),
    FilterLogSource(Option<LogSource>),
    ClearLogs,
    CopyLogs,
    SaveLogs,
    LogsSaved(Result<String, String>),
    ToggleAutoScroll,
    // Backup
    ExportBackup,
    ExportBackupReceived(Result<IpcResponse, String>),
    ExportBackupDone(Result<String, String>),
    StartImportBackup,
    FilePickedForImport(Result<JsonBackup, String>),
    ImportPreviewReady(Result<IpcResponse, String>),
    ToggleImportRollbackConfirm(bool),
    CancelImportModal,
    ConfirmApplyImport,
    ImportCompleted(Result<IpcResponse, String>),
    // Recovery & Flash
    PromptRestoreDeviceFromPc,
    ConfirmRestoreDeviceFromPc,
    PromptImportPcFromDevice,
    ConfirmImportPcFromDevice,
    CancelRecoveryModal,
    RecoveryCompleted(Result<IpcResponse, String>),
    // Firmware Update (§U-3b)
    FirmwareOffer(Result<IpcResponse, String>),
    PromptFirmwareConsent,
    CancelFirmwareModal,
    ConfirmFirmwareUpdate,
    FirmwareUpdateResult(Result<IpcResponse, String>),
    // Daemon offline recovery
    StartDaemon,
    DaemonStarted(Result<(), String>),
    InstallSystemdService,
    SystemdServiceInstalled(Result<(), String>),
    // Settings
    Key1(String),
    Key2(String),
    Key1Pin(KeyPin),
    Key2Pin(KeyPin),
    Debounce(u32),
    Brightness(u32),
    SleepSeconds(u32),
    GameplayDisplayHz(u32),
    ToggleAutostartTray(bool),
    SaveConfig,
    // Actions
    Sync,
    PromptResetCounters,
    ResetModalInput(String),
    CancelResetModal,
    ConfirmResetCounters,
    ResolveReplacement(bool),
    /// (take over, keep the pad's counters rather than this PC's)
    ResolveTakeover(bool, bool),
    UpdateStatus(Result<IpcResponse, String>),
    ToggleUpdater(osupad_ipc::UpdateComponent, bool),
    InstallUpdate(osupad_ipc::UpdateComponent),
    ResetLatency,
    ActionDone(Result<IpcResponse, String>),
    DismissBanner,
    // tosu configuration (§T-4)
    PickTosuPath,
    TosuPathPicked(Option<std::path::PathBuf>),
    ResetTosuPath,
    Designer(designer::Message),
    // Window & tray
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    Tray(tray::TrayEvent),
    ShowRequested(Option<String>),
    Window(chrome::WindowAction),
    Resized,
    Maximized(bool),
}

impl App {
    fn new(start_hidden: bool, initial_page: Option<Page>) -> (Self, Task<Message>) {
        let (designer, designer_task) = designer::Designer::new();
        let tosu_override_path = osupad_model::paths::data_dir()
            .ok()
            .and_then(|d| std::fs::read_to_string(d.join("tosu_path")).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("OSUPAD_TOSU_PATH").map(std::path::PathBuf::from));

        if let Some(ref p) = tosu_override_path {
            std::env::set_var("OSUPAD_TOSU_PATH", p);
        }

        let mut app = App {
            page: initial_page.unwrap_or(Page::Dashboard),
            window: None,
            maximized: false,
            tray: None,
            tray_available: None,
            daemon_online: false,
            device_connected: false,
            tosu_connected: false,
            mode: RuntimeMode::Idle,
            device_info: None,
            counters: CounterState::default(),
            counters_source: CounterSource::Pc,
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_sync_error: None,
            storage_error: None,
            latency: None,
            pending_replacement: None,
            pending_takeover: None,
            updates: None,
            incompatible: None,
            reset_modal: None,
            import_modal: None,
            recovery_modal: None,
            firmware_offer: None,
            firmware_modal: None,
            pc_counters: None,
            esp_counters: None,
            ui_values: HashMap::new(),
            k1_input: "Z".into(),
            k2_input: "X".into(),
            k1_gpio: osupad_model::DEFAULT_KEY1_GPIO,
            k2_gpio: osupad_model::DEFAULT_KEY2_GPIO,
            debounce: 3000,
            brightness: 100,
            sleep_seconds: 600,
            gameplay_display_hz: 10,
            #[cfg(target_os = "linux")]
            autostart_tray: platform_linux::is_gui_autostart_enabled(),
            #[cfg(windows)]
            autostart_tray: platform_windows::is_gui_autostart_enabled(),
            #[cfg(not(any(target_os = "linux", windows)))]
            autostart_tray: false,
            config_loaded: false,
            logs: Vec::new(),
            latest_log_seq: 0,
            log_filter_level: None,
            log_filter_source: None,
            log_cleared_seq: 0,
            log_auto_scroll: true,
            banner: None,
            tosu_override_path,
            designer,
        };
        let mut tasks = vec![designer_task.map(Message::Designer), app.poll()];
        if !start_hidden {
            tasks.push(app.open_window());
        }
        (app, Task::batch(tasks))
    }

    fn title(&self, _window: window::Id) -> String {
        "osu!pad".to_string()
    }

    fn open_window(&mut self) -> Task<Message> {
        if let Some(id) = self.window {
            return window::gain_focus(id);
        }
        let (id, open) = window::open(window::Settings {
            size: Size::new(1360.0, 860.0),
            min_size: Some(Size::new(1100.0, 700.0)),
            position: window::Position::Centered,
            exit_on_close_request: false,
            // Our own title bar and resize edges (chrome.rs)
            decorations: false,
            platform_specific: window::settings::PlatformSpecific {
                application_id: "osupad".to_string(),
                ..Default::default()
            },
            ..Default::default()
        });
        self.window = Some(id);
        open.map(Message::WindowOpened)
    }

    fn poll(&self) -> Task<Message> {
        let mut tasks = vec![
            Task::perform(ipc::request(IpcRequest::GetStatus), Message::Status),
            Task::perform(ipc::request(IpcRequest::GetUiValues), Message::UiValues),
            Task::perform(
                ipc::request(IpcRequest::GetUpdateStatus),
                Message::UpdateStatus,
            ),
            Task::perform(
                ipc::request(IpcRequest::GetFirmwareUpdate),
                Message::FirmwareOffer,
            ),
        ];
        if self.page == Page::Monitor && self.window.is_some() {
            let since_seq = if self.latest_log_seq > 0 {
                Some(self.latest_log_seq)
            } else {
                None
            };
            tasks.push(Task::perform(
                ipc::request(IpcRequest::GetLogEntries {
                    since_seq,
                    limit: 200,
                }),
                Message::Logs,
            ));
        }
        Task::batch(tasks)
    }

    fn update_tray(&self) {
        if let Some(handle) = &self.tray {
            let is_playing_or_cooldown =
                matches!(self.mode, RuntimeMode::Playing | RuntimeMode::Cooldown);
            let k1 = if let Some(pc) = &self.pc_counters {
                pc.lifetime_key1
            } else {
                self.counters.lifetime_key1
            };
            let k2 = if let Some(pc) = &self.pc_counters {
                pc.lifetime_key2
            } else {
                self.counters.lifetime_key2
            };
            tray::update(
                handle,
                tray::TrayStatus {
                    daemon_online: self.daemon_online,
                    device_connected: self.device_connected,
                    incompatible: self.incompatible.is_some(),
                    firmware_version: self
                        .device_info
                        .as_ref()
                        .map(|i| i.firmware_version.clone())
                        .filter(|v| !v.is_empty()),
                    key1_presses: k1,
                    key2_presses: k2,
                    last_sync_time: self.last_sync_time.clone(),
                    last_sync_error: self.last_sync_error.clone(),
                    is_playing_or_cooldown,
                    tosu_connected: self.tosu_connected,
                    total_presses: self.counters.total_lifetime_presses(),
                    mode: format!("{:?}", self.mode),
                },
            );
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => {
                self.page = page;
                return self.poll();
            }
            Message::Poll => return self.poll(),
            Message::Status(result) => {
                match result {
                    Ok(IpcResponse::Status {
                        mode,
                        device_connected,
                        device_info,
                        counters,
                        counters_source,
                        pc_counters,
                        esp_counters,
                        config,
                        last_sync_time,
                        last_sync_error,
                        storage_error,
                        tosu_connected,
                        latency,
                        pending_replacement,
                        incompatible,
                        pending_takeover,
                    }) => {
                        self.daemon_online = true;
                        self.mode = mode;
                        self.device_connected = device_connected;
                        self.device_info = device_info;
                        self.counters = counters;
                        self.counters_source = counters_source;
                        self.pc_counters = pc_counters;
                        self.esp_counters = esp_counters;
                        self.last_sync_error = last_sync_error;
                        self.storage_error = storage_error;
                        self.pending_replacement = pending_replacement;
                        self.pending_takeover = pending_takeover.map(|t| *t);
                        self.incompatible = incompatible;
                        self.tosu_connected = tosu_connected;
                        self.latency = latency;
                        self.last_sync_time = last_sync_time;
                        // Refresh the form only when the saved config changed, so polling
                        // never discards unsaved edits
                        if !self.config_loaded || self.config != config {
                            self.config_loaded = true;
                            self.k1_input = config.key1_char();
                            self.k2_input = config.key2_char();
                            self.k1_gpio = config.key1_gpio;
                            self.k2_gpio = config.key2_gpio;
                            self.debounce = config.debounce_us;
                            self.brightness = config.brightness;
                            self.sleep_seconds = config.display_sleep_seconds;
                            self.gameplay_display_hz = config.gameplay_display_hz;
                        }
                        self.config = config;
                    }
                    _ => {
                        self.daemon_online = false;
                        self.device_connected = false;
                        self.tosu_connected = false;
                    }
                }
                self.update_tray();
            }
            Message::UiValues(result) => {
                if let Ok(IpcResponse::UiValues(values)) = result {
                    self.ui_values = values.into_iter().collect();
                }
            }
            Message::Logs(result) => {
                if let Ok(IpcResponse::LogEntries {
                    entries,
                    latest_seq,
                }) = result
                {
                    if self.latest_log_seq == 0 {
                        self.logs = entries;
                    } else {
                        for entry in entries {
                            if !self.logs.iter().any(|e| e.seq == entry.seq) {
                                self.logs.push(entry);
                            }
                        }
                        if self.logs.len() > 2000 {
                            let excess = self.logs.len() - 2000;
                            self.logs.drain(0..excess);
                        }
                    }
                    if latest_seq > 0 {
                        self.latest_log_seq = latest_seq;
                    }
                }
            }
            Message::FilterLogLevel(lvl) => {
                self.log_filter_level = lvl;
            }
            Message::FilterLogSource(src) => {
                self.log_filter_source = src;
            }
            Message::ClearLogs => {
                self.log_cleared_seq = self.latest_log_seq;
            }
            Message::CopyLogs => {
                let text = self.formatted_visible_logs().join("\n");
                return iced::clipboard::write(text);
            }
            Message::SaveLogs => {
                let text = self.formatted_visible_logs().join("\n");
                return Task::perform(save_logs_dialog(text), Message::LogsSaved);
            }
            Message::LogsSaved(res) => match res {
                Ok(path) => self.banner = Some(format!("Saved log to {}", path)),
                Err(e) if e.is_empty() => {}
                Err(e) => self.banner = Some(format!("Failed to save log: {}", e)),
            },
            Message::ToggleAutoScroll => {
                self.log_auto_scroll = !self.log_auto_scroll;
            }
            Message::ExportBackup => {
                if matches!(self.mode, RuntimeMode::Playing | RuntimeMode::Cooldown) {
                    self.banner = Some("Cannot export backup during gameplay or cooldown.".into());
                    return Task::none();
                }
                return Task::perform(
                    ipc::request(IpcRequest::ExportBackup),
                    Message::ExportBackupReceived,
                );
            }
            Message::ExportBackupReceived(result) => match result {
                Ok(IpcResponse::BackupExported(backup)) => {
                    return Task::perform(export_backup_dialog(backup), Message::ExportBackupDone);
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.banner = Some(format!("Export rejected: {}", reason));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.banner = Some(format!("Export error: {}", e));
                }
                _ => {}
            },
            Message::ExportBackupDone(result) => match result {
                Ok(path) => self.banner = Some(format!("Backup exported to {}", path)),
                Err(e) if e.is_empty() => {}
                Err(e) => self.banner = Some(format!("Export failed: {}", e)),
            },
            Message::StartImportBackup => {
                if matches!(self.mode, RuntimeMode::Playing | RuntimeMode::Cooldown) {
                    self.banner = Some("Cannot import backup during gameplay or cooldown.".into());
                    return Task::none();
                }
                return Task::perform(pick_backup_dialog(), Message::FilePickedForImport);
            }
            Message::FilePickedForImport(result) => match result {
                Ok(backup) => {
                    return Task::perform(
                        ipc::request(IpcRequest::PreviewImport(backup)),
                        Message::ImportPreviewReady,
                    );
                }
                Err(e) if e.is_empty() => {}
                Err(e) => self.banner = Some(e),
            },
            Message::ImportPreviewReady(result) => match result {
                Ok(IpcResponse::ImportPreview {
                    current,
                    incoming,
                    device_id_matches,
                    is_counter_rollback,
                    warnings,
                }) => {
                    self.import_modal = Some(ImportModalState {
                        current,
                        incoming,
                        device_id_matches,
                        is_counter_rollback,
                        warnings,
                        rollback_confirmed: false,
                    });
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.banner = Some(format!("Import rejected: {}", reason));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.banner = Some(format!("Preview failed: {}", e));
                }
                _ => {}
            },
            Message::ToggleImportRollbackConfirm(confirmed) => {
                if let Some(modal) = &mut self.import_modal {
                    modal.rollback_confirmed = confirmed;
                }
            }
            Message::CancelImportModal => {
                self.import_modal = None;
            }
            Message::ConfirmApplyImport => {
                if let Some(modal) = self.import_modal.take() {
                    let backup = modal.incoming;
                    return Task::perform(
                        ipc::request(IpcRequest::ImportBackup {
                            backup,
                            confirm: true,
                        }),
                        Message::ImportCompleted,
                    );
                }
            }
            Message::ImportCompleted(result) => match result {
                Ok(IpcResponse::BackupImported {
                    success: true,
                    counters,
                    config,
                }) => {
                    self.counters = counters.clone();
                    self.config = config.clone();
                    self.k1_input = config.key1_char();
                    self.k2_input = config.key2_char();
                    self.k1_gpio = config.key1_gpio;
                    self.k2_gpio = config.key2_gpio;
                    self.debounce = config.debounce_us;
                    self.brightness = config.brightness;
                    self.sleep_seconds = config.display_sleep_seconds;
                    self.banner = Some(format!(
                        "Backup successfully imported and synced! (generation: {})",
                        counters.counter_generation
                    ));
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.banner = Some(format!("Import rejected: {}", reason));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.banner = Some(format!("Import failed: {}", e));
                }
                _ => {}
            },
            Message::PromptRestoreDeviceFromPc => {
                self.recovery_modal = Some(RecoveryAction::RestoreDeviceFromPc);
            }
            Message::ConfirmRestoreDeviceFromPc => {
                self.recovery_modal = None;
                self.banner = Some("Restoring pad counters from PC database...".into());
                return Task::perform(
                    ipc::request(IpcRequest::RestoreDeviceFromPc { confirm: true }),
                    Message::RecoveryCompleted,
                );
            }
            Message::PromptImportPcFromDevice => {
                self.recovery_modal = Some(RecoveryAction::ImportPcFromDevice);
            }
            Message::ConfirmImportPcFromDevice => {
                self.recovery_modal = None;
                self.banner = Some("Importing PC database counters from pad...".into());
                return Task::perform(
                    ipc::request(IpcRequest::ImportPcFromDevice { confirm: true }),
                    Message::RecoveryCompleted,
                );
            }
            Message::CancelRecoveryModal => {
                self.recovery_modal = None;
            }
            Message::RecoveryCompleted(result) => match result {
                Ok(IpcResponse::CountersRestored { counters }) => {
                    self.counters = counters.clone();
                    self.pc_counters = Some(counters.clone());
                    self.esp_counters = Some(counters.clone());
                    self.banner = Some(format!(
                        "Counters synchronized! New generation: {}",
                        counters.counter_generation
                    ));
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.banner = Some(format!("Operation rejected: {}", reason));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.banner = Some(format!("Operation failed: {}", e));
                }
                _ => {}
            },
            Message::FirmwareOffer(res) => {
                if let Ok(IpcResponse::FirmwareUpdateOffer(offer)) = res {
                    self.firmware_offer = Some(offer);
                }
            }
            Message::PromptFirmwareConsent => {
                if let Some(offer) = &self.firmware_offer {
                    if let (Some(consent_text), Some(available)) =
                        (&offer.consent_text, &offer.available)
                    {
                        let from = offer.installed.clone().unwrap_or_else(|| "unknown".into());
                        self.firmware_modal = Some(FirmwareModalState::Consent {
                            consent_text: consent_text.clone(),
                            from_version: from,
                            to_version: available.clone(),
                        });
                    }
                }
            }
            Message::CancelFirmwareModal => {
                self.firmware_modal = None;
                return self.poll();
            }
            Message::ConfirmFirmwareUpdate => {
                if let Some(FirmwareModalState::Consent {
                    from_version,
                    to_version,
                    ..
                }) = &self.firmware_modal
                {
                    let from = from_version.clone();
                    let to = to_version.clone();
                    self.firmware_modal = Some(FirmwareModalState::Flashing {
                        from_version: from,
                        to_version: to,
                    });
                    return Task::perform(
                        ipc::request(IpcRequest::InstallFirmwareUpdate { confirm: true }),
                        Message::FirmwareUpdateResult,
                    );
                }
            }
            Message::FirmwareUpdateResult(res) => match res {
                Ok(IpcResponse::FirmwareUpdateFinished {
                    from,
                    to,
                    firmware_version,
                    running_partition,
                    ..
                }) => {
                    self.firmware_modal = Some(FirmwareModalState::Success {
                        from,
                        to,
                        firmware_version,
                        running_partition,
                    });
                    return self.poll();
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.firmware_modal = Some(FirmwareModalState::Failed {
                        error: format!("Update rejected: {}", reason),
                    });
                    return self.poll();
                }
                Ok(IpcResponse::Error(err)) => {
                    self.firmware_modal = Some(FirmwareModalState::Failed { error: err });
                    return self.poll();
                }
                Err(err) => {
                    self.firmware_modal = Some(FirmwareModalState::Failed {
                        error: format!("Communication error: {}", err),
                    });
                    return self.poll();
                }
                Ok(other) => {
                    self.firmware_modal = Some(FirmwareModalState::Failed {
                        error: format!("Unexpected response from daemon: {:?}", other),
                    });
                    return self.poll();
                }
            },
            Message::StartDaemon => {
                self.banner = Some("Starting osupad-daemon...".into());
                return Task::perform(start_daemon_process(), Message::DaemonStarted);
            }
            Message::DaemonStarted(res) => match res {
                Ok(()) => {
                    self.banner = Some("Launched osupad-daemon. Connecting...".into());
                    return self.poll();
                }
                Err(e) => {
                    self.banner = Some(format!("Failed to start daemon: {}", e));
                }
            },
            Message::InstallSystemdService => {
                #[cfg(target_os = "linux")]
                {
                    let bin = find_daemon_executable();
                    self.banner = Some("Installing systemd user service...".into());
                    return Task::perform(
                        async move { platform_linux::install_systemd_user_service(&bin) },
                        Message::SystemdServiceInstalled,
                    );
                }
                #[cfg(windows)]
                {
                    let bin = find_daemon_executable();
                    self.banner = Some("Registering the daemon to start at login...".into());
                    return Task::perform(
                        async move { platform_windows::install_daemon_autostart(&bin) },
                        Message::SystemdServiceInstalled,
                    );
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    self.banner = Some("Daemon autostart is not supported on this platform".into());
                }
            }
            Message::SystemdServiceInstalled(res) => match res {
                Ok(()) => {
                    #[cfg(windows)]
                    {
                        self.banner =
                            Some("Daemon registered to start at login and started!".into());
                    }
                    #[cfg(not(windows))]
                    {
                        self.banner = Some("systemd service installed and started!".into());
                    }
                    return self.poll();
                }
                Err(e) => {
                    self.banner = Some(format!("Failed to install service: {}", e));
                }
            },
            Message::Key1(s) => {
                self.k1_input = s.chars().take(1).collect::<String>().to_uppercase()
            }
            Message::Key2(s) => {
                self.k2_input = s.chars().take(1).collect::<String>().to_uppercase()
            }
            Message::Key1Pin(pin) => self.k1_gpio = pin.gpio,
            Message::Key2Pin(pin) => self.k2_gpio = pin.gpio,
            Message::Debounce(v) => self.debounce = v,
            Message::Brightness(v) => self.brightness = v,
            Message::SleepSeconds(v) => self.sleep_seconds = v,
            Message::GameplayDisplayHz(v) => self.gameplay_display_hz = v,
            Message::ToggleAutostartTray(enabled) => {
                self.autostart_tray = enabled;
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = platform_linux::set_gui_autostart_enabled(enabled) {
                        self.banner = Some(format!("Failed to update autostart: {}", e));
                    }
                }
                #[cfg(windows)]
                {
                    if let Err(e) = platform_windows::set_gui_autostart_enabled(enabled) {
                        self.banner = Some(format!("Failed to update autostart: {}", e));
                    }
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    let _ = enabled;
                }
            }
            Message::SaveConfig => {
                let config = DeviceConfig {
                    key1_hid_usage: char_to_hid_usage(&self.k1_input)
                        .unwrap_or(self.config.key1_hid_usage),
                    key2_hid_usage: char_to_hid_usage(&self.k2_input)
                        .unwrap_or(self.config.key2_hid_usage),
                    debounce_us: self.debounce,
                    key1_gpio: self.k1_gpio,
                    key2_gpio: self.k2_gpio,
                    brightness: self.brightness,
                    display_sleep_seconds: self.sleep_seconds,
                    gameplay_display_hz: self.gameplay_display_hz,
                    ..self.config.clone()
                };
                self.banner = Some("Saving settings...".into());
                return Task::perform(
                    ipc::request(IpcRequest::UpdateConfig(config)),
                    Message::ActionDone,
                );
            }
            Message::Sync => {
                self.banner = Some("Syncing counters and clock with the pad...".into());
                return Task::perform(ipc::request(IpcRequest::ForceSync), Message::ActionDone);
            }
            Message::PromptResetCounters => {
                self.reset_modal = Some(String::new());
            }
            Message::ResetModalInput(input) => {
                self.reset_modal = Some(input);
            }
            Message::CancelResetModal => {
                self.reset_modal = None;
            }
            Message::ConfirmResetCounters => {
                self.reset_modal = None;
                self.banner = Some("Resetting lifetime counters...".into());
                return Task::perform(
                    ipc::request(IpcRequest::ResetCounters { confirm: true }),
                    Message::ActionDone,
                );
            }
            Message::ResolveReplacement(restore) => {
                self.banner = Some(if restore {
                    "Restoring counters from previous pad...".into()
                } else {
                    "Adopting new pad...".into()
                });
                return Task::perform(
                    ipc::request(IpcRequest::ResolveReplacement { restore }),
                    Message::ActionDone,
                );
            }
            Message::ResolveTakeover(take_over, keep_device_counters) => {
                self.banner = Some(if !take_over {
                    "Leaving the pad paired with its other installation...".into()
                } else if keep_device_counters {
                    "Taking over the pad, keeping its counters...".into()
                } else {
                    "Taking over the pad, using this PC's counters...".into()
                });
                return Task::perform(
                    ipc::request(IpcRequest::ResolveTakeover {
                        take_over,
                        keep_device_counters,
                    }),
                    Message::ActionDone,
                );
            }
            Message::UpdateStatus(res) => {
                // A daemon too old to answer simply leaves the panel empty;
                // that is not worth a banner.
                if let Ok(IpcResponse::UpdateStatus {
                    app,
                    tosu,
                    last_check,
                    last_error,
                    restart_required,
                }) = res
                {
                    self.updates = Some(UpdateView {
                        app,
                        tosu,
                        last_check,
                        last_error,
                        restart_required,
                    });
                }
            }
            Message::ToggleUpdater(component, enabled) => {
                // Reflect it now; the next poll confirms what the daemon saved
                if let Some(u) = &mut self.updates {
                    match component {
                        osupad_ipc::UpdateComponent::App => u.app.enabled = enabled,
                        osupad_ipc::UpdateComponent::Tosu => u.tosu.enabled = enabled,
                        osupad_ipc::UpdateComponent::Firmware => {}
                    }
                }
                return Task::perform(
                    ipc::request(IpcRequest::SetUpdateEnabled { component, enabled }),
                    Message::ActionDone,
                );
            }
            Message::InstallUpdate(component) => {
                self.banner = Some("Installing the update...".into());
                return Task::perform(
                    ipc::request(IpcRequest::InstallUpdate { component }),
                    Message::ActionDone,
                );
            }
            Message::ResetLatency => {
                return Task::perform(
                    ipc::request(IpcRequest::ResetLatencyStats),
                    Message::ActionDone,
                );
            }
            Message::PickTosuPath => {
                return Task::perform(
                    async {
                        let file = rfd::AsyncFileDialog::new()
                            .set_title("Select tosu Executable")
                            .pick_file()
                            .await;
                        file.map(|f| f.path().to_path_buf())
                    },
                    Message::TosuPathPicked,
                );
            }
            Message::TosuPathPicked(path) => {
                if let Some(p) = path {
                    if let Ok(d) = osupad_model::paths::data_dir() {
                        let _ = std::fs::create_dir_all(&d);
                        let _ = std::fs::write(d.join("tosu_path"), p.to_string_lossy().as_bytes());
                    }
                    std::env::set_var("OSUPAD_TOSU_PATH", &p);
                    self.banner = Some(format!("Using external tosu: {}", p.display()));
                    self.tosu_override_path = Some(p);
                }
            }
            Message::ResetTosuPath => {
                if let Ok(d) = osupad_model::paths::data_dir() {
                    let _ = std::fs::remove_file(d.join("tosu_path"));
                }
                std::env::remove_var("OSUPAD_TOSU_PATH");
                self.tosu_override_path = None;
                self.banner = Some("Reset to bundled tosu".into());
            }
            Message::ActionDone(result) => {
                self.banner = Some(match result {
                    Ok(IpcResponse::ConfigUpdated { .. }) => {
                        "Settings saved and sent to the pad".into()
                    }
                    Ok(IpcResponse::SyncCompleted { success: true, .. }) => "Pad synced".into(),
                    Ok(IpcResponse::CountersReset { .. }) => "Lifetime counters reset".into(),
                    Ok(IpcResponse::HandshakeAck { .. }) => "Latency statistics reset".into(),
                    Ok(IpcResponse::OperationRejected { reason }) => {
                        format!("Not possible right now: {}", reason)
                    }
                    Ok(IpcResponse::Error(e)) | Err(e) => format!("Error: {}", e),
                    Ok(other) => format!("Unexpected response: {:?}", other),
                });
                return self.poll();
            }
            Message::DismissBanner => self.banner = None,
            Message::Designer(msg) => return self.designer.update(msg).map(Message::Designer),

            Message::WindowOpened(_) => {}
            Message::CloseRequested(id) => {
                if self.tray_available == Some(false) {
                    // Nowhere to live without a window
                    return iced::exit();
                }
                self.window = None;
                return window::close(id);
            }
            Message::ShowRequested(target) => {
                if let Some(target_str) = target {
                    match target_str.to_lowercase().as_str() {
                        "monitor" => self.page = Page::Monitor,
                        "device" => self.page = Page::Device,
                        "settings" => self.page = Page::Settings,
                        "designer" => self.page = Page::Designer,
                        "dashboard" => self.page = Page::Dashboard,
                        _ => {}
                    }
                }
                return self.open_window();
            }
            Message::Window(action) => {
                let Some(id) = self.window else {
                    return Task::none();
                };
                return match action {
                    chrome::WindowAction::Drag => window::drag(id),
                    chrome::WindowAction::ToggleMaximize => window::toggle_maximize(id),
                    chrome::WindowAction::Minimize => window::minimize(id, true),
                    chrome::WindowAction::Close => self.update(Message::CloseRequested(id)),
                    chrome::WindowAction::Resize(direction) if !self.maximized => {
                        window::drag_resize(id, direction)
                    }
                    chrome::WindowAction::Resize(_) => Task::none(),
                };
            }
            Message::Resized => {
                if let Some(id) = self.window {
                    return window::is_maximized(id).map(Message::Maximized);
                }
            }
            Message::Maximized(maximized) => self.maximized = maximized,
            Message::Tray(event) => match event {
                tray::TrayEvent::Started(handle) => {
                    self.tray = Some(handle);
                    self.tray_available = Some(true);
                    self.update_tray();
                }
                tray::TrayEvent::Unavailable => {
                    self.tray_available = Some(false);
                    if self.banner.is_none() {
                        self.banner = Some("No system tray found; the app will quit when closed. The pad keeps working.".into());
                    }
                    if self.window.is_none() {
                        return self.open_window();
                    }
                }
                tray::TrayEvent::Action(tray::TrayAction::ShowWindow) => return self.open_window(),
                tray::TrayEvent::Action(tray::TrayAction::OpenMonitor) => {
                    self.page = Page::Monitor;
                    return self.open_window();
                }
                tray::TrayEvent::Action(tray::TrayAction::StartDaemon) => {
                    return self.update(Message::StartDaemon)
                }
                tray::TrayEvent::Action(tray::TrayAction::SyncNow) => {
                    return self.update(Message::Sync)
                }
                tray::TrayEvent::Action(tray::TrayAction::Quit) => return iced::exit(),
            },
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::time::every(Duration::from_millis(if !self.daemon_online {
                2000
            } else if self.window.is_some() {
                1000
            } else {
                3000
            }))
            .map(|_| Message::Poll),
            window::close_requests().map(Message::CloseRequested),
            window::resize_events().map(|_| Message::Resized),
            Subscription::run(tray::stream).map(Message::Tray),
            Subscription::run(single_instance::show_requests).map(Message::ShowRequested),
        ];
        if self.window.is_some() && self.page == Page::Designer {
            subscriptions.push(self.designer.subscription().map(Message::Designer));
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let nav = column(Page::ALL.iter().map(|(page, label)| {
            button(text(*label).size(15))
                .width(Length::Fill)
                .padding([10, 14])
                .style(theme::nav(self.page == *page))
                .on_press(Message::Navigate(*page))
                .into()
        }))
        .spacing(6);

        let status_line = |on: bool, label: &'static str, state: &'static str| {
            row![
                container(Space::new().width(10).height(10)).style(theme::dot(on)),
                text(label).size(13),
                Space::new().width(Length::Fill),
                theme::caption(state),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        let sidebar = container(
            column![
                nav,
                Space::new().height(Length::Fill),
                status_line(
                    self.device_connected,
                    "Pad",
                    if self.device_connected {
                        "connected"
                    } else {
                        "offline"
                    }
                ),
                status_line(
                    self.tosu_connected,
                    "tosu",
                    if self.tosu_connected {
                        "connected"
                    } else {
                        "offline"
                    }
                ),
                status_line(
                    self.daemon_online,
                    "Daemon",
                    if self.daemon_online {
                        "running"
                    } else {
                        "offline"
                    }
                ),
                Space::new().height(4),
                theme::caption("Close the window to keep osu!pad in the tray"),
            ]
            .spacing(8)
            .padding(18),
        )
        .width(220)
        .height(Length::Fill)
        .style(theme::sidebar);

        let page: Element<'_, Message> = match self.page {
            Page::Dashboard => pages::dashboard(self),
            Page::Designer => self.designer.view().map(Message::Designer),
            Page::Settings => pages::settings(self),
            Page::Device => pages::device(self),
            Page::Monitor => pages::monitor(self),
        };

        let mut main = column![]
            .spacing(14)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill);

        if !self.daemon_online {
            let mut offline_actions = row![button(text("Start daemon").size(12))
                .padding([6, 12])
                .style(theme::primary)
                .on_press(Message::StartDaemon),]
            .spacing(8)
            .align_y(Alignment::Center);

            #[cfg(target_os = "linux")]
            {
                offline_actions = offline_actions.push(
                    button(text("Install user service").size(12))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::InstallSystemdService),
                );
            }
            // Same button, same message; on Windows it writes the Run value
            #[cfg(windows)]
            if !platform_windows::daemon_autostart_installed() {
                offline_actions = offline_actions.push(
                    button(text("Start daemon at login").size(12))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::InstallSystemdService),
                );
            }

            main = main.push(
                container(
                    row![
                        text("⚠ osupad-daemon is offline. Communication, synchronization, and persistence are paused.")
                            .size(13)
                            .color(theme::YELLOW),
                        Space::new().width(Length::Fill),
                        offline_actions,
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding([10, 16])
                .style(theme::banner),
            );
        }
        if let Some(err) = &self.storage_error {
            main = main.push(
                container(
                    row![
                        text(format!("⚠ Database offline: {}. Desktop persistence, backup restore, and PC counter authority are paused.", err))
                            .size(13)
                            .color(theme::RED),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([10, 16])
                .style(theme::banner),
            );
        }
        if let Some(incompat) = &self.incompatible {
            main = main.push(
                container(
                    row![
                        text(format!("⚠ Incompatible device protocol (device: {}, required: {}). Update firmware.", incompat.protocol_version, osupad_ipc::IPC_PROTOCOL_VERSION)).size(14).color(theme::YELLOW),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if let Some(old_id) = &self.pending_replacement {
            main = main.push(
                container(
                    row![
                        text(format!(
                            "This looks like a new pad. Restore counters from {}?",
                            old_id
                        ))
                        .size(14),
                        Space::new().width(Length::Fill),
                        button(text("Restore from previous pad").size(12))
                            .style(theme::primary)
                            .on_press(Message::ResolveReplacement(true)),
                        Space::new().width(8),
                        button(text("Treat as new pad").size(12))
                            .style(theme::secondary)
                            .on_press(Message::ResolveReplacement(false)),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if self.updates.as_ref().is_some_and(|u| u.restart_required) {
            // §U-2: the files on disk are the new version but these processes
            // are still the old ones. The daemon's handshake already refuses a
            // mismatched client, so say plainly what is needed rather than
            // letting the app look broken.
            main = main.push(
                container(
                    row![text(
                        "osu!pad was updated. Restart the app and the daemon to \
                             finish — the pad keeps working as a keyboard meanwhile."
                    )
                    .size(14),]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if let Some(t) = &self.pending_takeover {
            // §W3-3: friction-light on purpose. The only other way out of this
            // is a full reflash, so the wording points at that too rather than
            // leaving anyone stuck.
            main = main.push(
                container(
                    column![
                        text("This osu!pad is paired with another installation.").size(14),
                        text(format!(
                            "Its counters: {} / {}   ·   this PC's: {} / {}",
                            pages::grouped(t.device_key1),
                            pages::grouped(t.device_key2),
                            pages::grouped(t.pc_key1),
                            pages::grouped(t.pc_key2),
                        ))
                        .size(12)
                        .color(theme::MUTED),
                        Space::new().height(6),
                        row![
                            button(text("Take over, keep the pad's counters").size(12))
                                .style(theme::primary)
                                .on_press(Message::ResolveTakeover(true, true)),
                            Space::new().width(8),
                            button(text("Take over, use this PC's counters").size(12))
                                .style(theme::secondary)
                                .on_press(Message::ResolveTakeover(true, false)),
                            Space::new().width(8),
                            button(text("Leave it alone").size(12))
                                .style(theme::secondary)
                                .on_press(Message::ResolveTakeover(false, false)),
                        ]
                        .align_y(Alignment::Center),
                        Space::new().height(4),
                        text(
                            "Leaving it alone keeps the pad working as a keyboard; \
                             osu!pad just will not configure or count for it. \
                             To unpair a pad completely, see docs/recovery.md."
                        )
                        .size(11)
                        .color(theme::MUTED),
                    ]
                    .spacing(2),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if let Some(banner) = &self.banner {
            main = main.push(
                container(
                    row![
                        text(banner).size(14),
                        Space::new().width(Length::Fill),
                        button(text("Dismiss").size(12))
                            .style(theme::secondary)
                            .on_press(Message::DismissBanner),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        main = main.push(page);

        let body = column![
            chrome::title_bar(self.maximized),
            row![sidebar, main].height(Length::Fill)
        ];
        let framed = container(body).style(chrome::frame);

        if let Some(input_text) = &self.reset_modal {
            let modal_box = container(
                column![
                    text("Reset Lifetime Counters")
                        .size(20)
                        .font(theme::FONT_BOLD)
                        .color(theme::RED),
                    text("This action will permanently reset hardware and database counters to 0.")
                        .size(13)
                        .color(theme::MUTED),
                    Space::new().height(6),
                    text(format!(
                        "Key 1 (K1): {} presses",
                        self.counters.lifetime_key1
                    ))
                    .size(14),
                    text(format!(
                        "Key 2 (K2): {} presses",
                        self.counters.lifetime_key2
                    ))
                    .size(14),
                    text(format!(
                        "Total: {} presses",
                        self.counters.total_lifetime_presses()
                    ))
                    .size(14)
                    .font(theme::FONT_BOLD),
                    Space::new().height(10),
                    text("Type RESET below to confirm:")
                        .size(13)
                        .color(theme::MUTED),
                    text_input("RESET", input_text)
                        .on_input(Message::ResetModalInput)
                        .padding(10)
                        .size(14),
                    Space::new().height(14),
                    row![
                        button(text("Cancel").size(14))
                            .padding([10, 20])
                            .style(theme::secondary)
                            .on_press(Message::CancelResetModal),
                        Space::new().width(Length::Fill),
                        if input_text == "RESET" {
                            button(text("Confirm Reset").size(14))
                                .padding([10, 20])
                                .style(theme::danger)
                                .on_press(Message::ConfirmResetCounters)
                        } else {
                            button(text("Confirm Reset").size(14))
                                .padding([10, 20])
                                .style(theme::secondary)
                        }
                    ]
                ]
                .spacing(8)
                .padding(24)
                .width(420),
            )
            .style(theme::card);

            let modal_overlay = container(modal_box)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.75,
                            ..theme::BG
                        }
                        .into(),
                    ),
                    ..Default::default()
                });

            return if self.maximized {
                stack![framed, modal_overlay].into()
            } else {
                stack![framed, chrome::resize_edges(), modal_overlay].into()
            };
        }

        if let Some(modal) = &self.import_modal {
            let cur_dev = modal
                .current
                .as_ref()
                .map(|c| c.device_id.as_str())
                .unwrap_or("None");
            let inc_dev = modal.incoming.device.device_id.as_str();

            let mut modal_col = column![
                text("Import Backup Preview")
                    .size(20)
                    .font(theme::FONT_BOLD)
                    .color(theme::WHITE),
                text("Review the changes before applying this backup to your pad and host.")
                    .size(13)
                    .color(theme::MUTED),
                Space::new().height(6),
            ]
            .spacing(8);

            if !modal.device_id_matches {
                modal_col = modal_col.push(
                    container(
                        text(format!("⚠ Device ID mismatch: backup was created for pad '{}', but current pad is '{}'.", inc_dev, cur_dev))
                            .size(13)
                            .color(theme::YELLOW)
                    )
                    .padding(8)
                    .style(theme::card)
                );
            }

            let cur_k1 = modal.current.as_ref().map(|c| c.lifetime_key1).unwrap_or(0);
            let inc_k1 = modal.incoming.stats.lifetime_key1;
            let cur_k2 = modal.current.as_ref().map(|c| c.lifetime_key2).unwrap_or(0);
            let inc_k2 = modal.incoming.stats.lifetime_key2;
            let cur_gen = modal
                .current
                .as_ref()
                .map(|c| c.counter_generation)
                .unwrap_or(0);
            let next_gen = cur_gen + 1;

            let k1_color = if inc_k1 < cur_k1 {
                theme::RED
            } else {
                theme::WHITE
            };
            let k2_color = if inc_k2 < cur_k2 {
                theme::RED
            } else {
                theme::WHITE
            };

            let cur_k1_str = modal
                .current
                .as_ref()
                .map(|c| c.config.key1_char())
                .unwrap_or_else(|| "Z".to_string());
            let cur_k2_str = modal
                .current
                .as_ref()
                .map(|c| c.config.key2_char())
                .unwrap_or_else(|| "X".to_string());

            let table = column![
                row![
                    text("Field")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(2)),
                    text("Current")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(3)),
                    text("Incoming")
                        .size(12)
                        .color(theme::MUTED)
                        .width(Length::FillPortion(3)),
                ],
                row![
                    text("Generation").size(13).width(Length::FillPortion(2)),
                    text(format!("{}", cur_gen))
                        .size(13)
                        .width(Length::FillPortion(3)),
                    text(format!("{} (+1)", next_gen))
                        .size(13)
                        .color(theme::CYAN)
                        .width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 1").size(13).width(Length::FillPortion(2)),
                    text(format!("{} ({} presses)", cur_k1_str, cur_k1))
                        .size(13)
                        .width(Length::FillPortion(3)),
                    text(format!(
                        "{} ({} presses)",
                        modal.incoming.config.key1, inc_k1
                    ))
                    .size(13)
                    .color(k1_color)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Key 2").size(13).width(Length::FillPortion(2)),
                    text(format!("{} ({} presses)", cur_k2_str, cur_k2))
                        .size(13)
                        .width(Length::FillPortion(3)),
                    text(format!(
                        "{} ({} presses)",
                        modal.incoming.config.key2, inc_k2
                    ))
                    .size(13)
                    .color(k2_color)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Key pins").size(13).width(Length::FillPortion(2)),
                    text(
                        modal
                            .current
                            .as_ref()
                            .map(|c| format!(
                                "GPIO{} / GPIO{}",
                                c.config.key1_gpio, c.config.key2_gpio
                            ))
                            .unwrap_or_else(|| "-".to_string())
                    )
                    .size(13)
                    .width(Length::FillPortion(3)),
                    text(format!(
                        "GPIO{} / GPIO{}",
                        modal.incoming.config.key1_gpio, modal.incoming.config.key2_gpio
                    ))
                    .size(13)
                    .width(Length::FillPortion(3)),
                ],
                row![
                    text("Debounce").size(13).width(Length::FillPortion(2)),
                    text(format!(
                        "{} µs",
                        modal
                            .current
                            .as_ref()
                            .map(|c| c.config.debounce_us)
                            .unwrap_or(0)
                    ))
                    .size(13)
                    .width(Length::FillPortion(3)),
                    text(format!("{} µs", modal.incoming.config.debounce_us))
                        .size(13)
                        .width(Length::FillPortion(3)),
                ],
            ]
            .spacing(6);

            modal_col = modal_col.push(table);

            if !modal.warnings.is_empty() {
                let mut warn_col = column![theme::caption("WARNINGS:")].spacing(4);
                for w in &modal.warnings {
                    warn_col =
                        warn_col.push(text(format!("• {}", w)).size(12).color(theme::YELLOW));
                }
                modal_col = modal_col.push(warn_col);
            }

            if modal.is_counter_rollback {
                modal_col = modal_col.push(
                    checkbox(modal.rollback_confirmed)
                        .label("I understand this replaces my counters with lower values")
                        .on_toggle(Message::ToggleImportRollbackConfirm)
                        .size(14),
                );
            }

            let can_apply = !modal.is_counter_rollback || modal.rollback_confirmed;
            let mut apply_btn = button(text("Apply Backup").size(14))
                .padding([10, 20])
                .style(if can_apply {
                    theme::primary
                } else {
                    theme::secondary
                });
            if can_apply {
                apply_btn = apply_btn.on_press(Message::ConfirmApplyImport);
            }

            let actions_row = row![
                button(text("Cancel").size(14))
                    .padding([10, 20])
                    .style(theme::secondary)
                    .on_press(Message::CancelImportModal),
                Space::new().width(Length::Fill),
                apply_btn,
            ];
            modal_col = modal_col.push(actions_row);

            let modal_box = container(modal_col)
                .padding(24)
                .width(480)
                .style(theme::card);

            let modal_overlay = container(modal_box)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.75,
                            ..theme::BG
                        }
                        .into(),
                    ),
                    ..Default::default()
                });

            return if self.maximized {
                stack![framed, modal_overlay].into()
            } else {
                stack![framed, chrome::resize_edges(), modal_overlay].into()
            };
        }

        if let Some(action) = self.recovery_modal {
            let (title, desc, confirm_msg) = match action {
                RecoveryAction::RestoreDeviceFromPc => (
                    "Restore PAD from PC",
                    "This action will force-overwrite your physical pad's counters with the counters stored in this PC database. The pad's generation number will be incremented.",
                    Message::ConfirmRestoreDeviceFromPc,
                ),
                RecoveryAction::ImportPcFromDevice => (
                    "Import PC from PAD",
                    "This action will overwrite this PC database with the counters currently reported by your physical pad. The database generation number will be incremented.",
                    Message::ConfirmImportPcFromDevice,
                ),
            };

            let pc_k1 = self
                .pc_counters
                .as_ref()
                .map(|c| c.lifetime_key1)
                .unwrap_or(0);
            let pc_k2 = self
                .pc_counters
                .as_ref()
                .map(|c| c.lifetime_key2)
                .unwrap_or(0);
            let esp_k1 = self
                .esp_counters
                .as_ref()
                .map(|c| c.lifetime_key1)
                .unwrap_or(self.counters.lifetime_key1);
            let esp_k2 = self
                .esp_counters
                .as_ref()
                .map(|c| c.lifetime_key2)
                .unwrap_or(self.counters.lifetime_key2);

            let modal_box = container(
                column![
                    text(title)
                        .size(20)
                        .font(theme::FONT_BOLD)
                        .color(theme::WHITE),
                    text(desc).size(13).color(theme::MUTED),
                    Space::new().height(8),
                    container(
                        column![
                            row![
                                text("").width(Length::FillPortion(2)),
                                text("PC Database")
                                    .size(12)
                                    .color(theme::MUTED)
                                    .width(Length::FillPortion(3)),
                                text("PAD Hardware")
                                    .size(12)
                                    .color(theme::MUTED)
                                    .width(Length::FillPortion(3)),
                            ],
                            row![
                                text("Key 1").size(13).width(Length::FillPortion(2)),
                                text(format!("{} presses", pc_k1))
                                    .size(13)
                                    .width(Length::FillPortion(3)),
                                text(format!("{} presses", esp_k1))
                                    .size(13)
                                    .width(Length::FillPortion(3)),
                            ],
                            row![
                                text("Key 2").size(13).width(Length::FillPortion(2)),
                                text(format!("{} presses", pc_k2))
                                    .size(13)
                                    .width(Length::FillPortion(3)),
                                text(format!("{} presses", esp_k2))
                                    .size(13)
                                    .width(Length::FillPortion(3)),
                            ],
                            row![
                                text("Total").size(13).width(Length::FillPortion(2)),
                                text(format!("{} presses", pc_k1 + pc_k2))
                                    .size(13)
                                    .font(theme::FONT_BOLD)
                                    .width(Length::FillPortion(3)),
                                text(format!("{} presses", esp_k1 + esp_k2))
                                    .size(13)
                                    .font(theme::FONT_BOLD)
                                    .width(Length::FillPortion(3)),
                            ],
                        ]
                        .spacing(6)
                    )
                    .padding(12)
                    .style(theme::card),
                    Space::new().height(12),
                    row![
                        button(text("Cancel").size(14))
                            .padding([10, 20])
                            .style(theme::secondary)
                            .on_press(Message::CancelRecoveryModal),
                        Space::new().width(Length::Fill),
                        button(text("Confirm").size(14))
                            .padding([10, 20])
                            .style(theme::danger)
                            .on_press(confirm_msg),
                    ]
                ]
                .spacing(10)
                .padding(24)
                .width(460),
            )
            .style(theme::card);

            let modal_overlay = container(modal_box)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.75,
                            ..theme::BG
                        }
                        .into(),
                    ),
                    ..Default::default()
                });

            return if self.maximized {
                stack![framed, modal_overlay].into()
            } else {
                stack![framed, chrome::resize_edges(), modal_overlay].into()
            };
        }

        if let Some(modal) = &self.firmware_modal {
            let modal_box = match modal {
                FirmwareModalState::Consent {
                    consent_text,
                    from_version,
                    to_version,
                } => {
                    let header = column![
                        text("Confirm Firmware Update")
                            .size(20)
                            .font(theme::FONT_BOLD)
                            .color(theme::YELLOW),
                        text(format!(
                            "Update pad firmware: {} → {}",
                            from_version, to_version
                        ))
                        .size(13)
                        .color(theme::MUTED),
                    ]
                    .spacing(4);

                    let consent_lines = column(
                        consent_text
                            .lines()
                            .map(|l| text(l.to_string()).size(13).into()),
                    )
                    .spacing(4);

                    let consent_box = container(scrollable(consent_lines).height(200))
                        .padding(12)
                        .style(theme::card);

                    let actions = row![
                        button(text("Cancel").size(14))
                            .padding([10, 20])
                            .style(theme::secondary)
                            .on_press(Message::CancelFirmwareModal),
                        Space::new().width(Length::Fill),
                        button(text("Confirm & Flash Pad").size(14))
                            .padding([10, 20])
                            .style(theme::danger)
                            .on_press(Message::ConfirmFirmwareUpdate),
                    ];

                    container(
                        column![header, consent_box, actions]
                            .spacing(14)
                            .padding(24)
                            .width(520),
                    )
                    .style(theme::card)
                }
                FirmwareModalState::Flashing {
                    from_version,
                    to_version,
                } => {
                    let warning_box = container(
                        column![
                            text("⚠ THE PAD IS CURRENTLY UNUSABLE AS A KEYBOARD")
                                .size(14)
                                .font(theme::FONT_BOLD)
                                .color(theme::RED),
                            text("Do NOT unplug the USB cable or close the application.")
                                .size(13)
                                .color(theme::WHITE),
                            text("Writing app partition over USB. The pad will reboot automatically when finished (~30 seconds).")
                                .size(12)
                                .color(theme::MUTED),
                        ]
                        .spacing(6),
                    )
                    .padding(14)
                    .style(theme::card);

                    container(
                        column![
                            text("Flashing Firmware in Progress")
                                .size(20)
                                .font(theme::FONT_BOLD)
                                .color(theme::YELLOW),
                            warning_box,
                            text(format!(
                                "Flashing target: {} → {}",
                                from_version, to_version
                            ))
                            .size(13)
                            .color(theme::CYAN),
                            text("Flashing and rebooting... Please wait.")
                                .size(13)
                                .color(theme::YELLOW),
                            row![button(text("Flashing in progress...").size(14))
                                .padding([10, 20])
                                .style(theme::secondary),],
                        ]
                        .spacing(12)
                        .padding(24)
                        .width(520),
                    )
                    .style(theme::card)
                }
                FirmwareModalState::Success {
                    from,
                    to,
                    firmware_version,
                    running_partition,
                } => {
                    let partition_str = running_partition.as_deref().unwrap_or("unknown");
                    container(
                        column![
                            text("✓ Firmware Update Complete")
                                .size(20)
                                .font(theme::FONT_BOLD)
                                .color(theme::GREEN),
                            text(format!(
                                "Firmware was successfully updated from {} to {}.",
                                from, to
                            ))
                            .size(14),
                            text(format!(
                                "Verified running version: {}",
                                firmware_version
                            ))
                            .size(13)
                            .font(theme::FONT_BOLD),
                            text(format!("Running slot (OTA partition): {}", partition_str))
                                .size(13)
                                .color(theme::MUTED),
                            text("The pad has rebooted and resumed normal 1000 Hz HID keyboard operation.")
                                .size(12)
                                .color(theme::MUTED),
                            Space::new().height(6),
                            row![
                                Space::new().width(Length::Fill),
                                button(text("Close").size(14))
                                    .padding([10, 20])
                                    .style(theme::primary)
                                    .on_press(Message::CancelFirmwareModal),
                            ],
                        ]
                        .spacing(10)
                        .padding(24)
                        .width(480),
                    )
                    .style(theme::card)
                }
                FirmwareModalState::Failed { error } => {
                    let error_box = container(
                        column![
                            text("Error Details:")
                                .size(13)
                                .font(theme::FONT_BOLD)
                                .color(theme::RED),
                            text(error.clone()).size(13).color(theme::WHITE),
                        ]
                        .spacing(4),
                    )
                    .padding(12)
                    .style(theme::card);

                    let recovery_box = container(
                        column![
                            text("Disaster Recovery:")
                                .size(13)
                                .font(theme::FONT_BOLD)
                                .color(theme::YELLOW),
                            text("Lifetime counters were saved to this PC before flashing began.")
                                .size(12),
                            text("If the pad does not respond or boot, refer to docs/recovery.md (§7 Disaster Reflash).")
                                .size(12),
                            text("You can reflash the pad over USB via osupadctl flash without loss of lifetime press stats.")
                                .size(12)
                                .color(theme::MUTED),
                        ]
                        .spacing(4),
                    )
                    .padding(12)
                    .style(theme::card);

                    container(
                        column![
                            text("✗ Firmware Update Failed")
                                .size(20)
                                .font(theme::FONT_BOLD)
                                .color(theme::RED),
                            error_box,
                            recovery_box,
                            Space::new().height(6),
                            row![
                                Space::new().width(Length::Fill),
                                button(text("Close").size(14))
                                    .padding([10, 20])
                                    .style(theme::secondary)
                                    .on_press(Message::CancelFirmwareModal),
                            ],
                        ]
                        .spacing(10)
                        .padding(24)
                        .width(520),
                    )
                    .style(theme::card)
                }
            };

            let modal_overlay = container(modal_box)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.75,
                            ..theme::BG
                        }
                        .into(),
                    ),
                    ..Default::default()
                });

            return if self.maximized {
                stack![framed, modal_overlay].into()
            } else {
                stack![framed, chrome::resize_edges(), modal_overlay].into()
            };
        }

        if self.maximized {
            framed.into()
        } else {
            stack![framed, chrome::resize_edges()].into()
        }
    }

    pub fn formatted_visible_logs(&self) -> Vec<String> {
        self.logs
            .iter()
            .filter(|e| e.seq > self.log_cleared_seq)
            .filter(|e| self.log_filter_level.is_none_or(|l| e.level >= l))
            .filter(|e| self.log_filter_source.is_none_or(|s| e.source == s))
            .map(|e| e.format_line())
            .collect()
    }
}

async fn save_logs_dialog(content: String) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_file_name("osupad.log")
        .add_filter("Log file", &["log", "txt"])
        .save_file()
        .await
        .ok_or_else(String::new)?;
    std::fs::write(file.path(), content).map_err(|e| format!("Save failed: {}", e))?;
    Ok(file.path().display().to_string())
}

async fn export_backup_dialog(backup: JsonBackup) -> Result<String, String> {
    let filename = format!(
        "osupad-backup-{}.json",
        chrono::Local::now().format("%Y%m%d")
    );
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(filename)
        .add_filter("JSON Backup", &["json"])
        .save_file()
        .await
        .ok_or_else(String::new)?;
    let pretty = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
    std::fs::write(file.path(), pretty).map_err(|e| format!("Export failed: {}", e))?;
    Ok(file.path().display().to_string())
}

async fn pick_backup_dialog() -> Result<JsonBackup, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("JSON Backup", &["json"])
        .pick_file()
        .await
        .ok_or_else(String::new)?;
    let content =
        std::fs::read_to_string(file.path()).map_err(|e| format!("Failed to read file: {}", e))?;
    let backup: JsonBackup =
        serde_json::from_str(&content).map_err(|e| format!("Malformed JSON backup: {}", e))?;
    backup
        .validate()
        .map_err(|e| format!("Backup validation error: {}", e))?;
    Ok(backup)
}

/// A binary installed next to this one. `EXE_SUFFIX` matters: the sibling is
/// `osupadctl.exe` on Windows, and without it the lookup always misses.
fn find_sibling_executable(stem: &str) -> std::path::PathBuf {
    let name = format!("{}{}", stem, std::env::consts::EXE_SUFFIX);
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&name);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    std::path::PathBuf::from(name)
}

fn find_daemon_executable() -> std::path::PathBuf {
    find_sibling_executable("osupad-daemon")
}

async fn start_daemon_process() -> Result<(), String> {
    let exe = find_daemon_executable();
    #[cfg(target_os = "linux")]
    tokio::task::spawn_blocking(move || platform_linux::start_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to start osupad-daemon: {}", e))??;
    #[cfg(windows)]
    tokio::task::spawn_blocking(move || platform_windows::start_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to start osupad-daemon: {}", e))??;
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = exe;
        return Err("Starting the daemon is not supported on this platform".to_string());
    }
    #[allow(unreachable_code)]
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(())
}

pub fn tosu_source_status(override_path: Option<&std::path::Path>) -> String {
    if let Some(p) = override_path {
        return format!("Custom ({})", p.display());
    }
    if let Some(p) = std::env::var_os("OSUPAD_TOSU_PATH") {
        return format!("Custom ({})", std::path::Path::new(&p).display());
    }
    let home_install = dirs::home_dir().map(|h| {
        h.join(".local/opt/tosu")
            .join(osupad_model::paths::TOSU_BINARY)
    });
    if let Some(p) = home_install.filter(|p| p.is_file()) {
        return format!("Installed at {}", p.display());
    }
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(osupad_model::paths::TOSU_BINARY))
            .find(|p| p.is_file())
    });
    if let Some(p) = on_path {
        return format!("Installed at PATH ({})", p.display());
    }
    if let Ok(bundled_dir) = osupad_model::paths::bundled_tosu_dir() {
        if let Ok(v) = std::fs::read_to_string(bundled_dir.join("VERSION")) {
            let v = v.trim();
            if !v.is_empty() {
                return format!("Bundled (v{})", v);
            }
        }
        if bundled_dir.join(osupad_model::paths::TOSU_BINARY).is_file() {
            return "Bundled".to_string();
        }
    }
    "Bundled (default)".to_string()
}
