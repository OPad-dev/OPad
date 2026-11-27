//! The Troubleshooting page: what to open when something is wrong. Always in
//! the sidebar, last.
//!
//! 1. Logs: what the service, this app, tosu and the pad wrote
//! 2. Key test: every press as the PC receives it (bounce, repeats, held
//!    time) and, on debug firmware, the pad's own record (he_monitor.rs)
//! 3. Board test: the pad checks its wiring (`opad_model::board_test` judges),
//!    the magnet test, and restarting the pad
//! 4. Connection: what is connected, measured (ping, key speed) and the Linux
//!    permission tosu needs
//! 5. Report: everything above in one file to send with a bug report

use crate::he_monitor::{HeFinding, HeMonitorEvent};
use crate::theme::{self, caption, heading, muted};
use crate::widgets::{line, scrolling, tab_bar};
use crate::{App, Message};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, Space};
use iced::{Alignment, Border, Element, Length};
use opad_model::board_test::{
    self, BoardCheck, BoardFix, BoardTestReport, CheckGroup, CheckStatus, MagnetKey, MagnetTest,
};
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

/// 40 ms of a 1760 Hz sine as a 16-bit mono WAV, the same tone as Windows' Beep
#[cfg(target_os = "linux")]
fn chatter_beep_wav() -> Vec<u8> {
    const RATE: u32 = 44_100;
    let samples: Vec<i16> = (0..RATE * 40 / 1000)
        .map(|n| {
            let t = n as f32 / RATE as f32;
            (0.2 * i16::MAX as f32 * (2.0 * std::f32::consts::PI * 1760.0 * t).sin()) as i16
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut wav = Vec::with_capacity(44 + data_len as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        wav.extend_from_slice(&s.to_le_bytes());
    }
    wav
}

/// Plays the beep through the desktop's own player (PipeWire, PulseAudio or
/// ALSA's aplay, whichever exists) rather than linking an audio library: a
/// linked libasound made opad-gui refuse to start on systems without it, the
/// AppImage included, for the sake of a diagnostic beep.
#[cfg(target_os = "linux")]
pub fn play_chatter_beep() {
    std::thread::spawn(|| {
        static WAV: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
        let wav = WAV.get_or_init(|| {
            let dir = std::env::var_os("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let path = dir.join("opad-chatter-beep.wav");
            std::fs::write(&path, chatter_beep_wav()).ok().map(|_| path)
        });
        if let Some(path) = wav {
            for player in ["pw-play", "paplay", "aplay"] {
                let mut cmd = std::process::Command::new(player);
                if player == "aplay" {
                    cmd.arg("-q");
                }
                let spawned = cmd
                    .arg(path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                if let Ok(mut child) = spawned {
                    let _ = child.wait();
                    return;
                }
            }
        }
        // No player: the terminal bell, if there is a terminal
        print!("\x07");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn play_chatter_beep() {}

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
            if lower.contains("opad") {
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
        // 0x4001: release-candidate firmware before it had OPad's own PID
        if id.vendor() == 0x303a && matches!(id.product(), 0x8415 | 0x4001) {
            return Some(dev);
        }
    }

    None
}

/// Every press and release of the two keys on the pad's input device, in
/// order and stamped with the kernel's own event time. Reading the key state
/// every few ms instead lost a 5 ms ghost tap and its 5 ms gap between two
/// reads, so the chatter check never saw it.
#[cfg(target_os = "linux")]
fn linux_key_events(
    k1_str: &str,
    k2_str: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<DiagnosticsMessage>,
) {
    use std::time::SystemTime;
    let codes = [char_to_evdev_key(k1_str), char_to_evdev_key(k2_str)];
    let mut dev: Option<evdev::Device> = None;
    let mut last_scan: Option<Instant> = None;
    let mut last: [Option<bool>; 2] = [None, None];
    let send = |i: usize, is_down: bool, at: Instant, last: &mut [Option<bool>; 2]| {
        if last[i] != Some(is_down) {
            last[i] = Some(is_down);
            let _ = tx.send(DiagnosticsMessage::KeyEvent {
                key_idx: i as u8 + 1,
                is_down,
                at,
            });
        }
    };
    while !tx.is_closed() {
        if dev.is_none() && !last_scan.is_some_and(|t| t.elapsed() < EVDEV_RESCAN_INTERVAL) {
            last_scan = Some(Instant::now());
            dev = open_opad_evdev().filter(|d| d.set_nonblocking(true).is_ok());
            // The first reading of each key is always sent
            if let Some(keys) = dev.as_ref().and_then(|d| d.get_key_state().ok()) {
                for (i, code) in codes.iter().enumerate() {
                    if let Some(code) = code {
                        send(i, keys.contains(*code), Instant::now(), &mut last);
                    }
                }
            }
        }
        let lost = dev.as_mut().is_some_and(|d| match d.fetch_events() {
            Ok(events) => {
                let (now, wall) = (Instant::now(), SystemTime::now());
                for ev in events {
                    let evdev::EventSummary::Key(_, code, value) = ev.destructure() else {
                        continue;
                    };
                    if value == 2 {
                        continue; // autorepeat
                    }
                    let age = wall.duration_since(ev.timestamp()).unwrap_or_default();
                    let at = now.checked_sub(age).unwrap_or(now);
                    for (i, c) in codes.iter().enumerate() {
                        if *c == Some(code) {
                            send(i, value == 1, at, &mut last);
                        }
                    }
                }
                false
            }
            Err(e) => e.kind() != std::io::ErrorKind::WouldBlock,
        });
        if lost {
            dev = None; // unplugged: look for it again
        }
        std::thread::sleep(SWITCH_POLL_INTERVAL);
    }
}

/// Whether each key is down, from the global async key state
#[cfg(windows)]
fn switch_states(k1_str: &str, k2_str: &str) -> [Option<bool>; 2] {
    [
        char_to_vk(k1_str).map(is_key_down),
        char_to_vk(k2_str).map(is_key_down),
    ]
}

/// How often the switch tester samples the keys (Windows), or collects the
/// pad's queued key events (Linux, each stamped with its own time)
#[cfg(any(windows, target_os = "linux"))]
const SWITCH_POLL_INTERVAL: Duration = Duration::from_millis(4);

/// Watches the two keys on a thread of its own and yields a `KeyEvent` only
/// when one changes, so the UI is not woken 250 times a second for nothing.
/// Linux reads every transition from the pad's input device
/// ([`linux_key_events`]); Windows samples the async key state every
/// [`SWITCH_POLL_INTERVAL`] and can miss a change shorter than that. The first
/// reading of each key is always sent. The thread stops once the subscription
/// is dropped.
#[cfg(any(windows, target_os = "linux"))]
pub fn switch_input_stream(
    keys: &(String, String),
) -> impl futures_util::Stream<Item = DiagnosticsMessage> {
    let (k1, k2) = keys.clone();
    iced::stream::channel(
        64,
        move |mut output: iced::futures::channel::mpsc::Sender<DiagnosticsMessage>| async move {
            use iced::futures::SinkExt;
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            #[cfg(target_os = "linux")]
            std::thread::spawn(move || linux_key_events(&k1, &k2, &tx));
            #[cfg(windows)]
            std::thread::spawn(move || {
                let mut last: [Option<bool>; 2] = [None, None];
                while !tx.is_closed() {
                    let now = switch_states(&k1, &k2);
                    for (i, state) in now.into_iter().enumerate() {
                        if let Some(is_down) = state {
                            if last[i] != Some(is_down) {
                                last[i] = Some(is_down);
                                let _ = tx.send(DiagnosticsMessage::KeyEvent {
                                    key_idx: i as u8 + 1,
                                    is_down,
                                    at: Instant::now(),
                                });
                            }
                        }
                    }
                    std::thread::sleep(SWITCH_POLL_INTERVAL);
                }
            });
            while let Some(event) = rx.recv().await {
                if output.send(event).await.is_err() {
                    break;
                }
            }
        },
    )
}

/// opad_tosu::ptrace_access() runs `getcap`, and view() runs every frame
fn cached_ptrace_access() -> opad_tosu::PtraceAccess {
    static CACHE: std::sync::Mutex<Option<(Instant, opad_tosu::PtraceAccess)>> =
        std::sync::Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    match cache.as_ref() {
        Some((at, access)) if at.elapsed() < Duration::from_secs(10) => access.clone(),
        _ => {
            let access = opad_tosu::ptrace_access();
            *cache = Some((Instant::now(), access.clone()));
            access
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
pub enum Tab {
    #[default]
    Logs,
    KeyTest,
    BoardTest,
    Connection,
    Report,
}

const TABS: [(Tab, &str); 5] = [
    (Tab::Logs, "Logs"),
    (Tab::KeyTest, "Key test"),
    (Tab::BoardTest, "Board test"),
    (Tab::Connection, "Connection"),
    (Tab::Report, "Report"),
];

#[derive(Debug, Clone)]
pub enum DiagnosticsMessage {
    SelectTab(Tab),
    ToggleSound(bool),
    ResetInputTester,
    /// A key changed state at `at`
    KeyEvent {
        key_idx: u8,
        is_down: bool,
        at: Instant,
    },
    PingDaemon,
    PingDone(Result<Duration, String>),
    RunBoardTest,
    BoardTestDone(Result<BoardTestReport, String>),
    /// Set Key 1 = GPIO10 and Key 2 = GPIO7, then test again
    UseCarrierKeyPins,
    /// Hall Effect V3: run the board test over and over while a magnet is
    /// moved over each key
    StartMagnetTest,
    StopMagnetTest,
    MagnetTick(Result<BoardTestReport, String>),
    RestartPad,
    RestartDone(Result<(), String>),
    /// From the HE debug log (he_monitor.rs)
    He(HeMonitorEvent),
}

/// What the HE monitor found for one key, this session
#[derive(Debug, Clone, Default)]
pub struct HeKeyCounts {
    pub presses: u32,
    pub phantoms: u32,
    pub short: u32,
    pub repress: u32,
    /// MX: taps the pad never sent, presses sent late, releases missed
    pub missed: u32,
    pub late: u32,
    pub stuck: u32,
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
    pub active_tab: Tab,
    pub sound_enabled: bool,
    pub k1: KeyTrackerState,
    pub k2: KeyTrackerState,
    /// Hall Effect V3's key 3
    pub k3: KeyTrackerState,
    pub simultaneous_press_count: u32,
    pub event_log: VecDeque<String>,
    pub ping_in_progress: bool,
    pub last_ping_ms: Option<f64>,
    pub board_test_running: bool,
    /// The last board test: the pad's report, or why there is none
    pub board_test: Option<Result<BoardTestReport, String>>,
    /// The magnet test: what it has seen, whether it is still asking the pad,
    /// the newest report and the last error
    pub magnet: Option<MagnetTest>,
    pub magnet_running: bool,
    pub magnet_last: Option<BoardTestReport>,
    pub magnet_error: Option<String>,
    pub restart_running: bool,
    /// The last restart request: asked, or why not
    pub restart: Option<Result<(), String>>,
    pub he: [HeKeyCounts; 2],
    /// When the HE debug log last showed the pad's once-a-second line
    pub he_alive: Option<Instant>,
    /// The log is the MX module's (he.mx* lines)
    pub he_mx: bool,
    /// MX: times the module check turned the keys off, this session
    pub keys_off: u32,
}

impl Default for DiagnosticsState {
    fn default() -> Self {
        Self {
            active_tab: Tab::default(),
            sound_enabled: true,
            k1: KeyTrackerState::default(),
            k2: KeyTrackerState::default(),
            k3: KeyTrackerState::default(),
            simultaneous_press_count: 0,
            event_log: VecDeque::with_capacity(30),
            ping_in_progress: false,
            last_ping_ms: None,
            board_test_running: false,
            board_test: None,
            magnet: None,
            magnet_running: false,
            magnet_last: None,
            magnet_error: None,
            restart_running: false,
            restart: None,
            he: Default::default(),
            he_alive: None,
            he_mx: false,
            keys_off: 0,
        }
    }
}

impl DiagnosticsState {
    pub fn reset_input_tester(&mut self) {
        self.k1 = KeyTrackerState::default();
        self.k2 = KeyTrackerState::default();
        self.k3 = KeyTrackerState::default();
        self.simultaneous_press_count = 0;
        self.he = Default::default();
        self.keys_off = 0;
        self.event_log.clear();
        self.event_log
            .push_back("Tester reset. Ready to monitor switch inputs.".to_string());
    }

    pub fn handle_he_event(&mut self, event: HeMonitorEvent) {
        match event {
            HeMonitorEvent::Alive { mx } => {
                self.he_alive = Some(Instant::now());
                self.he_mx = mx;
            }
            HeMonitorEvent::KeysOff { time, mv } => {
                self.keys_off += 1;
                if self.sound_enabled {
                    play_chatter_beep();
                }
                self.push_event(format!(
                    "🔴 KEYS OFF: the module check read the ID line at {mv} mV at {time}; both keys dead until it reads MX again"
                ));
            }
            HeMonitorEvent::KeysOn { time, off_ms } => {
                self.push_event(format!("🟡 KEYS ON at {time}, after {off_ms} ms off"));
            }
            HeMonitorEvent::Press { key } => {
                self.he[(key as usize - 1).min(1)].presses += 1;
            }
            HeMonitorEvent::Found { key, time, finding } => {
                let counts = &mut self.he[(key as usize - 1).min(1)];
                let entry = match finding {
                    HeFinding::Phantom { jump, frames } => {
                        counts.phantoms += 1;
                        if self.sound_enabled {
                            play_chatter_beep();
                        }
                        format!("🔴 PHANTOM: Key {key} jumped +{jump} in one frame at {time} ({frames})")
                    }
                    HeFinding::Short { ms } if self.he_mx => {
                        counts.short += 1;
                        if self.sound_enabled {
                            play_chatter_beep();
                        }
                        format!("🔴 GHOST TAP: Key {key} sent a {ms} ms press at {time} (switch bounce)")
                    }
                    HeFinding::Short { ms } => {
                        counts.short += 1;
                        format!("🔴 PHANTOM: Key {key} held only {ms} ms at {time}")
                    }
                    HeFinding::Repress { ms } => {
                        counts.repress += 1;
                        format!("🟡 DOUBLE PRESS: Key {key} pressed again {ms} ms after its release at {time}")
                    }
                    HeFinding::Missed { ms } => {
                        counts.missed += 1;
                        if self.sound_enabled {
                            play_chatter_beep();
                        }
                        format!("🔴 MISSED: Key {key} was down {ms} ms at {time} and the pad sent no press")
                    }
                    HeFinding::Late { ms } => {
                        counts.late += 1;
                        format!("🟡 LATE: Key {key} press sent {ms} ms after the switch closed at {time}")
                    }
                    HeFinding::Stuck { ms } => {
                        counts.stuck += 1;
                        format!("🔴 STUCK: Key {key} reported down {ms} ms after the switch opened at {time}")
                    }
                };
                self.push_event(entry);
            }
        }
    }

    fn push_event(&mut self, entry: String) {
        if self.event_log.len() >= 25 {
            self.event_log.pop_front();
        }
        self.event_log.push_back(entry);
    }

    pub fn handle_key_event(&mut self, key_idx: u8, is_down: bool) {
        self.handle_key_event_at(key_idx, is_down, Instant::now());
    }

    /// Edge-triggered: a repeat of the key's current state changes nothing
    pub fn handle_key_event_at(&mut self, key_idx: u8, is_down: bool, now: Instant) {
        let key_name = match key_idx {
            1 => "Key 1",
            3 => "Key 3",
            _ => "Key 2",
        };

        // The key itself, and the press times of the others that are down
        let (target, others) = match key_idx {
            1 => (&mut self.k1, [&self.k2, &self.k3]),
            3 => (&mut self.k3, [&self.k1, &self.k2]),
            _ => (&mut self.k2, [&self.k1, &self.k3]),
        };
        let other_presses: Vec<Instant> = others
            .iter()
            .filter(|o| o.is_down)
            .filter_map(|o| o.last_press_time)
            .collect();

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

            // Check simultaneous press with another key (within 10ms)
            if other_presses
                .iter()
                .any(|&p| (now - p).as_secs_f64() * 1000.0 <= 10.0)
            {
                self.simultaneous_press_count += 1;
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
        let content: Element<'_, Message> = match self.active_tab {
            Tab::Logs => crate::pages::logs(app),
            Tab::KeyTest => self.view_input_tester(app),
            Tab::BoardTest => self.view_board_test(app),
            Tab::Connection => self.view_connection(app),
            Tab::Report => self.view_export_bundle(app),
        };

        scrolling(
            column![
                heading("Troubleshooting"),
                muted("For when something is not working: what happened, tests, and a report to send.")
                    .size(14),
                tab_bar(&TABS, self.active_tab, |tab| {
                    Message::Diagnostics(DiagnosticsMessage::SelectTab(tab))
                }),
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

        let key_card =
            |label: &'a str, k: &'a KeyTrackerState, pin: u32, key: u8| -> Element<'a, Message> {
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
                                text(format!("Key {key}")).size(16).font(theme::FONT_BOLD),
                                Space::new().width(Length::Fill),
                                muted(if pin == 0 {
                                    String::new()
                                } else {
                                    format!("GPIO Pin: {}", pin)
                                })
                                .size(12),
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

        let card_k1 = key_card(k1_label, &self.k1, app.k1_gpio, 1);
        let card_k2 = key_card(k2_label, &self.k2, app.k2_gpio, 2);
        let three = app.device_info.as_ref().is_some_and(|i| i.key_count >= 3);
        let mut cards = row![card_k1, card_k2].spacing(14);
        if three {
            let k3_label = if app.k3_input.is_empty() {
                "—"
            } else {
                &app.k3_input
            };
            // Key 3 is on the module's ADC, not a GPIO of the pad's: no pin shown
            cards = cards.push(key_card(k3_label, &self.k3, 0, 3));
        }

        let total_presses = self.k1.press_count + self.k2.press_count + self.k3.press_count;
        let total_chatter = self.k1.definite_chatter_count
            + self.k2.definite_chatter_count
            + self.k3.definite_chatter_count;
        let total_flutter =
            self.k1.fast_flutter_count + self.k2.fast_flutter_count + self.k3.fast_flutter_count;

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
                let color = if line.contains("CHATTER") || line.contains("PHANTOM") {
                    theme::RED
                } else if line.contains("FLUTTER") || line.contains("DOUBLE PRESS") {
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

        column![
            cards,
            summary_card,
            self.view_he_monitor(app.device_info.as_ref().is_some_and(|i| i.he_calibration)),
            event_card
        ]
        .spacing(14)
        .into()
    }

    /// The pad's own judgement of each HE press, from the HE debug log
    /// Nothing at all unless the pad runs an HE module (`he_active`) or the
    /// debug log is live: an MX pad shows no HE card
    fn view_he_monitor(&self, he_active: bool) -> Element<'_, Message> {
        let live = self
            .he_alive
            .is_some_and(|at| at.elapsed() < Duration::from_secs(3));
        if live && self.he_mx {
            return self.view_mx_monitor();
        }
        if !he_active && !live {
            return Space::new().height(0).into();
        }
        let status = if live {
            text("● Following the pad's HE debug log")
                .size(12)
                .color(theme::GREEN)
        } else {
            text(
                "○ No HE debug data: needs the HE debug firmware (+hedebug) and the daemon running",
            )
            .size(12)
            .color(theme::MUTED)
        };
        let count = |label: &'static str, v: u32, bad: iced::Color| {
            column![
                caption(label),
                text(v.to_string())
                    .size(18)
                    .font(theme::FONT_BOLD)
                    .color(if v > 0 { bad } else { theme::GREEN }),
            ]
            .width(Length::FillPortion(1))
        };
        let key_row = |i: usize| {
            let c = &self.he[i];
            row![
                column![
                    text(format!("Key {}", i + 1))
                        .size(14)
                        .font(theme::FONT_BOLD),
                    muted(format!("{} presses checked", c.presses)).size(12),
                ]
                .width(Length::FillPortion(1)),
                count("PHANTOM (JUMP)", c.phantoms, theme::RED),
                count("PHANTOM (< 8 ms)", c.short, theme::RED),
                count("DOUBLE PRESS (< 25 ms)", c.repress, theme::YELLOW),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
        };
        container(
            column![
                row![
                    text("HE Phantom & Double-Press Monitor")
                        .size(15)
                        .font(theme::FONT_BOLD),
                    Space::new().width(Length::Fill),
                    status,
                ]
                .align_y(Alignment::Center),
                muted(
                    "Judged on the pad from its own readings, so it catches presses too short for the \
                     tester above. A phantom is a press made by a one-frame jump from a steady reading; a \
                     double press is a key pressed again within 25 ms of its release. Works during maps."
                )
                .size(12),
                key_row(0),
                key_row(1),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card)
        .into()
    }

    /// The MX half of the same log: taps the pad dropped, from its own pin sampler
    fn view_mx_monitor(&self) -> Element<'_, Message> {
        let count = |label: &'static str, v: u32, bad: iced::Color| {
            column![
                caption(label),
                text(v.to_string())
                    .size(18)
                    .font(theme::FONT_BOLD)
                    .color(if v > 0 { bad } else { theme::GREEN }),
            ]
            .width(Length::FillPortion(1))
        };
        let key_row = |i: usize| {
            let c = &self.he[i];
            row![
                column![
                    text(format!("Key {}", i + 1))
                        .size(14)
                        .font(theme::FONT_BOLD),
                    muted(format!("{} taps sent", c.presses)).size(12),
                ]
                .width(Length::FillPortion(1)),
                count("MISSED (NOT SENT)", c.missed, theme::RED),
                count("STUCK (RELEASE LATE)", c.stuck, theme::RED),
                count("LATE (> 3 ms)", c.late, theme::YELLOW),
                count("GHOST TAP (< 8 ms)", c.short, theme::RED),
            ]
            .spacing(14)
            .align_y(Alignment::Center)
        };
        container(
            column![
                row![
                    text("MX Missed-Tap Monitor").size(15).font(theme::FONT_BOLD),
                    Space::new().width(Length::Fill),
                    text("● Following the pad's MX debug log")
                        .size(12)
                        .color(theme::GREEN),
                ]
                .align_y(Alignment::Center),
                muted(
                    "The pad reads its switch pins every millisecond and compares them with what it sent. \
                     Missed: a switch closed for 3 ms or more and no press went out. Keys off: the module \
                     check lost the MX module and turned both keys off. Works during maps."
                )
                .size(12),
                row![
                    caption("KEYS OFF (MODULE CHECK)"),
                    text(self.keys_off.to_string())
                        .size(18)
                        .font(theme::FONT_BOLD)
                        .color(if self.keys_off > 0 { theme::RED } else { theme::GREEN }),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                key_row(0),
                key_row(1),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card)
        .into()
    }

    /// What is connected and what was measured. Nothing here is a fixed
    /// claim: every line is the state now or a number taken on this machine.
    fn view_connection<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let info = app.device_info.as_ref();
        let known = |v: Option<&str>| v.filter(|s| !s.is_empty()).unwrap_or("-").to_string();

        let ping_text = if self.ping_in_progress {
            "Measuring…".to_string()
        } else if let Some(ms) = self.last_ping_ms {
            format!("{ms:.2} ms there and back")
        } else {
            "Not measured yet".to_string()
        };

        let status_card = container(
            column![
                text("Connection").size(16).font(theme::FONT_BOLD),
                line(
                    "OPad service",
                    if app.daemon_online {
                        "Running"
                    } else {
                        "Not running"
                    }
                ),
                line(
                    "Pad",
                    if app.device_connected {
                        "Connected"
                    } else if app.incompatible.is_some() {
                        "Connected, needs an update"
                    } else {
                        "Not connected"
                    }
                ),
                line(
                    "osu! (tosu)",
                    if app.tosu_connected {
                        "Connected"
                    } else {
                        "Not connected"
                    }
                ),
                line("Pad ID", known(info.map(|i| i.device_id.as_str()))),
                line("Board", known(info.map(|i| i.board_profile.as_str()))),
                line("Firmware", known(info.map(|i| i.firmware_version.as_str()))),
                row![
                    muted("App to service").size(14).width(180),
                    text(ping_text).size(14),
                    Space::new().width(12),
                    button(text("Measure").size(13))
                        .padding([4, 12])
                        .style(theme::secondary)
                        .on_press_maybe(
                            (!self.ping_in_progress)
                                .then_some(Message::Diagnostics(DiagnosticsMessage::PingDaemon))
                        ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        // Linux: tosu can only read osu! if ptrace allows it
        let yama_card: Option<Element<'a, Message>> = match cached_ptrace_access() {
            opad_tosu::PtraceAccess::Blocked { scope, fix }
            | opad_tosu::PtraceAccess::Unknown { scope, fix } => Some(
                container(
                    column![
                        text("tosu may not be allowed to read osu!")
                            .size(15)
                            .font(theme::FONT_BOLD)
                            .color(theme::RED),
                        text(format!(
                            "This Linux only lets a program read its own children's memory \
                             (ptrace_scope is {scope}), and osu! is not tosu's child: the pad \
                             would get no map data."
                        ))
                        .size(13),
                        text(format!("To fix it: {fix}"))
                            .size(13)
                            .font(theme::FONT_BOLD)
                            .color(theme::YELLOW),
                    ]
                    .spacing(8),
                )
                .padding(14)
                .width(Length::Fill)
                .style(|t| {
                    let mut s = theme::card(t);
                    s.border = Border {
                        color: theme::RED,
                        width: 1.0,
                        radius: 8.0.into(),
                    };
                    s
                })
                .into(),
            ),
            _ => None,
        };

        let mut cards: Vec<Element<'a, Message>> = Vec::new();
        if let Some(y) = yama_card {
            cards.push(y);
        }
        cards.push(status_card.into());
        column(cards).spacing(14).into()
    }

    fn view_board_test<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let can_run = app.device_connected && !self.board_test_running && !self.magnet_running;
        let run_label = if self.board_test_running {
            "Testing…"
        } else if self.board_test.is_some() {
            "Run again"
        } else {
            "Run the board test"
        };
        let run_btn = button(text(run_label).size(13))
            .padding([8, 16])
            .style(theme::primary)
            .on_press_maybe(
                can_run.then_some(Message::Diagnostics(DiagnosticsMessage::RunBoardTest)),
            );

        let intro = container(
            column![
                text("Board test").size(16).font(theme::FONT_BOLD),
                muted(
                    "The pad checks its own wiring: which module answers on the carrier's \
                     connector and which way round its cable is, whether each key line reaches \
                     the module, and whether the spare connector pins are shorted. On a Hall \
                     Effect V3 module it also tests the ADC and its SPI link (answer, register \
                     writes, signal margin, supply) and each sensor's rest level and noise, then \
                     offers a magnet test. Release every key while it runs. Not available during \
                     a map."
                )
                .size(12),
                row![
                    run_btn,
                    Space::new().width(12),
                    if app.device_connected {
                        muted("Takes a fraction of a second.").size(12)
                    } else {
                        muted("Connect the pad first.").size(12)
                    },
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        let mut page = column![intro].spacing(16);
        match &self.board_test {
            None => {}
            Some(Err(e)) => {
                page = page.push(
                    container(text(e.clone()).size(13).color(theme::RED))
                        .padding(16)
                        .width(Length::Fill)
                        .style(theme::card),
                )
            }
            Some(Ok(report)) => {
                page = page.push(board_test_results(report, self.magnet.as_ref()));
                if let Some(m) = board_test_measurements(report) {
                    page = page.push(m);
                }
                if has_sensors(report) {
                    page = page.push(self.view_magnet_test(app));
                }
            }
        }

        let he_v3 = matches!(&self.board_test, Some(Ok(r)) if r.he_adc.is_some());
        let press_hint = container(
            column![
                text("Then press each key").size(15).font(theme::FONT_BOLD),
                muted(if he_v3 {
                    "The magnet test proves each sensor sees its key's magnet. Keys on a Hall \
                     Effect V3 module need firmware that reads them; until then the pad sends \
                     nothing for them."
                } else {
                    "The test proves the lines are connected, not that the switches work. Press \
                     each key on the Key test tab: Key 1 must light the left card and Key 2 \
                     the right one, each on its own."
                })
                .size(12),
            ]
            .spacing(6),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card);

        page.push(press_hint).push(self.view_restart(app)).into()
    }

    fn view_magnet_test<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let button_row: Element<'a, Message> = if self.magnet_running {
            button(text("Stop").size(13))
                .padding([8, 16])
                .style(theme::secondary)
                .on_press(Message::Diagnostics(DiagnosticsMessage::StopMagnetTest))
                .into()
        } else {
            let label = if self.magnet.is_some() {
                "Start Again"
            } else {
                "Start Magnet Test"
            };
            button(text(label).size(13))
                .padding([8, 16])
                .style(theme::primary)
                .on_press_maybe(
                    (app.device_connected && !self.board_test_running)
                        .then_some(Message::Diagnostics(DiagnosticsMessage::StartMagnetTest)),
                )
                .into()
        };

        let mut body = column![
            text("Magnet test").size(15).font(theme::FONT_BOLD),
            muted(format!(
                "Hold a magnet (a key's own, or any small one) over key 1, then key 2, then key \
                 3, a few millimetres above the sensor. Each sensor must move at least {:.0} \
                 codes (about 0.2 V) while the others stay put. It stops by itself once all \
                 three have seen it.",
                board_test::MAGNET_MOVE_CODES
            ))
            .size(12),
            button_row,
        ]
        .spacing(10);

        if let (Some(test), Some(report)) = (&self.magnet, &self.magnet_last) {
            if let Some(adc) = &report.he_adc {
                let base = test.baseline().unwrap_or(&[]);
                for (i, c) in adc.channels.iter().enumerate().take(3) {
                    let start = base.get(i).copied().unwrap_or(c.mean());
                    body = body.push(sensor_bar(i, c.mean(), start, test.key(i)));
                }
            }
            if test.done() {
                body = body.push(text("All three sensors see the magnet.").size(12).color(
                    if test.checks().iter().all(|c| c.status == CheckStatus::Pass) {
                        theme::GREEN
                    } else {
                        theme::YELLOW
                    },
                ));
            }
        }
        if let Some(e) = &self.magnet_error {
            body = body.push(text(e.clone()).size(12).color(theme::RED));
        }

        container(body)
            .padding(16)
            .width(Length::Fill)
            .style(theme::card)
            .into()
    }

    fn view_restart<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let can_run = app.device_connected && !self.restart_running;
        let restart_btn = button(text("Restart pad").size(13))
            .padding([8, 16])
            .style(theme::secondary)
            .on_press_maybe(
                can_run.then_some(Message::Diagnostics(DiagnosticsMessage::RestartPad)),
            );
        let status: Element<'a, Message> = match &self.restart {
            _ if !app.device_connected => muted("Connect the pad first.").size(12).into(),
            None => Space::new().width(0).into(),
            Some(Ok(())) => muted("Restarting; it reconnects in a few seconds.")
                .size(12)
                .into(),
            Some(Err(e)) => text(e.clone()).size(12).color(theme::RED).into(),
        };
        container(
            column![
                text("Restart the pad").size(15).font(theme::FONT_BOLD),
                muted(
                    "The pad picks its key input (MX or Hall Effect) when it starts. After \
                     swapping the input module, restart it here or replug its USB cable. It \
                     saves its counters first, and waits until no map is being played."
                )
                .size(12),
                row![restart_btn, Space::new().width(12), status].align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(16)
        .width(Length::Fill)
        .style(theme::card)
        .into()
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
                text("Report").size(16).font(theme::FONT_BOLD),
                muted(
                    "One file with the pad's version and board, its counters, the key test's \
                     numbers and the latest log lines. Send it with a bug report."
                )
                .size(14),
                row![
                    button(text("Save report…").size(14).font(theme::FONT_BOLD))
                        .padding([10, 20])
                        .style(theme::primary)
                        .on_press(Message::SaveDiagnosticBundle),
                    Space::new().width(12),
                    button(text("Copy").size(14))
                        .padding([10, 20])
                        .style(theme::secondary)
                        .on_press(Message::CopyDiagnosticBundle),
                ]
                .align_y(Alignment::Center),
                Space::new().height(8),
                caption("WHAT IS IN IT"),
                container(scrollable(
                    text(format!("{}\n...", preview_snippet))
                        .size(11)
                        .color(theme::MUTED)
                ))
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

fn status_color(s: CheckStatus) -> iced::Color {
    match s {
        CheckStatus::Pass => theme::GREEN,
        CheckStatus::Warn => theme::YELLOW,
        CheckStatus::Fail => theme::RED,
        CheckStatus::Skip => theme::MUTED,
    }
}

fn has_sensors(report: &BoardTestReport) -> bool {
    report
        .he_adc
        .as_ref()
        .is_some_and(|a| a.ran && !a.channels.is_empty())
}

/// Every check, the magnet test's too, under one heading per part of the board
fn board_test_results<'a>(
    report: &'a BoardTestReport,
    magnet: Option<&MagnetTest>,
) -> Element<'a, Message> {
    let mut checks = board_test::evaluate(report);
    if let Some(m) = magnet.filter(|_| has_sensors(report)) {
        checks.extend(m.checks());
    }
    let overall = board_test::overall(&checks);

    let mut rows = column![row![
        text("Result").size(15).font(theme::FONT_BOLD),
        Space::new().width(Length::Fill),
        text(overall.label().to_uppercase())
            .size(13)
            .font(theme::FONT_BOLD)
            .color(status_color(overall)),
    ]
    .align_y(Alignment::Center)]
    .spacing(12);

    let mut groups: Vec<CheckGroup> = checks.iter().map(|c| c.group).collect();
    groups.sort();
    groups.dedup();
    for group in groups {
        let in_group: Vec<&BoardCheck> = checks.iter().filter(|c| c.group == group).collect();
        let worst = in_group
            .iter()
            .map(|c| c.status)
            .max()
            .unwrap_or(CheckStatus::Skip);
        let passed = in_group
            .iter()
            .filter(|c| c.status == CheckStatus::Pass)
            .count();
        rows = rows.push(
            row![
                caption(group.label().to_uppercase()),
                Space::new().width(Length::Fill),
                text(format!("{passed}/{} passed", in_group.len()))
                    .size(11)
                    .color(status_color(worst)),
            ]
            .align_y(Alignment::Center),
        );
        for c in in_group {
            let mut detail = column![
                text(c.name).size(13).font(theme::FONT_BOLD),
                muted(c.detail.clone()).size(12),
            ]
            .spacing(2)
            .width(Length::Fill);
            if c.fix == Some(BoardFix::UseCarrierKeyPins) {
                detail = detail.push(
                    button(text("Set Key 1 = GPIO10, Key 2 = GPIO7").size(12))
                        .padding([4, 10])
                        .style(theme::secondary)
                        .on_press(Message::Diagnostics(DiagnosticsMessage::UseCarrierKeyPins)),
                );
            }
            rows = rows.push(
                row![
                    text(c.status.label().to_uppercase())
                        .size(12)
                        .font(theme::FONT_BOLD)
                        .color(status_color(c.status))
                        .width(48),
                    detail,
                ]
                .spacing(12),
            );
        }
    }

    if report.ran {
        let mv = |v: Option<u32>| v.map_or("—".to_string(), |v| format!("{v} mV"));
        rows = rows.push(
            caption(format!(
                "Measured: {} (at boot: {}) · ID {} / {} loaded · reverse probe {} · keys GPIO{} / GPIO{}{}",
                report.module.label(),
                report.boot_module.label(),
                mv(report.id_mv),
                mv(report.id_loaded_mv),
                mv(report.reverse_probe_mv),
                report.key1_gpio,
                report.key2_gpio,
                if report.cable_reversed { " · cable turned over" } else { "" }
            ))
            .size(11),
        );
    }

    container(rows)
        .padding(16)
        .width(Length::Fill)
        .style(theme::card)
        .into()
}

/// The raw numbers behind the Hall Effect V3 checks: the SPI sweep and each sensor
fn board_test_measurements(report: &BoardTestReport) -> Option<Element<'_, Message>> {
    let adc = report.he_adc.as_ref().filter(|a| a.ran)?;
    let hex = |v: Option<u8>| v.map_or("—".to_string(), |v| format!("0x{v:02X}"));
    let mut body = column![
        text("Measurements").size(15).font(theme::FONT_BOLD),
        muted(format!(
            "ADC status {} after reset, {} after clearing, {} at the end · {} of {} register \
             writes wrong · offset calibration {}",
            hex(adc.status_after_reset),
            hex(adc.status_after_clear),
            hex(adc.status_end),
            adc.walk_mismatches,
            adc.walk_writes,
            if adc.cal_done { "done" } else { "not done" }
        ))
        .size(12),
        caption("SPI SIGNAL MARGIN (FIXED PATTERN 0xA5A)"),
    ]
    .spacing(8);

    let mut sweep = row![].spacing(8);
    for sp in &adc.speeds {
        let (label, color) = if sp.frames == 0 {
            ("not tried".to_string(), theme::MUTED)
        } else if sp.errors == 0 {
            (format!("0 / {}", sp.frames), theme::GREEN)
        } else {
            (format!("{} / {}", sp.errors, sp.frames), theme::RED)
        };
        sweep = sweep.push(
            container(
                column![
                    text(format!("{:.0} MHz", sp.sclk_hz as f64 / 1e6))
                        .size(13)
                        .font(theme::FONT_BOLD),
                    text(label).size(11).color(color),
                ]
                .spacing(2)
                .align_x(Alignment::Center),
            )
            .padding([6, 10])
            .width(Length::Fill)
            .style(theme::outlined_card),
        );
    }
    body = body.push(sweep).push(caption("SENSORS AT REST"));

    for (i, c) in adc.channels.iter().enumerate() {
        body = body.push(
            row![
                text(format!("Key {}", i + 1))
                    .size(13)
                    .font(theme::FONT_BOLD)
                    .width(60),
                muted(format!("AIN{}", c.ain)).size(12).width(50),
                text(format!("{:.3} V", c.volts())).size(13).width(80),
                muted(format!("{:.0} codes", c.mean())).size(12).width(90),
                muted(format!("noise {:.2} rms", c.noise()))
                    .size(12)
                    .width(120),
                muted(format!(
                    "{}–{} ({} p-p)",
                    c.min,
                    c.max,
                    c.max.saturating_sub(c.min)
                ))
                .size(12),
            ]
            .align_y(Alignment::Center),
        );
    }

    Some(
        container(body)
            .padding(16)
            .width(Length::Fill)
            .style(theme::card)
            .into(),
    )
}

/// One sensor in the magnet test: where it reads now across the ADC's range,
/// how far it moved from rest, and whether it passed
fn sensor_bar<'a>(i: usize, now: f64, start: f64, state: MagnetKey) -> Element<'a, Message> {
    let full = board_test::HE_V3_FULL_SCALE;
    let filled = now.clamp(1.0, full - 1.0) as u16;
    let (mark, color) = match state {
        MagnetKey::Waiting { .. } => ("waiting", theme::MUTED),
        MagnetKey::Seen { .. } => ("seen", theme::GREEN),
        MagnetKey::Crosstalk { .. } => ("crosstalk", theme::YELLOW),
    };
    let bar_color = match state {
        MagnetKey::Waiting { .. } => theme::CYAN,
        _ => color,
    };
    let track = container(
        row![
            container(Space::new().height(10))
                .width(Length::FillPortion(filled))
                .style(move |_| container::Style {
                    background: Some(bar_color.into()),
                    border: Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            Space::new().width(Length::FillPortion(full as u16 - filled)),
        ]
        .width(Length::Fill),
    )
    .padding(2)
    .width(Length::Fill)
    .style(theme::outlined_card);
    row![
        text(format!("Key {}", i + 1))
            .size(13)
            .font(theme::FONT_BOLD)
            .width(60),
        track,
        text(format!("{:.3} V", now * board_test::HE_V3_AVDD_V / full))
            .size(12)
            .width(70),
        muted(format!("{:+.0}", now - start)).size(12).width(60),
        text(mark).size(12).color(color).width(80),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

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
        "board_test": diag.board_test.as_ref().map(|r| match r {
            Ok(report) => serde_json::json!({
                "report": report,
                "checks": board_test::evaluate(report),
            }),
            Err(e) => serde_json::json!({ "error": e }),
        }),
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
            "k3": app.key3,
            "game_keys": app.config.game_keys,
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
            "key3": {
                "press_count": diag.k3.press_count,
                "definite_chatter_count": diag.k3.definite_chatter_count,
                "fast_flutter_count": diag.k3.fast_flutter_count,
                "shortest_repress_ms": diag.k3.shortest_repress_ms,
                "last_hold_ms": diag.k3.last_hold_ms,
            },
            "simultaneous_presses": diag.simultaneous_press_count,
        },
        "recent_logs": logs,
    });

    serde_json::to_string_pretty(&bundle).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
}

#[cfg(all(test, target_os = "linux"))]
mod beep_tests {
    #[test]
    fn the_chatter_beep_is_a_well_formed_40ms_wav() {
        let wav = super::chatter_beep_wav();
        let u32_at = |i: usize| u32::from_le_bytes(wav[i..i + 4].try_into().unwrap());
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(4) as usize, wav.len() - 8);
        assert_eq!(u32_at(24), 44_100);
        assert_eq!(&wav[36..40], b"data");
        // 40 ms of 16-bit mono at 44.1 kHz
        assert_eq!(u32_at(40), 1764 * 2);
        assert_eq!(wav.len(), 44 + 1764 * 2);
    }
}
