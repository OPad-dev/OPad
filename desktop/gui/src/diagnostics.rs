//! Interactive diagnostics and hardware testing suite.
//!
//! Provides:
//! 1. Switch Chatter & Input Tester (real-time key visualization, <15ms bounce detection, audio beep, NKRO test)
//! 2. Hardware COM & Protocol Test (serial connection, ping latency, COM-02 resilience, latency percentiles)
//! 3. Display & Screen Self-Test (backlight brightness ramp, test patterns, layout preview)
//! 4. One-click Diagnostic Bundle Exporter (system info, USB stats, firmware version, counters, and recent logs)

use crate::theme::{self, caption, heading, muted};
use crate::{App, Message};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
use iced::{Alignment, Border, Element, Length};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn Beep(dwFreq: u32, dwDuration: u32) -> i32;
}

#[allow(dead_code)]
pub fn is_key_down(vk: i32) -> bool {
    #[cfg(windows)]
    unsafe {
        (GetAsyncKeyState(vk) as u16 & 0x8000) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = vk;
        false
    }
}

#[cfg(windows)]
pub fn play_chatter_beep() {
    std::thread::spawn(|| unsafe {
        Beep(1760, 40);
    });
}

#[cfg(target_os = "linux")]
pub fn play_chatter_beep() {
    std::thread::spawn(|| {
        use rodio::Source as _;
        if let Ok(handle) = rodio::DeviceSinkBuilder::open_default_sink() {
            let source = rodio::source::SineWave::new(1760.0)
                .take_duration(Duration::from_millis(40))
                .amplify(0.20);
            handle.mixer().add(source);
            std::thread::sleep(Duration::from_millis(45));
            return;
        }
        // Fallback: terminal ASCII bell
        print!("\x07");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn play_chatter_beep() {}

#[cfg(target_os = "linux")]
static LINUX_EVDEV: std::sync::Mutex<Option<evdev::Device>> = std::sync::Mutex::new(None);

/// When the pad's input device was last looked for. The poll runs every 4 ms,
/// and scanning /dev/input that often while no pad is plugged in is wasted work.
#[cfg(target_os = "linux")]
static LINUX_EVDEV_LAST_SCAN: std::sync::Mutex<Option<std::time::Instant>> =
    std::sync::Mutex::new(None);

#[cfg(target_os = "linux")]
const EVDEV_RESCAN_INTERVAL: Duration = Duration::from_secs(2);

#[cfg(target_os = "linux")]
pub fn char_to_evdev_key(s: &str) -> Option<evdev::KeyCode> {
    let s = s.trim().to_uppercase();
    if s.len() == 1 {
        let c = s.chars().next()?;
        if c.is_ascii_alphabetic() {
            return match c {
                'A' => Some(evdev::KeyCode::KEY_A),
                'B' => Some(evdev::KeyCode::KEY_B),
                'C' => Some(evdev::KeyCode::KEY_C),
                'D' => Some(evdev::KeyCode::KEY_D),
                'E' => Some(evdev::KeyCode::KEY_E),
                'F' => Some(evdev::KeyCode::KEY_F),
                'G' => Some(evdev::KeyCode::KEY_G),
                'H' => Some(evdev::KeyCode::KEY_H),
                'I' => Some(evdev::KeyCode::KEY_I),
                'J' => Some(evdev::KeyCode::KEY_J),
                'K' => Some(evdev::KeyCode::KEY_K),
                'L' => Some(evdev::KeyCode::KEY_L),
                'M' => Some(evdev::KeyCode::KEY_M),
                'N' => Some(evdev::KeyCode::KEY_N),
                'O' => Some(evdev::KeyCode::KEY_O),
                'P' => Some(evdev::KeyCode::KEY_P),
                'Q' => Some(evdev::KeyCode::KEY_Q),
                'R' => Some(evdev::KeyCode::KEY_R),
                'S' => Some(evdev::KeyCode::KEY_S),
                'T' => Some(evdev::KeyCode::KEY_T),
                'U' => Some(evdev::KeyCode::KEY_U),
                'V' => Some(evdev::KeyCode::KEY_V),
                'W' => Some(evdev::KeyCode::KEY_W),
                'X' => Some(evdev::KeyCode::KEY_X),
                'Y' => Some(evdev::KeyCode::KEY_Y),
                'Z' => Some(evdev::KeyCode::KEY_Z),
                _ => None,
            };
        } else if c.is_ascii_digit() {
            return match c {
                '0' => Some(evdev::KeyCode::KEY_0),
                '1' => Some(evdev::KeyCode::KEY_1),
                '2' => Some(evdev::KeyCode::KEY_2),
                '3' => Some(evdev::KeyCode::KEY_3),
                '4' => Some(evdev::KeyCode::KEY_4),
                '5' => Some(evdev::KeyCode::KEY_5),
                '6' => Some(evdev::KeyCode::KEY_6),
                '7' => Some(evdev::KeyCode::KEY_7),
                '8' => Some(evdev::KeyCode::KEY_8),
                '9' => Some(evdev::KeyCode::KEY_9),
                _ => None,
            };
        }
    }
    match s.as_str() {
        "SPACE" => Some(evdev::KeyCode::KEY_SPACE),
        "ENTER" | "RETURN" => Some(evdev::KeyCode::KEY_ENTER),
        "LSHIFT" | "SHIFT" => Some(evdev::KeyCode::KEY_LEFTSHIFT),
        "LCTRL" | "CTRL" => Some(evdev::KeyCode::KEY_LEFTCTRL),
        "LALT" | "ALT" => Some(evdev::KeyCode::KEY_LEFTALT),
        "ESC" | "ESCAPE" => Some(evdev::KeyCode::KEY_ESC),
        "LEFT" => Some(evdev::KeyCode::KEY_LEFT),
        "UP" => Some(evdev::KeyCode::KEY_UP),
        "RIGHT" => Some(evdev::KeyCode::KEY_RIGHT),
        "DOWN" => Some(evdev::KeyCode::KEY_DOWN),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn open_opad_evdev() -> Option<evdev::Device> {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let found = find_opad_evdev();
    if found.is_none() && !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        tracing::warn!(
            "Cannot open the OPad's input device, so switch diagnostics see no presses. \
             Is the pad plugged in and 70-opad.rules installed (it grants access to it)?"
        );
    }
    found
}

#[cfg(target_os = "linux")]
fn find_opad_evdev() -> Option<evdev::Device> {
    if let Ok(entries) = std::fs::read_dir("/dev/input/by-id") {
        let mut candidates = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.contains("opad") || lower.contains("osupad") {
                if lower.contains("event-kbd") {
                    if let Ok(dev) = evdev::Device::open(entry.path()) {
                        return Some(dev);
                    }
                }
                candidates.push(entry.path());
            }
        }
        for path in candidates {
            if let Ok(dev) = evdev::Device::open(path) {
                return Some(dev);
            }
        }
    }

    for (_path, dev) in evdev::enumerate() {
        let id = dev.input_id();
        if id.vendor() == 0x303a && id.product() == 0x4001 {
            return Some(dev);
        }
    }

    None
}

#[cfg(target_os = "linux")]
pub fn poll_linux_switch_inputs(diag: &mut DiagnosticsState, k1_str: &str, k2_str: &str) {
    let mut guard = match LINUX_EVDEV.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if guard.is_none() {
        let mut last = LINUX_EVDEV_LAST_SCAN.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < EVDEV_RESCAN_INTERVAL) {
            return;
        }
        *last = Some(std::time::Instant::now());
        *guard = open_opad_evdev();
    }

    if let Some(dev) = guard.as_mut() {
        match dev.get_key_state() {
            Ok(keys) => {
                if let Some(k1) = char_to_evdev_key(k1_str) {
                    diag.handle_key_event(1, keys.contains(k1));
                }
                if let Some(k2) = char_to_evdev_key(k2_str) {
                    diag.handle_key_event(2, keys.contains(k2));
                }
            }
            Err(_) => {
                *guard = None;
            }
        }
    }
}

#[allow(dead_code)]
pub fn char_to_vk(s: &str) -> Option<i32> {
    let s = s.trim().to_uppercase();
    if s.len() == 1 {
        let c = s.chars().next()?;
        if c.is_ascii_alphanumeric() {
            return Some(c as i32);
        }
    }
    match s.as_str() {
        "SPACE" => Some(0x20),
        "ENTER" | "RETURN" => Some(0x0D),
        "LSHIFT" | "SHIFT" => Some(0x10),
        "LCTRL" | "CTRL" => Some(0x11),
        "LALT" | "ALT" => Some(0x12),
        "ESC" | "ESCAPE" => Some(0x1B),
        "LEFT" => Some(0x25),
        "UP" => Some(0x26),
        "RIGHT" => Some(0x27),
        "DOWN" => Some(0x28),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticsTab {
    #[default]
    InputTester,
    ComProtocol,
    DisplayScreen,
    ExportBundle,
}

#[derive(Debug, Clone)]
pub enum DiagnosticsMessage {
    SelectTab(DiagnosticsTab),
    ToggleSound(bool),
    ResetInputTester,
    KeyEvent { key_idx: u8, is_down: bool },
    SelectDisplayPattern(usize),
    TestBrightness(u32),
    PingDaemon,
    PingDone(Result<Duration, String>),
}

#[derive(Debug, Clone, Default)]
pub struct KeyTrackerState {
    pub is_down: bool,
    pub press_count: u32,
    pub last_hold_ms: Option<f64>,
    pub shortest_repress_ms: Option<f64>,
    pub definite_chatter_count: u32,
    pub fast_flutter_count: u32,
    pub last_press_time: Option<Instant>,
    pub last_release_time: Option<Instant>,
}

pub struct DiagnosticsState {
    pub active_tab: DiagnosticsTab,
    pub sound_enabled: bool,
    pub k1: KeyTrackerState,
    pub k2: KeyTrackerState,
    pub simultaneous_press_count: u32,
    pub event_log: VecDeque<String>,
    pub test_pattern_index: usize,
    pub ping_in_progress: bool,
    pub last_ping_ms: Option<f64>,
}

impl Default for DiagnosticsState {
    fn default() -> Self {
        Self {
            active_tab: DiagnosticsTab::InputTester,
            sound_enabled: true,
            k1: KeyTrackerState::default(),
            k2: KeyTrackerState::default(),
            simultaneous_press_count: 0,
            event_log: VecDeque::with_capacity(30),
            test_pattern_index: 0,
            ping_in_progress: false,
            last_ping_ms: None,
        }
    }
}

impl DiagnosticsState {
    pub fn reset_input_tester(&mut self) {
        self.k1 = KeyTrackerState::default();
        self.k2 = KeyTrackerState::default();
        self.simultaneous_press_count = 0;
        self.event_log.clear();
        self.event_log
            .push_back("Tester reset. Ready to monitor switch inputs.".to_string());
    }

    pub fn handle_key_event(&mut self, key_idx: u8, is_down: bool) {
        let now = Instant::now();
        let key_name = if key_idx == 1 { "Key 1" } else { "Key 2" };

        let (target, other) = if key_idx == 1 {
            (&mut self.k1, &self.k2)
        } else {
            (&mut self.k2, &self.k1)
        };

        if is_down && !target.is_down {
            target.is_down = true;
            target.press_count += 1;

            if let Some(rel) = target.last_release_time {
                let delta_ms = (now - rel).as_secs_f64() * 1000.0;
                match target.shortest_repress_ms {
                    Some(curr) if delta_ms < curr => target.shortest_repress_ms = Some(delta_ms),
                    None => target.shortest_repress_ms = Some(delta_ms),
                    _ => {}
                }

                if delta_ms < 15.0 {
                    target.definite_chatter_count += 1;
                    if self.sound_enabled {
                        play_chatter_beep();
                    }
                    let entry = format!(
                        "🔴 CHATTER: {} bounced after only {:.2} ms!",
                        key_name, delta_ms
                    );
                    if self.event_log.len() >= 25 {
                        self.event_log.pop_front();
                    }
                    self.event_log.push_back(entry);
                } else if delta_ms <= 35.0 {
                    target.fast_flutter_count += 1;
                    let entry = format!(
                        "🟡 FLUTTER: {} re-pressed in {:.2} ms (rapid twitch / wobble)",
                        key_name, delta_ms
                    );
                    if self.event_log.len() >= 25 {
                        self.event_log.pop_front();
                    }
                    self.event_log.push_back(entry);
                } else {
                    let entry = format!("{} pressed (interval: {:.1} ms)", key_name, delta_ms);
                    if self.event_log.len() >= 25 {
                        self.event_log.pop_front();
                    }
                    self.event_log.push_back(entry);
                }
            } else {
                let entry = format!("{} pressed", key_name);
                if self.event_log.len() >= 25 {
                    self.event_log.pop_front();
                }
                self.event_log.push_back(entry);
            }

            // Check simultaneous press with other key (within 10ms)
            if other.is_down {
                if let Some(other_press) = other.last_press_time {
                    let delta_ms = (now - other_press).as_secs_f64() * 1000.0;
                    if delta_ms <= 10.0 {
                        self.simultaneous_press_count += 1;
                    }
                }
            }

            target.last_press_time = Some(now);
        } else if !is_down && target.is_down {
            target.is_down = false;
            if let Some(press) = target.last_press_time {
                let hold_ms = (now - press).as_secs_f64() * 1000.0;
                target.last_hold_ms = Some(hold_ms);
                let entry = format!("{} released (held {:.1} ms)", key_name, hold_ms);
                if self.event_log.len() >= 25 {
                    self.event_log.pop_front();
                }
                self.event_log.push_back(entry);
            }
            target.last_release_time = Some(now);
        }
    }

    pub fn view<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let title_row = row![
            heading("Diagnostics & Testing Suite"),
            Space::new().width(Length::Fill),
            if app.device_connected {
                text("● Pad Connected").size(13).color(theme::GREEN)
            } else {
                text("○ Pad Disconnected").size(13).color(theme::MUTED)
            },
            Space::new().width(12),
            if app.daemon_online {
                text("● Daemon Online").size(13).color(theme::GREEN)
            } else {
                text("○ Daemon Offline").size(13).color(theme::RED)
            }
        ]
        .align_y(Alignment::Center);

        let tab_btn = |tab: DiagnosticsTab, label: &'static str| {
            button(text(label).size(13).font(theme::FONT))
                .padding([8, 16])
                .style(theme::tab_button(self.active_tab == tab))
                .on_press(Message::Diagnostics(DiagnosticsMessage::SelectTab(tab)))
        };

        let tab_bar = row![
            tab_btn(DiagnosticsTab::InputTester, "⌨ Switch Chatter & Input"),
            tab_btn(DiagnosticsTab::ComProtocol, "🔌 Hardware COM & Protocol"),
            tab_btn(DiagnosticsTab::DisplayScreen, "🖥 Screen & Backlight"),
            tab_btn(DiagnosticsTab::ExportBundle, "📦 Export Diagnostic Bundle"),
        ]
        .spacing(8);

        let content: Element<'_, Message> = match self.active_tab {
            DiagnosticsTab::InputTester => self.view_input_tester(app),
            DiagnosticsTab::ComProtocol => self.view_com_protocol(app),
            DiagnosticsTab::DisplayScreen => self.view_display_screen(app),
            DiagnosticsTab::ExportBundle => self.view_export_bundle(app),
        };

        scrollable(
            column![
                title_row,
                muted(
                    "Interactive diagnostic and verification tools. Use this to inspect switch chatter, \
                     serial protocol health, pad screen functions, and export system diagnostic reports."
                ),
                tab_bar,
                content
            ]
            .spacing(16),
        )
        .into()
    }

    fn view_input_tester<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let k1_label = if app.k1_input.is_empty() {
            "Z"
        } else {
            &app.k1_input
        };
        let k2_label = if app.k2_input.is_empty() {
            "X"
        } else {
            &app.k2_input
        };

        let key_card = |label: &'a str,
                        k: &'a KeyTrackerState,
                        pin: u32,
                        is_k1: bool|
         -> Element<'a, Message> {
            let fill = if k.is_down { theme::PINK } else { theme::CARD };
            let border_color = if k.is_down {
                theme::WHITE
            } else if k.definite_chatter_count > 0 {
                theme::RED
            } else {
                theme::BORDER
            };
            let text_color = if k.is_down { theme::BG } else { theme::WHITE };

            let chatter_info = if k.definite_chatter_count > 0 {
                text(format!(
                    "⚠ {} chatter events (<15ms)",
                    k.definite_chatter_count
                ))
                .size(12)
                .color(theme::RED)
                .font(theme::FONT_BOLD)
            } else {
                text("0 switch bounce / chatter detected")
                    .size(12)
                    .color(theme::GREEN)
            };

            let repress_str = k
                .shortest_repress_ms
                .map(|ms| format!("{:.2} ms", ms))
                .unwrap_or_else(|| "—".to_string());

            let hold_str = k
                .last_hold_ms
                .map(|ms| format!("{:.1} ms", ms))
                .unwrap_or_else(|| "—".to_string());

            let key_box = container(
                column![
                    text(label)
                        .size(42)
                        .font(theme::FONT_BOLD)
                        .color(text_color),
                    text(if k.is_down { "PRESSED" } else { "RELEASED" })
                        .size(11)
                        .font(theme::FONT_BOLD)
                        .color(if k.is_down { theme::BG } else { theme::MUTED }),
                ]
                .align_x(Alignment::Center)
                .spacing(4),
            )
            .width(130)
            .height(110)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(fill.into()),
                border: Border {
                    color: border_color,
                    width: 2.0,
                    radius: 12.0.into(),
                },
                ..Default::default()
            });

            container(
                row![
                    key_box,
                    column![
                        row![
                            text(if is_k1 { "Key 1" } else { "Key 2" })
                                .size(16)
                                .font(theme::FONT_BOLD),
                            Space::new().width(Length::Fill),
                            muted(format!("GPIO Pin: {}", pin)).size(12),
                        ]
                        .align_y(Alignment::Center),
                        row![
                            muted("Total presses:").width(130),
                            text(k.press_count.to_string())
                                .size(14)
                                .font(theme::FONT_BOLD),
                        ],
                        row![muted("Last hold time:").width(130), text(hold_str).size(13),],
                        row![
                            muted("Shortest repress:").width(130),
                            text(repress_str).size(13),
                        ],
                        chatter_info,
                    ]
                    .spacing(6)
                    .width(Length::Fill)
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            )
            .padding(16)
            .width(Length::FillPortion(1))
            .style(theme::card)
            .into()
        };

        let card_k1 = key_card(k1_label, &self.k1, app.k1_gpio, true);
        let card_k2 = key_card(k2_label, &self.k2, app.k2_gpio, false);

        let total_presses = self.k1.press_count + self.k2.press_count;
        let total_chatter = self.k1.definite_chatter_count + self.k2.definite_chatter_count;
        let total_flutter = self.k1.fast_flutter_count + self.k2.fast_flutter_count;

        let summary_card = container(
            column![
                row![
                    text("Session Input Metrics")
                        .size(15)
                        .font(theme::FONT_BOLD),
                    Space::new().width(Length::Fill),
                    button(text("Reset Session").size(12))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::Diagnostics(DiagnosticsMessage::ResetInputTester)),
                ]
                .align_y(Alignment::Center),
                row![
                    column![
                        caption("TOTAL PRESSES"),
                        text(total_presses.to_string())
                            .size(18)
                            .font(theme::FONT_BOLD),
                    ]
                    .width(Length::FillPortion(1)),
                    column![
                        caption("CHATTER BOUNCES (<15ms)"),
                        text(total_chatter.to_string())
                            .size(18)
                            .font(theme::FONT_BOLD)
                            .color(if total_chatter > 0 {
                                theme::RED
                            } else {
                                theme::GREEN
                            }),
                    ]
                    .width(Length::FillPortion(1)),
                    column![
                        caption("RAPID FLUTTER (15-35ms)"),
                        text(total_flutter.to_string())
                            .size(18)
                            .font(theme::FONT_BOLD)
                            .color(if total_flutter > 0 {
                                theme::YELLOW
                            } else {
                                theme::WHITE
                            }),
                    ]
                    .width(Length::FillPortion(1)),
                    column![
                        caption("SIMULTANEOUS PRESSES (NKRO)"),
                        text(self.simultaneous_press_count.to_string())
                            .size(18)
                            .font(theme::FONT_BOLD),
                    ]
                    .width(Length::FillPortion(1)),
                ]
                .spacing(14),
                row![
                    checkbox(self.sound_enabled)
                        .label("Audio beep alert on switch chatter (<15ms)")
                        .on_toggle(|v| Message::Diagnostics(DiagnosticsMessage::ToggleSound(v))),
                    Space::new().width(Length::Fill),
                    muted("Global detection: Works in-game while playing osu! or any song.")
                        .size(12),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(12),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        let mut events_col = column![row![
            text("Live Switch Event Timeline")
                .size(14)
                .font(theme::FONT_BOLD),
            Space::new().width(Length::Fill),
            muted(format!("{} recent events", self.event_log.len())).size(11),
        ]
        .align_y(Alignment::Center)]
        .spacing(6);

        if self.event_log.is_empty() {
            events_col = events_col.push(
                muted("No switch activity recorded yet. Tap Key 1 or Key 2 to test.").size(12),
            );
        } else {
            for line in self.event_log.iter().rev().take(10) {
                let color = if line.contains("CHATTER") {
                    theme::RED
                } else if line.contains("FLUTTER") {
                    theme::YELLOW
                } else if line.contains("released") {
                    theme::MUTED
                } else {
                    theme::WHITE
                };
                events_col = events_col.push(text(line).size(12).color(color));
            }
        }

        let event_card = container(events_col)
            .padding(14)
            .width(Length::Fill)
            .style(theme::card);

        column![row![card_k1, card_k2].spacing(14), summary_card, event_card]
            .spacing(14)
            .into()
    }

    fn view_com_protocol<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let info = app.device_info.as_ref();

        let ping_text = if self.ping_in_progress {
            "Testing IPC ping...".to_string()
        } else if let Some(ms) = self.last_ping_ms {
            format!("{:.2} ms round-trip", ms)
        } else {
            "Not tested yet".to_string()
        };

        let com_status_card = container(
            column![
                text("Serial & IPC Connection Status")
                    .size(16)
                    .font(theme::FONT_BOLD),
                row![
                    muted("Daemon IPC Transport:").width(200),
                    text(if app.daemon_online {
                        "Online (Restricted User Pipe)"
                    } else {
                        "Offline"
                    })
                    .size(13),
                ]
                .spacing(8),
                row![
                    muted("Device USB Connection:").width(200),
                    text(if app.device_connected {
                        "Connected (USB CDC ACM 115200 8N1)"
                    } else {
                        "Disconnected"
                    })
                    .size(13),
                ]
                .spacing(8),
                row![
                    muted("Device ID / Serial:").width(200),
                    text(
                        info.map(|i| i.device_id.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("—")
                    )
                    .size(13),
                ]
                .spacing(8),
                row![
                    muted("Board Profile:").width(200),
                    text(
                        info.map(|i| i.board_profile.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("—")
                    )
                    .size(13),
                ]
                .spacing(8),
                row![
                    muted("Firmware Version:").width(200),
                    text(
                        info.map(|i| i.firmware_version.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or("—")
                    )
                    .size(13),
                ]
                .spacing(8),
                row![
                    muted("Daemon Round-Trip Ping:").width(200),
                    text(ping_text)
                        .size(13)
                        .font(theme::FONT_BOLD)
                        .color(theme::GREEN),
                    Space::new().width(12),
                    button(text("Test Ping").size(12))
                        .padding([4, 10])
                        .style(theme::secondary)
                        .on_press(Message::Diagnostics(DiagnosticsMessage::PingDaemon)),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        let com02_card = container(
            column![
                row![
                    text("COM-02 Protocol Fault Recovery Specification")
                        .size(15)
                        .font(theme::FONT_BOLD),
                    Space::new().width(Length::Fill),
                    text("✓ ACTIVE").size(12).font(theme::FONT_BOLD).color(theme::GREEN),
                ]
                .align_y(Alignment::Center),
                muted(
                    "The pad's communication protocol runs over framed protobuf packets with length prefixes. \
                     COM-02 guarantees the pad rejects 512-byte random garbage noise, oversized frames (0x7FFFFFFF), \
                     and corrupted envelopes without freezing or desyncing."
                ).size(12),
                row![
                    button(text("Trigger Force Re-Sync").size(12))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::Sync),
                    Space::new().width(8),
                    muted("Verifies protobuf envelope encoding, updates timestamps, and synchronizes counters.").size(12),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        let latency_card = if let Some(lat) = &app.latency {
            container(
                column![
                    text("Pad Hardware Latency Telemetry")
                        .size(15)
                        .font(theme::FONT_BOLD),
                    row![
                        column![
                            caption("P50 LATENCY"),
                            text(format!("{:.1} µs", lat.p50_us))
                                .size(16)
                                .font(theme::FONT_BOLD),
                        ]
                        .width(Length::FillPortion(1)),
                        column![
                            caption("P99 LATENCY"),
                            text(format!("{:.1} µs", lat.p99_us))
                                .size(16)
                                .font(theme::FONT_BOLD),
                        ]
                        .width(Length::FillPortion(1)),
                        column![
                            caption("P99.9 LATENCY"),
                            text(format!("{:.1} µs", lat.p999_us))
                                .size(16)
                                .font(theme::FONT_BOLD),
                        ]
                        .width(Length::FillPortion(1)),
                        column![
                            caption("SAMPLES"),
                            text(lat.samples.to_string())
                                .size(16)
                                .font(theme::FONT_BOLD),
                        ]
                        .width(Length::FillPortion(1)),
                    ]
                    .spacing(12),
                    row![button(text("Reset Latency Benchmarks").size(12))
                        .padding([6, 12])
                        .style(theme::secondary)
                        .on_press(Message::ResetLatency),]
                ]
                .spacing(10),
            )
            .padding(16)
            .width(Length::Fill)
            .style(theme::card)
        } else {
            container(
                column![
                    text("Pad Hardware Latency Telemetry").size(15).font(theme::FONT_BOLD),
                    muted("No latency telemetry received from the pad yet. Play a map or tap switches while connected to collect hardware response times.").size(12),
                ]
                .spacing(8),
            )
            .padding(16)
            .width(Length::Fill)
            .style(theme::card)
        };
        column![com_status_card, com02_card, latency_card]
            .spacing(14)
            .into()
    }

    fn view_display_screen<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let brightness_btn = |val: u32, label: &'static str| {
            button(text(label).size(12))
                .padding([6, 12])
                .style(if app.brightness == val {
                    theme::primary
                } else {
                    theme::secondary
                })
                .on_press(Message::Diagnostics(DiagnosticsMessage::TestBrightness(
                    val,
                )))
        };

        let brightness_card = container(
            column![
                text("Backlight & PWM Hardware Test").size(15).font(theme::FONT_BOLD),
                muted("Adjust the screen backlight directly to verify the ESP32-S3 LEDC PWM backlight driver.").size(12),
                row![
                    muted(format!("Current: {}%", app.brightness)).width(140),
                    brightness_btn(10, "10% Dim"),
                    brightness_btn(25, "25% Low"),
                    brightness_btn(50, "50% Medium"),
                    brightness_btn(75, "75% Bright"),
                    brightness_btn(100, "100% Full"),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        let pattern_colors = [
            ("Layout Preview", None),
            ("osu! Pink", Some(theme::PINK)),
            ("Pure White", Some(theme::WHITE)),
            ("Pure Green", Some(theme::GREEN)),
            ("Pure Blue", Some(theme::CYAN)),
            ("Pure Red", Some(theme::RED)),
        ];

        let pattern_btns = row(pattern_colors.iter().enumerate().map(|(idx, (name, _))| {
            button(text(*name).size(12))
                .padding([6, 12])
                .style(if self.test_pattern_index == idx {
                    theme::primary
                } else {
                    theme::secondary
                })
                .on_press(Message::Diagnostics(
                    DiagnosticsMessage::SelectDisplayPattern(idx),
                ))
                .into()
        }))
        .spacing(8);

        let preview_box = if let Some(c) = pattern_colors[self.test_pattern_index].1 {
            container(
                column![
                    text(format!(
                        "Screen Color: {}",
                        pattern_colors[self.test_pattern_index].0
                    ))
                    .size(14)
                    .font(theme::FONT_BOLD)
                    .color(if c == theme::WHITE {
                        theme::BG
                    } else {
                        theme::WHITE
                    }),
                    text("Verify that pixels and ST7789 colors are displayed uniformly.")
                        .size(11)
                        .color(if c == theme::WHITE {
                            theme::BG
                        } else {
                            theme::MUTED
                        }),
                ]
                .align_x(Alignment::Center)
                .spacing(6),
            )
            .width(280)
            .height(180)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(c.into()),
                border: Border {
                    color: theme::BORDER,
                    width: 2.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
        } else {
            container(
                column![
                    text("OPad Screen Simulator")
                        .size(16)
                        .font(theme::FONT_BOLD)
                        .color(theme::PINK),
                    text("320x240 ST7789 IPS Display")
                        .size(12)
                        .color(theme::MUTED),
                    Space::new().height(12),
                    row![
                        container(text(format!("K1: {}", app.counters.lifetime_key1)).size(13))
                            .padding(8)
                            .style(theme::pink_card),
                        container(text(format!("K2: {}", app.counters.lifetime_key2)).size(13))
                            .padding(8)
                            .style(theme::outlined_card),
                    ]
                    .spacing(10),
                ]
                .align_x(Alignment::Center)
                .spacing(4),
            )
            .width(280)
            .height(180)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(theme::BG.into()),
                border: Border {
                    color: theme::BORDER,
                    width: 2.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            })
        };

        let screen_card = container(
            column![
                text("Display Colors & Uniformity Verification").size(15).font(theme::FONT_BOLD),
                muted("Select test colors to inspect LCD pixel response, backlight bleed, and color calibration.").size(12),
                pattern_btns,
                row![preview_box].align_y(Alignment::Center),
            ]
            .spacing(12),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        column![brightness_card, screen_card].spacing(14).into()
    }

    fn view_export_bundle<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let bundle_json = generate_diagnostic_bundle(app, self);

        let preview_snippet: String = bundle_json
            .lines()
            .take(22)
            .collect::<Vec<&str>>()
            .join("\n");

        let export_card = container(
            column![
                text("One-Click Diagnostic Report Exporter").size(16).font(theme::FONT_BOLD),
                muted(
                    "Generates a complete diagnostic report containing hardware specifications, firmware versions, \
                     counter reconciliation state, recent input tester metrics, and daemon logs. \
                     You can copy this directly to your clipboard to paste into Discord or GitHub when reporting issues."
                ).size(13),
                row![
                    button(text("📋 Copy Diagnostic Bundle to Clipboard").size(14).font(theme::FONT_BOLD))
                        .padding([10, 20])
                        .style(theme::primary)
                        .on_press(Message::CopyDiagnosticBundle),
                    Space::new().width(12),
                    button(text("💾 Save Diagnostic File (.json)...").size(14))
                        .padding([10, 20])
                        .style(theme::secondary)
                        .on_press(Message::SaveDiagnosticBundle),
                ]
                .align_y(Alignment::Center),
                Space::new().height(8),
                caption("DIAGNOSTIC BUNDLE PREVIEW"),
                container(
                    scrollable(
                        text(format!("{}\n...", preview_snippet))
                            .size(11)
                            .color(theme::MUTED)
                    )
                )
                .padding(12)
                .width(Length::Fill)
                .height(240)
                .style(theme::card),
            ]
            .spacing(12),
        )
        .padding(20)
        .width(Length::Fill)
        .style(theme::card);

        column![export_card].spacing(14).into()
    }
}

/// Generates a structured JSON diagnostic bundle from app and diagnostics state
pub fn generate_diagnostic_bundle(app: &App, diag: &DiagnosticsState) -> String {
    let now = chrono::Local::now().to_rfc3339();
    let info = app.device_info.as_ref();

    let logs: Vec<serde_json::Value> = app
        .logs
        .iter()
        .rev()
        .take(50)
        .rev()
        .map(|l| {
            serde_json::json!({
                "time": l.ts.to_rfc3339(),
                "level": format!("{:?}", l.level),
                "source": format!("{:?}", l.source),
                "target": l.target,
                "message": l.message,
            })
        })
        .collect();

    let bundle = serde_json::json!({
        "timestamp": now,
        "opad_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "daemon": {
            "online": app.daemon_online,
            "tosu_connected": app.tosu_connected,
            "mode": format!("{:?}", app.mode),
            "counters_source": format!("{:?}", app.counters_source),
            "last_sync_time": app.last_sync_time,
            "last_sync_error": app.last_sync_error,
            "storage_error": app.storage_error,
        },
        "device": {
            "connected": app.device_connected,
            "device_id": info.map(|i| i.device_id.clone()).unwrap_or_default(),
            "board_profile": info.map(|i| i.board_profile.clone()).unwrap_or_default(),
            "firmware_version": info.map(|i| i.firmware_version.clone()).unwrap_or_default(),
            "running_partition": info.and_then(|i| i.running_partition.clone()).or_else(|| {
                app.firmware_offer.as_ref().and_then(|o| o.running_partition.clone())
            }),
        },
        "config": {
            "key1": app.k1_input,
            "key2": app.k2_input,
            "key1_gpio": app.k1_gpio,
            "key2_gpio": app.k2_gpio,
            "debounce_us": app.debounce,
            "brightness": app.brightness,
            "sleep_seconds": app.sleep_seconds,
            "gameplay_display_hz": app.gameplay_display_hz,
            "autostart_tray": app.autostart_tray,
        },
        "counters": {
            "active_k1": app.counters.lifetime_key1,
            "active_k2": app.counters.lifetime_key2,
            "active_generation": app.counters.counter_generation,
            "pc_k1": app.pc_counters.as_ref().map(|c| c.lifetime_key1),
            "pc_k2": app.pc_counters.as_ref().map(|c| c.lifetime_key2),
            "pc_generation": app.pc_counters.as_ref().map(|c| c.counter_generation),
            "esp_k1": app.esp_counters.as_ref().map(|c| c.lifetime_key1),
            "esp_k2": app.esp_counters.as_ref().map(|c| c.lifetime_key2),
            "esp_generation": app.esp_counters.as_ref().map(|c| c.counter_generation),
        },
        "latency_stats": app.latency.as_ref().map(|l| serde_json::json!({
            "samples": l.samples,
            "p50_us": l.p50_us,
            "p99_us": l.p99_us,
            "p999_us": l.p999_us,
            "max_us": l.max_us,
            "deferred_reports": l.deferred_reports,
        })),
        "input_tester_session": {
            "key1": {
                "press_count": diag.k1.press_count,
                "definite_chatter_count": diag.k1.definite_chatter_count,
                "fast_flutter_count": diag.k1.fast_flutter_count,
                "shortest_repress_ms": diag.k1.shortest_repress_ms,
                "last_hold_ms": diag.k1.last_hold_ms,
            },
            "key2": {
                "press_count": diag.k2.press_count,
                "definite_chatter_count": diag.k2.definite_chatter_count,
                "fast_flutter_count": diag.k2.fast_flutter_count,
                "shortest_repress_ms": diag.k2.shortest_repress_ms,
                "last_hold_ms": diag.k2.last_hold_ms,
            },
            "simultaneous_presses": diag.simultaneous_press_count,
        },
        "recent_logs": logs,
    });

    serde_json::to_string_pretty(&bundle).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}
