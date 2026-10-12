//! The update worker (docs/specs/release-and-updates.md §6, §8, §9).
//!
//! One task, woken on a slow tick, that does nothing at all unless the daemon
//! is IDLE. That is deliberate and is the §U-0.1 rule in its strongest form:
//! not merely "do not apply an update mid-map" but "do not even record that a
//! check happened", because recording it is a storage write and P1-3 forbids
//! those during PLAYING and COOLDOWN.
//!
//! It follows one channel's feed (stable or beta), sees it through this
//! install's gradual-rollout bucket, updates tosu on its own when allowed, and
//! applies an app update only when a person asks. How the last install went is
//! kept in storage, so an install the daemon did not live to finish (the
//! Windows installer stops it) is reported correctly after the restart.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, warn};

use opad_ipc::{ComponentUpdate, UpdateComponent};
use opad_storage::Storage;
use opad_tosu::TosuSupervisor;
use opad_update::app::{self, AppAction};
use opad_update::client::UpdateClient;
use opad_update::manifest::current_target;
use opad_update::tosu::{self, TosuAction};
use opad_update::{
    may_update_now, rollout, ApplyPolicy, CheckSchedule, InstallOrigin, ReleaseManifest,
    UpdateError, BETA, STABLE,
};

use crate::runtime::DaemonState;

/// How often the worker wakes to ask whether a check is due. The check itself
/// is daily (§U-0.6); this only decides how soon after becoming IDLE — or
/// after a laptop wakes up — the daily check actually runs.
const TICK: Duration = Duration::from_secs(15 * 60);

/// *Check now* runs at most this often, whatever the GUI sends
const MANUAL_CHECK_MIN: Duration = Duration::from_secs(60);

pub const SCHEDULE_KEY: &str = "update.schedule";
pub const TOSU_ENABLED_KEY: &str = "update.tosu.enabled";
pub const APP_ENABLED_KEY: &str = "update.app.enabled";
/// `stable` (default) or `beta`
pub const CHANNEL_KEY: &str = "update.channel";
/// How the last install went ([`UpdateResult`])
pub const LAST_RESULT_KEY: &str = "update.last_result";
/// Written just before an install is applied, removed once it has returned.
/// Still there at startup means the daemon did not see the install finish.
pub const IN_PROGRESS_KEY: &str = "update.in_progress";

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
    pub channel: String,
    /// A check is running right now
    pub checking: bool,
    /// How the last install went, as a sentence
    pub last_result: Option<String>,
}

pub type SharedUpdateStatus = Arc<Mutex<UpdateStatus>>;

#[derive(Debug, Clone, Copy)]
pub enum UpdateCommand {
    /// A person pressed Install. The daemon never applies an app update on
    /// its own (§U-2), so this is the only way one is applied.
    Install(UpdateComponent),
    /// *Check now*
    Check,
    /// The channel setting changed: drop the old channel's feed and check
    ChannelChanged,
}

/// The last feed that passed verification, as this install sees it (its
/// rollout bucket applied).
///
/// Shared rather than kept inside the worker because the firmware flash is
/// driven from an IPC handler, and it must use a feed this daemon actually
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
        self.status.lock().clone()
    }

    /// The verified feed, or `None` if no check has succeeded yet.
    pub fn manifest(&self) -> Option<ReleaseManifest> {
        self.manifest.lock().clone()
    }

    pub fn request_install(&self, component: UpdateComponent) -> Result<(), String> {
        self.send(UpdateCommand::Install(component))
    }

    pub fn request_check(&self) -> Result<(), String> {
        self.send(UpdateCommand::Check)
    }

    /// Call after writing a new [`CHANNEL_KEY`]
    pub fn channel_changed(&self) -> Result<(), String> {
        self.send(UpdateCommand::ChannelChanged)
    }

    fn send(&self, command: UpdateCommand) -> Result<(), String> {
        self.commands
            .send(command)
            .map_err(|_| "The update worker is not running".to_string())
    }
}

/// One install's outcome, kept in storage under [`LAST_RESULT_KEY`]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateResult {
    pub component: String,
    pub from: String,
    pub to: String,
    pub ok: bool,
    pub message: String,
    /// RFC 3339
    pub at: String,
}

impl UpdateResult {
    fn new(component: &str, from: &str, to: &str, ok: bool, message: impl Into<String>) -> Self {
        Self {
            component: component.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            ok,
            message: message.into(),
            at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        }
    }

    pub fn sentence(&self) -> String {
        let name = match self.component.as_str() {
            "app" => "OPad",
            "tosu" => "tosu",
            other => other,
        };
        if self.ok {
            format!(
                "{name} updated from {} to {} ({})",
                self.from, self.to, self.at
            )
        } else {
            format!(
                "{name} update from {} to {} failed: {} ({})",
                self.from, self.to, self.message, self.at
            )
        }
    }
}

/// The install the daemon was applying, kept under [`IN_PROGRESS_KEY`]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InProgress {
    pub component: String,
    pub from: String,
    pub to: String,
}

/// What an install still marked in progress at startup means. The Windows
/// installer stops the daemon on purpose, so a missing "finished" is not
/// itself a failure: what decides it is the version now running.
pub fn resolve_interrupted(in_progress: &InProgress, running_version: &str) -> UpdateResult {
    if in_progress.component == "app" && running_version == in_progress.to {
        UpdateResult::new("app", &in_progress.from, &in_progress.to, true, "")
    } else {
        UpdateResult::new(
            &in_progress.component,
            &in_progress.from,
            &in_progress.to,
            false,
            "it was interrupted before it finished; the previous version is still installed",
        )
    }
}

pub fn spawn_update_worker(
    daemon_state: Arc<Mutex<DaemonState>>,
    storage: Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: TosuSupervisor,
) -> UpdateService {
    let status: SharedUpdateStatus = Arc::new(Mutex::new(UpdateStatus {
        channel: read_channel(&storage),
        app: ComponentUpdate {
            installed: Some(env!("CARGO_PKG_VERSION").to_string()),
            enabled: read_flag(&storage, APP_ENABLED_KEY),
            ..Default::default()
        },
        ..Default::default()
    }));
    let shared_manifest: SharedManifest = Arc::new(Mutex::new(None));
    let (commands, mut command_rx) = tokio::sync::mpsc::unbounded_channel();
    let service = UpdateService {
        status: status.clone(),
        manifest: shared_manifest.clone(),
        commands,
    };

    recover_interrupted(&storage, &status);

    tokio::spawn(async move {
        // Spread the first check out a little: a machine that just booted is
        // busy, and every install checking at login is the pattern §U-0.6
        // warns about.
        tokio::time::sleep(Duration::from_secs(90)).await;
        let mut manifest: Option<ReleaseManifest> = None;
        let mut last_manual: Option<Instant> = None;

        // An interval, not a sleep inside the loop: `tick()` fires immediately
        // the first time, so the first check happens at the 90 s mark above.
        let mut ticker = tokio::time::interval(TICK);

        loop {
            let force = tokio::select! {
                _ = ticker.tick() => false,
                Some(command) = command_rx.recv() => match command {
                    UpdateCommand::Check => {
                        if last_manual.is_some_and(|t| t.elapsed() < MANUAL_CHECK_MIN) {
                            debug!("Check now ignored: the last one was under a minute ago");
                            continue;
                        }
                        last_manual = Some(Instant::now());
                        true
                    }
                    UpdateCommand::ChannelChanged => {
                        let channel = read_channel(&storage);
                        info!("Update channel is now {}", channel);
                        status.lock().channel = channel;
                        // The old channel's feed says nothing about this one
                        manifest = None;
                        shared_manifest.lock().take();
                        true
                    }
                    UpdateCommand::Install(component) => {
                        let result = match component {
                            UpdateComponent::App => {
                                install_app(manifest.as_ref(), &daemon_state, &storage, &status).await
                            }
                            UpdateComponent::Tosu => match &manifest {
                                Some(m) => {
                                    update_tosu(m, &daemon_state, &storage, &tosu_supervisor, &status, true).await
                                }
                                None => Err(UpdateError::Http(
                                    "No verified update feed yet; check for updates first".to_string(),
                                )),
                            },
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
                            status.lock().last_error = Some(e.to_string());
                        }
                        continue;
                    }
                },
            };

            status.lock().checking = true;
            let have_cached = manifest.is_some();
            let checked = check(
                &daemon_state,
                &storage,
                &tosu_supervisor,
                &status,
                have_cached,
                force,
            )
            .await;
            status.lock().checking = false;
            match checked {
                Ok(Some(m)) => {
                    shared_manifest.lock().replace(m.clone());
                    manifest = Some(m);
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("Update check failed: {}", e);
                    status.lock().last_error = Some(e.to_string());
                }
            }
        }
    });

    service
}

/// Turns an install still marked in progress into a result, and clears the
/// staging directory of anything it left behind.
fn recover_interrupted(storage: &Arc<Mutex<Option<Storage>>>, status: &SharedUpdateStatus) {
    let stored: Option<UpdateResult> = read_json(storage, LAST_RESULT_KEY);
    if let Some(in_progress) = read_json::<InProgress>(storage, IN_PROGRESS_KEY) {
        let result = resolve_interrupted(&in_progress, env!("CARGO_PKG_VERSION"));
        info!("{}", result.sentence());
        write_json(storage, LAST_RESULT_KEY, &result);
        remove_key(storage, IN_PROGRESS_KEY);
        status.lock().last_result = Some(result.sentence());
        if let Ok(dir) = opad_model::paths::state_dir() {
            let _ = std::fs::remove_dir_all(dir.join("updates"));
        }
    } else if let Some(result) = stored {
        status.lock().last_result = Some(result.sentence());
    }
}

/// Returns the feed when a fresh one was fetched, so the worker can keep it
/// for a later Install command without re-fetching and re-verifying.
/// `have_cached` is whether the worker already holds one; without it the
/// check is unconditional (the stored ETag survives a restart, the feed does
/// not). `force` is *Check now*: it skips the daily schedule, never the
/// IDLE rule.
async fn check(
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: &TosuSupervisor,
    status: &SharedUpdateStatus,
    have_cached: bool,
    force: bool,
) -> Result<Option<ReleaseManifest>, UpdateError> {
    let mode = daemon_state.lock().mode;

    // Not idle: change nothing, write nothing, do not even look.
    if let Err(reason) = may_update_now(mode, true) {
        debug!("Skipping the update check: {}", reason);
        return Ok(None);
    }

    let channel = read_channel(storage);
    let mut schedule: CheckSchedule = read_json(storage, SCHEDULE_KEY).unwrap_or_default();
    schedule.switch_channel(&channel);
    let now = SystemTime::now();
    if !force && !schedule.due_at(now) {
        return Ok(None);
    }

    let client = UpdateClient::new(&channel)?;
    let fetched = client.fetch_manifest(&mut schedule, now, have_cached).await;
    // The schedule is recorded whatever happened, so a failure backs off
    // instead of retrying on every tick.
    write_json(storage, SCHEDULE_KEY, &schedule);
    {
        let mut s = status.lock();
        s.last_check = schedule.last_check;
        s.last_error = schedule.last_error.clone();
    }

    let Some(feed) = fetched? else {
        debug!("Update feed unchanged since the last check");
        return Ok(None);
    };

    // What this install is offered: components still rolling out to others
    // are left out (§9.2)
    let install_id = daemon_state.lock().install_id.clone();
    let feed = rollout::for_install(&feed, install_id.as_deref());

    // tosu updates itself; the app only reports, and waits to be told (§U-2).
    if let Err(e) = update_tosu(&feed, daemon_state, storage, tosu_supervisor, status, false).await
    {
        warn!("tosu update failed: {}", e);
        status.lock().last_error = Some(e.to_string());
    }

    // Reported, not propagated — exactly like the tosu arm above. A feed that
    // carries no app artifact for this platform (an unsupported arch, or a
    // release that shipped one component late) must not throw the whole feed
    // away: the firmware offer is read from it too.
    if let Err(e) = check_app(&feed, daemon_state, storage, status) {
        warn!("App update check failed: {}", e);
        status.lock().last_error = Some(e.to_string());
    }
    Ok(Some(feed))
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
    let mode = daemon_state.lock().mode;

    let action = app::plan(
        manifest,
        installed,
        origin,
        current_target(),
        may_update_now(mode, enabled),
    )?;
    let notes_url = manifest
        .component(opad_update::APP)
        .and_then(|c| c.notes_url.clone());

    {
        let mut s = status.lock();
        s.app.installed = Some(installed.to_string());
        s.app.enabled = enabled;
        s.app.notes_url = None;
        match &action {
            AppAction::UpToDate { .. } | AppAction::Defer { .. } => {
                s.app.available = None;
                s.app.ready_to_install = false;
                s.app.notify_only = false;
            }
            AppAction::NotifyOnly { available, notes } => {
                s.app.available = Some(available.clone());
                s.app.notes = notes.clone();
                s.app.notes_url = notes_url;
                s.app.notify_only = true;
                s.app.ready_to_install = false;
            }
            AppAction::Available {
                available, notes, ..
            } => {
                s.app.available = Some(available.clone());
                s.app.notes = notes.clone();
                s.app.notes_url = notes_url;
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
        UpdateError::Http("No verified update feed yet; check for updates first".to_string())
    })?;

    let mode = daemon_state.lock().mode;
    let origin = InstallOrigin::detect();
    let installed = env!("CARGO_PKG_VERSION");
    let action = app::plan(
        manifest,
        installed,
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

    // From our apt/dnf repository: the package manager fetches and checks
    // the package against the repository key; nothing is downloaded here
    if let ApplyPolicy::PackageManager(pm) = policy {
        if app::repo_configured(pm) {
            return upgrade_from_repo(pm, installed, &available, storage, status).await;
        }
    }

    let client = UpdateClient::for_artifacts()?;
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

    // Marked before applying: the Windows installer stops this daemon on
    // purpose, and the next one reads this to say how it went
    write_json(
        storage,
        IN_PROGRESS_KEY,
        &InProgress {
            component: "app".to_string(),
            from: installed.to_string(),
            to: available.clone(),
        },
    );

    // Blocking, and a package manager can sit on a polkit prompt for minutes:
    // off the runtime's worker threads
    let applied = {
        let target = target.clone();
        tokio::task::spawn_blocking(move || apply_downloaded(policy, origin, &target))
            .await
            .map_err(|e| UpdateError::Io(std::io::Error::other(format!("apply step: {e}"))))
            .and_then(|r| r)
    };

    // A .deb or an installer is worth tens of megabytes and has done its job.
    // Leaving it behind would put it on §W2-3's list of things to clean up for
    // no reason. Direct replacement consumes the file itself, so a missing one
    // is not an error.
    let _ = std::fs::remove_file(&target);

    let result = match &applied {
        Ok(()) => UpdateResult::new("app", installed, &available, true, ""),
        Err(e) => UpdateResult::new("app", installed, &available, false, e.to_string()),
    };
    write_json(storage, LAST_RESULT_KEY, &result);
    remove_key(storage, IN_PROGRESS_KEY);
    status.lock().last_result = Some(result.sentence());
    applied?;

    {
        let mut s = status.lock();
        s.restart_required = true;
        s.app.ready_to_install = false;
        s.app.installed = Some(available.clone());
    }
    info!("OPad {} applied; a restart is needed to run it", available);
    Ok(())
}

/// Upgrades OPad from our repository (docs/specs/release-and-updates.md §6.1)
async fn upgrade_from_repo(
    pm: opad_update::PackageManager,
    installed: &str,
    available: &str,
    storage: &Arc<Mutex<Option<Storage>>>,
    status: &SharedUpdateStatus,
) -> Result<(), UpdateError> {
    let commands = app::repo_upgrade_commands(pm, in_path("pkcon"));
    info!("Upgrading OPad {} from the {:?} repository", available, pm);
    write_json(
        storage,
        IN_PROGRESS_KEY,
        &InProgress {
            component: "app".to_string(),
            from: installed.to_string(),
            to: available.to_string(),
        },
    );
    let applied = tokio::task::spawn_blocking(move || {
        use std::process::Command;
        for cmd in &commands {
            let status = Command::new(&cmd[0])
                .args(&cmd[1..])
                .status()
                .map_err(|e| UpdateError::Io(std::io::Error::other(format!("{}: {e}", cmd[0]))))?;
            if !status.success() {
                // A cancelled password prompt lands here; nothing changed
                return Err(UpdateError::Http(format!(
                    "{} exited with {:?}; the running version is unchanged",
                    cmd.join(" "),
                    status.code()
                )));
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| UpdateError::Io(std::io::Error::other(format!("apply step: {e}"))))
    .and_then(|r| r);

    let result = match &applied {
        Ok(()) => UpdateResult::new("app", installed, available, true, ""),
        Err(e) => UpdateResult::new("app", installed, available, false, e.to_string()),
    };
    write_json(storage, LAST_RESULT_KEY, &result);
    remove_key(storage, IN_PROGRESS_KEY);
    status.lock().last_result = Some(result.sentence());
    applied?;
    {
        let mut s = status.lock();
        s.restart_required = true;
        s.app.ready_to_install = false;
        s.app.installed = Some(available.to_string());
    }
    Ok(())
}

/// Whether `program` is on `$PATH`
fn in_path(program: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
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
            replace_file(file, &current)?;
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

/// Moves `staged` over `target`. The staged file sits in the state directory,
/// and `$APPIMAGE` can be on another filesystem (`/opt`, a second drive),
/// where rename fails with EXDEV. Then the bytes are copied to a temporary
/// file beside the target, made executable, synced and renamed there, so
/// the target is still replaced in one step. `staged` is only read here; the
/// caller removes it once this has returned.
fn replace_file(staged: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(staged, target) {
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            debug!(
                "{} is on another filesystem; copying the update into place",
                target.display()
            );
            copy_replace(staged, target)
        }
        other => other,
    }
}

fn copy_replace(staged: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    let dir = target.parent().ok_or_else(|| {
        std::io::Error::other(format!("{} has no parent directory", target.display()))
    })?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "opad-update".to_string());
    let tmp = dir.join(format!(".{name}.incoming"));
    let result = (|| {
        std::fs::copy(staged, &tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        }
        // Opened for writing: Windows refuses FlushFileBuffers on a read-only handle
        std::fs::OpenOptions::new()
            .write(true)
            .open(&tmp)?
            .sync_all()?;
        std::fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Updates the bundled tosu when the feed has a newer build. `force` is the
/// GUI's *Update now*: it runs even with automatic tosu updates switched off,
/// but never during a map.
async fn update_tosu(
    manifest: &ReleaseManifest,
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    supervisor: &TosuSupervisor,
    status: &SharedUpdateStatus,
    force: bool,
) -> Result<(), UpdateError> {
    let enabled = read_flag(storage, TOSU_ENABLED_KEY);
    let bundled_dir = tosu::bundled_dir()?;
    let installed = tosu::installed_version(&bundled_dir);
    let installed_revision = tosu::installed_revision(&bundled_dir);
    let origin = InstallOrigin::detect();

    // Re-read the mode: fetching the feed took time, and the user may have
    // started a map while it was in flight.
    let mode = daemon_state.lock().mode;

    let action = tosu::plan(
        manifest,
        installed.as_deref(),
        installed_revision,
        origin,
        current_target(),
        may_update_now(mode, enabled || force),
    )?;

    {
        let mut s = status.lock();
        s.tosu.installed = installed.clone();
        s.tosu.available = match &action {
            TosuAction::UpToDate { .. } => None,
            _ => manifest
                .component(opad_update::TOSU)
                .map(|c| c.version.clone()),
        };
        s.tosu.notes_url = manifest
            .component(opad_update::TOSU)
            .and_then(|c| c.notes_url.clone());
        s.tosu.enabled = enabled;
        s.tosu.notify_only = matches!(action, TosuAction::NotifyOnly { .. });
        s.tosu.ready_to_install = matches!(
            action,
            TosuAction::Install { .. } | TosuAction::Defer { .. }
        ) && !s.tosu.notify_only;
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
        TosuAction::Install {
            from,
            to,
            revision,
            artifact,
        } => {
            info!(
                "Updating the bundled tosu from {} to {} (r{})",
                from, to, revision
            );
            let client = UpdateClient::for_artifacts()?;
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
                revision,
                upstream_tag.as_deref(),
            );
            supervisor.resume();

            let shown_from = if from.is_empty() {
                "an unrecorded version"
            } else {
                from.as_str()
            };
            match result {
                Ok(()) => {
                    info!("Bundled tosu updated to {}", to);
                    let done = UpdateResult::new("tosu", shown_from, &to, true, "");
                    write_json(storage, LAST_RESULT_KEY, &done);
                    let mut s = status.lock();
                    s.last_result = Some(done.sentence());
                    s.tosu.installed = Some(to);
                    s.tosu.available = None;
                    s.tosu.ready_to_install = false;
                }
                Err(e) => {
                    // The previous binary is untouched; say so and try again
                    // on the next cycle rather than leaving a broken install.
                    warn!(
                        "tosu update to {} failed, keeping the current binary: {}",
                        to, e
                    );
                    let failed = UpdateResult::new("tosu", shown_from, &to, false, e.to_string());
                    write_json(storage, LAST_RESULT_KEY, &failed);
                    {
                        let mut s = status.lock();
                        s.last_result = Some(failed.sentence());
                        s.last_error = Some(e.to_string());
                    }
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// The update channel this install follows; anything unrecognised is stable
pub fn read_channel(storage: &Arc<Mutex<Option<Storage>>>) -> String {
    let guard = storage.lock();
    match guard.as_ref().map(|s| s.get_app_state(CHANNEL_KEY)) {
        Some(Ok(Some(v))) if v == BETA => BETA.to_string(),
        _ => STABLE.to_string(),
    }
}

fn remove_key(storage: &Arc<Mutex<Option<Storage>>>, key: &str) {
    let guard = storage.lock();
    if let Some(s) = guard.as_ref() {
        if let Err(e) = s.delete_app_state(key) {
            debug!("Could not remove {}: {}", key, e);
        }
    }
}

/// Reads a boolean setting, defaulting to enabled. A storage failure must not
/// silently switch an updater off — or on — so the default is the documented
/// one and the error is visible in the log.
fn read_flag(storage: &Arc<Mutex<Option<Storage>>>, key: &str) -> bool {
    let guard = storage.lock();
    match guard.as_ref().map(|s| s.get_app_state(key)) {
        Some(Ok(Some(v))) => v != "0" && !v.eq_ignore_ascii_case("false"),
        Some(Err(e)) => {
            warn!("Could not read {}: {}", key, e);
            true
        }
        _ => true,
    }
}

fn read_json<T: serde::de::DeserializeOwned>(
    storage: &Arc<Mutex<Option<Storage>>>,
    key: &str,
) -> Option<T> {
    let guard = storage.lock();
    let raw = guard.as_ref()?.get_app_state(key).ok()??;
    serde_json::from_str(&raw).ok()
}

fn write_json<T: serde::Serialize>(storage: &Arc<Mutex<Option<Storage>>>, key: &str, value: &T) {
    let Ok(json) = serde_json::to_string(value) else {
        return;
    };
    {
        let guard = storage.lock();
        if let Some(s) = guard.as_ref() {
            // A blocked write here means the mode changed under us; the next
            // idle tick records it instead.
            if let Err(e) = s.set_app_state(key, &json) {
                debug!("Could not persist {}: {}", key, e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_replace, replace_file, resolve_interrupted, InProgress};

    fn in_progress(component: &str) -> InProgress {
        InProgress {
            component: component.to_string(),
            from: "1.0.0".to_string(),
            to: "1.1.0".to_string(),
        }
    }

    #[test]
    fn an_app_install_the_new_version_survived_is_a_success() {
        // The Windows installer stops the daemon before it can record anything
        let r = resolve_interrupted(&in_progress("app"), "1.1.0");
        assert!(r.ok, "{r:?}");
        assert!(r.sentence().contains("updated from 1.0.0 to 1.1.0"));
    }

    #[test]
    fn an_install_that_left_the_old_version_running_was_interrupted() {
        let r = resolve_interrupted(&in_progress("app"), "1.0.0");
        assert!(!r.ok);
        assert!(r.sentence().contains("interrupted"), "{}", r.sentence());
        assert!(!resolve_interrupted(&in_progress("tosu"), "1.1.0").ok);
    }

    #[test]
    fn a_staged_update_replaces_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let staged = dir.path().join("staged.AppImage");
        let target = dir.path().join("opad.AppImage");
        std::fs::write(&staged, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();
        replace_file(&staged, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
    }

    /// The EXDEV fallback: rename cannot be made to cross a filesystem in a
    /// test, so the copy path is exercised directly.
    #[test]
    fn the_cross_filesystem_fallback_copies_and_keeps_the_staged_file() {
        let staging = tempfile::tempdir().unwrap();
        let install = tempfile::tempdir().unwrap();
        let staged = staging.path().join("opad-x86_64.AppImage");
        let target = install.path().join("OPad.AppImage");
        std::fs::write(&staged, b"new").unwrap();
        std::fs::write(&target, b"old").unwrap();

        copy_replace(&staged, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(staged.exists(), "the caller removes the staged file");
        assert_eq!(
            std::fs::read_dir(install.path()).unwrap().count(),
            1,
            "no temporary file is left beside the target"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "the AppImage must stay executable");
        }
    }

    #[test]
    fn a_failed_copy_leaves_the_old_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("OPad.AppImage");
        std::fs::write(&target, b"old").unwrap();
        assert!(copy_replace(&dir.path().join("missing"), &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"old");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
