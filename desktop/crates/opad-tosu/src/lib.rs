use futures_util::StreamExt;
use opad_model::paths;
use opad_model::ui_source::{self as src, SourceValue};
use opad_model::GameplayTelemetry;
use std::path::{Path, PathBuf};
use std::process::Stdio;
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
    let overridden =
        std::env::var_os("OPAD_TOSU_PATH").or_else(|| std::env::var_os("OSUPAD_TOSU_PATH"));
    if let Some(p) = overridden.and_then(|p| usable_override(PathBuf::from(p))) {
        return Some(p);
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

/// The `$OPAD_TOSU_PATH` target if it is a file. A stale override (tosu moved
/// or uninstalled) is named in a warning and then ignored, so it neither hides
/// the other candidates nor fails as if it were not set at all.
fn usable_override(p: PathBuf) -> Option<PathBuf> {
    if p.is_file() {
        return Some(p);
    }
    warn!(
        "$OPAD_TOSU_PATH points at {}, which is not a file; looking for tosu elsewhere",
        p.display()
    );
    None
}

/// Starts tosu for the GUI's "Start tosu" button, detached: it keeps running
/// after the GUI exits, in its own process group so closing the GUI's terminal
/// or session scope does not signal it. Output goes to the usual tosu log.
/// The daemon's supervisor ([`spawn_tosu_supervisor`]) is what normally runs
/// tosu; this is the manual path when no daemon is managing it.
pub fn launch_tosu_process(override_path: Option<&Path>) -> Result<(), String> {
    let bin = override_path
        .map(Path::to_path_buf)
        .or_else(find_tosu_binary)
        .ok_or_else(|| {
            "tosu executable not found (no bundled copy or system install)".to_string()
        })?;
    let log_path =
        paths::tosu_log_path().map_err(|e| format!("Cannot resolve tosu log path: {e}"))?;
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Cannot create {}: {e}", dir.display()))?;
    }
    disable_dashboard_autostart(&bin);

    let log = std::fs::File::create(&log_path)
        .map_err(|e| format!("Cannot open {}: {e}", log_path.display()))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("Cannot open {}: {e}", log_path.display()))?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.env("OPEN_DASHBOARD_ON_STARTUP", "false")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(log_err);
    if let Some(dir) = bin.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    // The Child is dropped, not killed: std never kills on drop
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch tosu from {}: {e}", bin.display()))
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
#[derive(Clone, Debug)]
pub struct TosuSupervisor {
    paused: Arc<watch::Sender<bool>>,
    /// True while the supervisor has no tosu child: not yet launched, exited,
    /// or killed and reaped. `pause()` waits on it.
    stopped: Arc<watch::Sender<bool>>,
}

impl Default for TosuSupervisor {
    fn default() -> Self {
        Self {
            paused: Arc::new(watch::channel(false).0),
            stopped: Arc::new(watch::channel(true).0),
        }
    }
}

impl TosuSupervisor {
    /// Kills the running tosu and stops the supervisor relaunching it.
    ///
    /// Returns once the child is actually gone (killed and reaped), so the
    /// caller may replace the binary immediately afterwards. A child that
    /// somehow outlives [`PAUSE_TIMEOUT`] is logged and not waited on further.
    pub async fn pause(&self) {
        self.paused.send_replace(true);
        let mut stopped = self.stopped.subscribe();
        if tokio::time::timeout(PAUSE_TIMEOUT, stopped.wait_for(|s| *s))
            .await
            .is_err()
        {
            warn!(
                "tosu did not stop within {:?} of being paused; the update continues anyway",
                PAUSE_TIMEOUT
            );
        }
    }

    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }
}

/// How long `pause()` waits for tosu to die before giving up on it
const PAUSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Removes terminal escape sequences from a tosu log line: CSI (`ESC [`
/// parameters, intermediates, then a final byte in `0x40..=0x7E`), OSC
/// (`ESC ]` up to `BEL` or `ESC \`, e.g. a window title or hyperlink), and
/// the short `ESC` + intermediates + final forms such as `ESC ( B`.
pub fn strip_ansi(s: &str) -> String {
    enum State {
        Text,
        Esc,
        EscIntermediate,
        Csi,
        Osc,
        OscEsc,
    }
    let mut out = String::with_capacity(s.len());
    let mut state = State::Text;
    for c in s.chars() {
        state = match state {
            State::Text if c == '\x1b' => State::Esc,
            State::Text => {
                out.push(c);
                State::Text
            }
            State::Esc => match c {
                '[' => State::Csi,
                ']' => State::Osc,
                '\x20'..='\x2f' => State::EscIntermediate,
                _ => State::Text,
            },
            State::EscIntermediate => match c {
                '\x20'..='\x2f' => State::EscIntermediate,
                _ => State::Text,
            },
            State::Csi => match c {
                '\x40'..='\x7e' => State::Text,
                _ => State::Csi,
            },
            State::Osc => match c {
                '\x07' => State::Text,
                '\x1b' => State::OscEsc,
                _ => State::Osc,
            },
            State::OscEsc => match c {
                '\\' => State::Text,
                _ => State::Osc,
            },
        };
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
    let addr = endpoint_socket_addr(&normalize_endpoint(&endpoint));
    tokio::spawn(supervise(
        supervisor.clone(),
        addr,
        find_tosu_binary,
        move |bin| launch_tosu(bin, &log_path, line_cb.clone()),
    ));
    supervisor
}

const INITIAL_BACKOFF: Duration = Duration::from_secs(5);

async fn until_paused(paused: &mut watch::Receiver<bool>) {
    let _ = paused.wait_for(|p| *p).await;
}

/// The supervisor loop, with the binary lookup and the launch passed in so a
/// test can run it on a stand-in child
async fn supervise(
    supervisor: TosuSupervisor,
    addr: String,
    find_bin: impl Fn() -> Option<PathBuf>,
    launch: impl Fn(&Path) -> std::io::Result<tokio::process::Child>,
) {
    let mut paused = supervisor.paused.subscribe();
    let mut backoff = INITIAL_BACKOFF;
    let mut warned_missing = false;

    loop {
        if *paused.borrow_and_update() {
            // Wakes on resume() rather than polling. A pause is not a crash,
            // so whatever the backoff had grown to starts over.
            if paused.wait_for(|p| !*p).await.is_err() {
                return;
            }
            backoff = INITIAL_BACKOFF;
        }

        if TcpStream::connect(&addr).await.is_ok() {
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        let Some(bin) = find_bin() else {
            if !warned_missing {
                warn!(
                    "tosu binary not found: none at $OPAD_TOSU_PATH (if set), in ~/.local/opt/tosu, on $PATH, or bundled"
                );
                warned_missing = true;
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        warned_missing = false;

        // Marked running *before* the pause flag is read again, so a pause()
        // landing now either stops this launch or waits for its child
        supervisor.stopped.send_replace(false);
        if *paused.borrow_and_update() {
            supervisor.stopped.send_replace(true);
            continue;
        }

        match launch(&bin) {
            Ok(mut child) => {
                info!(
                    "Launched tosu (pid {:?}) from {}",
                    child.id(),
                    bin.display()
                );
                let started = Instant::now();
                let stopped_for_pause = tokio::select! {
                    result = child.wait() => {
                        match result {
                            Ok(status) => warn!("tosu exited with {}", status),
                            Err(e) => warn!("Failed waiting for tosu: {}", e),
                        }
                        false
                    }
                    _ = until_paused(&mut paused) => {
                        // Killed and reaped before pause() is told, freeing
                        // the binary for a swap (§U-1)
                        info!("Stopping tosu for an update");
                        if let Err(e) = child.kill().await {
                            warn!("Failed to stop tosu: {}", e);
                        }
                        true
                    }
                };
                supervisor.stopped.send_replace(true);
                if stopped_for_pause {
                    continue;
                }
                backoff = if started.elapsed() > Duration::from_secs(60) {
                    INITIAL_BACKOFF
                } else {
                    (backoff * 2).min(Duration::from_secs(60))
                };
            }
            Err(e) => {
                supervisor.stopped.send_replace(true);
                warn!("Failed to launch tosu from {}: {}", bin.display(), e);
            }
        }
        // A pause during the backoff needs no waiting: nothing is running
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = until_paused(&mut paused) => {}
        }
    }
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

/// Keeps tosu from opening its web dashboard in a browser when it starts. Its
/// tosu.env wins over the OPEN_DASHBOARD_ON_STARTUP environment variable, so
/// the file is what has to say false.
fn disable_dashboard_autostart(bin: &Path) {
    let Some(dir) = tosu_config_dir(bin) else {
        return;
    };
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

fn launch_tosu(
    bin: &Path,
    log_path: &Path,
    line_cb: Option<LogLineCallback>,
) -> std::io::Result<tokio::process::Child> {
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    disable_dashboard_autostart(bin);

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

    #[test]
    fn a_missing_tosu_override_falls_through() {
        let dir = tempfile::tempdir().unwrap();
        let present = dir.path().join("tosu");
        std::fs::write(&present, b"").unwrap();
        assert_eq!(usable_override(present.clone()), Some(present));
        assert_eq!(usable_override(dir.path().join("uninstalled")), None);
        assert_eq!(usable_override(dir.path().to_path_buf()), None);
    }

    #[test]
    fn escape_sequences_are_stripped_whole() {
        // chalk colours and bold
        assert_eq!(strip_ansi("\x1b[31mred\x1b[39m plain"), "red plain");
        assert_eq!(strip_ansi("\x1b[1;38;5;208m[tosu]\x1b[0m ok"), "[tosu] ok");
        // ora: hide cursor, clear line, column 1, show cursor
        assert_eq!(
            strip_ansi("\x1b[?25l\x1b[2K\x1b[1G⠋ Loading\x1b[?25h"),
            "⠋ Loading"
        );
        // OSC window title ended by BEL: nothing of it leaks
        assert_eq!(
            strip_ansi("\x1b]0;tosu v4\x07Server started"),
            "Server started"
        );
        // OSC 8 hyperlink ended by ST (ESC \)
        assert_eq!(
            strip_ansi("see \x1b]8;;https://tosu.app\x1b\\docs\x1b]8;;\x1b\\ now"),
            "see docs now"
        );
        // Charset selection: ESC + intermediate + final
        assert_eq!(strip_ansi("\x1b(Bplain"), "plain");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
    }

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

    /// The GUI's "Start tosu" used to drop a kill_on_drop child, which killed
    /// tosu the moment it was launched. A stand-in tosu that writes a file
    /// after a short sleep must get to write it.
    #[cfg(unix)]
    #[test]
    fn a_manually_launched_tosu_outlives_the_launch_call() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("opad-tosu-launch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Keep the real tosu log and tosu.env out of this
        std::env::set_var("XDG_STATE_HOME", dir.join("state"));
        std::env::set_var("XDG_CONFIG_HOME", dir.join("config"));
        let marker = dir.join("still-running");
        let bin = dir.join("tosu");
        std::fs::write(
            &bin,
            format!("#!/bin/sh\nsleep 0.3\ntouch '{}'\n", marker.display()),
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();

        launch_tosu_process(Some(&bin)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !marker.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        let survived = marker.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(survived, "tosu was killed when the launcher returned");
    }

    /// DM#1 + DM#4: pause() returns only once the child is gone, and resume()
    /// relaunches at once instead of after a crash backoff
    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn pause_waits_for_the_child_and_resume_relaunches_promptly() {
        use std::sync::Mutex;

        // A port nothing listens on, so the supervisor launches. Not a freed
        // ephemeral one: other tests' listeners (or a TCP self-connect) can
        // land on that and look like a running tosu.
        let addr = "127.0.0.1:1".to_string();
        let pids = Arc::new(Mutex::new(Vec::<u32>::new()));
        let launched = pids.clone();
        let supervisor = TosuSupervisor::default();
        tokio::spawn(supervise(
            supervisor.clone(),
            addr,
            || Some(PathBuf::from("sleep")),
            move |bin| {
                let child = tokio::process::Command::new(bin)
                    .arg("30")
                    .kill_on_drop(true)
                    .spawn()?;
                launched.lock().unwrap().push(child.id().unwrap());
                Ok(child)
            },
        ));
        let alive = |pid: u32| std::path::Path::new(&format!("/proc/{pid}")).exists();
        let launches_reach = |n: usize| {
            let pids = pids.clone();
            async move {
                tokio::time::timeout(Duration::from_secs(2), async {
                    while pids.lock().unwrap().len() < n {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .is_ok()
            }
        };

        assert!(launches_reach(1).await, "first launch");
        let first = pids.lock().unwrap()[0];
        assert!(alive(first));

        supervisor.pause().await;
        assert!(
            !alive(first),
            "pause() returned with the child still running"
        );

        // Nothing is relaunched while paused
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(pids.lock().unwrap().len(), 1);

        // Well inside INITIAL_BACKOFF: a pause stop is not treated as a crash
        supervisor.resume();
        assert!(
            launches_reach(2).await,
            "resume() did not relaunch promptly"
        );

        // Pausing again works the same, and an idle supervisor pauses at once
        supervisor.pause().await;
        let second = pids.lock().unwrap()[1];
        assert!(!alive(second));
        tokio::time::timeout(Duration::from_millis(100), supervisor.pause())
            .await
            .expect("pausing with nothing running must not wait");
    }
}
