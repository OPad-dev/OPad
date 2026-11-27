#![cfg_attr(windows, windows_subsystem = "windows")]

mod chrome;
mod designer;
mod he_monitor;
mod ipc;
mod keys;
mod pages;
#[cfg(target_os = "linux")]
mod platform_linux;
#[cfg(windows)]
mod platform_windows;
mod screen;
mod settings;
mod single_instance;
mod stats;
mod theme;
mod tray;
mod troubleshooting;
mod widgets;

use troubleshooting as diagnostics;

use iced::widget::{
    button, checkbox, column, container, row, scrollable, stack, text, text_input, Space,
};
use iced::{window, Alignment, Element, Length, Size, Subscription, Task};
use opad_ipc::{CurrentBackupState, IpcRequest, IpcResponse};
use opad_model::result_anim::{ResultAnimations, Trigger};
use opad_model::swipe::{FreeKey, SwipeChoice, SwipeDir, SwipeSetting};
use opad_model::ui_source::SourceValue;
use opad_model::{
    char_to_hid_usage, CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice,
    JsonBackup, KeyPin, LatencyStats, LogEntry, LogLevel, LogSource, RuntimeMode,
};
use std::collections::HashMap;
use std::time::Duration;

pub fn parse_page_arg() -> Option<Target> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--page" || args[i] == "-p") && i + 1 < args.len() {
            return Target::from_token(&args[i + 1]);
        }
    }
    None
}

pub fn main() -> iced::Result {
    let target = parse_page_arg();
    let page_str = target.map(Target::token);

    if !single_instance::claim(page_str) {
        // Another instance was asked to show its window
        return Ok(());
    }
    let start_hidden = std::env::args().any(|a| a == "--tray");

    // A daemon keeps running with no window open, living in the tray (Discord-style)
    iced::daemon(
        move || App::new(start_hidden, target),
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

/// The sidebar's pages. What used to be pages of their own are tabs now:
/// Logs and the test tools under Troubleshooting, About under Settings, and
/// the Hall Effect calibration inside Keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Stats,
    /// Bindings, key feel (Hall Effect), wiring
    Keys,
    /// The pad's display, touchscreen and result animations
    Screen,
    Designer,
    /// This pad, its counters and backups
    Pad,
    /// The PC app only: startup, updates, the osu! connection, about
    Settings,
    Troubleshooting,
}

/// The part of a page a link or a `--page` name opens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// The page as it was left
    Any,
    Settings(settings::Tab),
    Trouble(troubleshooting::Tab),
    /// Keys, with the Hall Effect calibration open
    Calibration,
}

/// Where a link, the tray or a `--page` name goes: a page and, when it
/// matters, the tab on it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub page: Page,
    pub section: Section,
}

impl Page {
    /// The sidebar, top to bottom: what you look at while playing, what you
    /// set on the pad, and the PC app itself
    pub const GROUPS: [(&'static str, &'static [(Page, &'static str)]); 3] = [
        (
            "Play",
            &[(Page::Dashboard, "Dashboard"), (Page::Stats, "Stats")],
        ),
        (
            "Your pad",
            &[
                (Page::Keys, "Keys"),
                (Page::Screen, "Screen"),
                (Page::Designer, "Designer"),
                (Page::Pad, "Pad"),
            ],
        ),
        (
            "App",
            &[
                (Page::Settings, "Settings"),
                (Page::Troubleshooting, "Troubleshooting"),
            ],
        ),
    ];

    /// Pages that edit the pad's settings and share the save bar
    pub fn edits_pad_settings(self) -> bool {
        matches!(self, Page::Keys | Page::Screen)
    }

    pub const fn at(self) -> Target {
        Target {
            page: self,
            section: Section::Any,
        }
    }
}

impl Target {
    pub const UPDATES: Target = Target {
        page: Page::Settings,
        section: Section::Settings(settings::Tab::Updates),
    };
    pub const OSU: Target = Target {
        page: Page::Settings,
        section: Section::Settings(settings::Tab::Osu),
    };
    pub const LOGS: Target = Target {
        page: Page::Troubleshooting,
        section: Section::Trouble(troubleshooting::Tab::Logs),
    };
    pub const CALIBRATION: Target = Target {
        page: Page::Keys,
        section: Section::Calibration,
    };

    /// `--page` names and their aliases. The first name for each target is
    /// the token a second launch sends to the running instance, so both ends
    /// of the single-instance handoff read this one table. Names from before
    /// the pages were regrouped (`device`, `logs`, `about`, `diagnostics`)
    /// still open where their content went: the daemon and people's shortcuts
    /// use them.
    const TOKENS: [(&'static str, Target); 31] = {
        use settings::Tab as S;
        use troubleshooting::Tab as T;
        const fn on(page: Page, section: Section) -> Target {
            Target { page, section }
        }
        [
            ("dashboard", Page::Dashboard.at()),
            ("stats", Page::Stats.at()),
            ("kpm", Page::Stats.at()),
            // What the tap rate was called before KPM
            ("ppm", Page::Stats.at()),
            ("keys", Page::Keys.at()),
            ("rapid-trigger", Page::Keys.at()),
            // The daemon opens this for a Hall Effect pad that needs calibrating
            ("calibrate", Target::CALIBRATION),
            ("calibration", Target::CALIBRATION),
            ("screen", Page::Screen.at()),
            ("display", Page::Screen.at()),
            ("designer", Page::Designer.at()),
            ("pad", Page::Pad.at()),
            ("device", Page::Pad.at()),
            ("settings", Page::Settings.at()),
            ("general", on(Page::Settings, Section::Settings(S::General))),
            ("updates", Target::UPDATES),
            ("osu", Target::OSU),
            ("tosu", Target::OSU),
            ("about", on(Page::Settings, Section::Settings(S::About))),
            ("license", on(Page::Settings, Section::Settings(S::About))),
            ("licenses", on(Page::Settings, Section::Settings(S::About))),
            ("troubleshooting", Page::Troubleshooting.at()),
            ("diagnostics", Page::Troubleshooting.at()),
            ("logs", Target::LOGS),
            ("monitor", Target::LOGS),
            (
                "key-test",
                on(Page::Troubleshooting, Section::Trouble(T::KeyTest)),
            ),
            (
                "test",
                on(Page::Troubleshooting, Section::Trouble(T::KeyTest)),
            ),
            (
                "testing",
                on(Page::Troubleshooting, Section::Trouble(T::KeyTest)),
            ),
            (
                "board-test",
                on(Page::Troubleshooting, Section::Trouble(T::BoardTest)),
            ),
            (
                "connection",
                on(Page::Troubleshooting, Section::Trouble(T::Connection)),
            ),
            (
                "report",
                on(Page::Troubleshooting, Section::Trouble(T::Report)),
            ),
        ]
    };

    pub fn from_token(token: &str) -> Option<Target> {
        let token = token.to_lowercase();
        Self::TOKENS
            .iter()
            .find(|(name, _)| *name == token)
            .map(|(_, target)| *target)
    }

    pub fn token(self) -> &'static str {
        Self::TOKENS
            .iter()
            .find(|(_, target)| *target == self)
            .or_else(|| Self::TOKENS.iter().find(|(_, t)| t.page == self.page))
            .map(|(name, _)| *name)
            .expect("every page has a token")
    }
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
    pub app: opad_ipc::ComponentUpdate,
    pub tosu: opad_ipc::ComponentUpdate,
    pub last_check: Option<String>,
    pub last_error: Option<String>,
    pub restart_required: bool,
    /// Installed, runs from the next start (with `restart_required`)
    pub pending_version: Option<String>,
    /// `stable` or `beta`
    pub channel: String,
    /// A check is running in the daemon
    pub checking: bool,
    pub last_result: Option<String>,
}

pub struct App {
    pub page: Page,
    window: Option<window::Id>,
    pub maximized: bool,
    tray: Option<tray::TrayHandle>,
    tray_available: Option<bool>,
    /// The window was closed while the tray was still being probed: it is
    /// hidden for now, and the app exits if the probe finds no tray
    exit_if_no_tray: bool,
    /// Started with --tray, but the window was opened because no tray was
    /// found: a tray that turns up later hides it again
    start_hidden: bool,
    fallback_window: bool,

    // Daemon state
    pub daemon_online: bool,
    pub daemon_spawn_attempted: bool,
    /// The GUI and the daemon were different versions after an update, and
    /// this run already restarted one of them to match
    version_fix_attempted: bool,
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
    /// When the daemon last wrote its automatic counter backup, RFC 3339
    pub last_backup: Option<String>,
    pub storage_error: Option<String>,
    pub latency: Option<LatencyStats>,
    pub pending_replacement: Option<String>,
    /// A pad paired with another installation (§W3-3)
    pub pending_takeover: Option<opad_ipc::TakeoverPrompt>,
    /// The AppImage's path while its udev rule is not installed: the pad's
    /// serial port and input node stay root-only until it is
    pub appimage_udev: Option<std::path::PathBuf>,
    /// What each updater knows (§U-0.4)
    pub updates: Option<UpdateView>,
    pub incompatible: Option<IncompatibleDevice>,
    pub reset_modal: Option<String>,
    pub import_modal: Option<ImportModalState>,
    pub recovery_modal: Option<RecoveryAction>,
    pub firmware_offer: Option<opad_ipc::FirmwareOffer>,
    pub firmware_modal: Option<FirmwareModalState>,
    pub ui_values: HashMap<u8, SourceValue>,

    // Settings form
    pub k1_input: String,
    pub k2_input: String,
    /// Key 3 (Hall Effect V3); empty sends nothing
    pub k3_input: String,
    /// Settings form: the game keys (bit k = key k+1)
    pub game_keys: u32,
    pub k1_gpio: u32,
    pub k2_gpio: u32,
    /// Each swipe direction, in SwipeDir::ALL order
    pub swipes: [SwipeSetting; 4],
    /// Kept on this PC, so editable without a pad
    pub result_animations: ResultAnimations,
    pub debounce: u32,
    pub brightness: u32,
    pub sleep_seconds: u32,
    pub gameplay_display_hz: u32,
    pub autostart_tray: bool,
    pub detecting_pin: Option<u32>,
    config_loaded: bool,
    /// What Save to pad sent, until the pad answers
    pub pad_sent: Option<DeviceConfig>,
    /// The key (1-3) whose binding waits for a letter to be pressed
    pub capturing_key: Option<u8>,
    /// What the last key pressed while capturing was refused for
    pub capture_hint: Option<&'static str>,
    /// Where the person was going when the pad's settings had unsaved changes
    pub leave_prompt: Option<Target>,

    pub logs: Vec<LogEntry>,
    /// The daemon fetch cursor: the seq of the last daemon entry received.
    /// Only daemon responses move it (GetLogEntries is exclusive of it).
    pub latest_log_seq: u64,
    /// When GetUpdateStatus was last asked. Update state changes on the order
    /// of hours, so it is polled every UPDATE_STATUS_POLL, at once on opening
    /// Settings and after an update action; `None` forces the next poll.
    update_status_polled_at: Option<std::time::Instant>,
    /// A check or install was just asked for and its result is not in yet:
    /// poll the update state quickly until it is
    update_busy: bool,
    /// Seq for entries the GUI logs itself (source Program), a separate
    /// numbering from the daemon's
    local_log_seq: u64,
    /// Entries at or before this instant are hidden by Clear. A time, not a
    /// seq, because the daemon's and the GUI's entries are numbered apart.
    log_cleared_at: Option<chrono::DateTime<chrono::Local>>,
    pub log_filter_level: Option<LogLevel>,
    pub log_filter_source: Option<LogSource>,
    pub log_auto_scroll: bool,
    /// Results of what was just done, floating over the page until they
    /// expire; newest last
    pub notices: Vec<Notice>,
    next_notice: u64,
    /// The notice under the pointer keeps its time
    hovered_notice: Option<u64>,
    /// No tray host was found: closing the window quits the app
    pub no_tray: bool,
    pub tosu_override_path: Option<std::path::PathBuf>,
    pub designer: designer::Designer,
    pub diagnostics: diagnostics::DiagnosticsState,
    pub settings_tab: settings::Tab,
    /// The tosu version installed, read from its folder (About)
    pub tosu_version: Option<String>,
    /// Keys shows the Hall Effect calibration instead of its sections
    pub calibrating: bool,
    /// Pad shows its technical lines (running slot, counter generation)
    pub pad_details: bool,
    /// The Stats page's tap rate (KPM) statistics
    pub stats: stats::StatsState,
    pub he_calibration: Option<opad_model::HeCalibrationStatus>,
    /// Key 3's counters, with a pad that has three keys
    pub key3: Option<opad_model::Key3Counters>,
    pub cal_progress: Option<opad_model::HeCalibrationProgress>,
    pub keys: keys::KeysState,
    pub cal_status_msg: Option<String>,
    pub cal_starting: bool,
    cal_epoch: u64,
}

/// Shown when no tray host appeared: closing the window then quits the app
/// rather than leaving it running with no way back to it
/// How often the update state is re-asked while the window is open. Worst
/// case the "restart to finish updating" banner shows this much late.
const UPDATE_STATUS_POLL: Duration = Duration::from_secs(60);
/// While a check or an install is running the answer is seconds away
const UPDATE_STATUS_POLL_BUSY: Duration = Duration::from_secs(2);

/// How a notice reads at a glance, and how long it stays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Something started and its result follows
    Info,
    Good,
    Bad,
}

impl Tone {
    /// Long enough to read: twice as long when something went wrong
    fn stays(self) -> Duration {
        match self {
            Tone::Info | Tone::Good => Duration::from_secs(4),
            Tone::Bad => Duration::from_secs(8),
        }
    }
}

/// One floating notice (docs/specs/app-ux-decisions.md §10)
#[derive(Debug, Clone)]
pub struct Notice {
    pub id: u64,
    pub tone: Tone,
    pub text: String,
    until: std::time::Instant,
}

/// At most this many notices show at once; an older one makes room
const MAX_NOTICES: usize = 3;
/// How often notices are checked for having had their time
const NOTICE_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    /// A page on one of its tabs
    Open(Target),
    /// Settings, on its Updates tab
    OpenUpdates,
    /// The answer to "you have unsaved changes": save or discard them and
    /// go on, or stay (`None`)
    LeaveAnswer(Option<bool>),
    Poll,
    Status(Result<IpcResponse, String>),
    UiValues(Result<IpcResponse, String>),
    TapStats(Result<IpcResponse, String>),
    SetTapPeriod(u32),
    TapPeriodSet(Result<IpcResponse, String>),
    Logs(Result<IpcResponse, String>),
    FilterLogLevel(Option<LogLevel>),
    FilterLogSource(Option<LogSource>),
    ClearLogs,
    CopyLogs,
    SaveLogs,
    LogsSaved(Result<String, String>),
    ToggleAutoScroll,
    // Settings and Troubleshooting tabs
    SelectSettingsTab(settings::Tab),
    Diagnostics(diagnostics::DiagnosticsMessage),
    CopyDiagnosticBundle,
    SaveDiagnosticBundle,
    DiagnosticBundleSaved(Result<String, String>),
    DiagnosticsKeyEvent {
        key: iced::keyboard::Key,
        is_down: bool,
    },
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
    // Hall Effect Calibration
    StartCalibration,
    StartCalibrationDone(u64, Result<IpcResponse, String>),
    CancelCalibration,
    CancelCalibrationDone(u64, Result<IpcResponse, String>),
    PollCalibration,
    Keys(keys::KeysMessage),
    CalibrationProgress(u64, Result<IpcResponse, String>),
    // Daemon and tosu recovery
    StartDaemon,
    DaemonStarted(Result<(), String>),
    StartTosu,
    InstallUdevRule,
    UdevRuleInstalled(Result<(), String>),
    TosuStarted(Result<(), String>),
    InstallSystemdService,
    SystemdServiceInstalled(Result<(), String>),
    // The pad's settings (Keys and Screen)
    /// Listen for the letter this key (1-3) should send
    CaptureKey(u8),
    CancelCapture,
    /// A key pressed while a binding waits for its letter
    CaptureKeyPressed(iced::keyboard::Key),
    /// Key 3 sends nothing
    ClearKey3,
    /// Count this key (1-3) toward totals and the tap rate, or not
    GameKey(u8, bool),
    Key1Pin(KeyPin),
    Key2Pin(KeyPin),
    Swipe(SwipeDir, SwipeChoice),
    SwipeKey(SwipeDir, FreeKey),
    ResultAnimation(Trigger, screen::AnimChoice),
    /// Play the grade's animation on the pad now
    PreviewAnimation(Trigger),
    Debounce(u32),
    Brightness(u32),
    SleepSeconds(u32),
    GameplayDisplayHz(u32),
    ToggleAutostartTray(bool),
    StartDetectPin(u32),
    PinDetected(Result<IpcResponse, String>),
    CancelDetectPin,
    /// The save bar: send every unsaved pad setting, or drop them
    SavePad,
    RevertPad,
    OpenCalibration,
    CloseCalibration,
    TogglePadDetails,
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
    ToggleUpdater(opad_ipc::UpdateComponent, bool),
    InstallUpdate(opad_ipc::UpdateComponent),
    /// A release page from the verified update feed, in the browser
    OpenUrl(String),
    CheckForUpdates,
    SetUpdateChannel(String),
    ResetLatency,
    ActionDone(Action, Result<IpcResponse, String>),
    DismissNotice(u64),
    /// The pointer is over this notice (or none)
    HoverNotice(Option<u64>),
    NoticeTick,
    // tosu configuration (§T-4)
    PickTosuPath,
    TosuPathPicked(Option<std::path::PathBuf>),
    ResetTosuPath,
    Designer(designer::Message),
    // Window & tray
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    Tray(tray::TrayEvent),
    TrayTimeout,
    ShowRequested(Option<String>),
    Window(chrome::WindowAction),
    Resized,
    Maximized(bool),
}

impl App {
    fn new(start_hidden: bool, initial: Option<Target>) -> (Self, Task<Message>) {
        let (designer, designer_task) = designer::Designer::new();
        let tosu_override_path = opad_model::paths::data_dir()
            .ok()
            .and_then(|d| std::fs::read_to_string(d.join("tosu_path")).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("OPAD_TOSU_PATH")
                    .or_else(|| std::env::var_os("OPAD_TOSU_PATH"))
                    .map(std::path::PathBuf::from)
            });

        if let Some(ref p) = tosu_override_path {
            std::env::set_var("OPAD_TOSU_PATH", p);
        }

        let mut app = App {
            page: Page::Dashboard,
            window: None,
            maximized: false,
            tray: None,
            tray_available: None,
            exit_if_no_tray: false,
            start_hidden,
            fallback_window: false,
            daemon_online: false,
            daemon_spawn_attempted: false,
            version_fix_attempted: false,
            device_connected: false,
            tosu_connected: false,
            mode: RuntimeMode::Idle,
            device_info: None,
            counters: CounterState::default(),
            counters_source: CounterSource::Pc,
            config: DeviceConfig::default(),
            last_sync_time: None,
            last_backup: None,
            last_sync_error: None,
            storage_error: None,
            latency: None,
            pending_replacement: None,
            pending_takeover: None,
            #[cfg(target_os = "linux")]
            appimage_udev: platform_linux::appimage_needing_udev_rule(),
            #[cfg(not(target_os = "linux"))]
            appimage_udev: None,
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
            k3_input: "C".into(),
            game_keys: opad_model::DEFAULT_GAME_KEYS,
            k1_gpio: opad_model::DEFAULT_KEY1_GPIO,
            k2_gpio: opad_model::DEFAULT_KEY2_GPIO,
            swipes: SwipeDir::ALL.map(|d| DeviceConfig::default().swipe(d)),
            result_animations: ResultAnimations::default(),
            debounce: 5000,
            brightness: 100,
            sleep_seconds: 600,
            gameplay_display_hz: 10,
            #[cfg(target_os = "linux")]
            autostart_tray: platform_linux::is_gui_autostart_enabled(),
            #[cfg(windows)]
            autostart_tray: platform_windows::is_gui_autostart_enabled(),
            #[cfg(not(any(target_os = "linux", windows)))]
            autostart_tray: false,
            detecting_pin: None,
            config_loaded: false,
            pad_sent: None,
            capturing_key: None,
            capture_hint: None,
            leave_prompt: None,
            logs: Vec::new(),
            latest_log_seq: 0,
            update_status_polled_at: None,
            update_busy: false,
            local_log_seq: 0,
            log_cleared_at: None,
            log_filter_level: None,
            log_filter_source: None,
            log_auto_scroll: true,
            notices: Vec::new(),
            next_notice: 0,
            hovered_notice: None,
            no_tray: false,
            tosu_override_path,
            designer,
            diagnostics: diagnostics::DiagnosticsState::default(),
            settings_tab: settings::Tab::default(),
            tosu_version: settings::installed_tosu_version(),
            calibrating: false,
            pad_details: false,
            stats: stats::StatsState::default(),
            he_calibration: None,
            key3: None,
            cal_progress: None,
            keys: keys::KeysState::default(),
            cal_status_msg: None,
            cal_starting: false,
            cal_epoch: 0,
        };
        if let Some(target) = initial {
            app.show(target);
        }
        app.log_event(
            LogSource::Program,
            LogLevel::Info,
            "gui",
            "OPad application initialized",
        );
        let mut tasks = vec![
            designer_task.map(Message::Designer),
            app.poll(),
            Task::perform(tokio::time::sleep(Duration::from_secs(2)), |_| {
                Message::TrayTimeout
            }),
        ];
        if !start_hidden {
            tasks.push(app.open_window());
        }
        (app, Task::batch(tasks))
    }

    fn title(&self, _window: window::Id) -> String {
        "OPad".to_string()
    }

    /// Opens the window because there is no tray to live in. A --tray start
    /// hides it again if a tray turns up later.
    fn open_fallback_window(&mut self) -> Task<Message> {
        let task = self.open_window();
        self.fallback_window = self.start_hidden;
        task
    }

    fn open_window(&mut self) -> Task<Message> {
        // Shown again (e.g. by a second launch): closing decides anew, and
        // the window is the user's now, not a stand-in for a missing tray
        self.exit_if_no_tray = false;
        self.fallback_window = false;
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
            #[cfg(target_os = "linux")]
            platform_specific: window::settings::PlatformSpecific {
                application_id: "opad".to_string(),
                ..Default::default()
            },
            #[cfg(not(target_os = "linux"))]
            platform_specific: Default::default(),
            ..Default::default()
        });
        self.window = Some(id);
        open.map(Message::WindowOpened)
    }

    /// Asks the daemon for what is on screen: the status always (the tray shows
    /// it too), the rest only while the window shows something that reads it.
    /// Navigating and opening the window poll at once, so a page never waits a
    /// tick for its data.
    fn poll(&mut self) -> Task<Message> {
        let mut tasks = vec![Task::perform(
            ipc::request(IpcRequest::GetStatus),
            Message::Status,
        )];
        let page = self.window.is_some().then_some(self.page);
        // Live tosu values: the Dashboard's profile and now-playing cards
        if matches!(page, Some(Page::Dashboard | Page::Designer)) {
            tasks.push(Task::perform(
                ipc::request(IpcRequest::GetUiValues),
                Message::UiValues,
            ));
        }
        // restart_required drives a banner on every page, but it changes
        // rarely: a slow poll, forced when Settings opens or an update ran
        let busy = self.update_busy || self.updates.as_ref().is_some_and(|u| u.checking);
        let update_poll = if busy {
            UPDATE_STATUS_POLL_BUSY
        } else {
            UPDATE_STATUS_POLL
        };
        let update_status_due = self
            .update_status_polled_at
            .is_none_or(|t| t.elapsed() >= update_poll);
        if page.is_some() && update_status_due {
            self.update_status_polled_at = Some(std::time::Instant::now());
            tasks.push(Task::perform(
                ipc::request(IpcRequest::GetUpdateStatus),
                Message::UpdateStatus,
            ));
        }
        // Settings' firmware row, Pad's firmware line and running slot, the
        // report; and on every page for a pad that needs its firmware updated
        // before anything else works
        if matches!(
            page,
            Some(Page::Settings | Page::Pad | Page::Troubleshooting)
        ) || (page.is_some() && self.incompatible.is_some())
        {
            tasks.push(Task::perform(
                ipc::request(IpcRequest::GetFirmwareUpdate),
                Message::FirmwareOffer,
            ));
        }
        // Tap rate statistics, only while the Stats page shows them
        if page == Some(Page::Stats) {
            tasks.extend(self.stats.poll());
        }
        if page == Some(Page::Troubleshooting)
            && self.diagnostics.active_tab == troubleshooting::Tab::Logs
        {
            let since_seq = if self.latest_log_seq > 0 {
                Some(self.latest_log_seq)
            } else {
                None
            };
            tasks.push(Task::perform(
                ipc::request(IpcRequest::GetLogEntries {
                    since_seq,
                    limit: 200,
                    level: None,
                    source: None,
                }),
                Message::Logs,
            ));
        }
        Task::batch(tasks)
    }

    fn update_tray(&self) {
        if let Some(handle) = &self.tray {
            tray::update(
                handle,
                tray::TrayStatus {
                    daemon_online: self.daemon_online,
                    device_connected: self.device_connected,
                    incompatible: self.incompatible.is_some(),
                    last_sync_error: self.last_sync_error.clone(),
                    is_playing_or_cooldown: matches!(
                        self.mode,
                        RuntimeMode::Playing | RuntimeMode::Cooldown
                    ),
                    tosu_connected: self.tosu_connected,
                    total_presses: pages::total_presses(self),
                    mode: pages::mode_words(self.mode).to_string(),
                },
            );
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Navigate(page) => return self.go(page.at()),
            Message::Open(target) => return self.go(target),
            Message::OpenUpdates => return self.go(Target::UPDATES),
            Message::LeaveAnswer(answer) => {
                let Some(target) = self.leave_prompt.take() else {
                    return Task::none();
                };
                return match answer {
                    // Save: the result comes back as its own notice, and a
                    // refused save leaves the settings unsaved on their page
                    Some(true) => {
                        let save = self.update(Message::SavePad);
                        Task::batch([save, self.arrive(target)])
                    }
                    Some(false) => {
                        self.revert_pad_form();
                        self.arrive(target)
                    }
                    None => Task::none(),
                };
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
                        last_backup,
                        foreign_pad: _,
                        he_calibration,
                        key3,
                    }) => {
                        let was_offline = !self.daemon_online;
                        let device_just_connected = !self.device_connected && device_connected;
                        if device_just_connected || !device_connected {
                            self.cal_epoch = self.cal_epoch.wrapping_add(1);
                            self.cal_progress = None;
                            self.cal_starting = false;
                        }
                        self.daemon_online = true;
                        self.daemon_spawn_attempted = false;
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
                        self.he_calibration = he_calibration;
                        self.key3 = key3;
                        self.last_sync_time = last_sync_time;
                        self.last_backup = last_backup;
                        self.adopt_status_config(device_connected, device_just_connected, config);

                        self.update_tray();
                        if was_offline && self.designer.status.contains("Daemon unavailable") {
                            return self.designer.reload().map(Message::Designer);
                        }
                    }
                    Err(e) if !self.version_fix_attempted && version_mismatch(&e).is_some() => {
                        // An update replaced OPad's files and only one of the
                        // two programs has restarted since: the older one
                        // restarts to become the new version too. Once per run.
                        self.version_fix_attempted = true;
                        let (gui, daemon) = version_mismatch(&e).unwrap_or_default();
                        if opad_update_version_newer(&gui, &daemon) {
                            tracing::info!("The daemon is still {daemon}; restarting it as {gui}");
                            return Task::perform(restart_daemon_process(), Message::DaemonStarted);
                        }
                        tracing::info!(
                            "This GUI is {gui} and the daemon {daemon}; restarting the GUI"
                        );
                        restart_gui();
                    }
                    _ => {
                        self.daemon_online = false;
                        self.device_connected = false;
                        self.cal_epoch = self.cal_epoch.wrapping_add(1);
                        self.cal_progress = None;
                        self.cal_starting = false;
                        self.tosu_connected = false;
                        if !self.daemon_spawn_attempted {
                            self.daemon_spawn_attempted = true;
                            return Task::perform(start_daemon_process(), Message::DaemonStarted);
                        }
                        self.update_tray();
                    }
                }
            }
            Message::UiValues(result) => {
                if let Ok(IpcResponse::UiValues(values)) = result {
                    self.ui_values = values.into_iter().collect();
                }
            }
            Message::TapStats(result) => self.stats.received(result),
            Message::SetTapPeriod(days) => return self.stats.set_period(days),
            Message::TapPeriodSet(result) => self.stats.period_set(result),
            Message::Logs(result) => {
                if let Ok(IpcResponse::LogEntries {
                    entries,
                    latest_seq,
                }) = result
                {
                    self.latest_log_seq =
                        next_log_cursor(self.latest_log_seq, &entries, latest_seq);
                    for entry in entries {
                        if !self
                            .logs
                            .iter()
                            .any(|e| e.source == entry.source && e.seq == entry.seq)
                        {
                            self.logs.push(entry);
                        }
                    }
                    self.prune_logs();
                }
            }
            Message::FilterLogLevel(lvl) => {
                self.log_filter_level = lvl;
            }
            Message::FilterLogSource(src) => {
                self.log_filter_source = src;
            }
            Message::ClearLogs => {
                self.log_cleared_at = Some(chrono::Local::now());
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
                Ok(path) => self.notify(Tone::Good, format!("Log saved to {path}")),
                Err(e) if e.is_empty() => {}
                Err(e) => self.fail("The log could not be saved.", e),
            },
            Message::ToggleAutoScroll => {
                self.log_auto_scroll = !self.log_auto_scroll;
            }
            Message::ExportBackup => {
                if matches!(self.mode, RuntimeMode::Playing | RuntimeMode::Cooldown) {
                    self.notify(
                        Tone::Bad,
                        "Not during a map: export the backup once it has ended.",
                    );
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
                    self.notify(Tone::Bad, format!("The backup was not exported: {reason}"));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.fail("The backup could not be read. Try again.", e);
                }
                _ => {}
            },
            Message::ExportBackupDone(result) => match result {
                Ok(path) => self.notify(Tone::Good, format!("Backup saved to {path}")),
                Err(e) if e.is_empty() => {}
                Err(e) => self.fail("The backup file could not be written.", e),
            },
            Message::StartImportBackup => {
                if matches!(self.mode, RuntimeMode::Playing | RuntimeMode::Cooldown) {
                    self.notify(
                        Tone::Bad,
                        "Not during a map: import the backup once it has ended.",
                    );
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
                Err(e) => self.notify(Tone::Bad, e),
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
                    self.notify(Tone::Bad, format!("The backup was not imported: {reason}"));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.fail("The backup could not be checked. Try again.", e);
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
                    self.adopt_config(&config);
                    self.config = config;
                    self.notify(Tone::Good, "Backup imported.");
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.notify(Tone::Bad, format!("The backup was not imported: {reason}"));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.fail("The backup could not be imported.", e);
                }
                _ => {}
            },
            Message::PromptRestoreDeviceFromPc => {
                self.recovery_modal = Some(RecoveryAction::RestoreDeviceFromPc);
            }
            Message::ConfirmRestoreDeviceFromPc => {
                self.recovery_modal = None;
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
                    self.notify(Tone::Good, "The PC and the pad now have the same counters.");
                }
                Ok(IpcResponse::OperationRejected { reason }) => {
                    self.notify(Tone::Bad, format!("Not done: {reason}"));
                }
                Ok(IpcResponse::Error(e)) | Err(e) => {
                    self.fail("The counters could not be changed. Try again.", e);
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
            Message::StartCalibration => {
                self.cal_epoch = self.cal_epoch.wrapping_add(1);
                self.cal_starting = true;
                self.cal_progress = None;
                self.cal_status_msg = Some("Starting calibration on pad...".to_string());
                let epoch = self.cal_epoch;
                return Task::perform(
                    ipc::request(IpcRequest::StartHeCalibration),
                    move |response| Message::StartCalibrationDone(epoch, response),
                );
            }
            Message::StartCalibrationDone(epoch, res) => {
                if epoch != self.cal_epoch {
                    return Task::none();
                }
                self.cal_starting = matches!(res, Ok(IpcResponse::HeCalibrationStarted));
                match res {
                    Ok(IpcResponse::HeCalibrationStarted) => {
                        self.cal_status_msg = None;
                    }
                    Ok(IpcResponse::Error(e)) => {
                        self.cal_status_msg = Some(format!("Error: {e}"));
                    }
                    Err(e) => {
                        self.cal_status_msg = Some(format!("Daemon error: {e}"));
                    }
                    _ => {}
                }
            }
            Message::CancelCalibration => {
                self.cal_epoch = self.cal_epoch.wrapping_add(1);
                self.cal_starting = false;
                self.cal_status_msg = Some("Cancelling calibration...".to_string());
                let epoch = self.cal_epoch;
                return Task::perform(
                    ipc::request(IpcRequest::CancelHeCalibration),
                    move |response| Message::CancelCalibrationDone(epoch, response),
                );
            }
            Message::CancelCalibrationDone(epoch, res) => {
                if epoch != self.cal_epoch {
                    return Task::none();
                }
                match res {
                    Ok(IpcResponse::HeCalibrationCancelled) => {
                        self.cal_status_msg = Some("Calibration cancelled.".to_string());
                        self.cal_progress = None;
                    }
                    Ok(IpcResponse::Error(e)) => {
                        self.cal_status_msg = Some(format!("Error: {e}"));
                    }
                    Err(e) => {
                        self.cal_status_msg = Some(format!("Daemon error: {e}"));
                    }
                    _ => {}
                }
            }
            Message::Keys(msg) => return keys::update(self, msg),
            Message::PollCalibration => {
                if !self.device_connected {
                    if self.cal_progress.is_some() {
                        self.cal_progress = None;
                        self.cal_status_msg = Some("Pad disconnected.".to_string());
                    }
                    return Task::none();
                }
                let epoch = self.cal_epoch;
                return Task::perform(
                    ipc::request(IpcRequest::GetHeCalibrationProgress),
                    move |response| Message::CalibrationProgress(epoch, response),
                );
            }
            Message::CalibrationProgress(epoch, res) => {
                if epoch != self.cal_epoch {
                    return Task::none();
                }
                if let Ok(IpcResponse::HeCalibrationProgress(prog)) = res {
                    if prog.is_some() {
                        self.cal_starting = false;
                    }
                    self.cal_progress = prog;
                }
            }
            Message::StartDaemon => {
                self.notify(Tone::Info, "Starting the OPad service…");
                self.log_event(
                    LogSource::Program,
                    LogLevel::Info,
                    "gui",
                    "Launching opad-daemon process",
                );
                return Task::perform(start_daemon_process(), Message::DaemonStarted);
            }
            Message::DaemonStarted(res) => match res {
                Ok(()) => {
                    self.log_event(
                        LogSource::Program,
                        LogLevel::Info,
                        "gui",
                        "opad-daemon launched successfully",
                    );
                    return self.poll();
                }
                Err(e) => {
                    self.fail(
                        "The OPad service did not start. Try again, or restart your computer.",
                        &e,
                    );
                    self.log_event(
                        LogSource::Program,
                        LogLevel::Error,
                        "gui",
                        format!("Failed to start daemon: {}", e),
                    );
                }
            },
            Message::InstallUdevRule =>
            {
                #[cfg(target_os = "linux")]
                if let Some(appimage) = self.appimage_udev.clone() {
                    return Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                platform_linux::install_udev_rule_from_appimage(&appimage)
                            })
                            .await
                            .map_err(|e| e.to_string())?
                        },
                        Message::UdevRuleInstalled,
                    );
                }
            }
            Message::UdevRuleInstalled(res) => match res {
                Ok(()) => {
                    self.appimage_udev = None;
                    self.notify(
                        Tone::Good,
                        "Permissions installed. Unplug the pad and plug it back in if it is not found.",
                    );
                }
                Err(e) => self.fail("The permissions were not installed.", e),
            },
            Message::StartTosu => {
                self.notify(Tone::Info, "Starting tosu…");
                self.log_event(
                    LogSource::Program,
                    LogLevel::Info,
                    "gui",
                    "Launching tosu process",
                );
                return Task::perform(
                    start_tosu_process(self.tosu_override_path.clone()),
                    Message::TosuStarted,
                );
            }
            Message::TosuStarted(res) => match res {
                Ok(()) => {
                    self.notify(Tone::Good, "tosu started. Open osu! to connect.");
                    self.log_event(
                        LogSource::Program,
                        LogLevel::Info,
                        "gui",
                        "tosu launched successfully",
                    );
                    return self.poll();
                }
                Err(e) => {
                    self.fail("tosu did not start.", &e);
                    self.log_event(
                        LogSource::Program,
                        LogLevel::Error,
                        "gui",
                        format!("Failed to start tosu: {}", e),
                    );
                }
            },
            Message::InstallSystemdService => {
                #[cfg(target_os = "linux")]
                {
                    let bin = find_daemon_executable();
                    return Task::perform(
                        async move { platform_linux::install_systemd_user_service(&bin) },
                        Message::SystemdServiceInstalled,
                    );
                }
                #[cfg(windows)]
                {
                    let bin = find_daemon_executable();
                    return Task::perform(
                        async move { platform_windows::install_daemon_autostart(&bin) },
                        Message::SystemdServiceInstalled,
                    );
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    self.notify(
                        Tone::Bad,
                        "Starting the OPad service at login is not supported here.",
                    );
                }
            }
            Message::SystemdServiceInstalled(res) => match res {
                Ok(()) => {
                    #[cfg(windows)]
                    {
                        self.notify(Tone::Good, "The OPad service now starts when you sign in.");
                    }
                    #[cfg(not(windows))]
                    {
                        self.notify(Tone::Good, "The OPad service now starts with your session.");
                    }
                    return self.poll();
                }
                Err(e) => {
                    self.fail("The OPad service could not be set to start at login.", e);
                }
            },
            Message::CaptureKey(key) => {
                if self.device_connected && (1..=3).contains(&key) {
                    self.capturing_key = Some(key);
                    self.capture_hint = None;
                }
            }
            Message::CancelCapture => {
                self.capturing_key = None;
                self.capture_hint = None;
            }
            Message::CaptureKeyPressed(key) => {
                let Some(which) = self.capturing_key else {
                    return Task::none();
                };
                match captured_letter(&key) {
                    Captured::Letter(letter) => {
                        match which {
                            1 => self.k1_input = letter,
                            2 => self.k2_input = letter,
                            _ => self.k3_input = letter,
                        }
                        self.capturing_key = None;
                        self.capture_hint = None;
                    }
                    Captured::Cancel => {
                        self.capturing_key = None;
                        self.capture_hint = None;
                    }
                    Captured::Refused => {
                        self.capture_hint = Some("Pick a letter key (A to Z).");
                    }
                }
            }
            Message::ClearKey3 => {
                if self.device_connected {
                    self.k3_input.clear();
                }
            }
            Message::GameKey(key, on) => {
                if self.device_connected && (1..=3).contains(&key) {
                    let bit = 1u32 << (key - 1);
                    let next = if on {
                        self.game_keys | bit
                    } else {
                        self.game_keys & !bit
                    };
                    // One or two keys count: osu!standard plays with two, never three
                    if next != 0 && next.count_ones() <= 2 {
                        self.game_keys = next;
                    }
                }
            }
            Message::Key1Pin(pin) => {
                if self.device_connected {
                    self.k1_gpio = pin.gpio;
                }
            }
            Message::Key2Pin(pin) => {
                if self.device_connected {
                    self.k2_gpio = pin.gpio;
                }
            }
            Message::Swipe(dir, choice) => {
                if self.device_connected {
                    let s = &mut self.swipes[dir.index()];
                    *s = s.chosen(choice);
                }
            }
            Message::SwipeKey(dir, key) => {
                if self.device_connected {
                    let s = &mut self.swipes[dir.index()];
                    s.key = key.0;
                    s.modifiers = 0;
                }
            }
            Message::ResultAnimation(trigger, choice) => {
                self.result_animations.set(trigger, choice.0);
            }
            Message::PreviewAnimation(trigger) => {
                let request = IpcRequest::PlayAnimation {
                    animation: self.result_animations.get(trigger),
                    grade: trigger.label().to_string(),
                };
                return act(Action::PreviewAnimation, request);
            }
            Message::Debounce(v) => {
                if self.device_connected {
                    self.debounce = v;
                }
            }
            Message::Brightness(v) => {
                if self.device_connected {
                    self.brightness = v;
                }
            }
            Message::SleepSeconds(v) => {
                if self.device_connected {
                    self.sleep_seconds = v;
                }
            }
            Message::GameplayDisplayHz(v) => {
                if self.device_connected {
                    self.gameplay_display_hz = v;
                }
            }
            Message::StartDetectPin(key_id) => {
                if !self.device_connected {
                    return Task::none();
                }
                self.detecting_pin = Some(key_id);
                let exclude = if key_id == 1 {
                    self.k2_gpio
                } else {
                    self.k1_gpio
                };
                return Task::perform(
                    ipc::request(IpcRequest::DetectPin {
                        key_id,
                        timeout_ms: 10000,
                        exclude_gpio: exclude,
                    }),
                    Message::PinDetected,
                );
            }
            Message::PinDetected(res) => {
                self.detecting_pin = None;
                match res {
                    Ok(IpcResponse::PinDetected {
                        key_id,
                        gpio,
                        success: true,
                    }) => {
                        if key_id == 1 {
                            self.k1_gpio = gpio;
                        } else if key_id == 2 {
                            self.k2_gpio = gpio;
                        }
                        self.notify(
                            Tone::Good,
                            format!("Key {key_id} is on GPIO{gpio}. Save to pad to keep it."),
                        );
                    }
                    Ok(IpcResponse::PinDetected { key_id, .. }) => {
                        self.notify(
                            Tone::Bad,
                            format!("No press was seen for Key {key_id}. Try again and press its switch."),
                        );
                    }
                    Ok(IpcResponse::Error(e)) | Err(e) => {
                        self.fail("The pin could not be detected. Try again.", e);
                    }
                    _ => {}
                }
            }
            Message::CancelDetectPin => {
                self.detecting_pin = None;
            }
            Message::ToggleAutostartTray(enabled) => {
                self.autostart_tray = enabled;
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = platform_linux::set_gui_autostart_enabled(enabled) {
                        self.fail("Starting at login could not be changed.", e);
                    }
                }
                #[cfg(windows)]
                {
                    if let Err(e) = platform_windows::set_gui_autostart_enabled(enabled) {
                        self.fail("Starting at login could not be changed.", e);
                    }
                }
                #[cfg(not(any(target_os = "linux", windows)))]
                {
                    let _ = enabled;
                }
            }
            Message::SelectSettingsTab(tab) => {
                self.settings_tab = tab;
                if tab == settings::Tab::About {
                    self.tosu_version = settings::installed_tosu_version();
                }
                return self.poll();
            }
            Message::Diagnostics(msg) => match msg {
                diagnostics::DiagnosticsMessage::SelectTab(tab) => {
                    self.diagnostics.active_tab = tab;
                    return self.poll();
                }
                diagnostics::DiagnosticsMessage::ToggleSound(s) => {
                    self.diagnostics.sound_enabled = s;
                }
                diagnostics::DiagnosticsMessage::ResetInputTester => {
                    self.diagnostics.reset_input_tester();
                }
                diagnostics::DiagnosticsMessage::KeyEvent {
                    key_idx,
                    is_down,
                    at,
                } => {
                    self.diagnostics.handle_key_event_at(key_idx, is_down, at);
                }
                diagnostics::DiagnosticsMessage::He(event) => {
                    self.diagnostics.handle_he_event(event);
                }
                diagnostics::DiagnosticsMessage::PingDaemon => {
                    self.diagnostics.ping_in_progress = true;
                    let start = std::time::Instant::now();
                    return Task::perform(
                        async move {
                            match ipc::request(IpcRequest::GetStatus).await {
                                Ok(_) => Ok(start.elapsed()),
                                Err(e) => Err(e.to_string()),
                            }
                        },
                        |res| Message::Diagnostics(diagnostics::DiagnosticsMessage::PingDone(res)),
                    );
                }
                diagnostics::DiagnosticsMessage::RunBoardTest => {
                    self.diagnostics.board_test_running = true;
                    return Task::perform(
                        async {
                            // An older daemon cannot parse the request and drops the connection
                            match ipc::request(IpcRequest::RunBoardTest).await {
                                Ok(IpcResponse::BoardTest(report)) => Ok(report),
                                Ok(IpcResponse::Error(e)) => Err(e),
                                Ok(other) => Err(format!("Unexpected response: {:?}", other)),
                                Err(_) => Err("The daemon did not answer; it may predate the \
                                               board test. Update OPad and try again."
                                    .to_string()),
                            }
                        },
                        |res| {
                            Message::Diagnostics(diagnostics::DiagnosticsMessage::BoardTestDone(
                                res,
                            ))
                        },
                    );
                }
                diagnostics::DiagnosticsMessage::RestartPad => {
                    self.diagnostics.restart_running = true;
                    return Task::perform(
                        async {
                            // An older daemon cannot parse the request and drops the connection
                            match ipc::request(IpcRequest::RestartPad).await {
                                Ok(IpcResponse::PadRestarting) => Ok(()),
                                Ok(IpcResponse::Error(e)) => Err(e),
                                Ok(other) => Err(format!("Unexpected response: {:?}", other)),
                                Err(_) => Err("The daemon did not answer; it may predate \
                                               restarting the pad. Replug the pad's USB cable."
                                    .to_string()),
                            }
                        },
                        |res| {
                            Message::Diagnostics(diagnostics::DiagnosticsMessage::RestartDone(res))
                        },
                    );
                }
                diagnostics::DiagnosticsMessage::RestartDone(res) => {
                    self.diagnostics.restart_running = false;
                    self.diagnostics.restart = Some(res);
                }
                diagnostics::DiagnosticsMessage::BoardTestDone(res) => {
                    self.diagnostics.board_test_running = false;
                    self.diagnostics.board_test = Some(res);
                }
                diagnostics::DiagnosticsMessage::StartMagnetTest => {
                    self.diagnostics.magnet = Some(opad_model::board_test::MagnetTest::new());
                    self.diagnostics.magnet_running = true;
                    self.diagnostics.magnet_last = None;
                    self.diagnostics.magnet_error = None;
                    return magnet_tick();
                }
                diagnostics::DiagnosticsMessage::StopMagnetTest => {
                    self.diagnostics.magnet_running = false;
                }
                diagnostics::DiagnosticsMessage::MagnetTick(res) => match res {
                    Ok(report) => {
                        if let Some(test) = self.diagnostics.magnet.as_mut() {
                            test.update(&report);
                            if test.done() {
                                self.diagnostics.magnet_running = false;
                            }
                        }
                        self.diagnostics.magnet_last = Some(report);
                        if self.diagnostics.magnet_running {
                            return magnet_tick();
                        }
                    }
                    Err(e) => {
                        self.diagnostics.magnet_running = false;
                        self.diagnostics.magnet_error = Some(e);
                    }
                },
                diagnostics::DiagnosticsMessage::UseCarrierKeyPins => {
                    self.k1_gpio = opad_model::board_test::CARRIER_KEY1_GPIO;
                    self.k2_gpio = opad_model::board_test::CARRIER_KEY2_GPIO;
                    let config = DeviceConfig {
                        key1_gpio: self.k1_gpio,
                        key2_gpio: self.k2_gpio,
                        ..self.config.clone()
                    };
                    self.diagnostics.board_test_running = true;
                    return Task::perform(
                        async move {
                            match ipc::request(IpcRequest::UpdateConfig(config)).await {
                                Ok(IpcResponse::ConfigUpdated { .. }) => {}
                                Ok(IpcResponse::Error(e)) | Err(e) => {
                                    return Err(format!("Could not set the key pins: {e}"))
                                }
                                Ok(other) => {
                                    return Err(format!("Unexpected response: {:?}", other))
                                }
                            }
                            // The pad moves the pins once both keys are released
                            tokio::time::sleep(Duration::from_millis(300)).await;
                            match ipc::request(IpcRequest::RunBoardTest).await {
                                Ok(IpcResponse::BoardTest(report)) => Ok(report),
                                Ok(IpcResponse::Error(e)) | Err(e) => Err(e),
                                Ok(other) => Err(format!("Unexpected response: {:?}", other)),
                            }
                        },
                        |res| {
                            Message::Diagnostics(diagnostics::DiagnosticsMessage::BoardTestDone(
                                res,
                            ))
                        },
                    );
                }
                diagnostics::DiagnosticsMessage::PingDone(res) => {
                    self.diagnostics.ping_in_progress = false;
                    match res {
                        Ok(dur) => {
                            self.diagnostics.last_ping_ms = Some(dur.as_secs_f64() * 1000.0);
                        }
                        Err(e) => {
                            self.diagnostics.last_ping_ms = None;
                            self.fail("The OPad service did not answer.", e);
                        }
                    }
                }
            },
            Message::DiagnosticsKeyEvent { key, is_down } => {
                let key_str = match &key {
                    iced::keyboard::Key::Character(s) => s.to_string().to_uppercase(),
                    iced::keyboard::Key::Named(n) => format!("{:?}", n).to_uppercase(),
                    _ => String::new(),
                };
                let k1 = self.k1_input.trim().to_uppercase();
                let k2 = self.k2_input.trim().to_uppercase();
                if !k1.is_empty() && (key_str == k1 || (k1 == "SPACE" && key_str == "SPACE")) {
                    self.diagnostics.handle_key_event(1, is_down);
                }
                if !k2.is_empty() && (key_str == k2 || (k2 == "SPACE" && key_str == "SPACE")) {
                    self.diagnostics.handle_key_event(2, is_down);
                }
                let k3 = self.k3_input.trim().to_uppercase();
                let three = self.device_info.as_ref().is_some_and(|i| i.key_count >= 3);
                if three && !k3.is_empty() && key_str == k3 {
                    self.diagnostics.handle_key_event(3, is_down);
                }
            }
            Message::CopyDiagnosticBundle => {
                let bundle = diagnostics::generate_diagnostic_bundle(self, &self.diagnostics);
                self.notify(Tone::Good, "Report copied.");
                return iced::clipboard::write(bundle);
            }
            Message::SaveDiagnosticBundle => {
                let bundle = diagnostics::generate_diagnostic_bundle(self, &self.diagnostics);
                return Task::perform(
                    save_diagnostic_dialog(bundle),
                    Message::DiagnosticBundleSaved,
                );
            }
            Message::DiagnosticBundleSaved(res) => match res {
                Ok(path) => self.notify(Tone::Good, format!("Report saved to {path}")),
                Err(e) if e.is_empty() => {}
                Err(e) => self.fail("The report could not be saved.", e),
            },
            Message::SavePad => {
                if !self.device_connected || self.pad_sent.is_some() || !self.pad_dirty() {
                    return Task::none();
                }
                keys::begin_save(self);
                let config = self.form_config();
                self.pad_sent = Some(config.clone());
                return act(Action::SavePad, IpcRequest::UpdateConfig(config));
            }
            Message::RevertPad => self.revert_pad_form(),
            Message::OpenCalibration => return self.go(Target::CALIBRATION),
            Message::CloseCalibration => {
                self.calibrating = false;
                return self.poll();
            }
            Message::TogglePadDetails => self.pad_details = !self.pad_details,
            Message::Sync => {
                return act(Action::Sync, IpcRequest::ForceSync);
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
                return act(
                    Action::ResetCounters,
                    IpcRequest::ResetCounters { confirm: true },
                );
            }
            Message::ResolveReplacement(restore) => {
                return act(
                    Action::Replacement { restore },
                    IpcRequest::ResolveReplacement { restore },
                );
            }
            Message::ResolveTakeover(take_over, keep_device_counters) => {
                return act(
                    Action::Takeover { take_over },
                    IpcRequest::ResolveTakeover {
                        take_over,
                        keep_device_counters,
                    },
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
                    pending_version,
                    channel,
                    checking,
                    last_result,
                }) = res
                {
                    // An answer that is not "still checking" ends the fast polling
                    self.update_busy = false;
                    self.updates = Some(UpdateView {
                        app,
                        tosu,
                        last_check,
                        last_error,
                        restart_required,
                        pending_version,
                        channel: if channel.is_empty() {
                            "stable".to_string()
                        } else {
                            channel
                        },
                        checking,
                        last_result,
                    });
                }
            }
            Message::OpenUrl(url) => {
                // Only web pages: the URL comes from the signed feed, but a
                // browser is the only thing it may open
                if url.starts_with("https://") {
                    open_in_browser(&url);
                }
            }
            Message::ToggleUpdater(component, enabled) => {
                // Reflect it now; the next poll confirms what the daemon saved
                if let Some(u) = &mut self.updates {
                    match component {
                        opad_ipc::UpdateComponent::App => u.app.enabled = enabled,
                        opad_ipc::UpdateComponent::Tosu => u.tosu.enabled = enabled,
                        opad_ipc::UpdateComponent::Firmware => {}
                    }
                }
                if component == opad_ipc::UpdateComponent::Firmware {
                    if let Some(o) = &mut self.firmware_offer {
                        o.auto = enabled;
                    }
                }
                return act(
                    Action::UpdateToggle,
                    IpcRequest::SetUpdateEnabled { component, enabled },
                );
            }
            Message::InstallUpdate(component) => {
                self.update_busy = true;
                return act(
                    Action::InstallUpdate,
                    IpcRequest::InstallUpdate { component },
                );
            }
            Message::CheckForUpdates => {
                if let Some(u) = &mut self.updates {
                    u.checking = true;
                }
                self.update_busy = true;
                return act(Action::CheckUpdates, IpcRequest::CheckForUpdates);
            }
            Message::SetUpdateChannel(channel) => {
                if let Some(u) = &mut self.updates {
                    u.channel = channel.clone();
                    u.checking = true;
                }
                self.update_busy = true;
                return act(
                    Action::CheckUpdates,
                    IpcRequest::SetUpdateChannel { channel },
                );
            }
            Message::ResetLatency => {
                return act(Action::ResetLatency, IpcRequest::ResetLatencyStats);
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
                    if let Ok(d) = opad_model::paths::data_dir() {
                        let _ = std::fs::create_dir_all(&d);
                        let _ = std::fs::write(d.join("tosu_path"), p.to_string_lossy().as_bytes());
                    }
                    std::env::set_var("OPAD_TOSU_PATH", &p);
                    self.notify(Tone::Good, format!("Using your tosu: {}", p.display()));
                    self.tosu_override_path = Some(p);
                }
            }
            Message::ResetTosuPath => {
                if let Ok(d) = opad_model::paths::data_dir() {
                    let _ = std::fs::remove_file(d.join("tosu_path"));
                }
                std::env::remove_var("OPAD_TOSU_PATH");
                std::env::remove_var("OPAD_TOSU_PATH");
                self.tosu_override_path = None;
                self.notify(Tone::Good, "Using the tosu that comes with OPad.");
            }
            Message::ActionDone(action, result) => {
                if action == Action::SavePad {
                    let sent = self.pad_sent.take();
                    if let Ok(IpcResponse::ConfigUpdated { config, .. }) = &result {
                        // Edited again while the save was on its way: those
                        // edits stay, unsaved. Otherwise the pages show what
                        // the pad now has, which is what was sent unless the
                        // pad adjusted a value.
                        let untouched = sent.as_ref() == Some(&self.form_config());
                        self.keys.save_finished(true);
                        self.config = config.clone();
                        if untouched {
                            self.adopt_config(config);
                            self.keys.adopt(config);
                        }
                    } else {
                        self.keys.save_finished(false);
                    }
                }
                if let Some((tone, text, raw)) = action_notice(action, result) {
                    if let Some(raw) = raw {
                        self.log_event(LogSource::Program, LogLevel::Error, "gui", raw);
                    }
                    self.notify(tone, text);
                }
                // An install may just have changed the update state
                self.update_status_polled_at = None;
                return self.poll();
            }
            Message::DismissNotice(id) => self.notices.retain(|n| n.id != id),
            Message::HoverNotice(id) => self.hovered_notice = id,
            Message::NoticeTick => {
                let now = std::time::Instant::now();
                // Being read: it gets its time again once the pointer leaves
                if let Some(n) = self
                    .notices
                    .iter_mut()
                    .find(|n| Some(n.id) == self.hovered_notice)
                {
                    n.until = now + n.tone.stays();
                }
                self.notices.retain(|n| n.until > now);
                if self.notices.is_empty() {
                    self.hovered_notice = None;
                }
            }
            Message::Designer(msg) => return self.designer.update(msg).map(Message::Designer),

            Message::WindowOpened(_) => return self.poll(),
            Message::CloseRequested(id) => {
                match self.tray_available {
                    Some(true) => {}
                    // Nowhere to live without a window; exit cleanly to avoid zombie background process
                    Some(false) => return iced::exit(),
                    // Still probing: hide now, and exit only once it is known
                    // there is no tray to live in
                    None => self.exit_if_no_tray = true,
                }
                self.window = None;
                return window::close(id);
            }
            Message::TrayTimeout => {
                if self.tray_available.is_none() {
                    tracing::warn!("Tray host not detected within 2 seconds; falling back to standard window lifecycle");
                    self.tray_available = Some(false);
                    if self.exit_if_no_tray {
                        // Closed during the probe: the tray stream's own
                        // verdict (Started or Unavailable, a few seconds at
                        // most) decides between hiding and exiting
                        return Task::none();
                    }
                    self.no_tray = true;
                    if self.window.is_none() {
                        return self.open_fallback_window();
                    }
                }
            }
            Message::ShowRequested(target) => {
                let open = self.open_window();
                return match target.as_deref().and_then(Target::from_token) {
                    Some(target) => Task::batch([open, self.go(target)]),
                    None => open,
                };
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
                    self.exit_if_no_tray = false;
                    // A late tray (retried in tray::stream): a --tray start
                    // goes back to living there
                    let hide = if self.fallback_window {
                        self.fallback_window = false;
                        self.window.take().map(window::close)
                    } else {
                        None
                    };
                    // The tray can come up after the 2 s fallback already fired
                    self.no_tray = false;
                    self.update_tray();
                    if let Some(hide) = hide {
                        return hide;
                    }
                }
                tray::TrayEvent::Unavailable => {
                    self.tray_available = Some(false);
                    if self.exit_if_no_tray {
                        return iced::exit();
                    }
                    self.no_tray = true;
                    if self.window.is_none() {
                        return self.open_fallback_window();
                    }
                }
                tray::TrayEvent::Action(tray::TrayAction::ShowWindow) => return self.open_window(),
                tray::TrayEvent::Action(tray::TrayAction::Open(page)) => {
                    let open = self.open_window();
                    return Task::batch([open, self.go(page.at())]);
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
        if !self.notices.is_empty() {
            subscriptions.push(iced::time::every(NOTICE_TICK).map(|_| Message::NoticeTick));
        }
        if self.window.is_some() && self.capturing_key.is_some() {
            subscriptions.push(iced::event::listen().filter_map(|event| match event {
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                    Some(Message::CaptureKeyPressed(key))
                }
                _ => None,
            }));
        }
        // The key test listens to the keyboard and the pad's own record
        if self.window.is_some()
            && self.page == Page::Troubleshooting
            && self.diagnostics.active_tab == troubleshooting::Tab::KeyTest
        {
            subscriptions.push(iced::event::listen().filter_map(|event| {
                if let iced::Event::Keyboard(key_event) = event {
                    match key_event {
                        iced::keyboard::Event::KeyPressed { key, .. } => {
                            Some(Message::DiagnosticsKeyEvent { key, is_down: true })
                        }
                        iced::keyboard::Event::KeyReleased { key, .. } => {
                            Some(Message::DiagnosticsKeyEvent {
                                key,
                                is_down: false,
                            })
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            }));
            #[cfg(any(windows, target_os = "linux"))]
            subscriptions.push(
                Subscription::run_with(
                    (self.k1_input.clone(), self.k2_input.clone()),
                    diagnostics::switch_input_stream,
                )
                .map(Message::Diagnostics),
            );
            // The HE debug firmware's own record of each press (he_monitor.rs)
            subscriptions.push(
                Subscription::run(he_monitor::stream)
                    .map(|e| Message::Diagnostics(diagnostics::DiagnosticsMessage::He(e))),
            );
        }
        if self.window.is_some() && self.page == Page::Keys && !self.calibrating {
            // The live switch view: the pad sends ~50 samples/s, and the
            // stems are redrawn every display frame so they glide
            subscriptions.push(
                iced::time::every(Duration::from_millis(20))
                    .map(|_| Message::Keys(keys::KeysMessage::PollLive)),
            );
            subscriptions
                .push(window::frames().map(|at| Message::Keys(keys::KeysMessage::Frame(at))));
        }
        if self.window.is_some() && self.page == Page::Keys && self.calibrating {
            subscriptions.push(
                iced::time::every(Duration::from_millis(100)).map(|_| Message::PollCalibration),
            );
        }
        Subscription::batch(subscriptions)
    }

    fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let mut nav = column![].spacing(4);
        for (group, pages) in Page::GROUPS {
            nav = nav.push(container(theme::caption(group.to_uppercase())).padding(
                iced::Padding {
                    top: 10.0,
                    left: 14.0,
                    ..iced::Padding::ZERO
                },
            ));
            for (page, label) in pages {
                nav = nav.push(
                    button(text(*label).size(15))
                        .width(Length::Fill)
                        .padding([9, 14])
                        .style(theme::nav(self.page == *page))
                        .on_press(Message::Navigate(*page)),
                );
            }
        }

        // The two things a player asks about. Each row only opens the page
        // where that thing is looked after; nothing is started from here.
        let status_row = |on: bool, label: String, target: Target| -> Element<'_, Message> {
            button(
                row![
                    container(Space::new().width(10).height(10)).style(theme::dot(on)),
                    text(label).size(13),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([6, 8])
            .style(theme::sidebar_status)
            .on_press(Message::Open(target))
            .into()
        };

        let sidebar = container(
            column![
                nav,
                Space::new().height(Length::Fill),
                if self.no_tray {
                    Element::from(
                        container(theme::caption(
                            "No system tray found: closing this window quits OPad. The pad keeps working.",
                        ))
                        .padding([6, 8]),
                    )
                } else {
                    Space::new().height(0).into()
                },
                status_row(
                    self.device_connected,
                    if self.device_connected {
                        "Pad connected".to_string()
                    } else {
                        "Pad not connected".to_string()
                    },
                    Page::Pad.at(),
                ),
                status_row(
                    self.tosu_connected,
                    if self.tosu_connected {
                        "osu! connected".to_string()
                    } else {
                        "osu! not connected".to_string()
                    },
                    Target::OSU,
                ),
            ]
            .spacing(4)
            .padding(18),
        )
        .width(220)
        .height(Length::Fill)
        .style(theme::sidebar);

        let page: Element<'_, Message> = match self.page {
            Page::Dashboard => pages::dashboard(self),
            Page::Stats => stats::view(self),
            Page::Keys => keys::view(self),
            Page::Screen => screen::view(self),
            Page::Designer => self.designer.view().map(Message::Designer),
            Page::Pad => pages::pad(self),
            Page::Settings => settings::view(self),
            Page::Troubleshooting => self.diagnostics.view(self),
        };

        let mut main = column![]
            .spacing(14)
            .padding(24)
            .width(Length::Fill)
            .height(Length::Fill);

        if !self.daemon_online {
            let mut offline_actions = row![button(text("Start it").size(13))
                .padding([6, 12])
                .style(theme::primary)
                .on_press(Message::StartDaemon),]
            .spacing(8)
            .align_y(Alignment::Center);

            #[cfg(target_os = "linux")]
            {
                offline_actions = offline_actions.push(
                    button(text("Start it with my session").size(13))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::InstallSystemdService),
                );
            }
            // Same button, same message; on Windows it writes the Run value
            #[cfg(windows)]
            if !platform_windows::daemon_autostart_installed() {
                offline_actions = offline_actions.push(
                    button(text("Start it when I sign in").size(13))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::InstallSystemdService),
                );
            }

            main = main.push(
                container(
                    row![
                        text("OPad's service is not running. The pad still types; counting, its screen and its settings wait for the service.")
                            .size(14)
                            .color(theme::YELLOW)
                            .width(Length::Fill),
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
                        text(format!("OPad cannot open its data on this PC, so counters and backups are not being saved here. The pad keeps its own count. ({err})"))
                            .size(14)
                            .color(theme::RED),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([10, 16])
                .style(theme::banner),
            );
        }
        if let Some(incompat) = &self.incompatible {
            let (message, fix) = incompatible_banner(incompat, self.firmware_offer.as_ref());
            let mut line = row![text(message)
                .size(14)
                .color(theme::YELLOW)
                .width(Length::Fill)]
            .spacing(12)
            .align_y(Alignment::Center);
            match fix {
                IncompatibleFix::UpdatePad => {
                    line = line.push(
                        button(text("Update the pad").size(13))
                            .style(theme::primary)
                            .on_press(Message::PromptFirmwareConsent),
                    );
                }
                IncompatibleFix::OpenUpdates => {
                    line = line.push(
                        button(text("Open Updates").size(13))
                            .style(theme::secondary)
                            .on_press(Message::OpenUpdates),
                    );
                }
                IncompatibleFix::Wait => {}
            }
            main = main.push(container(line).padding([8, 14]).style(theme::banner));
        }
        if let Some(old_id) = &self.pending_replacement {
            main = main.push(
                container(
                    row![
                        text(format!(
                            "This looks like a new pad. Bring over the counts from your old pad ({old_id})?"
                        ))
                        .size(14)
                        .width(Length::Fill),
                        button(text("Yes, bring them over").size(13))
                            .style(theme::primary)
                            .on_press(Message::ResolveReplacement(true)),
                        Space::new().width(8),
                        button(text("Start from zero").size(13))
                            .style(theme::secondary)
                            .on_press(Message::ResolveReplacement(false)),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if let Some(v) = self
            .updates
            .as_ref()
            .filter(|u| u.restart_required)
            .and_then(|u| u.pending_version.clone())
        {
            // §U-2: the update is installed (or, on Windows, waiting for the
            // next start) and these processes are still the old ones. Nothing
            // is restarted for the user; say when the new version arrives.
            main = main.push(
                container(
                    row![text(if cfg!(windows) {
                        format!("OPad {v} is ready and installs the next time OPad starts.")
                    } else {
                        format!("OPad {v} is installed and runs the next time OPad starts.")
                    })
                    .size(14),]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if self.appimage_udev.is_some() {
            main = main.push(
                container(
                    row![
                        text(
                            "OPad is not allowed to talk to the pad yet: its permissions \
                             are not installed on this computer."
                        )
                        .size(14)
                        .width(Length::Fill),
                        button(text("Install (asks for your password)").size(13))
                            .style(theme::primary)
                            .on_press(Message::InstallUdevRule),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        if let Some(t) = &self.pending_takeover {
            // §W3-3: friction-light on purpose. One question, one recommended
            // answer; the other way round is one click further.
            main = main.push(
                container(
                    column![
                        text("This pad was set up on another computer. Use it here?").size(14),
                        text(format!(
                            "The pad's counts: {} / {}   ·   this computer's: {} / {}",
                            pages::grouped(t.device_key1),
                            pages::grouped(t.device_key2),
                            pages::grouped(t.pc_key1),
                            pages::grouped(t.pc_key2),
                        ))
                        .size(13)
                        .color(theme::MUTED),
                        Space::new().height(6),
                        row![
                            button(text("Use it here").size(13))
                                .style(theme::primary)
                                .on_press(Message::ResolveTakeover(true, true)),
                            Space::new().width(8),
                            button(text("Use this computer's counts instead").size(13))
                                .style(theme::secondary)
                                .on_press(Message::ResolveTakeover(true, false)),
                            Space::new().width(8),
                            button(text("Not now").size(13))
                                .style(theme::secondary)
                                .on_press(Message::ResolveTakeover(false, false)),
                        ]
                        .align_y(Alignment::Center),
                        Space::new().height(4),
                        text(
                            "Use it here keeps the pad's counts. Not now leaves the pad as it \
                             is: it still types, and OPad does not count or configure it."
                        )
                        .size(13)
                        .color(theme::MUTED),
                    ]
                    .spacing(2),
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
        let framed: Element<'_, Message> = if self.notices.is_empty() {
            framed.into()
        } else {
            stack![framed, notices_layer(&self.notices)].into()
        };

        if self.leave_prompt.is_some() {
            let modal_box = container(
                column![
                    text("Save your changes?").size(20).font(theme::FONT_BOLD),
                    text("Some settings on this page are not saved to the pad yet.")
                        .size(14)
                        .color(theme::MUTED),
                    Space::new().height(14),
                    row![
                        button(text("Stay").size(14))
                            .padding([10, 20])
                            .style(theme::secondary)
                            .on_press(Message::LeaveAnswer(None)),
                        Space::new().width(Length::Fill),
                        button(text("Discard").size(14))
                            .padding([10, 20])
                            .style(theme::secondary)
                            .on_press(Message::LeaveAnswer(Some(false))),
                        Space::new().width(10),
                        button(text("Save to pad").size(14))
                            .padding([10, 20])
                            .style(theme::primary)
                            .on_press(Message::LeaveAnswer(Some(true))),
                    ]
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .padding(24)
                .width(460),
            )
            .style(theme::card);
            return modal_over(framed, modal_box.into(), self.maximized);
        }

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

            return modal_over(framed, modal_box.into(), self.maximized);
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

            let mut table = column![
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
            // Key 3 and the game keys, when the backup carries them (a three-key pad's)
            let inc = &modal.incoming;
            if inc.config.key3.is_some() || inc.stats.lifetime_key3.is_some() {
                let cur_k3 = modal.current.as_ref().and_then(|c| c.lifetime_key3);
                let k3_color = match (inc.stats.lifetime_key3, cur_k3) {
                    (Some(i), Some(c)) if i < c => theme::RED,
                    _ => theme::WHITE,
                };
                let cur_k3_str = modal
                    .current
                    .as_ref()
                    .map(|c| match cur_k3 {
                        Some(n) => format!("{} ({} presses)", c.config.key3_char(), n),
                        None => c.config.key3_char(),
                    })
                    .unwrap_or_else(|| "-".to_string());
                let inc_k3_str = format!(
                    "{} ({})",
                    inc.config.key3.as_deref().unwrap_or("kept"),
                    inc.stats
                        .lifetime_key3
                        .map_or("count kept".to_string(), |n| format!("{n} presses"))
                );
                table = table.push(row![
                    text("Key 3").size(13).width(Length::FillPortion(2)),
                    text(cur_k3_str).size(13).width(Length::FillPortion(3)),
                    text(inc_k3_str)
                        .size(13)
                        .color(k3_color)
                        .width(Length::FillPortion(3)),
                ]);
            }
            if let Some(game) = inc.config.game_keys {
                table = table.push(row![
                    text("Game keys").size(13).width(Length::FillPortion(2)),
                    text(
                        modal
                            .current
                            .as_ref()
                            .map(|c| opad_model::game_keys_names(c.config.game_keys))
                            .unwrap_or_else(|| "-".to_string())
                    )
                    .size(13)
                    .width(Length::FillPortion(3)),
                    text(opad_model::game_keys_names(game))
                        .size(13)
                        .width(Length::FillPortion(3)),
                ]);
            }

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

            return modal_over(framed, modal_box.into(), self.maximized);
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

            return modal_over(framed, modal_box.into(), self.maximized);
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
                            text("You can reflash the pad over USB via opadctl flash without loss of lifetime press stats.")
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

            return modal_over(framed, modal_box.into(), self.maximized);
        }

        if self.maximized {
            framed
        } else {
            stack![framed, chrome::resize_edges()].into()
        }
    }

    pub fn log_event(
        &mut self,
        source: LogSource,
        level: LogLevel,
        target: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.local_log_seq = self.local_log_seq.saturating_add(1);
        let entry = LogEntry::new(self.local_log_seq, source, level, target, message);
        self.logs.push(entry);
        self.prune_logs();
    }

    /// Float a notice over the page; it goes away by itself
    pub fn notify(&mut self, tone: Tone, text: impl Into<String>) {
        self.next_notice = self.next_notice.wrapping_add(1);
        self.notices.push(Notice {
            id: self.next_notice,
            tone,
            text: text.into(),
            until: std::time::Instant::now() + tone.stays(),
        });
        if self.notices.len() > MAX_NOTICES {
            let excess = self.notices.len() - MAX_NOTICES;
            self.notices.drain(..excess);
        }
    }

    /// Something did not work: say what and what to do in a notice, and put
    /// the raw error where it helps, in Logs
    pub fn fail(&mut self, text: &str, raw: impl std::fmt::Display) {
        self.log_event(
            LogSource::Program,
            LogLevel::Error,
            "gui",
            format!("{text} ({raw})"),
        );
        self.notify(Tone::Bad, text);
    }

    /// Show the page and tab `target` names
    fn show(&mut self, target: Target) {
        self.page = target.page;
        match target.section {
            Section::Any => {}
            Section::Settings(tab) => self.settings_tab = tab,
            Section::Trouble(tab) => self.diagnostics.active_tab = tab,
            Section::Calibration => {
                self.calibrating = true;
                self.cal_status_msg = None;
            }
        }
    }

    /// Go to `target`. Leaving Keys or Screen with settings not saved to the
    /// pad asks first (Save, Discard or Stay); moving between those two does
    /// not, since the save bar and what it holds come along.
    fn go(&mut self, target: Target) -> Task<Message> {
        if self.page.edits_pad_settings() && !target.page.edits_pad_settings() && self.pad_dirty() {
            self.leave_prompt = Some(target);
            return Task::none();
        }
        self.arrive(target)
    }

    fn arrive(&mut self, target: Target) -> Task<Message> {
        let page = target.page;
        // A calibration that is not running does not stay open behind the
        // other pages
        let calibration_running = self.cal_starting
            || self.cal_progress.as_ref().is_some_and(|p| {
                !matches!(
                    p.stage,
                    opad_model::HeCalStage::None
                        | opad_model::HeCalStage::Success
                        | opad_model::HeCalStage::Failed
                )
            });
        if page != Page::Keys && !calibration_running {
            self.calibrating = false;
        }
        self.show(target);
        self.capturing_key = None;
        self.capture_hint = None;
        if page != Page::Keys || self.calibrating {
            self.keys.stop_live();
        }
        if page == Page::Settings {
            // The Updates tab lives here; show it fresh
            self.update_status_polled_at = None;
            self.tosu_version = settings::installed_tosu_version();
        }
        if page == Page::Stats {
            self.stats.reset_backoff();
        }
        let mut tasks = vec![self.poll()];
        // Layouts change outside the designer too (opadctl, another
        // GUI, an update adding sources): show what the daemon holds
        // now, unless that would throw away edits not applied yet
        if page == Page::Designer && self.daemon_online && !self.designer.has_unapplied_edits() {
            tasks.push(self.designer.reload().map(Message::Designer));
        }
        Task::batch(tasks)
    }

    /// The pad's settings as the Keys and Screen pages have them now: what
    /// Save to pad sends, and what "unsaved changes" is measured against
    pub fn form_config(&self) -> DeviceConfig {
        let mut config = DeviceConfig {
            key1_hid_usage: char_to_hid_usage(&self.k1_input).unwrap_or(self.config.key1_hid_usage),
            key2_hid_usage: char_to_hid_usage(&self.k2_input).unwrap_or(self.config.key2_hid_usage),
            // Empty: key 3 sends nothing
            key3_hid_usage: if self.k3_input.trim().is_empty() {
                0
            } else {
                char_to_hid_usage(&self.k3_input).unwrap_or(self.config.key3_hid_usage)
            },
            game_keys: self.game_keys,
            debounce_us: self.debounce,
            key1_gpio: self.k1_gpio,
            key2_gpio: self.k2_gpio,
            brightness: self.brightness,
            display_sleep_seconds: self.sleep_seconds,
            gameplay_display_hz: self.gameplay_display_hz,
            result_animations: self.result_animations,
            ..self.config.clone()
        };
        for dir in SwipeDir::ALL {
            config.set_swipe(dir, self.swipes[dir.index()]);
        }
        keys::apply_edit(self, &mut config);
        config
    }

    /// Keys or Screen hold a pad setting that Save to pad has not sent yet
    pub fn pad_dirty(&self) -> bool {
        self.device_connected && self.config_loaded && self.form_config() != self.config
    }

    /// Back to what the pad has
    fn revert_pad_form(&mut self) {
        let config = self.config.clone();
        self.adopt_config(&config);
        let _ = keys::update(self, keys::KeysMessage::Revert);
        self.capturing_key = None;
        self.capture_hint = None;
    }

    /// The pad's settings from a status poll. The pages follow them when the
    /// pad connects and when they change elsewhere (opadctl, a backup), but
    /// never over edits not saved yet: those stay until Save to pad or Revert.
    fn adopt_status_config(&mut self, connected: bool, just_connected: bool, config: DeviceConfig) {
        // Against the settings the pad had until now
        let unsaved = self.pad_dirty();
        if connected
            && (!self.config_loaded || just_connected || (self.config != config && !unsaved))
        {
            self.config_loaded = true;
            self.adopt_config(&config);
        }
        if connected {
            self.keys.adopt(&config);
        } else {
            self.keys.stop_live();
        }
        self.config = config;
    }

    /// Loads every Keys and Screen field from `config`. The one place that
    /// does, so an import and a pad (re)connect cannot disagree on which
    /// fields exist (an import once left gameplay_display_hz behind).
    fn adopt_config(&mut self, config: &DeviceConfig) {
        self.k1_input = config.key1_char();
        self.k2_input = config.key2_char();
        self.k3_input = if config.key3_hid_usage == 0 {
            String::new()
        } else {
            config.key3_char()
        };
        self.game_keys = config.game_keys;
        self.k1_gpio = config.key1_gpio;
        self.k2_gpio = config.key2_gpio;
        self.swipes = SwipeDir::ALL.map(|d| config.swipe(d));
        self.result_animations = config.result_animations;
        self.debounce = config.debounce_us;
        self.brightness = config.brightness;
        self.sleep_seconds = config.display_sleep_seconds;
        self.gameplay_display_hz = config.gameplay_display_hz;
    }

    /// Whether Clear left this entry visible
    pub fn log_visible_after_clear(&self, entry: &LogEntry) -> bool {
        self.log_cleared_at.is_none_or(|t| entry.ts > t)
    }

    pub fn prune_logs(&mut self) {
        let cutoff = chrono::Local::now() - chrono::Duration::hours(24);
        self.logs.retain(|e| e.ts >= cutoff);
        if self.logs.len() > 5000 {
            let excess = self.logs.len() - 5000;
            self.logs.drain(0..excess);
        }
    }

    pub fn formatted_visible_logs(&self) -> Vec<String> {
        self.logs
            .iter()
            .filter(|e| self.log_visible_after_clear(e))
            .filter(|e| self.log_filter_level.is_none_or(|l| e.level >= l))
            .filter(|e| self.log_filter_source.is_none_or(|s| e.source == s))
            .map(|e| e.format_line())
            .collect()
    }
}

/// The daemon log cursor after a GetLogEntries reply. The daemon returns the
/// first `limit` entries after the cursor, so the cursor moves to the last
/// one received, never to `latest_seq`, which would skip the rest of a capped
/// reply. An empty reply means nothing newer is left: `latest_seq` is adopted,
/// which also follows a restarted daemon numbering from 1 again.
fn next_log_cursor(cursor: u64, entries: &[LogEntry], latest_seq: u64) -> u64 {
    match entries.last() {
        Some(last) => last.seq,
        None if latest_seq > 0 => latest_seq,
        None => cursor,
    }
}

async fn save_logs_dialog(content: String) -> Result<String, String> {
    let file = rfd::AsyncFileDialog::new()
        .set_file_name("opad.log")
        .add_filter("Log file", &["log", "txt"])
        .save_file()
        .await
        .ok_or_else(String::new)?;
    std::fs::write(file.path(), content).map_err(|e| format!("Save failed: {}", e))?;
    Ok(file.path().display().to_string())
}

async fn save_diagnostic_dialog(content: String) -> Result<String, String> {
    let filename = format!(
        "opad-diagnostics-{}.json",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(filename)
        .add_filter("JSON Diagnostic Bundle", &["json"])
        .save_file()
        .await
        .ok_or_else(String::new)?;
    std::fs::write(file.path(), content).map_err(|e| format!("Save failed: {}", e))?;
    Ok(file.path().display().to_string())
}

async fn export_backup_dialog(backup: JsonBackup) -> Result<String, String> {
    let filename = format!("opad-backup-{}.json", chrono::Local::now().format("%Y%m%d"));
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
/// `opadctl.exe` on Windows, and without it the lookup always misses.
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

/// `(this client's version, the daemon's)` from a handshake the daemon refused
/// because they differ (opad-daemon ipc_handlers.rs)
fn version_mismatch(error: &str) -> Option<(String, String)> {
    let rest = error.split("Version mismatch: this client is ").nth(1)?;
    let (client, rest) = rest.split_once(" but the running daemon is ")?;
    let daemon = rest
        .split_whitespace()
        .next()?
        .trim_end_matches('.')
        .to_string();
    Some((client.to_string(), daemon))
}

/// Whether version `a` is newer than `b` (release order, rc before final)
fn opad_update_version_newer(a: &str, b: &str) -> bool {
    opad_update::version::is_newer(a, b)
}

/// Starts this GUI again from its file on disk, which an update replaced,
/// and ends this one. With the window hidden it comes back to the tray.
fn restart_gui() {
    let exe = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    let Some(exe) = exe else {
        return;
    };
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if !args.iter().any(|a| a == "--tray") {
        args.push("--tray".into());
    }
    match std::process::Command::new(&exe).args(&args).spawn() {
        Ok(_) => std::process::exit(0),
        Err(e) => tracing::warn!("Could not restart the GUI ({}): {}", exe.display(), e),
    }
}

async fn restart_daemon_process() -> Result<(), String> {
    let exe = find_daemon_executable();
    #[cfg(target_os = "linux")]
    tokio::task::spawn_blocking(move || platform_linux::restart_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to restart opad-daemon: {}", e))??;
    #[cfg(windows)]
    tokio::task::spawn_blocking(move || platform_windows::restart_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to restart opad-daemon: {}", e))??;
    #[cfg(not(any(target_os = "linux", windows)))]
    let _ = exe;
    Ok(())
}

fn find_daemon_executable() -> std::path::PathBuf {
    find_sibling_executable("opad-daemon")
}

async fn start_daemon_process() -> Result<(), String> {
    let exe = find_daemon_executable();
    #[cfg(target_os = "linux")]
    tokio::task::spawn_blocking(move || platform_linux::start_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to start opad-daemon: {}", e))??;
    #[cfg(windows)]
    tokio::task::spawn_blocking(move || platform_windows::start_daemon(&exe))
        .await
        .map_err(|e| format!("Failed to start opad-daemon: {}", e))??;
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = exe;
        return Err("Starting the daemon is not supported on this platform".to_string());
    }
    #[allow(unreachable_code)]
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(())
}

async fn start_tosu_process(override_path: Option<std::path::PathBuf>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || opad_tosu::launch_tosu_process(override_path.as_deref()))
        .await
        .map_err(|e| format!("Failed to run tosu spawn task: {}", e))?
}

pub fn tosu_source_status(override_path: Option<&std::path::Path>) -> String {
    if let Some(p) = override_path {
        return format!("Custom ({})", p.display());
    }
    if let Some(p) =
        std::env::var_os("OPAD_TOSU_PATH").or_else(|| std::env::var_os("OPAD_TOSU_PATH"))
    {
        return format!("Custom ({})", std::path::Path::new(&p).display());
    }
    let home_install = dirs::home_dir().map(|h| {
        h.join(".local/opt/tosu")
            .join(opad_model::paths::TOSU_BINARY)
    });
    if let Some(p) = home_install.filter(|p| p.is_file()) {
        return format!("Installed at {}", p.display());
    }
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(opad_model::paths::TOSU_BINARY))
            .find(|p| p.is_file())
    });
    if let Some(p) = on_path {
        return format!("Installed at PATH ({})", p.display());
    }
    if let Ok(bundled_dir) = opad_model::paths::bundled_tosu_dir() {
        if let Ok(v) = std::fs::read_to_string(bundled_dir.join("VERSION")) {
            let v = v.trim();
            if !v.is_empty() {
                return format!("Bundled (v{})", v);
            }
        }
        if bundled_dir.join(opad_model::paths::TOSU_BINARY).is_file() {
            return "Bundled".to_string();
        }
    }
    "Bundled (default)".to_string()
}

/// One step of the PCB tab's magnet test: a board test, a short pause so the
/// pad's protocol task is not kept busy, then its report
fn magnet_tick() -> Task<Message> {
    Task::perform(
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            match ipc::request(IpcRequest::RunBoardTest).await {
                Ok(IpcResponse::BoardTest(report)) => Ok(report),
                Ok(IpcResponse::Error(e)) | Err(e) => Err(e),
                Ok(other) => Err(format!("Unexpected response: {:?}", other)),
            }
        },
        |res| Message::Diagnostics(diagnostics::DiagnosticsMessage::MagnetTick(res)),
    )
}

/// The one thing to do about a pad the daemon left alone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IncompatibleFix {
    /// Its firmware update is ready: ask for consent and flash it
    UpdatePad,
    /// Settings › Updates: the app is the old one, or no update is known yet
    OpenUpdates,
    /// Something is in the way right now, and the banner says what
    Wait,
}

/// The banner for a pad the daemon left alone because it speaks another pad
/// protocol than `opad_protocol::DEVICE_PROTOCOL_VERSION` (not the GUI-daemon
/// IPC version). Whichever side is the old one has to be updated before the
/// pad does anything here, so the banner says which, and offers it.
fn incompatible_banner(
    incompat: &opad_model::IncompatibleDevice,
    offer: Option<&opad_ipc::FirmwareOffer>,
) -> (String, IncompatibleFix) {
    let ours = opad_protocol::DEVICE_PROTOCOL_VERSION;
    let versions = format!("pad protocol {}, app {}", incompat.protocol_version, ours);
    if incompat.protocol_version > ours {
        return (
            format!(
                "This pad's firmware ({}) is newer than this OPad app understands ({versions}). \
                 Update OPad to use the pad here; it stays a working keyboard meanwhile.",
                incompat.firmware_version
            ),
            IncompatibleFix::OpenUpdates,
        );
    }
    let too_old = format!(
        "This pad's firmware ({}) is too old for this OPad app ({versions}). It stays a \
         working keyboard, but nothing else works until it is updated.",
        incompat.firmware_version
    );
    match offer {
        Some(offer) if !offer.blockers.is_empty() => (
            format!("{too_old} {}", offer.blockers.join(" ")),
            IncompatibleFix::Wait,
        ),
        Some(opad_ipc::FirmwareOffer {
            available: Some(available),
            consent_text: Some(_),
            ..
        }) => (
            format!("{too_old} Firmware {available} is ready."),
            IncompatibleFix::UpdatePad,
        ),
        _ => (
            format!("{too_old} Check for updates to get its firmware."),
            IncompatibleFix::OpenUpdates,
        ),
    }
}

/// What a key pressed while a binding waits for its letter means
#[derive(Debug, Clone, PartialEq, Eq)]
enum Captured {
    /// A to Z, upper case
    Letter(String),
    /// Escape: keep the binding as it was
    Cancel,
    Refused,
}

/// Bindings are letters only (docs/specs/app-ux-decisions.md §2): osu! binds
/// its keys to letters, and a pad key sending Enter or F4 by mistake is hard
/// to undo without a second keyboard.
fn captured_letter(key: &iced::keyboard::Key) -> Captured {
    use iced::keyboard::{key::Named, Key};
    match key {
        Key::Named(Named::Escape) => Captured::Cancel,
        Key::Character(c) => {
            let mut chars = c.chars();
            match (chars.next(), chars.next()) {
                (Some(l), None) if l.is_ascii_alphabetic() => {
                    Captured::Letter(l.to_ascii_uppercase().to_string())
                }
                _ => Captured::Refused,
            }
        }
        _ => Captured::Refused,
    }
}

/// `modal_box` centred over the dimmed window. The dim layer takes every click
/// and scroll, so nothing behind an open dialog can be pressed: a plain
/// container let them through to the page, the sidebar and the title bar.
/// The resize edges go on top, so the window can still be resized.
fn modal_over<'a>(
    framed: Element<'a, Message>,
    modal_box: Element<'a, Message>,
    maximized: bool,
) -> Element<'a, Message> {
    let dimmed = iced::widget::opaque(
        container(modal_box)
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
            }),
    );
    if maximized {
        stack![framed, dimmed].into()
    } else {
        stack![framed, dimmed, chrome::resize_edges()].into()
    }
}

/// What the person asked for, kept with the request so that the daemon's
/// answer is reported as the result of that action. Several requests share an
/// answer (`ConfigUpdated`, `CountersReset`), so the answer alone cannot say
/// what was done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The save bar on Keys and Screen
    SavePad,
    PreviewAnimation,
    Sync,
    ResetCounters,
    Replacement {
        restore: bool,
    },
    Takeover {
        take_over: bool,
    },
    UpdateToggle,
    InstallUpdate,
    CheckUpdates,
    ResetLatency,
}

/// Send `request` and report its answer as the result of `action`.
pub fn act(action: Action, request: IpcRequest) -> Task<Message> {
    Task::perform(ipc::request(request), move |result| {
        Message::ActionDone(action, result)
    })
}

/// The notices, newest on top, in the window's top right corner under the
/// title bar. Only the notices themselves take clicks: the rest of the layer
/// lets them through to the page.
fn notices_layer(notices: &[Notice]) -> Element<'_, Message> {
    let stack = column(notices.iter().rev().map(|n| {
        let id = n.id;
        iced::widget::opaque(
            iced::widget::mouse_area(
                container(
                    row![
                        text(n.text.as_str()).size(14).width(Length::Fill),
                        button(text("Close").size(12))
                            .padding([3, 8])
                            .style(theme::secondary)
                            .on_press(Message::DismissNotice(id)),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding([12, 14])
                .width(380)
                .style(theme::notice(n.tone)),
            )
            .on_enter(Message::HoverNotice(Some(id)))
            .on_exit(Message::HoverNotice(None)),
        )
    }))
    .spacing(8);
    container(stack)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .padding(iced::Padding {
            top: 52.0,
            right: 22.0,
            ..iced::Padding::ZERO
        })
        .into()
}

/// The notice for the daemon's answer to `action`: how it reads, what it
/// says, and the raw error for Logs when there is one. `None` when the
/// control that was used already shows the result (a switch).
fn action_notice(
    action: Action,
    result: Result<IpcResponse, String>,
) -> Option<(Tone, String, Option<String>)> {
    let done = |text: &str| Some((Tone::Good, text.to_string(), None));
    match (action, result) {
        // The request never got an answer
        (_, Err(e)) => Some((
            Tone::Bad,
            "OPad's service did not answer. Try again in a moment.".to_string(),
            Some(e),
        )),
        // The daemon's own sentence for why not
        (_, Ok(IpcResponse::Error(e))) => Some((Tone::Bad, e, None)),
        // Declining a takeover is answered as a refusal, but it is what was asked
        (
            Action::Takeover { take_over: false },
            Ok(IpcResponse::OperationRejected { .. } | IpcResponse::CountersRestored { .. }),
        ) => done("The pad is left as it is."),
        (_, Ok(IpcResponse::OperationDeferred { reason })) => Some((Tone::Info, reason, None)),
        (_, Ok(IpcResponse::OperationRejected { reason })) => {
            Some((Tone::Bad, format!("Not possible right now: {reason}"), None))
        }
        (Action::SavePad, Ok(IpcResponse::ConfigUpdated { .. })) => done("Saved to the pad."),
        (Action::UpdateToggle, Ok(IpcResponse::ConfigUpdated { .. })) => None,
        (Action::PreviewAnimation, Ok(IpcResponse::AnimationPlayed)) => done("Playing on the pad."),
        (Action::Sync, Ok(IpcResponse::SyncCompleted { success: true, .. })) => done("Pad synced."),
        (Action::Sync, Ok(IpcResponse::SyncCompleted { success: false, .. })) => Some((
            Tone::Bad,
            "The sync did not finish. Check the pad's USB cable and try again.".to_string(),
            None,
        )),
        (Action::ResetCounters, Ok(IpcResponse::CountersReset { .. })) => {
            done("Lifetime counters reset.")
        }
        (Action::Replacement { restore: true }, Ok(IpcResponse::CountersReset { .. })) => {
            done("Your old pad's counts are now on this pad.")
        }
        (Action::Replacement { restore: false }, Ok(IpcResponse::CountersReset { .. })) => {
            done("This pad starts from zero.")
        }
        (Action::Takeover { take_over: true }, Ok(IpcResponse::CountersRestored { .. })) => {
            done("The pad now belongs to this computer.")
        }
        (Action::InstallUpdate, Ok(IpcResponse::UpdateStarted { .. })) => {
            Some((Tone::Info, "Installing the update…".to_string(), None))
        }
        (Action::CheckUpdates, Ok(IpcResponse::UpdateCheckStarted)) => {
            Some((Tone::Info, "Checking for updates…".to_string(), None))
        }
        (Action::ResetLatency, Ok(IpcResponse::HandshakeAck { .. })) => {
            done("Key speed measurements reset.")
        }
        (_, Ok(other)) => Some((
            Tone::Bad,
            "OPad got an answer it did not expect. Try again; if it keeps happening, send a \
             report from Troubleshooting."
                .to_string(),
            Some(format!("Unexpected answer to {action:?}: {other:?}")),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_reported_as_the_result_of_the_action_that_asked() {
        let said =
            |action, result| action_notice(action, result).map(|(tone, text, _)| (tone, text));
        let config_updated = || {
            Ok(IpcResponse::ConfigUpdated {
                config: DeviceConfig::default(),
                deferred_persist: false,
            })
        };
        let counters = opad_model::CounterState::default;
        // The same answer, two different things done: a save announces
        // itself, a switch that was flipped does not
        assert_eq!(
            said(Action::SavePad, config_updated()),
            Some((Tone::Good, "Saved to the pad.".to_string()))
        );
        assert_eq!(said(Action::UpdateToggle, config_updated()), None);

        let reset = || {
            Ok(IpcResponse::CountersReset {
                counters: counters(),
            })
        };
        assert_eq!(
            said(Action::ResetCounters, reset()),
            Some((Tone::Good, "Lifetime counters reset.".to_string()))
        );
        let (_, restored) = said(Action::Replacement { restore: true }, reset()).unwrap();
        assert!(!restored.contains("reset"), "{restored}");

        // A takeover used to end in "Unexpected response", and leaving the pad
        // alone in "Not possible right now"
        let (tone, _) = said(
            Action::Takeover { take_over: true },
            Ok(IpcResponse::CountersRestored {
                counters: counters(),
            }),
        )
        .unwrap();
        assert_eq!(tone, Tone::Good);
        let (tone, left) = said(
            Action::Takeover { take_over: false },
            Ok(IpcResponse::OperationRejected {
                reason: "Left the pad paired with its other installation".into(),
            }),
        )
        .unwrap();
        assert_eq!(tone, Tone::Good);
        assert!(!left.contains("Not possible"), "{left}");
    }

    #[test]
    fn a_failure_says_what_to_do_and_keeps_the_raw_error_for_logs() {
        // No answer at all: a sentence for the person, the error for Logs
        let (tone, text, raw) = action_notice(
            Action::UpdateToggle,
            Err("daemon unreachable: ECONNREFUSED".into()),
        )
        .unwrap();
        assert_eq!(tone, Tone::Bad);
        assert!(!text.contains("ECONNREFUSED"), "{text}");
        assert_eq!(raw.as_deref(), Some("daemon unreachable: ECONNREFUSED"));

        // The daemon's own reason is already a sentence: it is shown
        let (tone, text, _) = action_notice(
            Action::Sync,
            Ok(IpcResponse::Error("The pad is not connected.".into())),
        )
        .unwrap();
        assert_eq!(
            (tone, text.as_str()),
            (Tone::Bad, "The pad is not connected.")
        );

        // An answer that makes no sense for the action never shows its debug form
        let (_, text, raw) = action_notice(Action::Sync, Ok(IpcResponse::PadRestarting)).unwrap();
        assert!(!text.contains("PadRestarting"), "{text}");
        assert!(raw.unwrap().contains("PadRestarting"));
    }

    #[tokio::test]
    async fn notices_stack_three_deep_and_go_away_by_themselves() {
        let (mut app, _) = App::new(true, None);
        for n in 1..=5 {
            app.notify(Tone::Good, format!("notice {n}"));
        }
        let texts: Vec<&str> = app.notices.iter().map(|n| n.text.as_str()).collect();
        assert_eq!(
            texts,
            ["notice 3", "notice 4", "notice 5"],
            "the oldest make room"
        );

        // A problem stays twice as long as a success
        assert!(Tone::Bad.stays() > Tone::Good.stays());

        // Past its time, a notice goes; the one under the pointer stays
        let past = std::time::Instant::now() - Duration::from_secs(1);
        for n in &mut app.notices {
            n.until = past;
        }
        let kept = app.notices[1].id;
        let _ = app.update(Message::HoverNotice(Some(kept)));
        let _ = app.update(Message::NoticeTick);
        assert_eq!(app.notices.len(), 1);
        assert_eq!(app.notices[0].id, kept);
        let _ = app.update(Message::HoverNotice(None));
        app.notices[0].until = past;
        let _ = app.update(Message::NoticeTick);
        assert!(app.notices.is_empty());

        // And it can be closed by hand
        app.notify(Tone::Bad, "something");
        let id = app.notices[0].id;
        let _ = app.update(Message::DismissNotice(id));
        assert!(app.notices.is_empty());
    }

    #[test]
    fn the_incompatible_banner_offers_the_update_that_makes_the_pad_work() {
        let ours = opad_protocol::DEVICE_PROTOCOL_VERSION;
        let pad = |protocol_version| opad_model::IncompatibleDevice {
            firmware_version: "0.9.0".to_string(),
            protocol_version,
            device_id: "OPAD-OLD".to_string(),
            board_profile: String::new(),
            running_partition: None,
        };
        let offer = |available: Option<&str>, blockers: &[&str]| opad_ipc::FirmwareOffer {
            installed: Some("0.9.0".to_string()),
            available: available.map(str::to_string),
            consent_text: available.map(|_| "Do not unplug".to_string()),
            blockers: blockers.iter().map(|b| b.to_string()).collect(),
            ..Default::default()
        };

        // An old pad whose update is ready: one button flashes it
        let (text, fix) = incompatible_banner(&pad(ours - 1), Some(&offer(Some("1.1.0"), &[])));
        assert_eq!(fix, IncompatibleFix::UpdatePad);
        assert!(text.contains("too old"), "{text}");
        assert!(text.contains("1.1.0"), "{text}");
        // The pad's protocol and the one the daemon speaks, not the IPC version
        assert!(
            text.contains(&format!("pad protocol {}, app {ours}", ours - 1)),
            "{text}"
        );

        // Mid-map: said, not offered
        let (text, fix) = incompatible_banner(
            &pad(ours - 1),
            Some(&offer(Some("1.1.0"), &["Wait until the map ends."])),
        );
        assert_eq!(fix, IncompatibleFix::Wait);
        assert!(text.contains("map ends"), "{text}");

        // No update known yet: Updates is where "Check now" is
        assert_eq!(
            incompatible_banner(&pad(ours - 1), None).1,
            IncompatibleFix::OpenUpdates
        );
        assert_eq!(
            incompatible_banner(&pad(ours - 1), Some(&offer(None, &[]))).1,
            IncompatibleFix::OpenUpdates
        );

        // A pad newer than the app: it is the app that is updated, never the pad
        let (text, fix) = incompatible_banner(&pad(ours + 1), Some(&offer(Some("9.0.0"), &[])));
        assert_eq!(fix, IncompatibleFix::OpenUpdates);
        assert!(text.contains("Update OPad"), "{text}");
    }

    #[test]
    fn every_page_token_round_trips() {
        for (_, pages) in Page::GROUPS {
            for (page, _) in pages {
                let token = page.at().token();
                assert_eq!(Target::from_token(token), Some(page.at()), "{page:?}");
            }
        }
        for (name, target) in Target::TOKENS {
            assert_eq!(Target::from_token(name), Some(target), "{name}");
            // What a second launch sends reaches the same place
            assert_eq!(Target::from_token(target.token()), Some(target), "{name}");
        }
        assert_eq!(Target::from_token("LOGS"), Some(Target::LOGS));
        assert_eq!(Target::from_token("nowhere"), None);
    }

    #[test]
    fn names_from_before_the_regrouping_open_where_their_content_went() {
        use settings::Tab as S;
        use troubleshooting::Tab as T;
        let at = |name: &str| Target::from_token(name).unwrap();
        assert_eq!(at("device").page, Page::Pad);
        assert_eq!(at("monitor"), Target::LOGS);
        assert_eq!(at("logs").section, Section::Trouble(T::Logs));
        assert_eq!(at("diagnostics").page, Page::Troubleshooting);
        assert_eq!(at("testing").section, Section::Trouble(T::KeyTest));
        assert_eq!(at("about").section, Section::Settings(S::About));
        assert_eq!(at("licenses").section, Section::Settings(S::About));
        assert_eq!(at("updates"), Target::UPDATES);
        assert_eq!(at("ppm").page, Page::Stats);
        assert_eq!(at("rapid-trigger").page, Page::Keys);
        // What the daemon launches for a pad that needs calibrating
        assert_eq!(at("calibrate"), Target::CALIBRATION);
        assert_eq!(at("calibration"), Target::CALIBRATION);
        assert_eq!(Target::CALIBRATION.token(), "calibrate");
    }

    #[tokio::test]
    async fn a_page_name_opens_its_tab() {
        let (app, _) = App::new(true, Target::from_token("logs"));
        assert_eq!(app.page, Page::Troubleshooting);
        assert_eq!(app.diagnostics.active_tab, troubleshooting::Tab::Logs);
        let (app, _) = App::new(true, Target::from_token("updates"));
        assert_eq!(app.page, Page::Settings);
        assert_eq!(app.settings_tab, settings::Tab::Updates);
        let (app, _) = App::new(true, Target::from_token("calibrate"));
        assert_eq!(app.page, Page::Keys);
        assert!(app.calibrating);
    }

    /// An app with a pad connected and its settings loaded
    fn app_with_pad() -> App {
        let (mut app, _) = App::new(true, None);
        app.device_connected = true;
        app.config_loaded = true;
        let config = DeviceConfig::default();
        app.adopt_config(&config);
        app.config = config;
        app
    }

    #[tokio::test]
    async fn the_pages_show_the_pads_settings_as_saved() {
        // What the pages read from the pad's settings and build back is the
        // same settings: nothing is "unsaved" before the person changes it
        let app = app_with_pad();
        assert_eq!(app.form_config(), app.config);
        assert!(!app.pad_dirty());
    }

    #[tokio::test]
    async fn a_change_is_unsaved_until_the_pad_confirms_it() {
        let mut app = app_with_pad();
        let _ = app.update(Message::Brightness(40));
        assert!(app.pad_dirty());
        let _ = app.update(Message::SavePad);
        let sent = app
            .pad_sent
            .clone()
            .expect("Save to pad sends the settings");
        assert_eq!(sent.brightness, 40);
        assert!(app.pad_dirty(), "not saved before the pad answers");

        // Refused: still unsaved, and Save works again
        let _ = app.update(Message::ActionDone(
            Action::SavePad,
            Ok(IpcResponse::Error("the pad did not answer".into())),
        ));
        assert!(app.pad_sent.is_none());
        assert!(app.pad_dirty());

        let _ = app.update(Message::SavePad);
        let _ = app.update(Message::ActionDone(
            Action::SavePad,
            Ok(IpcResponse::ConfigUpdated {
                config: sent,
                deferred_persist: false,
            }),
        ));
        assert!(!app.pad_dirty());
        assert_eq!(app.config.brightness, 40);
    }

    #[tokio::test]
    async fn revert_goes_back_to_what_the_pad_has() {
        let mut app = app_with_pad();
        let _ = app.update(Message::Brightness(40));
        let _ = app.update(Message::CaptureKey(1));
        let _ = app.update(Message::CaptureKeyPressed(iced::keyboard::Key::Character(
            "q".into(),
        )));
        assert_eq!(app.k1_input, "Q");
        assert!(app.pad_dirty());
        let _ = app.update(Message::RevertPad);
        assert!(!app.pad_dirty());
        assert_eq!(app.brightness, app.config.brightness);
        assert_eq!(app.k1_input, app.config.key1_char());
    }

    #[tokio::test]
    async fn a_binding_takes_letters_only() {
        use iced::keyboard::{key::Named, Key};
        let mut app = app_with_pad();
        let before = app.k2_input.clone();
        let _ = app.update(Message::CaptureKey(2));
        let _ = app.update(Message::CaptureKeyPressed(Key::Character("5".into())));
        assert_eq!(app.capturing_key, Some(2), "still waiting for a letter");
        assert_eq!(app.capture_hint, Some("Pick a letter key (A to Z)."));
        assert_eq!(app.k2_input, before);
        let _ = app.update(Message::CaptureKeyPressed(Key::Named(Named::Enter)));
        assert_eq!(app.k2_input, before);
        let _ = app.update(Message::CaptureKeyPressed(Key::Named(Named::Escape)));
        assert_eq!(app.capturing_key, None, "Escape keeps the binding");
        assert_eq!(app.k2_input, before);
    }

    #[tokio::test]
    async fn leaving_unsaved_pad_settings_asks_first() {
        let mut app = app_with_pad();
        let _ = app.update(Message::Navigate(Page::Keys));
        let _ = app.update(Message::Brightness(40));
        // Keys and Screen share the save bar: no question between them
        let _ = app.update(Message::Navigate(Page::Screen));
        assert_eq!(app.page, Page::Screen);
        assert!(app.leave_prompt.is_none());

        let _ = app.update(Message::Navigate(Page::Dashboard));
        assert_eq!(app.page, Page::Screen, "asked before leaving");
        assert_eq!(app.leave_prompt, Some(Page::Dashboard.at()));
        let _ = app.update(Message::LeaveAnswer(None));
        assert_eq!(app.page, Page::Screen);
        assert!(app.pad_dirty(), "Stay keeps the changes");

        let _ = app.update(Message::Navigate(Page::Dashboard));
        let _ = app.update(Message::LeaveAnswer(Some(false)));
        assert_eq!(app.page, Page::Dashboard);
        assert!(!app.pad_dirty(), "Discard drops them");
    }

    #[tokio::test]
    async fn a_status_poll_never_overwrites_unsaved_settings() {
        let mut app = app_with_pad();
        let _ = app.update(Message::Brightness(40));
        // The pad's settings change elsewhere (opadctl) while the page is edited
        let elsewhere = DeviceConfig {
            display_sleep_seconds: 120,
            ..app.config.clone()
        };
        app.adopt_status_config(true, false, elsewhere.clone());
        assert_eq!(app.brightness, 40, "the edit stays");
        assert!(app.pad_dirty());
        let _ = app.update(Message::RevertPad);
        assert_eq!(app.sleep_seconds, 120, "Revert shows what the pad has now");

        // Nothing unsaved: the pages follow the pad
        let again = DeviceConfig {
            brightness: 70,
            ..elsewhere
        };
        app.adopt_status_config(true, false, again);
        assert_eq!(app.brightness, 70);
        assert!(!app.pad_dirty());
    }

    fn entry(seq: u64) -> LogEntry {
        LogEntry::new(seq, LogSource::Host, LogLevel::Info, "daemon", "x")
    }

    #[tokio::test]
    async fn calibration_ignores_replies_from_before_start_or_cancel() {
        // Construct the app and drive its real update handler. Returned iced
        // tasks are not executed: no window, daemon connection or pad is used.
        let (mut app, _) = App::new(true, Some(Target::CALIBRATION));
        app.device_connected = true;
        let old_epoch = app.cal_epoch;
        let _ = app.update(Message::StartCalibration);
        let epoch = app.cal_epoch;
        assert!(app.cal_starting);
        let _ = app.update(Message::CalibrationProgress(
            old_epoch,
            Ok(IpcResponse::HeCalibrationProgress(Some(
                opad_model::HeCalibrationProgress {
                    stage: opad_model::HeCalStage::Success,
                    ..Default::default()
                },
            ))),
        ));
        assert!(app.cal_progress.is_none());
        assert!(app.cal_starting);
        let _ = app.update(Message::StartCalibrationDone(
            epoch,
            Ok(IpcResponse::HeCalibrationStarted),
        ));
        assert!(app.cal_starting, "wait for real progress after enqueueing");
        let _ = app.update(Message::CalibrationProgress(
            epoch,
            Ok(IpcResponse::HeCalibrationProgress(Some(
                opad_model::HeCalibrationProgress {
                    stage: opad_model::HeCalStage::Rest,
                    ..Default::default()
                },
            ))),
        ));
        assert_eq!(
            app.cal_progress.as_ref().unwrap().stage,
            opad_model::HeCalStage::Rest
        );
        assert!(!app.cal_starting);
        let _ = app.update(Message::CancelCalibration);
        let _ = app.update(Message::StartCalibrationDone(
            epoch,
            Ok(IpcResponse::HeCalibrationStarted),
        ));
        assert!(
            !app.cal_starting,
            "late start completion cannot undo cancel"
        );
    }

    #[test]
    fn the_log_cursor_follows_what_was_received() {
        // A reply capped at `limit`: the rest must still be fetched
        let capped: Vec<_> = (11..=13).map(entry).collect();
        assert_eq!(next_log_cursor(10, &capped, 500), 13);
        // Nothing newer: adopt the daemon's latest
        assert_eq!(next_log_cursor(13, &[], 13), 13);
        assert_eq!(next_log_cursor(0, &[], 0), 0);
        // A restarted daemon numbers from 1 again
        assert_eq!(next_log_cursor(900, &[], 4), 4);
    }
}

/// Opens `url` in the default browser, without waiting for it
fn open_in_browser(url: &str) {
    #[cfg(windows)]
    let spawned = std::process::Command::new("explorer").arg(url).spawn();
    #[cfg(not(windows))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = spawned {
        tracing::warn!("Could not open {}: {}", url, e);
    }
}

#[cfg(test)]
mod version_fix_tests {
    use super::{opad_update_version_newer, version_mismatch};

    #[test]
    fn the_daemons_refusal_names_both_versions() {
        let e = "daemon unreachable: Protocol error: Handshake rejected (daemon protocol v1, \
                 client v1): Version mismatch: this client is 1.0.0-rc.7 but the running \
                 daemon is 1.0.0-rc.6. The app was updated; restart opad-daemon to finish.";
        let (gui, daemon) = version_mismatch(e).unwrap();
        assert_eq!(
            (gui.as_str(), daemon.as_str()),
            ("1.0.0-rc.7", "1.0.0-rc.6")
        );
        assert!(
            opad_update_version_newer(&gui, &daemon),
            "the GUI restarts the daemon"
        );
        assert_eq!(
            version_mismatch("daemon unreachable: connection refused"),
            None
        );
    }
}
