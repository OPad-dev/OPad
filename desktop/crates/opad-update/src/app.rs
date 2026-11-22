//! The OPad app updater (§U-2).
//!
//! Every install checks and, where it may, downloads and installs. What
//! differs between platforms is only **how the update is applied**, and that
//! comes entirely from who owns the installed files (§U-2a):
//!
//! | origin | apply |
//! |---|---|
//! | Windows installer | the new installer `/SILENT /NORESTART` (per user, no admin) |
//! | `.deb`, `.rpm`, Arch `opad-bin` | the update helper, through pkexec: it checks the package against the signed feed and installs it with apt, dnf or pacman |
//! | `make install-user`, AppImage | replace the files directly |
//! | `make install` (source), no marker | notify only |
//!
//! **The thing that must not be done** is writing `/usr/bin/opad-daemon`
//! directly, even where a way could be found. That desynchronises the package
//! database: `dpkg -V` then reports modified files and the next `apt upgrade`
//! silently reverts the update. The helper goes through the package manager
//! for exactly that reason.
//!
//! With automatic updates on (the default) the daemon installs an update on
//! its own while no map is being played. The running OPad keeps running; the
//! new version is the one that starts next time.

use crate::manifest::{Artifact, ArtifactKind, ReleaseManifest, APP, PACMAN_TARGET};
use crate::origin::{ApplyPolicy, InstallOrigin, PackageManager};
use crate::version::is_newer;
use crate::{DeferReason, UpdateError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    UpToDate {
        version: String,
    },
    /// Newer version exists, but this install's files are not ours to change
    NotifyOnly {
        available: String,
        notes: Option<String>,
    },
    Defer {
        reason: DeferReason,
    },
    /// Ready to install: on its own with automatic updates on, otherwise when
    /// the user presses Install
    Available {
        installed: String,
        available: String,
        notes: Option<String>,
        artifact: Artifact,
        policy: ApplyPolicy,
    },
}

/// The artifact (target, kind) each origin installs, so the right one is
/// picked out of a release that carries a `.deb`, an `.rpm`, the Arch package
/// and an installer.
///
/// The two direct-replacement origins differ: a `make install-user` tree is
/// replaced from an archive, while an AppImage *is* one file and is replaced
/// by the new one. The Arch package has a target of its own
/// ([`PACMAN_TARGET`]).
pub fn wanted_artifact(
    policy: ApplyPolicy,
    origin: InstallOrigin,
    target: &str,
) -> Option<(String, ArtifactKind)> {
    let kind = match policy {
        ApplyPolicy::Installer => ArtifactKind::Installer,
        ApplyPolicy::Helper(PackageManager::Apt) => ArtifactKind::Deb,
        ApplyPolicy::Helper(PackageManager::Dnf) => ArtifactKind::Rpm,
        ApplyPolicy::Helper(PackageManager::Pacman) => {
            return Some((PACMAN_TARGET.to_string(), ArtifactKind::Archive))
        }
        ApplyPolicy::DirectReplace if origin == InstallOrigin::AppImage => ArtifactKind::Binary,
        ApplyPolicy::DirectReplace => ArtifactKind::Archive,
        ApplyPolicy::NotifyOnly => return None,
    };
    Some((target.to_string(), kind))
}

/// The whole decision, with no IO and no network, so every row of the table
/// above is testable.
pub fn plan(
    manifest: &ReleaseManifest,
    installed_version: &str,
    origin: InstallOrigin,
    target: &str,
    gate: Result<(), DeferReason>,
) -> Result<AppAction, UpdateError> {
    // A feed without the app is normal: this install may not be in a
    // gradual rollout yet (rollout::for_install drops it), or the feed is
    // for a channel that has no app release yet
    let Some(component) = manifest.component(APP) else {
        return Ok(AppAction::UpToDate {
            version: installed_version.to_string(),
        });
    };

    if !is_newer(&component.version, installed_version) {
        return Ok(AppAction::UpToDate {
            version: installed_version.to_string(),
        });
    }

    let policy = origin.apply_policy();
    // Checked before the gate: telling the user costs nothing and writes
    // nothing, so it is not something to defer.
    if policy == ApplyPolicy::NotifyOnly {
        return Ok(AppAction::NotifyOnly {
            available: component.version.clone(),
            notes: component.notes.clone(),
        });
    }

    if let Err(reason) = gate {
        return Ok(AppAction::Defer { reason });
    }

    let artifact = wanted_artifact(policy, origin, target)
        .and_then(|(target, kind)| manifest.artifact_for(APP, &target, Some(kind)))
        .ok_or_else(|| UpdateError::NoArtifact {
            component: APP.to_string(),
            target: "this platform and packaging",
        })?;

    Ok(AppAction::Available {
        installed: installed_version.to_string(),
        available: component.version.clone(),
        notes: component.notes.clone(),
        artifact: artifact.clone(),
        policy,
    })
}

/// The Windows installer's command line, as program plus arguments.
/// Returned rather than run so it can be asserted on in tests.
pub fn installer_command(file: &std::path::Path) -> Vec<String> {
    // Inno's CloseApplications stops the running processes for us, and
    // /RELAUNCH=1 has it start the GUI again afterwards (installer.iss)
    vec![
        file.display().to_string(),
        "/SILENT".into(),
        "/SUPPRESSMSGBOXES".into(),
        "/NORESTART".into(),
        "/RELAUNCH=1".into(),
    ]
}

/// Where the packages install the update helper, and the path their polkit
/// action names (`packaging/linux/polkit/dev.opad.update.policy`)
pub const HELPER_PATH: &str = "/usr/lib/opad/opad-update-helper";

/// The helper's command line: the downloaded package, and the signed feed it
/// is checked against. pkexec runs it as root without a password for the
/// user at the computer (the polkit action allows it); the helper trusts
/// none of these files until it has checked them itself.
pub fn helper_command(
    package: &std::path::Path,
    feed: &std::path::Path,
    signature: &std::path::Path,
) -> Vec<String> {
    vec![
        "pkexec".into(),
        HELPER_PATH.into(),
        "--feed".into(),
        feed.display().to_string(),
        "--signature".into(),
        signature.display().to_string(),
        "--package".into(),
        package.display().to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Component;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn manifest(version: &str) -> ReleaseManifest {
        let artifact = |kind, url: &str| Artifact {
            target: "linux-x86_64".to_string(),
            kind,
            url: url.to_string(),
            sha256: "00".to_string(),
            size: None,
            board: None,
        };
        let mut components = BTreeMap::new();
        components.insert(
            APP.to_string(),
            Component {
                version: version.to_string(),
                upstream_tag: None,
                notes: Some("Fixed the thing".to_string()),
                artifacts: vec![
                    artifact(ArtifactKind::Deb, "https://x/opad.deb"),
                    artifact(ArtifactKind::Rpm, "https://x/opad.rpm"),
                    artifact(ArtifactKind::Archive, "https://x/opad.tar.gz"),
                    Artifact {
                        target: PACMAN_TARGET.to_string(),
                        kind: ArtifactKind::Archive,
                        url: "https://x/opad-bin.pkg.tar.zst".to_string(),
                        sha256: "00".to_string(),
                        size: None,
                        board: None,
                    },
                    Artifact {
                        target: "windows-x86_64".to_string(),
                        kind: ArtifactKind::Installer,
                        url: "https://x/opad-setup.exe".to_string(),
                        sha256: "00".to_string(),
                        size: None,
                        board: None,
                    },
                ],
                ..Default::default()
            },
        );
        ReleaseManifest {
            schema: 1,
            generated: "2026-09-16T00:00:00Z".to_string(),
            components,
            ..Default::default()
        }
    }

    fn available(origin: InstallOrigin, target: &str) -> AppAction {
        plan(&manifest("1.1.0"), "1.0.0", origin, target, Ok(())).unwrap()
    }

    #[test]
    fn a_feed_without_the_app_is_up_to_date_not_an_error() {
        // The app may be held back by a gradual rollout (rollout::for_install)
        let m = ReleaseManifest::default();
        assert_eq!(
            plan(
                &m,
                "1.0.0",
                InstallOrigin::Windows,
                "windows-x86_64",
                Ok(())
            )
            .unwrap(),
            AppAction::UpToDate {
                version: "1.0.0".to_string()
            }
        );
    }

    #[test]
    fn the_same_or_an_older_version_is_not_an_update() {
        for installed in ["1.1.0", "1.2.0"] {
            let action = plan(
                &manifest("1.1.0"),
                installed,
                InstallOrigin::User,
                "linux-x86_64",
                Ok(()),
            )
            .unwrap();
            assert_eq!(
                action,
                AppAction::UpToDate {
                    version: installed.to_string()
                }
            );
        }
    }

    #[test]
    fn each_origin_picks_the_artifact_its_packaging_needs() {
        match available(InstallOrigin::Deb, "linux-x86_64") {
            AppAction::Available {
                artifact, policy, ..
            } => {
                assert_eq!(artifact.kind, ArtifactKind::Deb);
                assert_eq!(policy, ApplyPolicy::Helper(PackageManager::Apt));
            }
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::Rpm, "linux-x86_64") {
            AppAction::Available { artifact, .. } => assert_eq!(artifact.kind, ArtifactKind::Rpm),
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::User, "linux-x86_64") {
            AppAction::Available { artifact, .. } => {
                assert_eq!(artifact.kind, ArtifactKind::Archive);
                assert!(artifact.url.ends_with(".tar.gz"), "{}", artifact.url);
            }
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::Aur, "linux-x86_64") {
            AppAction::Available {
                artifact, policy, ..
            } => {
                assert!(artifact.url.ends_with(".pkg.tar.zst"), "{}", artifact.url);
                assert_eq!(policy, ApplyPolicy::Helper(PackageManager::Pacman));
            }
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::Windows, "windows-x86_64") {
            AppAction::Available { artifact, .. } => {
                assert_eq!(artifact.kind, ArtifactKind::Installer)
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_install_with_no_origin_marker_is_notify_only() {
        assert!(matches!(
            available(InstallOrigin::Unknown, "linux-x86_64"),
            AppAction::NotifyOnly { .. }
        ));
    }

    #[test]
    fn an_update_waits_while_a_map_is_running() {
        for reason in [DeferReason::Playing, DeferReason::Cooldown] {
            let action = plan(
                &manifest("1.1.0"),
                "1.0.0",
                InstallOrigin::Deb,
                "linux-x86_64",
                Err(reason),
            )
            .unwrap();
            assert_eq!(action, AppAction::Defer { reason });
        }
    }

    #[test]
    fn notifying_is_not_deferred_because_it_changes_nothing() {
        let action = plan(
            &manifest("1.1.0"),
            "1.0.0",
            InstallOrigin::Source,
            "linux-x86_64",
            Err(DeferReason::Playing),
        )
        .unwrap();
        assert!(matches!(action, AppAction::NotifyOnly { .. }));
    }

    #[test]
    fn packages_go_through_the_helper_never_around_it() {
        // Writing /usr/bin directly would make dpkg -V report modified files
        // and the next apt upgrade silently revert the update.
        let cmd = helper_command(
            Path::new("/s/opad.deb"),
            Path::new("/s/beta.json"),
            Path::new("/s/beta.json.minisig"),
        );
        assert_eq!(
            cmd,
            [
                "pkexec",
                HELPER_PATH,
                "--feed",
                "/s/beta.json",
                "--signature",
                "/s/beta.json.minisig",
                "--package",
                "/s/opad.deb"
            ]
        );
    }

    #[test]
    fn the_windows_installer_runs_silently_without_rebooting() {
        let cmd = installer_command(Path::new("opad-setup.exe"));
        assert_eq!(cmd[0], "opad-setup.exe");
        assert!(cmd.contains(&"/SILENT".to_string()));
        assert!(cmd.contains(&"/SUPPRESSMSGBOXES".to_string()));
        assert!(
            cmd.contains(&"/NORESTART".to_string()),
            "an update must never reboot the machine out from under a player"
        );
        assert!(
            cmd.contains(&"/RELAUNCH=1".to_string()),
            "a silent install skips the GUI's [Run] entry; the updater asks for it back"
        );
    }

    #[test]
    fn an_appimage_is_replaced_by_a_file_not_an_archive() {
        // An AppImage is one file; there is no tree to extract over.
        assert_eq!(
            wanted_artifact(
                ApplyPolicy::DirectReplace,
                InstallOrigin::AppImage,
                "linux-x86_64"
            ),
            Some(("linux-x86_64".to_string(), ArtifactKind::Binary))
        );
        assert_eq!(
            wanted_artifact(
                ApplyPolicy::DirectReplace,
                InstallOrigin::User,
                "linux-x86_64"
            ),
            Some(("linux-x86_64".to_string(), ArtifactKind::Archive))
        );
        // The Arch package's own target, which builds up to 1.0.0-rc.6 never
        // look at
        assert_eq!(
            wanted_artifact(
                ApplyPolicy::Helper(PackageManager::Pacman),
                InstallOrigin::Aur,
                "linux-x86_64"
            ),
            Some((PACMAN_TARGET.to_string(), ArtifactKind::Archive))
        );
    }

    #[test]
    fn a_missing_artifact_for_this_packaging_is_an_error_not_a_wrong_file() {
        // A Windows install must never be handed a .deb because that is what
        // the release happened to contain for another target.
        let err = plan(
            &manifest("1.1.0"),
            "1.0.0",
            InstallOrigin::Windows,
            "linux-x86_64",
            Ok(()),
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::NoArtifact { .. }));
    }
}
