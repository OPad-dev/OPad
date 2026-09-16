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

use osupad_model::RuntimeMode;
use osupad_storage::Storage;
use osupad_tosu::TosuSupervisor;
use osupad_update::client::UpdateClient;
use osupad_update::manifest::current_target;
use osupad_update::tosu::{self, TosuAction};
use osupad_update::{may_update_now, CheckSchedule, InstallOrigin, ReleaseManifest, UpdateError};

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
    pub tosu_installed: Option<String>,
    pub tosu_available: Option<String>,
    pub app_available: Option<String>,
    pub last_check: Option<SystemTime>,
    pub last_error: Option<String>,
    /// Set when a newer version exists but this install must not touch its own
    /// files — an AUR install, or one with no origin marker (§U-2a)
    pub notify_only: bool,
}

pub type SharedUpdateStatus = Arc<Mutex<UpdateStatus>>;

pub fn spawn_update_worker(
    daemon_state: Arc<Mutex<DaemonState>>,
    storage: Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: TosuSupervisor,
    status: SharedUpdateStatus,
) {
    tokio::spawn(async move {
        // Spread the first check out a little: a machine that just booted is
        // busy, and every install checking at login is the pattern §U-0.6
        // warns about.
        tokio::time::sleep(Duration::from_secs(90)).await;

        loop {
            if let Err(e) = tick(&daemon_state, &storage, &tosu_supervisor, &status).await {
                warn!("Update check failed: {}", e);
                if let Ok(mut s) = status.lock() {
                    s.last_error = Some(e.to_string());
                }
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

async fn tick(
    daemon_state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    tosu_supervisor: &TosuSupervisor,
    status: &SharedUpdateStatus,
) -> Result<(), UpdateError> {
    let mode = daemon_state
        .lock()
        .map(|s| s.mode)
        .unwrap_or(RuntimeMode::Playing);

    // Not idle: change nothing, write nothing, do not even look.
    if let Err(reason) = may_update_now(mode, true) {
        debug!("Skipping the update check: {}", reason);
        return Ok(());
    }

    let mut schedule: CheckSchedule = read_json(storage, SCHEDULE_KEY).unwrap_or_default();
    let now = SystemTime::now();
    if !schedule.due_at(now) {
        return Ok(());
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
        return Ok(());
    };

    update_tosu(
        &client,
        &manifest,
        daemon_state,
        storage,
        tosu_supervisor,
        status,
    )
    .await
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
        s.tosu_installed = installed.clone();
        s.tosu_available = manifest
            .component(osupad_update::TOSU)
            .map(|c| c.version.clone());
        s.notify_only = matches!(action, TosuAction::NotifyOnly { .. });
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
                .component(osupad_update::TOSU)
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
