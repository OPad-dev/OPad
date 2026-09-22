use futures_util::StreamExt;
use opad_model::paths;
use opad_model::ui_source::{self as src, SourceValue};
use opad_model::GameplayTelemetry;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, watch};
use tokio_tungstenite::connect_async;
use tracing::{debug, info, warn};

/// tosu's v2 WebSocket; the legacy `/ws` endpoint uses the gosumemory schema instead
pub const DEFAULT_TOSU_ENDPOINT: &str = "ws://127.0.0.1:24050/websocket/v2";

#[derive(Debug, Error)]
pub enum TosuError {
    #[error("WebSocket connection error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct TosuManager {
    endpoint: String,
    telemetry_tx: broadcast::Sender<GameplayTelemetry>,
    connected_tx: watch::Sender<bool>,
}

impl TosuManager {
    pub fn new(endpoint: String) -> (Self, broadcast::Receiver<GameplayTelemetry>) {
        let (tx, rx) = broadcast::channel(32);
        let (connected_tx, _) = watch::channel(false);
        (
            Self {
                endpoint: normalize_endpoint(&endpoint),
                telemetry_tx: tx,
                connected_tx,
            },
            rx,
        )
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GameplayTelemetry> {
        self.telemetry_tx.subscribe()
    }

    /// Whether the tosu WebSocket is currently connected
    pub fn subscribe_connected(&self) -> watch::Receiver<bool> {
        self.connected_tx.subscribe()
    }

    /// Spawns the background tosu worker loop with auto-reconnect
    pub fn start(self) {
        tokio::spawn(async move {
            loop {
                debug!("Connecting to tosu WebSocket at {}", self.endpoint);
                match connect_async(&self.endpoint).await {
                    Ok((mut stream, _)) => {
                        info!("Connected to tosu WebSocket at {}", self.endpoint);
                        self.connected_tx.send_replace(true);
                        while let Some(msg_result) = stream.next().await {
                            match msg_result {
                                Ok(msg) => {
                                    if msg.is_text() {
                                        if let Ok(text) = msg.to_text() {
                                            if let Some(telemetry) = parse_tosu_v2_json(text) {
                                                let _ = self.telemetry_tx.send(telemetry);
                                            }
                                        }
                                    } else if msg.is_close() {
                                        warn!("tosu WebSocket received close frame");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Error reading tosu WebSocket frame: {}", e);
                                    break;
                                }
                            }
                        }
                        info!("tosu WebSocket disconnected");
                        self.connected_tx.send_replace(false);
                    }
                    Err(e) => {
                        debug!(
                            "tosu WebSocket connection failed (tosu not running?): {}",
                            e
                        );
                    }
                }

                // Wait before reconnecting
                tokio::time::sleep(Duration::from_millis(2500)).await;
            }
        });
    }
}

/// Map stored legacy endpoints (`/ws`, gosumemory schema) onto the v2 endpoint this parser reads
fn normalize_endpoint(endpoint: &str) -> String {
    match endpoint.strip_suffix("/ws") {
        Some(base) => format!("{}/websocket/v2", base),
        None => endpoint.to_string(),
    }
}

/// Parses a tosu v2 WebSocket payload defensively (§15)
pub fn parse_tosu_v2_json(json_str: &str) -> Option<GameplayTelemetry> {
    let root: serde_json::Value = serde_json::from_str(json_str).ok()?;
    // Reject payloads that are not v2 state frames
    let state_num = root.pointer("/state/number")?.as_i64()?;

    let num = |ptr: &str| root.pointer(ptr).and_then(|v| v.as_f64());
    let text = |ptr: &str| {
        root.pointer(ptr)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let as_num = |v: Option<f64>| v.map_or(SourceValue::Clear, SourceValue::Number);
    let as_text = |v: Option<String>| v.map_or(SourceValue::Clear, SourceValue::Text);

    let live = num("/beatmap/time/live");
    let first = num("/beatmap/time/firstObject");
    let last = num("/beatmap/time/lastObject");
    // Progress between the first and last hit object, matching osu!'s own progress bar
    let (elapsed, remaining, length, progress) = match (live, first, last) {
        (Some(live), Some(first), Some(last)) if last > first => (
            Some((live - first).max(0.0)),
            Some((last - live).max(0.0)),
            Some(last - first),
            Some(((live - first) / (last - first)).clamp(0.0, 1.0)),
        ),
        _ => (None, None, None, None),
    };

    let values = vec![
        (src::MAP_TITLE, as_text(text("/beatmap/title"))),
        (src::MAP_ARTIST, as_text(text("/beatmap/artist"))),
        (src::MAP_MAPPER, as_text(text("/beatmap/mapper"))),
        (src::MAP_DIFFICULTY, as_text(text("/beatmap/version"))),
        (src::MAP_STATUS, as_text(text("/beatmap/status/name"))),
        (src::MAP_STARS, as_num(num("/beatmap/stats/stars/total"))),
        (
            src::MAP_STARS_LIVE,
            as_num(num("/beatmap/stats/stars/live")),
        ),
        (src::MAP_AR, as_num(num("/beatmap/stats/ar/converted"))),
        (src::MAP_CS, as_num(num("/beatmap/stats/cs/converted"))),
        (src::MAP_OD, as_num(num("/beatmap/stats/od/converted"))),
        (src::MAP_HP, as_num(num("/beatmap/stats/hp/converted"))),
        (src::MAP_BPM, as_num(num("/beatmap/stats/bpm/common"))),
        (
            src::MAP_OBJECTS,
            as_num(num("/beatmap/stats/objects/total")),
        ),
        (src::MAP_MAX_COMBO, as_num(num("/beatmap/stats/maxCombo"))),
        (src::MAP_LENGTH, as_num(length)),
        (src::MAP_TIME_ELAPSED, as_num(elapsed)),
        (src::MAP_TIME_REMAINING, as_num(remaining)),
        (src::MAP_PROGRESS, as_num(progress)),
        (
            src::MAP_KIAI,
            as_num(
                root.pointer("/beatmap/isKiai")
                    .and_then(|v| v.as_bool())
                    .map(f64::from),
            ),
        ),
        (src::PLAY_PP, as_num(num("/play/pp/current"))),
        (src::PLAY_PP_FC, as_num(num("/play/pp/fc"))),
        (src::PLAY_PP_MAX, as_num(num("/play/pp/maxAchievable"))),
        (src::PLAY_ACCURACY, as_num(num("/play/accuracy"))),
        (src::PLAY_SCORE, as_num(num("/play/score"))),
        (src::PLAY_COMBO, as_num(num("/play/combo/current"))),
        (src::PLAY_MAX_COMBO, as_num(num("/play/combo/max"))),
        (src::PLAY_GRADE, as_text(text("/play/rank/current"))),
        (src::PLAY_HITS_300, as_num(num("/play/hits/300"))),
        (src::PLAY_HITS_100, as_num(num("/play/hits/100"))),
        (src::PLAY_HITS_50, as_num(num("/play/hits/50"))),
        (src::PLAY_HITS_MISS, as_num(num("/play/hits/0"))),
        (
            src::PLAY_SLIDER_BREAKS,
            as_num(num("/play/hits/sliderBreaks")),
        ),
        (src::PLAY_UR, as_num(num("/play/unstableRate"))),
        (
            src::PLAY_HEALTH,
            as_num(num("/play/healthBar/normal").map(|h| (h / 100.0).clamp(0.0, 1.0))),
        ),
        (src::PLAY_MODS, as_text(text("/play/mods/name"))),
        (src::PLAY_PLAYER, as_text(text("/play/playerName"))),
        (
            src::PLAY_FAILED,
            as_num(
                root.pointer("/play/failed")
                    .and_then(|v| v.as_bool())
                    .map(f64::from),
            ),
        ),
        (src::PROFILE_NAME, as_text(text("/profile/name"))),
        (
            src::PROFILE_RANK,
            as_num(num("/profile/globalRank").filter(|r| *r > 0.0)),
        ),
        (
            src::PROFILE_PP,
            as_num(num("/profile/pp").filter(|p| *p > 0.0)),
        ),
        (src::PROFILE_ACCURACY, as_num(num("/profile/accuracy"))),
        (src::PROFILE_PLAYCOUNT, as_num(num("/profile/playCount"))),
        (src::PROFILE_LEVEL, as_num(num("/profile/level"))),
        (
            src::PROFILE_COUNTRY,
            as_text(text("/profile/countryCode/name")),
        ),
        (src::SESSION_PLAYTIME, as_num(num("/session/playTime"))),
        (src::SESSION_PLAYCOUNT, as_num(num("/session/playCount"))),
        (
            src::GAME_STATE,
            as_text(text("/state/name").map(|s| friendly_state(&s))),
        ),
        (src::STATUS_OSU, SourceValue::Number(1.0)),
    ];

    Some(GameplayTelemetry {
        // tosu state 2 = play
        is_playing: state_num == 2,
        title: text("/beatmap/title").unwrap_or_default(),
        live_time_ms: live.unwrap_or(0.0),
        values,
    })
}

/// tosu state names ("selectPlay") as shown on the pad ("Song select")
fn friendly_state(name: &str) -> String {
    match name {
        "menu" => "Main menu",
        "play" => "Playing",
        "selectPlay" => "Song select",
        "resultScreen" => "Results",
        "edit" | "selectEdit" => "Editor",
        "multiplayerRooms" | "multiplayerRoom" | "multiplayerResultsscreen" => "Multiplayer",
        "lobby" => "Lobby",
        other => other,
    }
    .to_string()
}

// -----------------------------------------------------------------------------
// tosu process supervisor
// -----------------------------------------------------------------------------

/// Resolve the tosu binary: `$OPAD_TOSU_PATH`, legacy `$OSUPAD_TOSU_PATH`,
/// `~/.local/opt/tosu/tosu`, `tosu` on `$PATH`, then the bundled copy (§T-2).
///
/// The bundled copy is deliberately last: a tosu the user installed themselves
/// always wins, and §T-2 forbids us from updating or overwriting one we did
/// not install.
pub fn find_tosu_binary() -> Option<PathBuf> {
    if let Some(p) =
        std::env::var_os("OPAD_TOSU_PATH").or_else(|| std::env::var_os("OSUPAD_TOSU_PATH"))
    {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let home_install = dirs::home_dir().map(|h| h.join(".local/opt/tosu").join(paths::TOSU_BINARY));
    if let Some(p) = home_install.filter(|p| p.is_file()) {
        return Some(p);
    }
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(paths::TOSU_BINARY))
            .find(|p| p.is_file())
    });
    if let Some(p) = on_path {
        return Some(p);
    }
    paths::bundled_tosu_binary().ok().filter(|p| p.is_file())
}

/// Why tosu may be unable to read osu!'s memory on Linux, and what fixes it.
///
/// Yama (`/proc/sys/kernel/yama/ptrace_scope` > 0) lets a process read only its
/// own children's memory, and osu! is not tosu's child. A binary with
/// `cap_sys_ptrace` is exempt, but only a real executable can carry it: on the
/// AppImage the squashfs is mounted nosuid, which ignores file capabilities,
/// and where tosu is a script run by the system `node` the capability would
/// have to go on `node` itself, handing it to every Node program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PtraceAccess {
    /// Not Linux, no Yama, or scope 0: nothing stands in the way
    Unrestricted,
    /// Yama is on, and the tosu binary carries cap_sys_ptrace
    Capable { scope: u32 },
    /// Yama is on and tosu cannot read osu!. `fix` is the command that helps.
    Blocked { scope: u32, fix: String },
    /// Yama is on, but whether tosu has the capability cannot be told
    /// (no `getcap`); `fix` is what to run if telemetry stays empty
    Unknown { scope: u32, fix: String },
}

/// Yama's ptrace scope, or None where there is no Yama (or not Linux)
pub fn yama_ptrace_scope() -> Option<u32> {
    std::fs::read_to_string("/proc/sys/kernel/yama/ptrace_scope")
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn is_script(bin: &Path) -> bool {
    let mut head = [0u8; 2];
    std::fs::File::open(bin)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head))
        .is_ok_and(|_| &head == b"#!")
}

/// Whether tosu can read osu!'s memory under Yama, and the fix when it cannot.
pub fn ptrace_access() -> PtraceAccess {
    let scope = match yama_ptrace_scope() {
        Some(s) if s > 0 => s,
        _ => return PtraceAccess::Unrestricted,
    };
    let scope_fix = "echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope   \
                     (persist with kernel.yama.ptrace_scope = 0 in /etc/sysctl.d/10-ptrace.conf)"
        .to_string();
    if scope >= 3 {
        // No ptrace at all until reboot, capability or not
        return PtraceAccess::Blocked {
            scope,
            fix: "ptrace_scope 3 cannot be lowered until reboot; set 0 in /etc/sysctl.d and reboot"
                .into(),
        };
    }
    let Some(bin) = find_tosu_binary() else {
        return PtraceAccess::Blocked {
            scope,
            fix: scope_fix,
        };
    };
    if std::env::var_os("APPIMAGE").is_some() || is_script(&bin) {
        return PtraceAccess::Blocked {
            scope,
            fix: scope_fix,
        };
    }
    let setcap_fix = format!("sudo setcap cap_sys_ptrace=eip {}", bin.display());
    match std::process::Command::new("getcap").arg(&bin).output() {
        Ok(out) if String::from_utf8_lossy(&out.stdout).contains("cap_sys_ptrace") => {
            PtraceAccess::Capable { scope }
        }
        Ok(_) => PtraceAccess::Blocked {
            scope,
            fix: setcap_fix,
        },
        Err(_) => PtraceAccess::Unknown {
            scope,
            fix: setcap_fix,
        },
    }
}

/// Stops and restarts the supervised tosu.
///
/// Swapping the binary needs it held down: on Windows the file cannot be
/// replaced while it is running, and on every platform a tosu started from the
/// old inode would keep running after the swap and hide the update (§U-1).
#[derive(Clone, Debug, Default)]
pub struct TosuSupervisor {
    paused: Arc<AtomicBool>,
}

impl TosuSupervisor {
    /// Kills the running tosu and stops the supervisor relaunching it.
    ///
    /// Returns once the child is actually gone, so the caller may replace the
    /// binary immediately afterwards.
    pub async fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        // The supervisor drops its child on the next poll of the pause flag;
        // kill_on_drop makes that a real kill. One poll interval covers it.
        tokio::time::sleep(PAUSE_POLL_INTERVAL * 2).await;
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
}

const PAUSE_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Keeps a tosu process running for as long as the daemon runs.
///
/// Does nothing while something already listens on tosu's port (e.g. a tosu the
/// user started by hand). Otherwise launches tosu and restarts it with backoff
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub type LogLineCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// Keeps a tosu process running for as long as the daemon runs.
///
/// Does nothing while something already listens on tosu's port (e.g. a tosu the
/// user started by hand). Otherwise launches tosu and restarts it with backoff
/// if it exits. The child is killed when the daemon exits.
pub fn spawn_tosu_supervisor(
    endpoint: String,
    log_path: PathBuf,
    line_cb: Option<LogLineCallback>,
) -> TosuSupervisor {
    let supervisor = TosuSupervisor::default();
    let paused = supervisor.paused.clone();

    tokio::spawn(async move {
        let addr = endpoint_socket_addr(&normalize_endpoint(&endpoint));
        let mut backoff = Duration::from_secs(5);
        let mut warned_missing = false;

        loop {
            if paused.load(Ordering::SeqCst) {
                tokio::time::sleep(PAUSE_POLL_INTERVAL).await;
                continue;
            }

            if TcpStream::connect(&addr).await.is_ok() {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            let Some(bin) = find_tosu_binary() else {
                if !warned_missing {
                    warn!(
                        "tosu binary not found: no $OPAD_TOSU_PATH, none on $PATH, and no bundled copy"
                    );
                    warned_missing = true;
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            };
            warned_missing = false;

            match launch_tosu(&bin, &log_path, line_cb.clone()) {
                Ok(mut child) => {
                    info!(
                        "Launched tosu (pid {:?}) from {}",
                        child.id(),
                        bin.display()
                    );
                    let started = Instant::now();
                    loop {
                        tokio::select! {
                            result = child.wait() => {
                                match result {
                                    Ok(status) => warn!("tosu exited with {}", status),
                                    Err(e) => warn!("Failed waiting for tosu: {}", e),
                                }
                                break;
                            }
                            _ = tokio::time::sleep(PAUSE_POLL_INTERVAL) => {
                                if paused.load(Ordering::SeqCst) {
                                    // kill_on_drop turns this into a real kill,
                                    // freeing the binary for a swap (§U-1)
                                    info!("Stopping tosu for an update");
                                    drop(child);
                                    break;
                                }
                            }
                        }
                    }
                    backoff = if started.elapsed() > Duration::from_secs(60) {
                        Duration::from_secs(5)
                    } else {
                        (backoff * 2).min(Duration::from_secs(60))
                    };
                }
                Err(e) => warn!("Failed to launch tosu from {}: {}", bin.display(), e),
            }
            tokio::time::sleep(backoff).await;
        }
    });

    supervisor
}

/// Where tosu reads its `tosu.env`: `$XDG_CONFIG_HOME/tosu` on Linux, next to
/// the binary elsewhere (tosu's `getConfigPath`).
fn tosu_config_dir(bin: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        dirs::config_dir().map(|d| d.join("tosu"))
    } else {
        bin.parent().map(Path::to_path_buf)
    }
}

fn launch_tosu(
    bin: &Path,
    log_path: &Path,
    line_cb: Option<LogLineCallback>,
) -> std::io::Result<tokio::process::Child> {
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Prevent tosu from ever auto-opening the web dashboard on launch. Its
    // tosu.env wins over the environment variable below, so the file is what
    // has to say false.
    if let Some(dir) = tosu_config_dir(bin) {
        let _ = std::fs::create_dir_all(&dir);
        let env_file = dir.join("tosu.env");
        if let Ok(content) = std::fs::read_to_string(&env_file) {
            if content.contains("OPEN_DASHBOARD_ON_STARTUP=true") {
                let updated = content.replace(
                    "OPEN_DASHBOARD_ON_STARTUP=true",
                    "OPEN_DASHBOARD_ON_STARTUP=false",
                );
                let _ = std::fs::write(&env_file, updated);
            }
        } else {
            let _ = std::fs::write(&env_file, "OPEN_DASHBOARD_ON_STARTUP=false\n");
        }
    }

    let mut cmd = tokio::process::Command::new(bin);
    cmd.env("OPEN_DASHBOARD_ON_STARTUP", "false");
    cmd.stdin(Stdio::null());
    if let Some(dir) = bin.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: run tosu headlessly without console window

    if let Some(cb) = line_cb {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.kill_on_drop(true).spawn()?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let log_file = Arc::new(std::sync::Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .ok(),
        ));

        if let Some(p) = stdout {
            let cb_clone = cb.clone();
            let file_clone = log_file.clone();
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(p).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let clean = strip_ansi(&line);
                    cb_clone(&clean);
                    if let Ok(mut guard) = file_clone.lock() {
                        if let Some(f) = guard.as_mut() {
                            use std::io::Write;
                            let _ = writeln!(f, "{}", line);
                        }
                    }
                }
            });
        }
        if let Some(p) = stderr {
            let cb_clone = cb;
            let file_clone = log_file;
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(p).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let clean = strip_ansi(&line);
                    cb_clone(&clean);
                    if let Ok(mut guard) = file_clone.lock() {
                        if let Some(f) = guard.as_mut() {
                            use std::io::Write;
                            let _ = writeln!(f, "{}", line);
                        }
                    }
                }
            });
        }
        Ok(child)
    } else {
        let log = std::fs::File::create(log_path)?;
        cmd.stdout(log.try_clone()?).stderr(log).kill_on_drop(true);
        cmd.spawn()
    }
}

/// "ws://127.0.0.1:24050/websocket/v2" -> "127.0.0.1:24050"
fn endpoint_socket_addr(endpoint: &str) -> String {
    let without_scheme = endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest);
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{}:80", authority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trimmed from a real tosu v4.26.2 /websocket/v2 frame with osu!lazer playing
    const PLAYING_FRAME: &str = r#"{
        "state": { "number": 2, "name": "play" },
        "beatmap": {
            "time": { "live": 89152, "firstObject": 792, "lastObject": 177513, "mp3Length": 182238 },
            "artist": "takehirotei",
            "title": "Haiboku no Altra Vita",
            "version": "Expert",
            "stats": { "stars": { "live": 3.9, "total": 5.51 } }
        },
        "play": {
            "pp": { "current": 143.7, "fc": 220.09 },
            "accuracy": 97.4, "combo": { "current": 70, "max": 80 }, "rank": { "current": "A" },
            "hits": { "0": 1, "50": 0, "100": 16, "300": 85 }, "mods": { "name": "" }
        },
        "profile": { "name": "osu!player", "globalRank": 12345, "pp": 4321.0 }
    }"#;

    fn value(t: &GameplayTelemetry, source: u8) -> &SourceValue {
        &t.values
            .iter()
            .find(|(s, _)| *s == source)
            .expect("source present")
            .1
    }

    #[test]
    fn test_parse_tosu_v2_playing() {
        let parsed = parse_tosu_v2_json(PLAYING_FRAME).expect("valid parse");
        assert!(parsed.is_playing);
        assert_eq!(parsed.title, "Haiboku no Altra Vita");
        assert_eq!(
            value(&parsed, src::MAP_ARTIST),
            &SourceValue::Text("takehirotei".into())
        );
        assert_eq!(
            value(&parsed, src::MAP_DIFFICULTY),
            &SourceValue::Text("Expert".into())
        );
        assert_eq!(value(&parsed, src::MAP_STARS), &SourceValue::Number(5.51));
        assert_eq!(value(&parsed, src::PLAY_PP), &SourceValue::Number(143.7));
        assert_eq!(
            value(&parsed, src::PLAY_GRADE),
            &SourceValue::Text("A".into())
        );
        assert_eq!(
            value(&parsed, src::PLAY_HITS_MISS),
            &SourceValue::Number(1.0)
        );
        assert_eq!(value(&parsed, src::PLAY_MODS), &SourceValue::Clear);
        assert_eq!(
            value(&parsed, src::PROFILE_NAME),
            &SourceValue::Text("osu!player".into())
        );
        assert_eq!(
            value(&parsed, src::GAME_STATE),
            &SourceValue::Text("Playing".into())
        );
        match value(&parsed, src::MAP_PROGRESS) {
            SourceValue::Number(p) => assert!((p - 0.5).abs() < 0.01),
            other => panic!("progress: {:?}", other),
        }
    }

    #[test]
    fn test_parse_tosu_v2_menu() {
        let sample = r#"{ "state": { "number": 5, "name": "selectPlay" }, "beatmap": { "title": "Song Select" } }"#;
        let parsed = parse_tosu_v2_json(sample).expect("valid parse");
        assert!(!parsed.is_playing);
        assert_eq!(parsed.title, "Song Select");
        assert_eq!(
            value(&parsed, src::GAME_STATE),
            &SourceValue::Text("Song select".into())
        );
        assert_eq!(value(&parsed, src::MAP_PROGRESS), &SourceValue::Clear);
    }

    #[test]
    fn test_rejects_legacy_v1_frame() {
        let v1 = r#"{ "menu": { "state": 2, "bm": { "metadata": { "title": "x" } } }, "gameplay": { "pp": { "current": 1 } } }"#;
        assert!(parse_tosu_v2_json(v1).is_none());
    }

    #[test]
    fn test_endpoint_helpers() {
        assert_eq!(
            normalize_endpoint("ws://127.0.0.1:24050/ws"),
            DEFAULT_TOSU_ENDPOINT
        );
        assert_eq!(
            normalize_endpoint(DEFAULT_TOSU_ENDPOINT),
            DEFAULT_TOSU_ENDPOINT
        );
        assert_eq!(
            endpoint_socket_addr(DEFAULT_TOSU_ENDPOINT),
            "127.0.0.1:24050"
        );
    }
}
