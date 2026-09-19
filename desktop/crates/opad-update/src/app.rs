//! The OPad app updater (§U-2).
//!
//! Every install checks and, where it may, downloads. What differs between
//! platforms is only **how the update is applied**, and that difference comes
//! entirely from who owns the installed files (§U-2a):
//!
//! | origin | apply |
//! |---|---|
//! | Windows installer | run the new signed installer `/SILENT /NORESTART` |
//! | `.deb` / `.rpm` | hand the file to apt/dnf through one `pkexec` prompt |
//! | AUR | notify only; pacman owns those files |
//! | `make install-user`, AppImage | replace the files directly |
//!
//! **The thing that must not be done** is writing `/usr/bin/opad-daemon`
//! directly, even where a way could be found. That desynchronises the package
//! database: `dpkg -V` then reports modified files and the next `apt upgrade`
//! silently reverts the update. The prompt is not a limitation to engineer
//! around — it is the OS working correctly.
//!
//! Applying is never automatic by default (§U-2). The daemon decides only that
//! an update *is available*; a person presses Install.

use crate::manifest::{Artifact, ArtifactKind, ReleaseManifest, APP};
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
    /// Ready to offer. Nothing happens until the user says so.
    Available {
        installed: String,
        available: String,
        notes: Option<String>,
        artifact: Artifact,
        policy: ApplyPolicy,
    },
}

/// The artifact kind each origin needs, so the right one is picked out of a
/// release that carries a `.deb`, an `.rpm` and an installer for one target.
///
/// The two direct-replacement origins differ: a `make install-user` tree is
/// replaced from an archive, while an AppImage *is* one file and is replaced
/// by the new one.
pub fn wanted_kind(policy: ApplyPolicy, origin: InstallOrigin) -> Option<ArtifactKind> {
    match policy {
        ApplyPolicy::Installer => Some(ArtifactKind::Installer),
        ApplyPolicy::PackageManager(PackageManager::Apt) => Some(ArtifactKind::Deb),
        ApplyPolicy::PackageManager(PackageManager::Dnf) => Some(ArtifactKind::Rpm),
        ApplyPolicy::DirectReplace if origin == InstallOrigin::AppImage => {
            Some(ArtifactKind::Binary)
        }
        ApplyPolicy::DirectReplace => Some(ArtifactKind::Archive),
        ApplyPolicy::NotifyOnly => None,
    }
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
    let component = manifest
        .component(APP)
        .ok_or_else(|| UpdateError::NoArtifact {
            component: APP.to_string(),
            target: "any",
        })?;

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

    let artifact = manifest
        .artifact_for(APP, target, wanted_kind(policy, origin))
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

/// The command that applies a downloaded artifact, as program plus arguments.
///
/// Returned rather than run so it can be asserted on in tests: these are the
/// commands §U-2 specifies, and getting one wrong is how a package database
/// ends up desynchronised.
pub fn apply_command(policy: ApplyPolicy, file: &std::path::Path) -> Option<Vec<String>> {
    let path = file.display().to_string();
    match policy {
        // Inno's CloseApplications stops the running processes for us
        ApplyPolicy::Installer => Some(vec![path, "/SILENT".into(), "/NORESTART".into()]),
        ApplyPolicy::PackageManager(PackageManager::Apt) => Some(vec![
            "pkexec".into(),
            "apt-get".into(),
            "install".into(),
            "-y".into(),
            path,
        ]),
        ApplyPolicy::PackageManager(PackageManager::Dnf) => Some(vec![
            "pkexec".into(),
            "dnf".into(),
            "install".into(),
            "-y".into(),
            path,
        ]),
        // Handled by extracting over the prefix, not by running anything
        ApplyPolicy::DirectReplace => None,
        ApplyPolicy::NotifyOnly => None,
    }
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
        };
        let mut components = BTreeMap::new();
        components.insert(
            APP.to_string(),
            Component {
                version: version.to_string(),
                upstream_tag: None,
                notes: Some("Fixed the thing".to_string()),
                artifacts: vec![
                    artifact(ArtifactKind::Deb, "https://x/osupad.deb"),
                    artifact(ArtifactKind::Rpm, "https://x/osupad.rpm"),
                    artifact(ArtifactKind::Archive, "https://x/osupad.tar.gz"),
                    Artifact {
                        target: "windows-x86_64".to_string(),
                        kind: ArtifactKind::Installer,
                        url: "https://x/osupad-setup.exe".to_string(),
                        sha256: "00".to_string(),
                        size: None,
                    },
                ],
            },
        );
        ReleaseManifest {
            schema: 1,
            generated: "2026-09-16T00:00:00Z".to_string(),
            components,
        }
    }

    fn available(origin: InstallOrigin, target: &str) -> AppAction {
        plan(&manifest("1.1.0"), "1.0.0", origin, target, Ok(())).unwrap()
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
                assert_eq!(policy, ApplyPolicy::PackageManager(PackageManager::Apt));
            }
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::Rpm, "linux-x86_64") {
            AppAction::Available { artifact, .. } => assert_eq!(artifact.kind, ArtifactKind::Rpm),
            other => panic!("{other:?}"),
        }
        match available(InstallOrigin::User, "linux-x86_64") {
            AppAction::Available { artifact, .. } => {
                assert_eq!(artifact.kind, ArtifactKind::Archive)
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
    fn an_aur_install_is_never_offered_an_apply() {
        // pacman owns these files; yay/paru update them.
        let action = available(InstallOrigin::Aur, "linux-x86_64");
        assert_eq!(
            action,
            AppAction::NotifyOnly {
                available: "1.1.0".into(),
                notes: Some("Fixed the thing".into())
            }
        );
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
            InstallOrigin::Aur,
            "linux-x86_64",
            Err(DeferReason::Playing),
        )
        .unwrap();
        assert!(matches!(action, AppAction::NotifyOnly { .. }));
    }

    #[test]
    fn apply_commands_go_through_the_package_manager_never_around_it() {
        // Writing /usr/bin directly would make dpkg -V report modified files
        // and the next apt upgrade silently revert the update.
        let deb = Path::new("/tmp/osupad.deb");
        assert_eq!(
            apply_command(ApplyPolicy::PackageManager(PackageManager::Apt), deb).unwrap(),
            vec!["pkexec", "apt-get", "install", "-y", "/tmp/osupad.deb"]
        );
        let rpm = Path::new("/tmp/osupad.rpm");
        assert_eq!(
            apply_command(ApplyPolicy::PackageManager(PackageManager::Dnf), rpm).unwrap(),
            vec!["pkexec", "dnf", "install", "-y", "/tmp/osupad.rpm"]
        );
    }

    #[test]
    fn the_windows_installer_runs_silently_without_rebooting() {
        let exe = Path::new("osupad-setup.exe");
        let cmd = apply_command(ApplyPolicy::Installer, exe).unwrap();
        assert_eq!(cmd[0], "osupad-setup.exe");
        assert!(cmd.contains(&"/SILENT".to_string()));
        assert!(
            cmd.contains(&"/NORESTART".to_string()),
            "an update must never reboot the machine out from under a player"
        );
    }

    #[test]
    fn an_appimage_is_replaced_by_a_file_not_an_archive() {
        // An AppImage is one file; there is no tree to extract over.
        assert_eq!(
            wanted_kind(ApplyPolicy::DirectReplace, InstallOrigin::AppImage),
            Some(ArtifactKind::Binary)
        );
        assert_eq!(
            wanted_kind(ApplyPolicy::DirectReplace, InstallOrigin::User),
            Some(ArtifactKind::Archive)
        );
    }

    #[test]
    fn nothing_that_only_notifies_has_a_command_to_run() {
        assert!(apply_command(ApplyPolicy::NotifyOnly, Path::new("x")).is_none());
        assert!(apply_command(ApplyPolicy::DirectReplace, Path::new("x")).is_none());
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
