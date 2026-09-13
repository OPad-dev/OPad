mod chrome;
mod designer;
mod ipc;
mod pages;
mod single_instance;
mod theme;
mod tray;

use iced::widget::{button, column, container, row, stack, text, text_input, Space};
use iced::{window, Alignment, Element, Length, Size, Subscription, Task};
use osupad_ipc::{IpcRequest, IpcResponse};
use osupad_model::ui_source::SourceValue;
use osupad_model::{
    char_to_hid_usage, CounterSource, CounterState, DeviceConfig, DeviceInfo, IncompatibleDevice,
    LatencyStats, RuntimeMode,
};
use std::collections::HashMap;
use std::time::Duration;

pub fn main() -> iced::Result {
    if !single_instance::claim() {
        // Another instance was asked to show its window
        return Ok(());
    }
    let start_hidden = std::env::args().any(|a| a == "--tray");

    // A daemon keeps running with no window open, living in the tray (Discord-style)
    iced::daemon(move || App::new(start_hidden), App::update, App::view)
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

pub struct App {
    pub page: Page,
    window: Option<window::Id>,
    maximized: bool,
    tray: Option<tray::TrayHandle>,
    /// None until the tray reports in; false means closing the window quits
    tray_available: Option<bool>,

    pub daemon_online: bool,
    pub device_connected: bool,
    pub tosu_connected: bool,
    pub mode: RuntimeMode,
    pub device_info: Option<DeviceInfo>,
    pub counters: CounterState,
    pub counters_source: CounterSource,
    pub config: DeviceConfig,
    pub last_sync_time: Option<String>,
    pub last_sync_error: Option<String>,
    pub latency: Option<LatencyStats>,
    pub pending_replacement: Option<String>,
    pub incompatible: Option<IncompatibleDevice>,
    pub reset_modal: Option<String>,
    pub ui_values: HashMap<u8, SourceValue>,

    // Settings form
    pub k1_input: String,
    pub k2_input: String,
    pub debounce: u32,
    pub brightness: u32,
    pub sleep_seconds: u32,
    config_loaded: bool,

    pub logs: Vec<String>,
    pub banner: Option<String>,
    pub designer: designer::Designer,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Page),
    Poll,
    Status(Result<IpcResponse, String>),
    UiValues(Result<IpcResponse, String>),
    Logs(Result<IpcResponse, String>),
    // Settings
    Key1(String),
    Key2(String),
    Debounce(u32),
    Brightness(u32),
    SleepSeconds(u32),
    SaveConfig,
    // Actions
    Sync,
    PromptResetCounters,
    ResetModalInput(String),
    CancelResetModal,
    ConfirmResetCounters,
    ResolveReplacement(bool),
    ResetLatency,
    ActionDone(Result<IpcResponse, String>),
    DismissBanner,
    Designer(designer::Message),
    // Window & tray
    WindowOpened(window::Id),
    CloseRequested(window::Id),
    Tray(tray::TrayEvent),
    ShowRequested,
    Window(chrome::WindowAction),
    Resized,
    Maximized(bool),
}

impl App {
    fn new(start_hidden: bool) -> (Self, Task<Message>) {
        let (designer, designer_task) = designer::Designer::new();
        let mut app = App {
            page: Page::Dashboard,
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
            latency: None,
            pending_replacement: None,
            incompatible: None,
            reset_modal: None,
            ui_values: HashMap::new(),
            k1_input: "Z".into(),
            k2_input: "X".into(),
            debounce: 3000,
            brightness: 100,
            sleep_seconds: 600,
            config_loaded: false,
            logs: Vec::new(),
            banner: None,
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
        ];
        if self.page == Page::Monitor && self.window.is_some() {
            tasks.push(Task::perform(ipc::request(IpcRequest::GetLogEntries { limit: 200 }), Message::Logs));
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
                        config,
                        last_sync_time,
                        last_sync_error,
                        tosu_connected,
                        latency,
                        pending_replacement,
                        incompatible,
                    }) => {
                        self.daemon_online = true;
                        self.mode = mode;
                        self.device_connected = device_connected;
                        self.device_info = device_info;
                        self.counters = counters;
                        self.counters_source = counters_source;
                        self.last_sync_error = last_sync_error;
                        self.pending_replacement = pending_replacement;
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
                            self.debounce = config.debounce_us;
                            self.brightness = config.brightness;
                            self.sleep_seconds = config.display_sleep_seconds;
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
                if let Ok(IpcResponse::LogEntries(entries)) = result {
                    self.logs = entries;
                }
            }
            Message::Key1(s) => self.k1_input = s.chars().take(1).collect::<String>().to_uppercase(),
            Message::Key2(s) => self.k2_input = s.chars().take(1).collect::<String>().to_uppercase(),
            Message::Debounce(v) => self.debounce = v,
            Message::Brightness(v) => self.brightness = v,
            Message::SleepSeconds(v) => self.sleep_seconds = v,
            Message::SaveConfig => {
                let config = DeviceConfig {
                    key1_hid_usage: char_to_hid_usage(&self.k1_input).unwrap_or(self.config.key1_hid_usage),
                    key2_hid_usage: char_to_hid_usage(&self.k2_input).unwrap_or(self.config.key2_hid_usage),
                    debounce_us: self.debounce,
                    brightness: self.brightness,
                    display_sleep_seconds: self.sleep_seconds,
                    ..self.config.clone()
                };
                self.banner = Some("Saving settings...".into());
                return Task::perform(ipc::request(IpcRequest::UpdateConfig(config)), Message::ActionDone);
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
                return Task::perform(ipc::request(IpcRequest::ResetCounters { confirm: true }), Message::ActionDone);
            }
            Message::ResolveReplacement(restore) => {
                self.banner = Some(if restore { "Restoring counters from previous pad...".into() } else { "Adopting new pad...".into() });
                return Task::perform(ipc::request(IpcRequest::ResolveReplacement { restore }), Message::ActionDone);
            }
            Message::ResetLatency => {
                return Task::perform(ipc::request(IpcRequest::ResetLatencyStats), Message::ActionDone);
            }
            Message::ActionDone(result) => {
                self.banner = Some(match result {
                    Ok(IpcResponse::ConfigUpdated { .. }) => "Settings saved and sent to the pad".into(),
                    Ok(IpcResponse::SyncCompleted { success: true, .. }) => "Pad synced".into(),
                    Ok(IpcResponse::CountersReset { .. }) => "Lifetime counters reset".into(),
                    Ok(IpcResponse::HandshakeAck { .. }) => "Latency statistics reset".into(),
                    Ok(IpcResponse::OperationRejected { reason }) => format!("Not possible right now: {}", reason),
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
            Message::ShowRequested => return self.open_window(),
            Message::Window(action) => {
                let Some(id) = self.window else { return Task::none() };
                return match action {
                    chrome::WindowAction::Drag => window::drag(id),
                    chrome::WindowAction::ToggleMaximize => window::toggle_maximize(id),
                    chrome::WindowAction::Minimize => window::minimize(id, true),
                    chrome::WindowAction::Close => self.update(Message::CloseRequested(id)),
                    chrome::WindowAction::Resize(direction) if !self.maximized => window::drag_resize(id, direction),
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
                    if self.window.is_none() {
                        return self.open_window();
                    }
                }
                tray::TrayEvent::Action(tray::TrayAction::ShowWindow) => return self.open_window(),
                tray::TrayEvent::Action(tray::TrayAction::SyncNow) => return self.update(Message::Sync),
                tray::TrayEvent::Action(tray::TrayAction::Quit) => return iced::exit(),
            },
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::time::every(Duration::from_millis(if self.window.is_some() { 1000 } else { 3000 }))
                .map(|_| Message::Poll),
            window::close_requests().map(Message::CloseRequested),
            window::resize_events().map(|_| Message::Resized),
            Subscription::run(tray::stream).map(Message::Tray),
            Subscription::run(single_instance::show_requests).map(|_| Message::ShowRequested),
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
                status_line(self.device_connected, "Pad", if self.device_connected { "connected" } else { "offline" }),
                status_line(self.tosu_connected, "tosu", if self.tosu_connected { "connected" } else { "offline" }),
                status_line(self.daemon_online, "Daemon", if self.daemon_online { "running" } else { "offline" }),
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

        let mut main = column![].spacing(14).padding(24).width(Length::Fill).height(Length::Fill);
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
                        text(format!("This looks like a new pad. Restore counters from {}?", old_id)).size(14),
                        Space::new().width(Length::Fill),
                        button(text("Restore from previous pad").size(12)).style(theme::primary).on_press(Message::ResolveReplacement(true)),
                        Space::new().width(8),
                        button(text("Treat as new pad").size(12)).style(theme::secondary).on_press(Message::ResolveReplacement(false)),
                    ]
                    .align_y(Alignment::Center),
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
                        button(text("Dismiss").size(12)).style(theme::secondary).on_press(Message::DismissBanner),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(theme::banner),
            );
        }
        main = main.push(page);

        let body = column![chrome::title_bar(self.maximized), row![sidebar, main].height(Length::Fill)];
        let framed = container(body).style(chrome::frame);

        if let Some(input_text) = &self.reset_modal {
            let modal_box = container(
                column![
                    text("Reset Lifetime Counters").size(20).font(theme::FONT_BOLD).color(theme::RED),
                    text("This action will permanently reset hardware and database counters to 0.").size(13).color(theme::MUTED),
                    Space::new().height(6),
                    text(format!("Key 1 (K1): {} presses", self.counters.lifetime_key1)).size(14),
                    text(format!("Key 2 (K2): {} presses", self.counters.lifetime_key2)).size(14),
                    text(format!("Total: {} presses", self.counters.total_lifetime_presses())).size(14).font(theme::FONT_BOLD),
                    Space::new().height(10),
                    text("Type RESET below to confirm:").size(13).color(theme::MUTED),
                    text_input("RESET", input_text)
                        .on_input(Message::ResetModalInput)
                        .padding(10)
                        .size(14),
                    Space::new().height(14),
                    row![
                        button(text("Cancel").size(14)).padding([10, 20]).style(theme::secondary).on_press(Message::CancelResetModal),
                        Space::new().width(Length::Fill),
                        if input_text == "RESET" {
                            button(text("Confirm Reset").size(14)).padding([10, 20]).style(theme::danger).on_press(Message::ConfirmResetCounters)
                        } else {
                            button(text("Confirm Reset").size(14)).padding([10, 20]).style(theme::secondary)
                        }
                    ]
                ]
                .spacing(8)
                .padding(24)
                .width(420)
            )
            .style(theme::card);

            let modal_overlay = container(modal_box)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(iced::Color { a: 0.75, ..theme::BG }.into()),
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
}
