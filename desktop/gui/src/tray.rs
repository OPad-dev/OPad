//! System tray icon (StatusNotifierItem via ksni). The app lives here while its window is closed.

use futures_util::SinkExt;
use ksni::TrayMethods;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    OpenMonitor,
    SyncNow,
    StartDaemon,
    Quit,
}

#[derive(Clone)]
pub struct TrayHandle(pub ksni::Handle<OsuPadTray>);

impl std::fmt::Debug for TrayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayHandle").finish()
    }
}

#[derive(Debug, Clone)]
pub enum TrayEvent {
    Started(TrayHandle),
    /// No StatusNotifier host (e.g. GNOME without the AppIndicator extension)
    Unavailable,
    Action(TrayAction),
}

/// Formats a number with thousands separators (e.g. 1,284,391, §19)
fn format_grouped(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.push(',');
        }
        result.push(c);
        count += 1;
    }
    result.chars().rev().collect()
}

/// What the tray status items and tooltip show
#[derive(Debug, Clone, Default)]
pub struct TrayStatus {
    pub daemon_online: bool,
    pub device_connected: bool,
    pub incompatible: bool,
    pub firmware_version: Option<String>,
    pub key1_presses: u64,
    pub key2_presses: u64,
    pub last_sync_time: Option<String>,
    pub last_sync_error: Option<String>,
    pub is_playing_or_cooldown: bool,
    pub tosu_connected: bool,
    pub total_presses: u64,
    pub mode: String,
}

pub struct OsuPadTray {
    tx: tokio::sync::mpsc::UnboundedSender<TrayAction>,
    pub status: TrayStatus,
}

impl ksni::Tray for OsuPadTray {
    fn id(&self) -> String {
        "osupad".into()
    }

    fn title(&self) -> String {
        "osu!pad".into()
    }

    fn icon_name(&self) -> String {
        if !self.status.daemon_online {
            "dialog-warning-symbolic".into()
        } else if self.status.device_connected {
            "input-keyboard-symbolic".into()
        } else {
            "network-offline-symbolic".into()
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let s = &self.status;
        let description = if !s.daemon_online {
            "Daemon offline".to_string()
        } else {
            format!(
                "Pad: {}\ntosu: {}\n{} presses · {}",
                if s.device_connected { "connected" } else { "disconnected" },
                if s.tosu_connected { "connected" } else { "not running" },
                format_grouped(s.total_presses),
                s.mode
            )
        };
        ksni::ToolTip {
            title: "osu!pad".to_string(),
            description,
            icon_name: self.icon_name(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayAction::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;

        let disabled_item = |label: String| -> ksni::MenuItem<Self> {
            StandardItem {
                label,
                enabled: false,
                ..Default::default()
            }
            .into()
        };

        let action_item = |label: &str, action: TrayAction, enabled: bool| -> ksni::MenuItem<Self> {
            StandardItem {
                label: label.into(),
                enabled,
                activate: Box::new(move |tray: &mut OsuPadTray| {
                    let _ = tray.tx.send(action);
                }),
                ..Default::default()
            }
            .into()
        };

        let s = &self.status;
        let mut items = Vec::new();

        // 1. Header: osu!pad
        items.push(disabled_item("osu!pad".to_string()));
        items.push(ksni::MenuItem::Separator);

        // 2. Pad status
        let pad_status_str = if !s.daemon_online {
            "Pad: Daemon offline"
        } else if s.incompatible {
            "Pad: Incompatible firmware"
        } else if s.device_connected {
            "Pad: Connected"
        } else {
            "Pad: Disconnected"
        };
        items.push(disabled_item(pad_status_str.to_string()));

        // 3. Firmware (hidden when unknown or device disconnected)
        if s.daemon_online && s.device_connected {
            if let Some(fw) = &s.firmware_version {
                if !fw.is_empty() {
                    items.push(disabled_item(format!("Firmware: {}", fw)));
                }
            }
        }

        // 4. Counters
        items.push(disabled_item(format!("Key 1: {}", format_grouped(s.key1_presses))));
        items.push(disabled_item(format!("Key 2: {}", format_grouped(s.key2_presses))));

        // 5. Last sync
        let sync_str = if s.last_sync_error.is_some() {
            "Last sync: Sync failed".to_string()
        } else if let Some(sync_time) = &s.last_sync_time {
            let display_time = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(sync_time) {
                parsed.format("%H:%M").to_string()
            } else {
                sync_time.clone()
            };
            format!("Last sync: {}", display_time)
        } else {
            "Last sync: Never".to_string()
        };
        items.push(disabled_item(sync_str));

        items.push(ksni::MenuItem::Separator);

        // 6. Navigation and actions
        items.push(action_item("Open osu!pad", TrayAction::ShowWindow, true));
        items.push(action_item("Open Monitor", TrayAction::OpenMonitor, true));

        if !s.daemon_online {
            items.push(action_item("Start daemon", TrayAction::StartDaemon, true));
        } else {
            let sync_enabled = s.device_connected && !s.is_playing_or_cooldown;
            items.push(action_item("Sync pad now", TrayAction::SyncNow, sync_enabled));
        }

        items.push(ksni::MenuItem::Separator);

        // 7. Quit osu!pad app (exits GUI only, not daemon)
        items.push(action_item("Quit osu!pad app", TrayAction::Quit, true));

        items
    }
}

pub fn stream() -> impl futures_util::Stream<Item = TrayEvent> {
    iced::stream::channel(20, |mut output: iced::futures::channel::mpsc::Sender<TrayEvent>| async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TrayAction>();
        let tray = OsuPadTray { tx, status: TrayStatus::default() };
        match tray.spawn().await {
            Ok(handle) => {
                let _ = output.send(TrayEvent::Started(TrayHandle(handle))).await;
                while let Some(action) = rx.recv().await {
                    let _ = output.send(TrayEvent::Action(action)).await;
                }
            }
            Err(e) => {
                tracing::warn!("System tray unavailable: {e}");
                let _ = output.send(TrayEvent::Unavailable).await;
            }
        }
    })
}

pub fn update(handle: &TrayHandle, status: TrayStatus) {
    let handle = handle.0.clone();
    tokio::spawn(async move {
        let _ = handle.update(move |t| t.status = status).await;
    });
}
