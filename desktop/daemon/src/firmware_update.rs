//! Host-driven firmware flash over USB (§U-3b).
//!
//! The v1.0 mechanism, and deliberately the path that already exists rather
//! than new firmware attack surface: the daemon verifies an image against the
//! signed manifest, hands it to the same flashing engine `opadctl flash`
//! uses, and the pad comes back on `reset_to_app`.
//!
//! The order below is the safety argument, and none of it is optional:
//!
//! 1. Consent, connection and IDLE are checked before anything is downloaded.
//! 2. The counters are synced to this PC. If everything after this goes wrong,
//!    the host can still put them back (`docs/recovery.md` §3).
//! 3. The image is verified twice — SHA-256 from the signed manifest, then the
//!    ESP32-S3 chip ID in its own header — and the version is checked to be
//!    newer before either.
//! 4. The daemon releases the serial port. On Windows it is exclusive, so this
//!    is a precondition and not a courtesy (§W1-3).
//! 5. The **app partition only** is written. Never `erase-flash`: that is where
//!    the lifetime counters and the owner record live.
//! 6. The pad is rebooted into the app and has to come back reporting the
//!    version we just wrote. If it does not, that is said loudly.

use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use opad_device::flash::{self, APP_PARTITION_OFFSET};
use opad_ipc::FirmwareOffer;
use opad_model::DeviceInfo;
use opad_storage::Storage;
use opad_update::firmware::{
    blockers, consent_text, Blocker, FirmwareAction, Preconditions, TYPICAL_OUTAGE_SECONDS,
};
use opad_update::{may_update_now, ReleaseManifest, UpdateError};

use crate::runtime::DaemonState;
use crate::sync::DeviceLink;

/// How long to wait for the pad to come back as the app after the reboot.
/// Generous: a first boot after a flash also initialises NVS and the display.
const RECONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// What a firmware update would do right now, and everything in its way.
///
/// Reads state and writes nothing, so the GUI may poll it.
pub fn offer(
    manifest: Option<&ReleaseManifest>,
    state: &Arc<Mutex<DaemonState>>,
    storage_available: bool,
) -> FirmwareOffer {
    let (mode, device_info, connected) = {
        let st = state.lock();
        (st.mode, st.device_info.clone(), st.device_connected)
    };
    let installed = device_info.as_ref().map(|i| i.firmware_version.clone());

    let mut out = FirmwareOffer {
        installed: installed.clone(),
        running_partition: device_info
            .as_ref()
            .and_then(|i| i.running_partition.clone()),
        outage_seconds: TYPICAL_OUTAGE_SECONDS,
        ..Default::default()
    };

    // The blockers are worth showing even when there is nothing to install:
    // "your pad is not plugged in" explains an empty offer better than silence.
    out.blockers = blockers(Preconditions {
        connected,
        gate: may_update_now(mode, true),
        counters_synced: true,
        storage_available,
        // Consent is given with the request, not held as state, so it is never
        // one of the things this answer is waiting on.
        consented: true,
    })
    .iter()
    .map(|b| capitalise(&b.to_string()))
    .collect();

    let Some(manifest) = manifest else {
        return out;
    };
    let action = match opad_update::firmware::plan(
        manifest,
        installed.as_deref(),
        may_update_now(mode, true),
    ) {
        Ok(a) => a,
        Err(e) => {
            out.blockers.push(capitalise(&e.to_string()));
            return out;
        }
    };

    if let FirmwareAction::Available {
        installed,
        available,
        notes,
        ..
    } = action
    {
        out.consent_text = Some(consent_text(&installed, &available));
        out.available = Some(available);
        out.notes = notes;
    }
    out
}

#[derive(Debug)]
pub enum FirmwareUpdateOutcome {
    /// The pad came back as the version we wrote
    Installed {
        from: String,
        to: String,
        info: DeviceInfo,
    },
    /// Refused before anything was downloaded or written. The pad is untouched.
    Refused { reasons: Vec<String> },
}

#[derive(Debug, thiserror::Error)]
pub enum FirmwareUpdateError {
    #[error("{0}")]
    Update(#[from] UpdateError),
    #[error("The counters could not be synced to this PC, so nothing was flashed: {0}")]
    Sync(String),
    #[error("{0}")]
    Flash(#[from] flash::FlashError),
    #[error("The serial port could not be released for the flash, so nothing was written")]
    PortNotReleased,
    /// The dangerous one: the image is on the pad and the pad did not come back
    /// as expected. Never silently swallowed.
    #[error("The firmware was written, but the pad came back as {found}, not {expected}. See docs/recovery.md.")]
    WrongVersionBack { expected: String, found: String },
    #[error("The firmware was written, but the pad did not come back within {0:?}. See docs/recovery.md — it is not damaged, but it needs a manual flash.")]
    NoReconnect(Duration),
}

/// Flash the pad. `consented` is the person's answer and there is no default.
#[allow(clippy::too_many_arguments)]
pub async fn install<D: DeviceLink>(
    manifest: Option<&ReleaseManifest>,
    consented: bool,
    state: &Arc<Mutex<DaemonState>>,
    storage: &Arc<Mutex<Option<Storage>>>,
    device: &D,
    pending_ops: &Arc<Mutex<crate::runtime::PendingOperations>>,
) -> Result<FirmwareUpdateOutcome, FirmwareUpdateError> {
    let storage_available = storage.lock().is_some();
    let (mode, connected, installed) = {
        let st = state.lock();
        (
            st.mode,
            st.device_connected,
            st.device_info.as_ref().map(|i| i.firmware_version.clone()),
        )
    };

    // Everything cheap first. `counters_synced` is false here because the sync
    // has not happened yet — it is the one blocker this function is allowed to
    // clear itself, and it only does so once nothing else is in the way.
    let stopping = blockers(Preconditions {
        connected,
        gate: may_update_now(mode, true),
        counters_synced: false,
        storage_available,
        consented,
    });
    if stopping.iter().any(|b| *b != Blocker::CountersNotSynced) {
        return Ok(FirmwareUpdateOutcome::Refused {
            reasons: stopping
                .iter()
                .map(|b| capitalise(&b.to_string()))
                .collect(),
        });
    }

    let manifest = manifest.ok_or_else(|| {
        UpdateError::Http("No verified release manifest yet; check for updates first".to_string())
    })?;
    let action =
        opad_update::firmware::plan(manifest, installed.as_deref(), may_update_now(mode, true))?;
    let FirmwareAction::Available {
        installed,
        available,
        artifact,
        ..
    } = action
    else {
        return Ok(FirmwareUpdateOutcome::Refused {
            reasons: vec![format!(
                "There is no firmware update to install ({action:?})"
            )],
        });
    };

    // Step 2. Before a single byte is written to the pad, this PC has its
    // counters. A pad that comes back blank is then recoverable.
    info!("Syncing counters to this PC before flashing the pad");
    crate::sync::perform_sync(state, storage, device, pending_ops)
        .await
        .map_err(|e| FirmwareUpdateError::Sync(e.to_string()))?;

    // Step 3. Download, then verify the hash the signed manifest gave, then
    // verify the image is actually for this chip.
    let client = opad_update::client::UpdateClient::new()?;
    let bytes = client.fetch_artifact(&artifact).await?;
    let staging = opad_model::paths::state_dir()
        .map_err(|e| UpdateError::Io(std::io::Error::other(e.to_string())))?
        .join("updates");
    let target = staging.join("osupad-firmware.bin");
    // stage_bytes checks the SHA-256 and refuses to hand back a file that does
    // not match, so nothing unverified ever reaches the disk under this name.
    let staged =
        opad_update::download::stage_bytes(&target, &bytes, &artifact.sha256, &artifact.url)?;
    flash::check_esp32s3_image(&bytes).map_err(|e| {
        UpdateError::Http(format!(
            "The signed manifest's firmware image is not one this pad can run: {e}"
        ))
    })?;
    staged.install_to(&target)?;
    let result = flash_and_verify(&target, &installed, &available, state, device).await;
    // The image is a megabyte and has done its job either way; §W2-3 should
    // not inherit it as something to clean up.
    let _ = std::fs::remove_file(&target);
    result
}

async fn flash_and_verify<D: DeviceLink>(
    image: &std::path::Path,
    installed: &str,
    available: &str,
    state: &Arc<Mutex<DaemonState>>,
    device: &D,
) -> Result<FirmwareUpdateOutcome, FirmwareUpdateError> {
    // Step 4. Windows serial handles are exclusive, so this is a precondition
    // for the flash rather than politeness towards the device loop (§W1-3).
    info!("Releasing the serial port for the firmware flash");
    if !device.pause_and_release(Duration::from_secs(2)).await {
        device.resume();
        return Err(FirmwareUpdateError::PortNotReleased);
    }
    state.lock().device_connected = false;
    let app_port = device.port().or_else(opad_device::find_target_port);
    let device_id = state
        .lock()
        .device_info
        .as_ref()
        .map(|i| i.device_id.clone());

    // Step 5. The app partition and nothing else.
    let images = [(APP_PARTITION_OFFSET, image.to_path_buf())];
    let flashed = flash::flash(&images, app_port.as_deref(), device_id.as_deref(), &|m| {
        info!("{m}")
    })
    .await;

    // Step 6. Whatever happened, the device loop gets its port back: if the
    // write failed the pad may still be a working keyboard on the old image,
    // and refusing to talk to it again would help nobody.
    let mut events = device.subscribe();
    device.resume();
    flashed?;

    let reconnected = tokio::time::timeout(RECONNECT_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(opad_device::DeviceEvent::Connected(info, _)) => return Some(info),
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return None,
            }
        }
    })
    .await;

    let Ok(Some(info)) = reconnected else {
        warn!("The pad did not come back after the firmware flash");
        return Err(FirmwareUpdateError::NoReconnect(RECONNECT_TIMEOUT));
    };

    if info.firmware_version != available {
        warn!(
            "The pad came back running {} after being flashed with {}",
            info.firmware_version, available
        );
        return Err(FirmwareUpdateError::WrongVersionBack {
            expected: available.to_string(),
            found: info.firmware_version.clone(),
        });
    }

    info!(
        "Firmware updated from {} to {} (running from {})",
        installed,
        available,
        info.running_partition.as_deref().unwrap_or("unknown")
    );
    Ok(FirmwareUpdateOutcome::Installed {
        from: installed.to_string(),
        to: available.to_string(),
        info,
    })
}

/// These sentences are shown straight to a person, one per line.
fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opad_model::{CounterState, RuntimeMode};

    fn state_with(mode: RuntimeMode, connected: bool, firmware: &str) -> Arc<Mutex<DaemonState>> {
        let controller = crate::runtime::RuntimeController::new(
            opad_model::DeviceConfig::default(),
            Some(DeviceInfo {
                device_id: "OSUPAD-TEST".to_string(),
                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                firmware_version: firmware.to_string(),
                protocol_version: 1,
                running_partition: Some("ota_0".to_string()),
            }),
            CounterState::default(),
            Default::default(),
            None,
            Vec::new(),
            std::time::Instant::now(),
        );
        let mut st = controller.state.clone();
        st.mode = mode;
        st.device_connected = connected;
        Arc::new(Mutex::new(st))
    }

    fn manifest(version: &str) -> ReleaseManifest {
        use opad_update::manifest::{Artifact, ArtifactKind, Component};
        let mut components = std::collections::BTreeMap::new();
        components.insert(
            opad_update::FIRMWARE.to_string(),
            Component {
                version: version.to_string(),
                upstream_tag: None,
                notes: None,
                artifacts: vec![Artifact {
                    target: opad_update::firmware::FIRMWARE_TARGET.to_string(),
                    kind: ArtifactKind::Firmware,
                    url: "https://x/osupad-firmware.bin".to_string(),
                    sha256: "00".to_string(),
                    size: None,
                }],
            },
        );
        ReleaseManifest {
            schema: 1,
            generated: "2026-09-17T00:00:00Z".to_string(),
            components,
        }
    }

    #[test]
    fn an_offer_carries_the_consent_wording_and_the_running_slot() {
        let st = state_with(RuntimeMode::Idle, true, "1.0.0");
        let offer = offer(Some(&manifest("1.1.0")), &st, true);
        assert_eq!(offer.installed.as_deref(), Some("1.0.0"));
        assert_eq!(offer.available.as_deref(), Some("1.1.0"));
        assert_eq!(offer.running_partition.as_deref(), Some("ota_0"));
        assert!(offer.blockers.is_empty());
        let text = offer.consent_text.expect("an offer must say what it costs");
        assert!(text.contains("Do not unplug"));
        assert!(offer.outage_seconds > 0);
    }

    #[test]
    fn an_offer_made_mid_map_lists_that_as_the_blocker_and_offers_nothing() {
        let st = state_with(RuntimeMode::Playing, true, "1.0.0");
        let offer = offer(Some(&manifest("1.1.0")), &st, true);
        assert!(offer.available.is_none(), "nothing is offered mid-map");
        assert!(offer.consent_text.is_none());
        assert_eq!(offer.blockers.len(), 1);
        assert!(
            offer.blockers[0].contains("map ends"),
            "{:?}",
            offer.blockers
        );
    }

    #[test]
    fn an_offer_without_a_manifest_still_explains_itself() {
        let st = state_with(RuntimeMode::Idle, false, "1.0.0");
        let offer = offer(None, &st, true);
        assert!(offer.available.is_none());
        assert!(
            offer.blockers.iter().any(|b| b.contains("not connected")),
            "{:?}",
            offer.blockers
        );
    }

    #[test]
    fn an_up_to_date_pad_is_offered_nothing_but_is_not_an_error() {
        let st = state_with(RuntimeMode::Idle, true, "1.1.0");
        let offer = offer(Some(&manifest("1.1.0")), &st, true);
        assert!(offer.available.is_none());
        assert!(offer.consent_text.is_none());
        assert!(offer.blockers.is_empty());
    }

    #[test]
    fn blockers_are_shown_as_sentences_starting_with_a_capital() {
        let st = state_with(RuntimeMode::Cooldown, false, "1.0.0");
        let offer = offer(Some(&manifest("1.1.0")), &st, false);
        assert!(!offer.blockers.is_empty());
        for b in &offer.blockers {
            let first = b.chars().next().unwrap();
            assert!(first.is_uppercase(), "{b}");
        }
    }
}
