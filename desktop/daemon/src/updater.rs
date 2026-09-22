//! The update worker (§U-0, §U-1, §U-2).
//!
//! One task, woken on a slow tick, that does nothing at all unless the daemon
//! is IDLE. That is deliberate and is the §U-0.1 rule in its strongest form:
//! not merely "do not apply an update mid-map" but "do not even record that a
//! check happened", because recording it is a storage write and P1-3 forbids
//! those during PLAYING and COOLDOWN.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

use opad_ipc::{ComponentUpdate, UpdateComponent};
use opad_model::RuntimeMode;
use opad_storage::Storage;
use opad_tosu::TosuSupervisor;
use opad_update::app::{self, AppAction};
use opad_update::client::UpdateClient;
use opad_update::manifest::current_target;
use opad_update::tosu::{self, TosuAction};
use opad_update::{
    may_update_now, ApplyPolicy, CheckSchedule, InstallOrigin, ReleaseManifest, UpdateError,
};

use crate::runtime::DaemonState;

/// How often the worker wakes to ask whether a check is due. The check itself
/// is daily (§U-0.6); this only decides how soon after becoming IDLE — or
/// after a laptop wakes up — the daily check actually runs.
const TICK: Duration = Duration::from_secs(15 * 60);

pub const SCHEDULE_KEY: &str = "update.schedule";
pub const TOSU_ENABLED_KEY: &str = "update.tosu.enabled";
pub const APP_ENABLED_KEY: &str = "update.app.enabled";

/// What the GUI shows for each updater (§U-0.4)
#[derive(Debug, Clone, Default)]
pub struct UpdateStatus {
    pub tosu: ComponentUpdate,
    pub app: ComponentUpdate,
    pub last_check: Option<SystemTime>,
    pub last_error: Option<String>,
    /// The app was replaced on disk, so the running daemon and GUI are the old
    /// binaries and must be restarted (§U-2)
    pub restart_required: bool,
}

pub type SharedUpdateStatus = Arc<Mutex<UpdateStatus>>;

/// A person pressed Install. The daemon never applies an app update on its own
/// (§U-2), so this is the only way one is applied.
#[derive(Debug, Clone, Copy)]
pub enum UpdateCommand {
    Install(UpdateComponent),
}

/// The last manifest that passed signature verification.
///
/// Shared rather than kept inside the worker because §U-3b's firmware flash is
/// driven from an IPC handler, and it must use a manifest this daemon actually
/// verified — never re-fetch one at the moment of flashing, where a failure
/// would land between the consent and the write.
pub type SharedManifest = Arc<Mutex<Option<ReleaseManifest>>>;

/// The handle IPC handlers use to read status and ask for an install
#[derive(Clone)]
pub struct UpdateService {
    status: SharedUpdateStatus,
    manifest: SharedManifest,
    commands: tokio::sync::mpsc::UnboundedSender<UpdateCommand>,
}

impl UpdateService {
    pub fn status(&self) -> UpdateStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// The verified manifest, or `None` if no check has succeeded yet.
    pub fn manifest(&self) -> Option<ReleaseManifest> {
        self.manifest.lock().ok().and_then(|m| m.clone())
    }

    pub fn request_install(&self, component: UpdateComponent) -> Result<(), String> {
        self.commands
            .send(UpdateCommand::Install(component))
            .map_err(|_| "The update worker is not running".to_string())
    }
}

pub fn spawn_update_worker(
    daemon_state: Arc<Mutex<DaemonState>>,
    storage: Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: TosuSupervisor,
) -> UpdateService {
    let status: SharedUpdateStatus = Arc::new(Mutex::new(UpdateStatus::default()));
    let shared_manifest: SharedManifest = Arc::new(Mutex::new(None));
    let (commands, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
    let service = UpdateService {
        status: status.clone(),
        manifest: shared_manifest.clone(),
        commands,
    };

    tokio::spawn(async move {
        // Spread the first check out a little: a machine that just booted is
        // busy, and every install checking at login is the pattern §U-0.6
        // warns about.
        tokio::time::sleep(Duration::from_secs(90)).await;
        let mut manifest: Option<ReleaseManifest> = None;

        // An interval, not a sleep inside the loop: `tick()` fires immediately
        // the first time, so the first check happens at the 90 s mark above.
        // A `sleep(TICK)` there would have made it 90 s *plus* a full tick, so
        // a fresh install went its first quarter of an hour without ever
        // checking — and the 90 s spread would have bought nothing.
        let mut ticker = tokio::time::interval(TICK);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match tick(&daemon_state, &storage, &tosu_supervisor, &status).await {
                        Ok(m) => {
                            if m.is_some() {
                                if let Ok(mut shared) = shared_manifest.lock() {
                                    shared.clone_from(&m);
                                }
                                manifest = m;
                            }
                        }
                        Err(e) => {
                            warn!("Update check failed: {}", e);
                            if let Ok(mut s) = status.lock() {
                                s.last_error = Some(e.to_string());
                            }
                        }
                    }
                }
                Some(UpdateCommand::Install(component)) = command_rx.recv() => {
                    let result = match component {
                        UpdateComponent::App => {
                            install_app(manifest.as_ref(), &daemon_state, &storage, &status).await
                        }
                        UpdateComponent::Tosu => Ok(()),
                        // §U-3: an app update must never trigger a firmware
                        // update. A flash is unreachable from here by design;
                        // it goes through InstallFirmwareUpdate, which takes
                        // consent and syncs the counters first.
                        UpdateComponent::Firmware => Err(UpdateError::Http(
                            "Firmware updates are not applied by the updater (§U-3)".to_string(),
                        )),
                    };
                    if let Err(e) = result {
                        warn!("Applying the update failed: {}", e);
                        if let Ok(mut s) = status.lock() {
                            s.last_error = Some(e.to_string());
                        }
                    }
                }
            }
        }
    });

    service
}

/// Returns the manifest when a fresh one was fetched, so the worker can keep
/// it for a later Install command without re-fetching and re-verifying.
async fn tick(
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: &TosuSupervisor,
    status: &SharedUpdateStatus,
) -> Result<Option<ReleaseManifest>, UpdateError> {
    let mode = daemon_state
        .lock()
        .map(|s| s.mode)
        .unwrap_or(RuntimeMode::Playing);

    // Not idle: change nothing, write nothing, do not even look.
    if let Err(reason) = may_update_now(mode, true) {
        debug!("Skipping the update check: {}", reason);
        return Ok(None);
    }

    let mut schedule: CheckSchedule = read_json(storage, SCHEDULE_KEY).unwrap_or_default();
    let now = SystemTime::now();
    if !schedule.due_at(now) {
        return Ok(None);
    }

    let client = UpdateClient::new()?;
    let fetched = client.fetch_manifest(&mut schedule, now).await;
    // The schedule is recorded whatever happened, so a failure backs off
    // instead of retrying on every tick.
    write_json(storage, SCHEDULE_KEY, &schedule);
    if let Ok(mut s) = status.lock() {
        s.last_check = schedule.last_check;
        s.last_error = schedule.last_error.clone();
    }

    let Some(manifest) = fetched? else {
        debug!("Release manifest unchanged since the last check");
        return Ok(None);
    };

    // tosu updates itself; the app only reports, and waits to be told (§U-2).
    if let Err(e) = update_tosu(
        &client,
        &manifest,
        daemon_state,
        storage,
        tosu_supervisor,
        status,
    )
    .await
    {
        warn!("tosu update failed: {}", e);
        if let Ok(mut s) = status.lock() {
            s.last_error = Some(e.to_string());
        }
    }

    // Reported, not propagated — exactly like the tosu arm above. A manifest
    // that carries no app artifact for this platform (an unsupported arch, or
    // a release that shipped one component late) must not throw the whole
    // manifest away: the firmware offer (§U-3b) is read from it too, and
    // losing it would silently disable firmware updates for a reason that has
    // nothing to do with the firmware.
    if let Err(e) = check_app(&manifest, daemon_state, storage, status) {
        warn!("App update check failed: {}", e);
        if let Ok(mut s) = status.lock() {
            s.last_error = Some(e.to_string());
        }
    }
    Ok(Some(manifest))
}

/// Records whether an app update is available. Applies nothing: §U-2 is
/// explicit that the app never auto-installs by default, so the daemon's job
/// is to have the answer ready when the user asks.
fn check_app(
    manifest: &ReleaseManifest,
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    status: &SharedUpdateStatus,
) -> Result<(), UpdateError> {
    let enabled = read_flag(storage, APP_ENABLED_KEY);
    let installed = env!("CARGO_PKG_VERSION");
    let origin = InstallOrigin::detect();
    let mode = daemon_state
        .lock()
        .map(|s| s.mode)
        .unwrap_or(RuntimeMode::Playing);

    let action = app::plan(
        manifest,
        installed,
        origin,
        current_target(),
        may_update_now(mode, enabled),
    )?;

    if let Ok(mut s) = status.lock() {
        s.app.installed = Some(installed.to_string());
        s.app.enabled = enabled;
        match &action {
            AppAction::UpToDate { .. } | AppAction::Defer { .. } => {
                s.app.available = None;
                s.app.ready_to_install = false;
                s.app.notify_only = false;
            }
            AppAction::NotifyOnly { available, notes } => {
                s.app.available = Some(available.clone());
                s.app.notes = notes.clone();
                s.app.notify_only = true;
                s.app.ready_to_install = false;
            }
            AppAction::Available {
                available, notes, ..
            } => {
                s.app.available = Some(available.clone());
                s.app.notes = notes.clone();
                s.app.notify_only = false;
                s.app.ready_to_install = true;
            }
        }
    }

    if let AppAction::Available { available, .. } = &action {
        info!("OPad {} is available to install", available);
    }
    Ok(())
}

/// Applies the app update the user asked for.
///
/// Order matters (§U-2): apply first, and only then report that a restart is
/// needed. The daemon is not stopped before the package manager has actually
/// succeeded — a failed or cancelled update must leave a running app behind.
async fn install_app(
    manifest: Option<&ReleaseManifest>,
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    status: &SharedUpdateStatus,
) -> Result<(), UpdateError> {
    let manifest = manifest.ok_or_else(|| {
        UpdateError::Http("No verified release manifest yet; check for updates first".to_string())
    })?;

    let mode = daemon_state
        .lock()
        .map(|s| s.mode)
        .unwrap_or(RuntimeMode::Playing);
    let origin = InstallOrigin::detect();
    let action = app::plan(
        manifest,
        env!("CARGO_PKG_VERSION"),
        origin,
        current_target(),
        may_update_now(mode, read_flag(storage, APP_ENABLED_KEY)),
    )?;

    let AppAction::Available {
        available,
        artifact,
        policy,
        ..
    } = action
    else {
        return Err(UpdateError::Http(format!(
            "There is nothing to install right now ({action:?})"
        )));
    };

    let client = UpdateClient::new()?;
    let bytes = client.fetch_artifact(&artifact).await?;

    // The verified bytes go to a file the apply step can hand to a package
    // manager or run. It lives in the state directory, not /tmp, so a
    // hardened /tmp mounted noexec cannot break the Windows installer path.
    let staging = opad_model::paths::state_dir()
        .map_err(|e| UpdateError::Io(std::io::Error::other(e.to_string())))?
        .join("updates");
    let file_name = artifact
        .url
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .unwrap_or("opad-update");
    let target = staging.join(file_name);
    opad_update::download::stage_bytes(&target, &bytes, &artifact.sha256, &artifact.url)?
        .install_to(&target)?;

    let applied = apply_downloaded(policy, origin, &target);

    // A .deb or an installer is worth tens of megabytes and has done its job.
    // Leaving it behind would put it on §W2-3's list of things to clean up for
    // no reason. Direct replacement consumes the file itself, so a missing one
    // is not an error.
    let _ = std::fs::remove_file(&target);
    applied?;

    if let Ok(mut s) = status.lock() {
        s.restart_required = true;
        s.app.ready_to_install = false;
        s.app.installed = Some(available.clone());
    }
    info!("OPad {} applied; a restart is needed to run it", available);
    Ok(())
}

fn apply_downloaded(
    policy: ApplyPolicy,
    origin: InstallOrigin,
    file: &std::path::Path,
) -> Result<(), UpdateError> {
    use std::process::Command;

    if let Some(cmd) = app::apply_command(policy, file) {
        let status = Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .map_err(|e| UpdateError::Io(std::io::Error::other(format!("{}: {e}", cmd[0]))))?;
        if !status.success() {
            // A cancelled polkit prompt lands here. The running version is
            // untouched and the update stays pending (§U-2 acceptance).
            return Err(UpdateError::Http(format!(
                "{} exited with {:?}; the running version is unchanged",
                cmd[0],
                status.code()
            )));
        }
        return Ok(());
    }

    // Direct replacement: the user owns every one of these files.
    match origin {
        InstallOrigin::AppImage => {
            let current = std::env::var_os("APPIMAGE")
                .map(std::path::PathBuf::from)
                .ok_or_else(|| {
                    UpdateError::Io(std::io::Error::other(
                        "$APPIMAGE is not set, so there is no AppImage to replace",
                    ))
                })?;
            std::fs::rename(file, &current)?;
            Ok(())
        }
        _ => {
            let prefix = opad_model::paths::install_prefix()
                .map_err(|e| UpdateError::Io(std::io::Error::other(e.to_string())))?;
            let status = Command::new("tar")
                .arg("-xzf")
                .arg(file)
                .arg("-C")
                .arg(&prefix)
                .status()
                .map_err(|e| UpdateError::Io(std::io::Error::other(format!("tar: {e}"))))?;
            if !status.success() {
                return Err(UpdateError::Http(format!(
                    "tar exited with {:?}; the running version is unchanged",
                    status.code()
                )));
            }
            Ok(())
        }
    }
}

async fn update_tosu(
    client: &UpdateClient,
    manifest: &ReleaseManifest,
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    supervisor: &TosuSupervisor,
    status: &SharedUpdateStatus,
) -> Result<(), UpdateError> {
    let enabled = read_flag(storage, TOSU_ENABLED_KEY);
    let bundled_dir = tosu::bundled_dir()?;
    let installed = tosu::installed_version(&bundled_dir);
    let origin = InstallOrigin::detect();

    // Re-read the mode: fetching the manifest took time, and the user may have
    // started a map while it was in flight.
    let mode = daemon_state
        .lock()
        .map(|s| s.mode)
        .unwrap_or(RuntimeMode::Playing);

    let action = tosu::plan(
        manifest,
        installed.as_deref(),
        origin,
        current_target(),
        may_update_now(mode, enabled),
    )?;

    if let Ok(mut s) = status.lock() {
        s.tosu.installed = installed.clone();
        s.tosu.available = manifest
            .component(opad_update::TOSU)
            .map(|c| c.version.clone());
        s.tosu.enabled = enabled;
        s.tosu.notify_only = matches!(action, TosuAction::NotifyOnly { .. });
    }

    match action {
        TosuAction::UpToDate { version } => {
            debug!("Bundled tosu {} is current", version);
        }
        TosuAction::NotifyOnly {
            installed,
            available,
        } => {
            info!(
                "tosu {} is available (this install has {}), but its files belong to the package manager; not touching them",
                available,
                if installed.is_empty() { "an unrecorded version" } else { &installed }
            );
        }
        TosuAction::Defer { reason } => {
            debug!("Deferring the tosu update: {}", reason);
        }
        TosuAction::Install { from, to, artifact } => {
            info!("Updating the bundled tosu from {} to {}", from, to);
            let bytes = client.fetch_artifact(&artifact).await?;

            // Hold tosu down for the swap. Windows will not replace a running
            // binary at all, and on any platform a tosu still running from the
            // old inode would hide the update until the next restart.
            supervisor.pause().await;
            let upstream_tag = manifest
                .component(opad_update::TOSU)
                .and_then(|c| c.upstream_tag.clone());
            let result = tosu::install(
                &bundled_dir,
                &bytes,
                &artifact,
                &to,
                upstream_tag.as_deref(),
            );
            supervisor.resume();

            match result {
                Ok(()) => info!("Bundled tosu updated to {}", to),
                Err(e) => {
                    // The previous binary is untouched; say so and try again
                    // on the next cycle rather than leaving a broken install.
                    warn!(
                        "tosu update to {} failed, keeping the current binary: {}",
                        to, e
                    );
                    if let Ok(mut s) = status.lock() {
                        s.last_error = Some(e.to_string());
                    }
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// Reads a boolean setting, defaulting to enabled. A storage failure must not
/// silently switch an updater off — or on — so the default is the documented
/// one and the error is visible in the log.
fn read_flag(storage: &Arc<Mutex<Option<Storage>>>, key: &str) -> bool {
    match storage.lock() {
        Ok(guard) => match guard.as_ref().map(|s| s.get_app_state(key)) {
            Some(Ok(Some(v))) => v != "0" && !v.eq_ignore_ascii_case("false"),
            Some(Err(e)) => {
                warn!("Could not read {}: {}", key, e);
                true
            }
            _ => true,
        },
        Err(_) => true,
    }
}

fn read_json<T: serde::de::DeserializeOwned>(
    storage: &Arc<Mutex<Option<Storage>>>,
    key: &str,
) -> Option<T> {
    let guard = storage.lock().ok()?;
    let raw = guard.as_ref()?.get_app_state(key).ok()??;
    serde_json::from_str(&raw).ok()
}

fn write_json<T: serde::Serialize>(storage: &Arc<Mutex<Option<Storage>>>, key: &str, value: &T) {
    let Ok(json) = serde_json::to_string(value) else {
        return;
    };
    if let Ok(guard) = storage.lock() {
        if let Some(s) = guard.as_ref() {
            // A blocked write here means the mode changed under us; the next
            // idle tick records it instead.
            if let Err(e) = s.set_app_state(key, &json) {
                debug!("Could not persist {}: {}", key, e);
            }
        }
    }
}
