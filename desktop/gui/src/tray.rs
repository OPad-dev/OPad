//! System tray icon (StatusNotifierItem via ksni). The app lives here while its window is closed.

use futures_util::SinkExt;
use ksni::TrayMethods;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    SyncNow,
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

/// What the tooltip shows
#[derive(Debug, Clone, Default)]
pub struct TrayStatus {
    pub daemon_online: bool,
    pub device_connected: bool,
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
        "input-keyboard-symbolic".into()
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
                s.total_presses,
                s.mode
            )
        };
        ksni::ToolTip { title: "osu!pad".to_string(), description, icon_name: self.icon_name(), ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayAction::ShowWindow);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let item = |label: &str, action: TrayAction| -> ksni::MenuItem<Self> {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |tray: &mut OsuPadTray| {
                    let _ = tray.tx.send(action);
                }),
                ..Default::default()
            }
            .into()
        };
        vec![
            item("Open osu!pad", TrayAction::ShowWindow),
            item("Sync pad now", TrayAction::SyncNow),
            ksni::MenuItem::Separator,
            item("Quit", TrayAction::Quit),
        ]
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
