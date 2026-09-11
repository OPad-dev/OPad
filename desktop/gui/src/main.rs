use iced::widget::{
    button, column, container, row, scrollable, slider, text, text_input, Space,
};
use iced::{Alignment, Element, Length, Task, Theme};
use osupad_ipc::{get_socket_path, send_request, IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_model::{
    char_to_hid_usage, CounterState, DeviceConfig, DeviceInfo, RuntimeMode,
};
use std::time::Duration;
use tokio::net::UnixStream;

pub fn main() -> iced::Result {
    iced::application("osu!pad Configuration & Monitor", OsuPadGui::update, OsuPadGui::view)
        .theme(|_| Theme::Dark)
        .subscription(OsuPadGui::subscription)
        .run_with(OsuPadGui::new)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Statistics,
    Input,
    Display,
    Device,
    Backup,
    Monitor,
}

struct OsuPadGui {
    current_tab: Tab,
    daemon_online: bool,
    device_connected: bool,
    mode: RuntimeMode,
    device_info: Option<DeviceInfo>,
    counters: CounterState,
    config: DeviceConfig,
    last_sync_time: Option<String>,
    // Input form state
    k1_input: String,
    k2_input: String,
    debounce_slider: u32,
    brightness_slider: u32,
    sleep_slider: u32,
    // Monitor state
    logs: Vec<String>,
    status_banner: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    SelectTab(Tab),
    PollDaemon,
    StatusReceived(Result<IpcResponse, String>),
    LogsReceived(Result<IpcResponse, String>),
    // Config controls
    Key1Changed(String),
    Key2Changed(String),
    DebounceChanged(u32),
    BrightnessChanged(u32),
    SleepChanged(u32),
    SaveConfig,
    ConfigSaved(Result<IpcResponse, String>),
    // Actions
    SyncRequested,
    SyncCompleted(Result<IpcResponse, String>),
    ResetRequested,
    ResetCompleted(Result<IpcResponse, String>),
}

impl OsuPadGui {
    fn new() -> (Self, Task<Message>) {
        let initial = Self {
            current_tab: Tab::Overview,
            daemon_online: false,
            device_connected: false,
            mode: RuntimeMode::Idle,
            device_info: None,
            counters: CounterState::default(),
            config: DeviceConfig::default(),
            last_sync_time: None,
            k1_input: "Z".to_string(),
            k2_input: "X".to_string(),
            debounce_slider: 3000,
            brightness_slider: 100,
            sleep_slider: 600,
            logs: Vec::new(),
            status_banner: None,
        };

        (initial, Task::perform(fetch_status(), Message::StatusReceived))
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        // Poll daemon every 1.5 seconds
        iced::time::every(Duration::from_millis(1500)).map(|_| Message::PollDaemon)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectTab(tab) => {
                self.current_tab = tab;
                if tab == Tab::Monitor {
                    return Task::perform(fetch_logs(), Message::LogsReceived);
                }
                Task::none()
            }

            Message::PollDaemon => {
                let status_task = Task::perform(fetch_status(), Message::StatusReceived);
                if self.current_tab == Tab::Monitor {
                    Task::batch([status_task, Task::perform(fetch_logs(), Message::LogsReceived)])
                } else {
                    status_task
                }
            }

            Message::StatusReceived(res) => {
                match res {
                    Ok(IpcResponse::Status {
                        mode,
                        device_connected,
                        device_info,
                        counters,
                        config,
                        last_sync_time,
                        ..
                    }) => {
                        self.daemon_online = true;
                        self.mode = mode;
                        self.device_connected = device_connected;
                        self.device_info = device_info;
                        self.counters = counters;
                        // Synchronize form values if not edited
                        self.k1_input = config.key1_char();
                        self.k2_input = config.key2_char();
                        self.debounce_slider = config.debounce_us;
                        self.brightness_slider = config.brightness;
                        self.sleep_slider = config.display_sleep_seconds;
                        self.config = config;
                        self.last_sync_time = last_sync_time;
                    }
                    _ => {
                        self.daemon_online = false;
                        self.device_connected = false;
                    }
                }
                Task::none()
            }

            Message::LogsReceived(res) => {
                if let Ok(IpcResponse::LogEntries(entries)) = res {
                    self.logs = entries;
                }
                Task::none()
            }

            Message::Key1Changed(s) => {
                self.k1_input = s.chars().take(2).collect();
                Task::none()
            }

            Message::Key2Changed(s) => {
                self.k2_input = s.chars().take(2).collect();
                Task::none()
            }

            Message::DebounceChanged(val) => {
                self.debounce_slider = val;
                Task::none()
            }

            Message::BrightnessChanged(val) => {
                self.brightness_slider = val;
                Task::none()
            }

            Message::SleepChanged(val) => {
                self.sleep_slider = val;
                Task::none()
            }

            Message::SaveConfig => {
                let k1_usage = char_to_hid_usage(&self.k1_input).unwrap_or(self.config.key1_hid_usage);
                let k2_usage = char_to_hid_usage(&self.k2_input).unwrap_or(self.config.key2_hid_usage);

                let new_config = DeviceConfig {
                    key1_hid_usage: k1_usage,
                    key2_hid_usage: k2_usage,
                    debounce_us: self.debounce_slider,
                    brightness: self.brightness_slider,
                    display_sleep_seconds: self.sleep_slider,
                    gameplay_display_hz: self.config.gameplay_display_hz,
                    tosu_endpoint: self.config.tosu_endpoint.clone(),
                };

                self.status_banner = Some("Applying new configuration...".to_string());
                Task::perform(update_config_request(new_config), Message::ConfigSaved)
            }

            Message::ConfigSaved(res) => {
                match res {
                    Ok(IpcResponse::ConfigUpdated { config }) => {
                        self.config = config;
                        self.status_banner = Some("Configuration updated successfully!".to_string());
                    }
                    Ok(IpcResponse::OperationRejected { reason }) => {
                        self.status_banner = Some(format!("Rejected: {}", reason));
                    }
                    Err(e) => {
                        self.status_banner = Some(format!("Error: {}", e));
                    }
                    _ => {}
                }
                Task::none()
            }

            Message::SyncRequested => {
                self.status_banner = Some("Synchronizing counters and clock with ESP...".to_string());
                Task::perform(force_sync_request(), Message::SyncCompleted)
            }

            Message::SyncCompleted(res) => {
                match res {
                    Ok(IpcResponse::SyncCompleted { success: true, counters }) => {
                        self.counters = counters;
                        self.status_banner = Some("Synchronization successful!".to_string());
                    }
                    Ok(IpcResponse::OperationRejected { reason }) => {
                        self.status_banner = Some(format!("Rejected: {}", reason));
                    }
                    _ => {
                        self.status_banner = Some("Synchronization failed".to_string());
                    }
                }
                Task::none()
            }

            Message::ResetRequested => {
                self.status_banner = Some("Resetting lifetime counters...".to_string());
                Task::perform(reset_counters_request(), Message::ResetCompleted)
            }

            Message::ResetCompleted(res) => {
                match res {
                    Ok(IpcResponse::CountersReset { counters }) => {
                        self.counters = counters;
                        self.status_banner = Some("Lifetime counters reset to zero with incremented generation.".to_string());
                    }
                    Ok(IpcResponse::OperationRejected { reason }) => {
                        self.status_banner = Some(format!("Rejected: {}", reason));
                    }
                    _ => {}
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let nav_item = |tab: Tab, label: &'static str| {
            let is_selected = self.current_tab == tab;
            button(text(label))
                .padding([8, 16])
                .style(if is_selected {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::SelectTab(tab))
        };

        let sidebar = column![
            text("osu!pad").size(24),
            Space::with_height(Length::Fixed(16.0)),
            nav_item(Tab::Overview, "Overview"),
            nav_item(Tab::Statistics, "Statistics"),
            nav_item(Tab::Input, "Input"),
            nav_item(Tab::Display, "Display"),
            nav_item(Tab::Device, "Device"),
            nav_item(Tab::Backup, "Backup"),
            nav_item(Tab::Monitor, "Monitor"),
            Space::with_height(Length::Fill),
            text(format!(
                "Daemon: {}",
                if self.daemon_online { "Online" } else { "Offline" }
            ))
            .size(13),
            text(format!(
                "Device: {}",
                if self.device_connected { "Connected" } else { "Disconnected" }
            ))
            .size(13),
        ]
        .spacing(8)
        .padding(16)
        .width(Length::Fixed(180.0));

        let content: Element<'_, Message> = match self.current_tab {
            Tab::Overview => self.view_overview(),
            Tab::Statistics => self.view_statistics(),
            Tab::Input => self.view_input(),
            Tab::Display => self.view_display(),
            Tab::Device => self.view_device(),
            Tab::Backup => self.view_backup(),
            Tab::Monitor => self.view_monitor(),
        };

        let main_view = column![
            if let Some(banner) = &self.status_banner {
                container(text(banner).size(14))
                    .padding(8)
                    .style(container::bordered_box)
            } else {
                container(Space::with_height(Length::Fixed(0.0)))
            },
            content,
        ]
        .spacing(12)
        .padding(20)
        .width(Length::Fill);

        row![sidebar, main_view].into()
    }

    fn view_overview(&self) -> Element<'_, Message> {
        column![
            text("System Overview").size(22),
            Space::with_height(Length::Fixed(12.0)),
            text(format!("Operational Mode: {:?}", self.mode)),
            text(format!(
                "ESP32 Connection: {}",
                if self.device_connected { "Connected" } else { "Disconnected" }
            )),
            text(format!(
                "Hardware Profile: {}",
                self.device_info.as_ref().map(|i| i.board_profile.as_str()).unwrap_or("Waveshare ESP32-S3-Touch-LCD-2")
            )),
            text(format!(
                "Firmware Version: {}",
                self.device_info.as_ref().map(|i| i.firmware_version.as_str()).unwrap_or("v1.0.0")
            )),
            Space::with_height(Length::Fixed(12.0)),
            text(format!(
                "Key 1 ({}): {} presses",
                self.config.key1_char(),
                self.counters.lifetime_key1
            )),
            text(format!(
                "Key 2 ({}): {} presses",
                self.config.key2_char(),
                self.counters.lifetime_key2
            )),
            text(format!(
                "Total Presses: {}",
                self.counters.total_lifetime_presses()
            )),
            Space::with_height(Length::Fixed(12.0)),
            text(format!(
                "Last Sync: {}",
                self.last_sync_time.as_deref().unwrap_or("Never")
            )),
        ]
        .spacing(8)
        .into()
    }

    fn view_statistics(&self) -> Element<'_, Message> {
        column![
            text("Lifetime Statistics").size(22),
            Space::with_height(Length::Fixed(12.0)),
            text(format!("Key 1 Lifetime: {} presses", self.counters.lifetime_key1)),
            text(format!("Key 2 Lifetime: {} presses", self.counters.lifetime_key2)),
            text(format!("Total Combined: {} presses", self.counters.total_lifetime_presses())),
            text(format!("Counter Generation: {}", self.counters.counter_generation)),
            Space::with_height(Length::Fixed(16.0)),
            text("Current Session / Map:").size(16),
            text(format!("Map Key 1: {} presses", self.counters.map_key1)),
            text(format!("Map Key 2: {} presses", self.counters.map_key2)),
        ]
        .spacing(8)
        .into()
    }

    fn view_input(&self) -> Element<'_, Message> {
        column![
            text("Input Configuration").size(22),
            Space::with_height(Length::Fixed(12.0)),
            row![
                text("Key 1 Character:").width(Length::Fixed(140.0)),
                text_input("Z", &self.k1_input)
                    .on_input(Message::Key1Changed)
                    .width(Length::Fixed(80.0)),
            ]
            .align_y(Alignment::Center),
            row![
                text("Key 2 Character:").width(Length::Fixed(140.0)),
                text_input("X", &self.k2_input)
                    .on_input(Message::Key2Changed)
                    .width(Length::Fixed(80.0)),
            ]
            .align_y(Alignment::Center),
            Space::with_height(Length::Fixed(8.0)),
            text(format!("Eager Debounce Lockout: {} µs", self.debounce_slider)),
            slider(500..=10000, self.debounce_slider, Message::DebounceChanged),
            Space::with_height(Length::Fixed(16.0)),
            button("Apply Input Settings").on_press(Message::SaveConfig),
        ]
        .spacing(10)
        .into()
    }

    fn view_display(&self) -> Element<'_, Message> {
        column![
            text("Display Settings").size(22),
            Space::with_height(Length::Fixed(12.0)),
            text(format!("Backlight Brightness: {}%", self.brightness_slider)),
            slider(10..=100, self.brightness_slider, Message::BrightnessChanged),
            Space::with_height(Length::Fixed(8.0)),
            text(format!("Display Sleep Timeout: {} seconds", self.sleep_slider)),
            slider(60..=3600, self.sleep_slider, Message::SleepChanged),
            Space::with_height(Length::Fixed(16.0)),
            button("Save Display Settings").on_press(Message::SaveConfig),
        ]
        .spacing(10)
        .into()
    }

    fn view_device(&self) -> Element<'_, Message> {
        column![
            text("Device Management").size(22),
            Space::with_height(Length::Fixed(12.0)),
            text(format!("Device ID: {}", self.device_info.as_ref().map(|i| i.device_id.as_str()).unwrap_or("N/A"))),
            text("Actions:"),
            row![
                button("Synchronize Now").on_press(Message::SyncRequested),
                button("Reset Counters").on_press(Message::ResetRequested),
            ]
            .spacing(12),
        ]
        .spacing(10)
        .into()
    }

    fn view_backup(&self) -> Element<'_, Message> {
        column![
            text("Backup & Portable Restore").size(22),
            Space::with_height(Length::Fixed(12.0)),
            text("You can export or import portable JSON backups using `osupadctl`:"),
            text("  $ osupadctl export backup.json"),
            text("  $ osupadctl import backup.json"),
        ]
        .spacing(10)
        .into()
    }

    fn view_monitor(&self) -> Element<'_, Message> {
        let log_lines: Element<'_, Message> = column(
            self.logs
                .iter()
                .map(|line| text(line).size(13).into())
                .collect::<Vec<_>>(),
        )
        .spacing(4)
        .into();

        column![
            text("Live System & ESP Diagnostic Monitor").size(22),
            Space::with_height(Length::Fixed(8.0)),
            scrollable(log_lines).height(Length::Fixed(400.0)),
        ]
        .spacing(8)
        .into()
    }
}

// -----------------------------------------------------------------------------
// IPC Async Tasks
// -----------------------------------------------------------------------------

async fn fetch_status() -> Result<IpcResponse, String> {
    let socket = get_socket_path();
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|e| e.to_string())?;

    let _ = send_request(
        &mut stream,
        &IpcRequest::Handshake {
            client_version: "1.0.0".to_string(),
            client_protocol: IPC_PROTOCOL_VERSION,
        },
    )
    .await;

    send_request(&mut stream, &IpcRequest::GetStatus)
        .await
        .map_err(|e| e.to_string())
}

async fn fetch_logs() -> Result<IpcResponse, String> {
    let socket = get_socket_path();
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|e| e.to_string())?;

    send_request(&mut stream, &IpcRequest::GetLogEntries { limit: 100 })
        .await
        .map_err(|e| e.to_string())
}

async fn update_config_request(cfg: DeviceConfig) -> Result<IpcResponse, String> {
    let socket = get_socket_path();
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|e| e.to_string())?;

    send_request(&mut stream, &IpcRequest::UpdateConfig(cfg))
        .await
        .map_err(|e| e.to_string())
}

async fn force_sync_request() -> Result<IpcResponse, String> {
    let socket = get_socket_path();
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|e| e.to_string())?;

    send_request(&mut stream, &IpcRequest::ForceSync)
        .await
        .map_err(|e| e.to_string())
}

async fn reset_counters_request() -> Result<IpcResponse, String> {
    let socket = get_socket_path();
    let mut stream = UnixStream::connect(&socket)
        .await
        .map_err(|e| e.to_string())?;

    send_request(&mut stream, &IpcRequest::ResetCounters)
        .await
        .map_err(|e| e.to_string())
}
