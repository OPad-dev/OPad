//! System tray icon (StatusNotifierItem via ksni on Linux, tray-icon on Windows).
//! The app lives here while its window is closed.

use futures_util::SinkExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    OpenMonitor,
    SyncNow,
    StartDaemon,
    Quit,
}

#[derive(Clone)]
pub struct TrayHandle {
    #[cfg(target_os = "linux")]
    inner: ksni::Handle<linux::OsuPadTray>,
    #[cfg(windows)]
    inner: windows::WindowsTrayHandle,
    #[cfg(not(any(target_os = "linux", windows)))]
    _inner: (),
}

impl std::fmt::Debug for TrayHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayHandle").finish()
    }
}

#[derive(Debug, Clone)]
pub enum TrayEvent {
    Started(TrayHandle),
    /// No StatusNotifier host (e.g. GNOME without the AppIndicator extension) or tray unavailable
    Unavailable,
    Action(TrayAction),
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

// ---------------------------------------------------------------------------
// Backend-agnostic Tray Model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconState {
    Warning,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItemModel {
    Disabled(String),
    Separator,
    Action {
        label: String,
        action: TrayAction,
        enabled: bool,
    },
}

#[derive(Debug, Clone)]
pub struct TrayViewModel {
    pub title: String,
    pub tooltip_description: String,
    pub icon_state: IconState,
    pub menu_items: Vec<MenuItemModel>,
}

impl TrayViewModel {
    pub fn from_status(s: &TrayStatus) -> Self {
        let icon_state = if !s.daemon_online {
            IconState::Warning
        } else if s.device_connected {
            IconState::Connected
        } else {
            IconState::Disconnected
        };

        let tooltip_description = if !s.daemon_online {
            "Daemon offline".to_string()
        } else {
            format!(
                "Pad: {}\ntosu: {}\n{} presses · {}",
                if s.device_connected {
                    "connected"
                } else {
                    "disconnected"
                },
                if s.tosu_connected {
                    "connected"
                } else {
                    "not running"
                },
                crate::pages::grouped(s.total_presses),
                s.mode
            )
        };

        let mut items = Vec::new();

        // 1. Header: osu!pad
        items.push(MenuItemModel::Disabled("osu!pad".to_string()));
        items.push(MenuItemModel::Separator);

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
        items.push(MenuItemModel::Disabled(pad_status_str.to_string()));

        // 3. Firmware (hidden when unknown or device disconnected)
        if s.daemon_online && s.device_connected {
            if let Some(fw) = &s.firmware_version {
                if !fw.is_empty() {
                    items.push(MenuItemModel::Disabled(format!("Firmware: {fw}")));
                }
            }
        }

        // 4. Counters
        items.push(MenuItemModel::Disabled(format!(
            "Key 1: {}",
            crate::pages::grouped(s.key1_presses)
        )));
        items.push(MenuItemModel::Disabled(format!(
            "Key 2: {}",
            crate::pages::grouped(s.key2_presses)
        )));

        // 5. Last sync
        let sync_str = if s.last_sync_error.is_some() {
            "Last sync: Sync failed".to_string()
        } else if let Some(sync_time) = &s.last_sync_time {
            let display_time = if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(sync_time) {
                parsed.format("%H:%M").to_string()
            } else {
                sync_time.clone()
            };
            format!("Last sync: {display_time}")
        } else {
            "Last sync: Never".to_string()
        };
        items.push(MenuItemModel::Disabled(sync_str));

        items.push(MenuItemModel::Separator);

        // 6. Navigation and actions
        items.push(MenuItemModel::Action {
            label: "Open osu!pad".into(),
            action: TrayAction::ShowWindow,
            enabled: true,
        });
        items.push(MenuItemModel::Action {
            label: "Open Monitor".into(),
            action: TrayAction::OpenMonitor,
            enabled: true,
        });

        if !s.daemon_online {
            items.push(MenuItemModel::Action {
                label: "Start daemon".into(),
                action: TrayAction::StartDaemon,
                enabled: true,
            });
        } else {
            let sync_enabled = s.device_connected && !s.is_playing_or_cooldown;
            items.push(MenuItemModel::Action {
                label: "Sync pad now".into(),
                action: TrayAction::SyncNow,
                enabled: sync_enabled,
            });
        }

        items.push(MenuItemModel::Separator);

        // 7. Quit osu!pad app (exits GUI only, not daemon)
        items.push(MenuItemModel::Action {
            label: "Quit osu!pad app".into(),
            action: TrayAction::Quit,
            enabled: true,
        });

        Self {
            title: "osu!pad".to_string(),
            tooltip_description,
            icon_state,
            menu_items: items,
        }
    }
}

// ---------------------------------------------------------------------------
// Linux Backend (ksni)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    pub struct OsuPadTray {
        pub tx: tokio::sync::mpsc::UnboundedSender<TrayAction>,
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
            let model = TrayViewModel::from_status(&self.status);
            match model.icon_state {
                IconState::Warning => "dialog-warning-symbolic".into(),
                IconState::Connected => "input-keyboard-symbolic".into(),
                IconState::Disconnected => "network-offline-symbolic".into(),
            }
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            let model = TrayViewModel::from_status(&self.status);
            ksni::ToolTip {
                title: model.title,
                description: model.tooltip_description,
                icon_name: self.icon_name(),
                ..Default::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let _ = self.tx.send(TrayAction::ShowWindow);
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            let model = TrayViewModel::from_status(&self.status);
            model
                .menu_items
                .into_iter()
                .map(|item| match item {
                    MenuItemModel::Disabled(label) => StandardItem {
                        label,
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                    MenuItemModel::Separator => ksni::MenuItem::Separator,
                    MenuItemModel::Action {
                        label,
                        action,
                        enabled,
                    } => StandardItem {
                        label,
                        enabled,
                        activate: Box::new(move |tray: &mut OsuPadTray| {
                            let _ = tray.tx.send(action);
                        }),
                        ..Default::default()
                    }
                    .into(),
                })
                .collect()
        }
    }
}

#[cfg(target_os = "linux")]
pub fn stream() -> impl futures_util::Stream<Item = TrayEvent> {
    use ksni::TrayMethods;
    iced::stream::channel(
        20,
        |mut output: iced::futures::channel::mpsc::Sender<TrayEvent>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TrayAction>();
            let tray = linux::OsuPadTray {
                tx,
                status: TrayStatus::default(),
            };
            match tray.spawn().await {
                Ok(handle) => {
                    let _ = output
                        .send(TrayEvent::Started(TrayHandle { inner: handle }))
                        .await;
                    while let Some(action) = rx.recv().await {
                        let _ = output.send(TrayEvent::Action(action)).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("System tray unavailable: {e}");
                    let _ = output.send(TrayEvent::Unavailable).await;
                }
            }
        },
    )
}

#[cfg(target_os = "linux")]
pub fn update(handle: &TrayHandle, status: TrayStatus) {
    let handle = handle.inner.clone();
    tokio::spawn(async move {
        let _ = handle.update(move |t| t.status = status).await;
    });
}

// ---------------------------------------------------------------------------
// Windows Backend (tray-icon + muda)
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows {
    use super::*;
    use std::collections::HashMap;
    use std::sync::mpsc::{channel, Sender};
    use tray_icon::menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, PostThreadMessageW, TranslateMessage, MSG, WM_USER,
    };

    #[derive(Clone)]
    pub struct WindowsTrayHandle {
        thread_id: u32,
        status_tx: Sender<TrayStatus>,
    }

    impl WindowsTrayHandle {
        pub fn update(&self, status: TrayStatus) {
            let _ = self.status_tx.send(status);
            unsafe {
                PostThreadMessageW(self.thread_id, WM_USER, 0, 0);
            }
        }
    }

    fn create_icon(state: IconState) -> Icon {
        let (r, g, b) = match state {
            IconState::Warning => (245, 158, 11),       // amber
            IconState::Connected => (235, 91, 142),     // osu! pink
            IconState::Disconnected => (156, 163, 175), // gray
        };
        let size = 16u32;
        let mut rgba = Vec::with_capacity((size * size * 4) as usize);
        let center = 7.5f32;
        let radius = 6.5f32;
        for y in 0..size {
            for x in 0..size {
                let dx = x as f32 - center;
                let dy = y as f32 - center;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= radius {
                    rgba.extend_from_slice(&[r, g, b, 255]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        Icon::from_rgba(rgba, size, size).expect("valid tray icon")
    }

    fn build_menu(model: &TrayViewModel) -> (Menu, HashMap<tray_icon::menu::MenuId, TrayAction>) {
        let menu = Menu::new();
        let mut action_map = HashMap::new();

        for item in &model.menu_items {
            match item {
                MenuItemModel::Disabled(label) => {
                    let mi = MenuItem::new(label, false, None);
                    let _ = menu.append(&mi);
                }
                MenuItemModel::Separator => {
                    let sep = PredefinedMenuItem::separator();
                    let _ = menu.append(&sep);
                }
                MenuItemModel::Action {
                    label,
                    action,
                    enabled,
                } => {
                    let mi = MenuItem::new(label, *enabled, None);
                    action_map.insert(mi.id().clone(), *action);
                    let _ = menu.append(&mi);
                }
            }
        }
        (menu, action_map)
    }

    pub fn start_tray_thread(
        action_tx: tokio::sync::mpsc::UnboundedSender<TrayAction>,
    ) -> Result<WindowsTrayHandle, String> {
        let (status_tx, status_rx) = channel::<TrayStatus>();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32, String>>();

        let builder = std::thread::Builder::new().name("osupad-tray".into());
        let spawn_res = builder.spawn(move || {
            let thread_id = unsafe { GetCurrentThreadId() };

            let initial_status = TrayStatus::default();
            let initial_model = TrayViewModel::from_status(&initial_status);
            let (initial_menu, mut action_map) = build_menu(&initial_model);
            let icon = create_icon(initial_model.icon_state);

            let tray = match TrayIconBuilder::new()
                .with_menu(Box::new(initial_menu))
                .with_tooltip(&initial_model.tooltip_description)
                .with_icon(icon)
                .with_menu_on_left_click(false)
                .build()
            {
                Ok(t) => {
                    let _ = ready_tx.send(Ok(thread_id));
                    t
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };

            let menu_channel = tray_icon::menu::MenuEvent::receiver();
            let tray_channel = TrayIconEvent::receiver();

            let mut msg: MSG = unsafe { std::mem::zeroed() };
            loop {
                let res = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };
                if res <= 0 {
                    break;
                }

                if msg.message == WM_USER {
                    let mut latest_status = None;
                    while let Ok(st) = status_rx.try_recv() {
                        latest_status = Some(st);
                    }
                    if let Some(st) = latest_status {
                        let model = TrayViewModel::from_status(&st);
                        let (new_menu, new_action_map) = build_menu(&model);
                        action_map = new_action_map;
                        tray.set_menu(Some(Box::new(new_menu) as Box<dyn ContextMenu>));
                        let _ = tray.set_tooltip(Some(&model.tooltip_description));
                        let _ = tray.set_icon(Some(create_icon(model.icon_state)));
                    }
                } else {
                    unsafe {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }

                // Drain menu click events
                while let Ok(event) = menu_channel.try_recv() {
                    if let Some(&action) = action_map.get(&event.id) {
                        let _ = action_tx.send(action);
                    }
                }

                // Drain tray click events (left click or double click opens window)
                while let Ok(event) = tray_channel.try_recv() {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                        | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } => {
                            let _ = action_tx.send(TrayAction::ShowWindow);
                        }
                        _ => {}
                    }
                }
            }
        });

        if let Err(e) = spawn_res {
            return Err(format!("Failed to spawn tray thread: {e}"));
        }

        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(WindowsTrayHandle {
                thread_id,
                status_tx,
            }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Tray thread exited prematurely".into()),
        }
    }
}

#[cfg(windows)]
pub fn stream() -> impl futures_util::Stream<Item = TrayEvent> {
    iced::stream::channel(
        20,
        |mut output: iced::futures::channel::mpsc::Sender<TrayEvent>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TrayAction>();
            match windows::start_tray_thread(tx) {
                Ok(handle) => {
                    let _ = output
                        .send(TrayEvent::Started(TrayHandle { inner: handle }))
                        .await;
                    while let Some(action) = rx.recv().await {
                        let _ = output.send(TrayEvent::Action(action)).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("System tray unavailable: {e}");
                    let _ = output.send(TrayEvent::Unavailable).await;
                }
            }
        },
    )
}

#[cfg(windows)]
pub fn update(handle: &TrayHandle, status: TrayStatus) {
    handle.inner.update(status);
}

// ---------------------------------------------------------------------------
// Fallback for unsupported platforms
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "linux", windows)))]
pub fn stream() -> impl futures_util::Stream<Item = TrayEvent> {
    iced::stream::channel(
        20,
        |mut output: iced::futures::channel::mpsc::Sender<TrayEvent>| async move {
            let _ = output.send(TrayEvent::Unavailable).await;
        },
    )
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn update(_handle: &TrayHandle, _status: TrayStatus) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_view_model_offline() {
        let status = TrayStatus {
            daemon_online: false,
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert_eq!(vm.icon_state, IconState::Warning);
        assert_eq!(vm.tooltip_description, "Daemon offline");
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Pad: Daemon offline".into())));
        assert!(vm.menu_items.contains(&MenuItemModel::Action {
            label: "Start daemon".into(),
            action: TrayAction::StartDaemon,
            enabled: true,
        }));
    }

    #[test]
    fn test_tray_view_model_connected() {
        let status = TrayStatus {
            daemon_online: true,
            device_connected: true,
            firmware_version: Some("1.2.0".into()),
            key1_presses: 1_284_391,
            key2_presses: 987_654,
            total_presses: 2_272_045,
            mode: "osu!".into(),
            tosu_connected: true,
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert_eq!(vm.icon_state, IconState::Connected);
        assert!(vm.tooltip_description.contains("Pad: connected"));
        assert!(vm.tooltip_description.contains("2,272,045 presses · osu!"));
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Pad: Connected".into())));
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Firmware: 1.2.0".into())));
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Key 1: 1,284,391".into())));
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Key 2: 987,654".into())));
        assert!(vm.menu_items.contains(&MenuItemModel::Action {
            label: "Sync pad now".into(),
            action: TrayAction::SyncNow,
            enabled: true,
        }));
    }

    #[test]
    fn test_tray_view_model_sync_failed() {
        let status = TrayStatus {
            daemon_online: true,
            device_connected: true,
            last_sync_error: Some("timeout".into()),
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert!(vm
            .menu_items
            .contains(&MenuItemModel::Disabled("Last sync: Sync failed".into())));
    }
}
