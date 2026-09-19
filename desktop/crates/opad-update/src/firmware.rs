//! The firmware updater's decision layer (§U-3b).
//!
//! **The device is a keyboard. A firmware update is the only operation in this
//! entire project that can stop it being one.** Everything here follows from
//! that, and the shape is deliberately different from the other two updaters:
//!
//! - There is no automatic path at all. Not on by default, not opt-in. A
//!   person is shown what will happen and answers yes, every single time.
//! - The app partition is written and nothing else. `erase-flash` would take
//!   NVS with it, and NVS holds the lifetime counters and the owner record.
//! - Counters are synced to the host first, so the worst case is that the pad
//!   comes back blank and the host puts them back (`docs/recovery.md` §3).
//! - The image is checked twice before it goes anywhere near the pad: the
//!   SHA-256 from the signed manifest, and then the ESP32-S3 chip ID in the
//!   image header itself. A correctly-signed image for another chip is still
//!   an image that would not boot.
//!
//! What can still go wrong is honest and documented: interrupt the write and
//! the app partition is incomplete, so the pad does not run as a keyboard
//! until it is flashed again. It is not bricked — the ROM bootloader is in
//! mask ROM — but it needs manual recovery. That is the argument for U-3c.

use crate::manifest::{Artifact, ArtifactKind, ReleaseManifest, FIRMWARE};
use crate::version::is_newer;
use crate::{DeferReason, UpdateError};

/// The `target` firmware artifacts carry. Unlike the app and tosu, a firmware
/// image is for a chip, not for the host that flashes it: the same `.bin` is
/// written from Windows and from Linux.
pub const FIRMWARE_TARGET: &str = "esp32s3";

/// Roughly how long the pad is not a keyboard, for the consent prompt. Erase
/// plus write of a ~1 MB app image at 460800 baud, rounded up, plus the
/// re-enumeration. Deliberately pessimistic: a prompt that under-promises is
/// worse than one that over-promises.
pub const TYPICAL_OUTAGE_SECONDS: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirmwareAction {
    /// No pad connected, or one whose firmware version is not known yet.
    /// Nothing can be decided, and nothing is offered.
    Unknown,
    UpToDate {
        version: String,
    },
    Defer {
        reason: DeferReason,
    },
    /// There is a newer image and it could be applied. **Nothing happens until
    /// a person consents** — this is an offer, not a decision.
    Available {
        installed: String,
        available: String,
        notes: Option<String>,
        artifact: Artifact,
    },
}

/// Everything that must be true before a flash may start (§U-3b).
///
/// Separate from [`plan`] because "is there an update" and "may we apply one
/// right now" are different questions asked at different times: the first is
/// answered when the manifest arrives, the second at the moment a person
/// presses the button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocker {
    /// No pad on the USB bus
    NotConnected,
    /// Not IDLE. Flashing mid-map is the worst version of the P1-3 hazard.
    NotIdle(DeferReason),
    /// The host has not got the pad's current counters, so a pad that came
    /// back blank could not be restored
    CountersNotSynced,
    /// Nobody said yes
    NoConsent,
    /// The daemon has no database, so there is nowhere to sync counters to
    NoStorage,
}

impl std::fmt::Display for Blocker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Blocker::NotConnected => f.write_str("the pad is not connected"),
            Blocker::NotIdle(r) => write!(f, "the pad is busy: {r}"),
            Blocker::CountersNotSynced => f.write_str(
                "the lifetime counters have not been synced to this PC yet, and a firmware \
                 update must never be the reason they are lost",
            ),
            Blocker::NoConsent => {
                f.write_str("a firmware update needs explicit confirmation every time")
            }
            Blocker::NoStorage => f.write_str(
                "the database is unavailable, so the counters cannot be saved before flashing",
            ),
        }
    }
}

/// The facts the daemon knows at the moment someone presses Update.
#[derive(Debug, Clone, Copy)]
pub struct Preconditions {
    pub connected: bool,
    pub gate: Result<(), DeferReason>,
    pub counters_synced: bool,
    pub storage_available: bool,
    pub consented: bool,
}

/// Every reason a flash may not start, in the order a user should hear them.
pub fn blockers(p: Preconditions) -> Vec<Blocker> {
    let mut out = Vec::new();
    if !p.connected {
        out.push(Blocker::NotConnected);
    }
    if let Err(reason) = p.gate {
        out.push(Blocker::NotIdle(reason));
    }
    if !p.storage_available {
        out.push(Blocker::NoStorage);
    } else if !p.counters_synced {
        out.push(Blocker::CountersNotSynced);
    }
    if !p.consented {
        out.push(Blocker::NoConsent);
    }
    out
}

/// Is there a newer firmware image for this pad, and which artifact is it?
///
/// `installed` is what the pad reported in its `HelloAck`. `None` means no pad,
/// or firmware too old to say, and in both cases the answer is [`FirmwareAction::Unknown`]:
/// guessing that an unknown version is older would offer a downgrade as an
/// update.
pub fn plan(
    manifest: &ReleaseManifest,
    installed: Option<&str>,
    gate: Result<(), DeferReason>,
) -> Result<FirmwareAction, UpdateError> {
    let Some(installed) = installed.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(FirmwareAction::Unknown);
    };

    let Some(component) = manifest.component(FIRMWARE) else {
        // A release that ships no firmware is normal: most do not change it.
        return Ok(FirmwareAction::UpToDate {
            version: installed.to_string(),
        });
    };

    if !is_newer(&component.version, installed) {
        return Ok(FirmwareAction::UpToDate {
            version: installed.to_string(),
        });
    }

    if let Err(reason) = gate {
        return Ok(FirmwareAction::Defer { reason });
    }

    let artifact = manifest
        .artifact_for(FIRMWARE, FIRMWARE_TARGET, Some(ArtifactKind::Firmware))
        .ok_or_else(|| UpdateError::NoArtifact {
            component: FIRMWARE.to_string(),
            target: FIRMWARE_TARGET,
        })?;

    Ok(FirmwareAction::Available {
        installed: installed.to_string(),
        available: component.version.clone(),
        notes: component.notes.clone(),
        artifact: artifact.clone(),
    })
}

/// What the consent prompt says. Kept here rather than in the GUI so the CLI
/// and the GUI cannot drift into promising different things (§U-3b).
pub fn consent_text(installed: &str, available: &str) -> String {
    format!(
        "Update the pad's firmware from {installed} to {available}?\n\
         \n\
         Your pad will be unusable as a keyboard for about {TYPICAL_OUTAGE_SECONDS} seconds. \
         Do not unplug it.\n\
         \n\
         Only the app partition is written, so your lifetime counters are kept. If the update \
         is interrupted the pad will not work as a keyboard until it is flashed again — it is \
         not damaged, and docs/recovery.md explains how.\n\
         \n\
         Plug the pad straight into the PC rather than through a hub you might disturb."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Component;
    use std::collections::BTreeMap;

    fn manifest(version: &str, target: &str) -> ReleaseManifest {
        let mut components = BTreeMap::new();
        components.insert(
            FIRMWARE.to_string(),
            Component {
                version: version.to_string(),
                upstream_tag: None,
                notes: Some("Faster debounce".to_string()),
                artifacts: vec![Artifact {
                    target: target.to_string(),
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
    fn a_newer_image_is_offered_never_applied() {
        let action = plan(&manifest("1.1.0", FIRMWARE_TARGET), Some("1.0.0"), Ok(())).unwrap();
        match action {
            FirmwareAction::Available {
                installed,
                available,
                artifact,
                ..
            } => {
                assert_eq!((installed.as_str(), available.as_str()), ("1.0.0", "1.1.0"));
                assert_eq!(artifact.kind, ArtifactKind::Firmware);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_same_or_an_older_image_is_not_an_update() {
        for installed in ["1.1.0", "1.2.0"] {
            assert_eq!(
                plan(&manifest("1.1.0", FIRMWARE_TARGET), Some(installed), Ok(())).unwrap(),
                FirmwareAction::UpToDate {
                    version: installed.to_string()
                }
            );
        }
    }

    #[test]
    fn a_pad_that_does_not_report_its_version_is_never_offered_one() {
        // Treating "unknown" as "old" would push a downgrade, or flash a pad
        // we know nothing about.
        for installed in [None, Some(""), Some("   ")] {
            assert_eq!(
                plan(&manifest("1.1.0", FIRMWARE_TARGET), installed, Ok(())).unwrap(),
                FirmwareAction::Unknown
            );
        }
    }

    #[test]
    fn a_release_with_no_firmware_component_changes_nothing() {
        let m = ReleaseManifest {
            schema: 1,
            generated: "2026-09-17T00:00:00Z".to_string(),
            components: BTreeMap::new(),
        };
        assert_eq!(
            plan(&m, Some("1.0.0"), Ok(())).unwrap(),
            FirmwareAction::UpToDate {
                version: "1.0.0".to_string()
            }
        );
    }

    #[test]
    fn an_image_for_another_chip_is_not_silently_accepted() {
        // The manifest's target is the first of the two chip checks; the
        // image header is the second, in opad_device::flash.
        let err = plan(&manifest("1.1.0", "esp32c3"), Some("1.0.0"), Ok(())).unwrap_err();
        assert!(matches!(err, UpdateError::NoArtifact { .. }));
    }

    #[test]
    fn nothing_is_offered_while_a_map_is_running() {
        for reason in [DeferReason::Playing, DeferReason::Cooldown] {
            assert_eq!(
                plan(
                    &manifest("1.1.0", FIRMWARE_TARGET),
                    Some("1.0.0"),
                    Err(reason)
                )
                .unwrap(),
                FirmwareAction::Defer { reason }
            );
        }
    }

    fn ok_preconditions() -> Preconditions {
        Preconditions {
            connected: true,
            gate: Ok(()),
            counters_synced: true,
            storage_available: true,
            consented: true,
        }
    }

    #[test]
    fn everything_in_place_blocks_nothing() {
        assert!(blockers(ok_preconditions()).is_empty());
    }

    #[test]
    fn consent_is_required_every_single_time() {
        let p = Preconditions {
            consented: false,
            ..ok_preconditions()
        };
        assert_eq!(blockers(p), vec![Blocker::NoConsent]);
    }

    #[test]
    fn counters_must_be_on_the_host_before_the_pad_is_flashed() {
        let p = Preconditions {
            counters_synced: false,
            ..ok_preconditions()
        };
        assert_eq!(blockers(p), vec![Blocker::CountersNotSynced]);
    }

    #[test]
    fn no_database_means_no_flash_and_says_so_instead_of_blaming_the_sync() {
        let p = Preconditions {
            storage_available: false,
            counters_synced: false,
            ..ok_preconditions()
        };
        assert_eq!(blockers(p), vec![Blocker::NoStorage]);
    }

    #[test]
    fn a_running_map_blocks_a_flash() {
        for reason in [
            DeferReason::Playing,
            DeferReason::Cooldown,
            DeferReason::Syncing,
        ] {
            let p = Preconditions {
                gate: Err(reason),
                ..ok_preconditions()
            };
            assert_eq!(blockers(p), vec![Blocker::NotIdle(reason)]);
        }
    }

    #[test]
    fn a_disconnected_pad_blocks_a_flash() {
        let p = Preconditions {
            connected: false,
            ..ok_preconditions()
        };
        assert_eq!(blockers(p), vec![Blocker::NotConnected]);
    }

    #[test]
    fn every_blocker_reads_as_a_sentence() {
        for b in [
            Blocker::NotConnected,
            Blocker::NotIdle(DeferReason::Playing),
            Blocker::CountersNotSynced,
            Blocker::NoConsent,
            Blocker::NoStorage,
        ] {
            assert!(!b.to_string().is_empty());
        }
    }

    #[test]
    fn the_consent_text_says_what_it_costs_and_where_to_go_when_it_breaks() {
        let text = consent_text("1.0.0", "1.1.0");
        assert!(text.contains("1.0.0") && text.contains("1.1.0"));
        assert!(text.contains("Do not unplug"));
        assert!(text.contains("counters are kept"));
        assert!(text.contains("docs/recovery.md"));
    }
}
