//! System tray icon (StatusNotifierItem via ksni on Linux, tray-icon on Windows).
//! The app lives here while its window is closed.

use futures_util::SinkExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    ShowWindow,
    /// Open the window on this page
    Open(crate::Page),
    SyncNow,
    StartDaemon,
    Quit,
}

#[derive(Clone)]
pub struct TrayHandle {
    #[cfg(target_os = "linux")]
    inner: ksni::Handle<linux::OpadTray>,
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

/// What the tray's status lines and tooltip show
#[derive(Debug, Clone, Default)]
pub struct TrayStatus {
    pub daemon_online: bool,
    pub device_connected: bool,
    pub incompatible: bool,
    pub last_sync_error: Option<String>,
    pub is_playing_or_cooldown: bool,
    pub tosu_connected: bool,
    /// Presses on the game keys, as the Dashboard counts them
    pub total_presses: u64,
    /// What the pad is doing, in words (`pages::mode_words`)
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
    #[cfg_attr(windows, allow(dead_code))]
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

        // One line that says how things are, the same in the menu and the tooltip
        let status = if !s.daemon_online {
            "OPad service stopped".to_string()
        } else if s.incompatible {
            "Pad needs an update".to_string()
        } else if !s.device_connected {
            "Pad not connected".to_string()
        } else if s.last_sync_error.is_some() {
            "Pad connected · last sync failed".to_string()
        } else {
            format!("Pad connected · {}", s.mode)
        };
        let total = format!("Total presses: {}", crate::pages::grouped(s.total_presses));

        let tooltip_description = if !s.daemon_online {
            status.clone()
        } else {
            format!(
                "{status}\nosu! {}\n{total}",
                if s.tosu_connected {
                    "connected"
                } else {
                    "not connected"
                },
            )
        };

        let mut items = vec![MenuItemModel::Disabled(status)];
        if s.daemon_online {
            items.push(MenuItemModel::Disabled(total));
        }
        items.push(MenuItemModel::Separator);

        // Where people go from the tray
        items.push(MenuItemModel::Action {
            label: "Open OPad".into(),
            action: TrayAction::ShowWindow,
            enabled: true,
        });
        items.push(MenuItemModel::Action {
            label: "Keys".into(),
            action: TrayAction::Open(crate::Page::Keys),
            enabled: true,
        });
        items.push(MenuItemModel::Action {
            label: "Troubleshooting".into(),
            action: TrayAction::Open(crate::Page::Troubleshooting),
            enabled: true,
        });
        items.push(MenuItemModel::Separator);

        // The one thing to do from here: sync, or start the service when it
        // is what stopped
        if s.daemon_online {
            items.push(MenuItemModel::Action {
                label: "Sync pad now".into(),
                action: TrayAction::SyncNow,
                enabled: s.device_connected && !s.is_playing_or_cooldown,
            });
        } else {
            items.push(MenuItemModel::Action {
                label: "Start OPad service".into(),
                action: TrayAction::StartDaemon,
                enabled: true,
            });
        }
        items.push(MenuItemModel::Separator);

        // Quits this app only: the service, and so the pad's counting and
        // screen, keep running
        items.push(MenuItemModel::Action {
            label: "Quit OPad".into(),
            action: TrayAction::Quit,
            enabled: true,
        });

        Self {
            title: "OPad".to_string(),
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

    pub struct OpadTray {
        pub tx: tokio::sync::mpsc::UnboundedSender<TrayAction>,
        pub status: TrayStatus,
    }

    impl ksni::Tray for OpadTray {
        fn id(&self) -> String {
            "opad".into()
        }

        fn title(&self) -> String {
            "OPad".into()
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
                        activate: Box::new(move |tray: &mut OpadTray| {
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
async fn check_status_notifier_watcher() -> bool {
    let Ok(conn) = zbus::connection::Connection::session().await else {
        return false;
    };
    let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await else {
        return false;
    };
    let Ok(well_known) = "org.kde.StatusNotifierWatcher".try_into() else {
        return false;
    };
    proxy.name_has_owner(well_known).await.unwrap_or(false)
}

/// How long to wait before probing for a tray host again, `since_start` into
/// the retries: often at first (a login autostart can beat the AppIndicator
/// extension or SNI host by a few seconds), then rarely, for a host
/// installed or enabled later in the session.
#[cfg(any(target_os = "linux", test))]
fn tray_retry_delay(since_start: std::time::Duration) -> std::time::Duration {
    if since_start < std::time::Duration::from_secs(60) {
        std::time::Duration::from_secs(5)
    } else {
        std::time::Duration::from_secs(30)
    }
}

#[cfg(target_os = "linux")]
pub fn stream() -> impl futures_util::Stream<Item = TrayEvent> {
    use ksni::TrayMethods;
    iced::stream::channel(
        20,
        |mut output: iced::futures::channel::mpsc::Sender<TrayEvent>| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<TrayAction>();
            let started = std::time::Instant::now();
            let mut reported_unavailable = false;

            // Probed until a host appears, not once: Unavailable is reported
            // the first time only, and Started whenever the tray comes up
            let handle = loop {
                // 2-second timeout to check if a StatusNotifierItem host is present on GNOME Shell / desktop
                let watcher_available = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    check_status_notifier_watcher(),
                )
                .await
                .unwrap_or(false);

                let failure = if watcher_available {
                    let tray = linux::OpadTray {
                        tx: tx.clone(),
                        status: TrayStatus::default(),
                    };
                    match tokio::time::timeout(std::time::Duration::from_secs(2), tray.spawn())
                        .await
                    {
                        Ok(Ok(handle)) => break handle,
                        Ok(Err(e)) => format!("System tray spawn failed: {e}"),
                        Err(_) => "System tray spawn timed out after 2 seconds".to_string(),
                    }
                } else {
                    "No StatusNotifierWatcher host found on DBus within 2 seconds (e.g. GNOME Shell without AppIndicator extension)".to_string()
                };

                if !reported_unavailable {
                    tracing::warn!("{failure}; marking tray unavailable and retrying");
                    let _ = output.send(TrayEvent::Unavailable).await;
                    reported_unavailable = true;
                } else {
                    tracing::debug!("{failure}; retrying");
                }
                tokio::time::sleep(tray_retry_delay(started.elapsed())).await;
            };

            if reported_unavailable {
                tracing::info!("A system tray host appeared; the tray icon is up");
            }
            let _ = output
                .send(TrayEvent::Started(TrayHandle { inner: handle }))
                .await;
            while let Some(action) = rx.recv().await {
                let _ = output.send(TrayEvent::Action(action)).await;
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

        let builder = std::thread::Builder::new().name("opad-tray".into());
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
    fn a_missing_tray_is_retried_often_at_first_then_rarely() {
        use std::time::Duration;
        assert_eq!(tray_retry_delay(Duration::ZERO), Duration::from_secs(5));
        assert_eq!(
            tray_retry_delay(Duration::from_secs(59)),
            Duration::from_secs(5)
        );
        assert_eq!(
            tray_retry_delay(Duration::from_secs(60)),
            Duration::from_secs(30)
        );
        assert_eq!(
            tray_retry_delay(Duration::from_secs(3600)),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn the_tray_says_the_service_stopped_and_offers_to_start_it() {
        let status = TrayStatus {
            daemon_online: false,
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert_eq!(vm.icon_state, IconState::Warning);
        assert_eq!(vm.tooltip_description, "OPad service stopped");
        assert_eq!(
            vm.menu_items[0],
            MenuItemModel::Disabled("OPad service stopped".into())
        );
        assert!(vm.menu_items.contains(&MenuItemModel::Action {
            label: "Start OPad service".into(),
            action: TrayAction::StartDaemon,
            enabled: true,
        }));
        // Nothing to sync with, and no counters to show
        assert!(!vm.menu_items.iter().any(|i| matches!(
            i,
            MenuItemModel::Action {
                action: TrayAction::SyncNow,
                ..
            }
        )));
        assert!(!vm
            .menu_items
            .iter()
            .any(|i| matches!(i, MenuItemModel::Disabled(l) if l.starts_with("Total presses"))));
    }

    #[test]
    fn the_tray_shows_the_pad_its_total_and_where_to_go() {
        let status = TrayStatus {
            daemon_online: true,
            device_connected: true,
            total_presses: 2_272_045,
            mode: "Ready".into(),
            tosu_connected: true,
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert_eq!(vm.icon_state, IconState::Connected);
        assert_eq!(
            vm.tooltip_description,
            "Pad connected · Ready\nosu! connected\nTotal presses: 2,272,045"
        );
        let labels: Vec<String> = vm
            .menu_items
            .iter()
            .map(|i| match i {
                MenuItemModel::Disabled(l) => format!("({l})"),
                MenuItemModel::Separator => "-".into(),
                MenuItemModel::Action { label, .. } => label.clone(),
            })
            .collect();
        assert_eq!(
            labels,
            [
                "(Pad connected · Ready)",
                "(Total presses: 2,272,045)",
                "-",
                "Open OPad",
                "Keys",
                "Troubleshooting",
                "-",
                "Sync pad now",
                "-",
                "Quit OPad",
            ]
        );
        assert!(vm.menu_items.contains(&MenuItemModel::Action {
            label: "Troubleshooting".into(),
            action: TrayAction::Open(crate::Page::Troubleshooting),
            enabled: true,
        }));
    }

    #[test]
    fn sync_is_greyed_out_during_a_map_and_without_a_pad() {
        let sync_enabled = |status: &TrayStatus| {
            TrayViewModel::from_status(status)
                .menu_items
                .iter()
                .find_map(|i| match i {
                    MenuItemModel::Action {
                        action: TrayAction::SyncNow,
                        enabled,
                        ..
                    } => Some(*enabled),
                    _ => None,
                })
                .unwrap()
        };
        let mut status = TrayStatus {
            daemon_online: true,
            device_connected: true,
            mode: "In a map".into(),
            is_playing_or_cooldown: true,
            ..Default::default()
        };
        assert!(!sync_enabled(&status));
        status.is_playing_or_cooldown = false;
        assert!(sync_enabled(&status));
        status.device_connected = false;
        assert!(!sync_enabled(&status));
    }

    #[test]
    fn a_failed_sync_shows_in_the_status_line() {
        let status = TrayStatus {
            daemon_online: true,
            device_connected: true,
            last_sync_error: Some("timeout".into()),
            ..Default::default()
        };
        let vm = TrayViewModel::from_status(&status);
        assert_eq!(
            vm.menu_items[0],
            MenuItemModel::Disabled("Pad connected · last sync failed".into())
        );
    }
}
