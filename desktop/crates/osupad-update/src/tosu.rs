//! The bundled tosu updater (§U-1).
//!
//! **What "stable" means here, written down so it is not reinterpreted later:**
//! the newest tosu GitHub release with `prerelease == false` and
//! `draft == false`. Nothing else is stable, whatever its version number looks
//! like.
//!
//! **What this code actually installs, and why it is not that query.** §U-0.3
//! forbids trusting an artifact because it arrived over HTTPS from GitHub, and
//! tosu's binaries are not ours to sign. So a tosu version becomes installable
//! only once it appears in our own signed release manifest, with a SHA-256 we
//! put there. Meeting §U-1's "within a day of a new stable release" is
//! therefore a release-automation duty: watch upstream for a release matching
//! the definition above, record it, re-sign the manifest. The daemon's side of
//! that promise is checking daily, which it does.
//!
//! **We only ever update a tosu we own** (§T-2). A tosu on `$PATH`, one the
//! user pointed `$OSUPAD_TOSU_PATH` at, or one a package manager installed is
//! reported and left alone — replacing it would desynchronise dpkg/rpm/pacman
//! and overwrite a choice the user made deliberately.

use crate::download::stage_bytes;
use crate::manifest::{Artifact, ReleaseManifest, TOSU};
use crate::origin::InstallOrigin;
use crate::{DeferReason, UpdateError};
use osupad_model::paths;
use std::path::{Path, PathBuf};

/// Records the version we installed, so the next check knows what is on disk.
/// tosu's own `--version` is not relied on: the answer must be available
/// without executing a binary we are about to replace.
pub const VERSION_FILE: &str = "VERSION";
/// The §T-3 notice, rewritten on every swap so a self-updated install stays
/// compliant instead of citing the version it originally shipped with.
pub const NOTICE_FILE: &str = "NOTICE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TosuAction {
    /// Nothing to do
    UpToDate { version: String },
    /// A newer tosu exists but this install must not touch the binary (§T-2)
    NotifyOnly {
        installed: String,
        available: String,
    },
    /// Wait; the reason is shown in the GUI
    Defer { reason: DeferReason },
    Install {
        from: String,
        to: String,
        artifact: Artifact,
    },
}

/// The whole decision, with no IO, so every branch is testable.
pub fn plan(
    manifest: &ReleaseManifest,
    installed: Option<&str>,
    origin: InstallOrigin,
    target: &str,
    gate: Result<(), DeferReason>,
) -> Result<TosuAction, UpdateError> {
    let component = manifest
        .component(TOSU)
        .ok_or_else(|| UpdateError::NoArtifact {
            component: TOSU.to_string(),
            target: "any",
        })?;

    let installed = installed.unwrap_or("").trim();
    if !installed.is_empty() && installed == component.version {
        return Ok(TosuAction::UpToDate {
            version: component.version.clone(),
        });
    }

    // Checked before the gate: "a different version exists" is news the user
    // can act on even mid-map, and it changes nothing on disk.
    if !origin.owns_bundled_tosu() {
        return Ok(TosuAction::NotifyOnly {
            installed: installed.to_string(),
            available: component.version.clone(),
        });
    }

    if let Err(reason) = gate {
        return Ok(TosuAction::Defer { reason });
    }

    let artifact =
        manifest
            .artifact_for(TOSU, target, None)
            .ok_or_else(|| UpdateError::NoArtifact {
                component: TOSU.to_string(),
                target: "this platform",
            })?;

    Ok(TosuAction::Install {
        from: installed.to_string(),
        to: component.version.clone(),
        artifact: artifact.clone(),
    })
}

/// The version recorded next to the bundled binary, if there is one
pub fn installed_version(bundled_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(bundled_dir.join(VERSION_FILE)).ok()?;
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

pub fn bundled_dir() -> Result<PathBuf, UpdateError> {
    paths::bundled_tosu_dir().map_err(|e| {
        UpdateError::Io(std::io::Error::other(format!(
            "Cannot locate the bundled tosu directory: {e}"
        )))
    })
}

/// Writes the binary, its version and its §T-3 notice as one step.
///
/// The binary goes in last. If anything fails before that, the old tosu is
/// still there and still runs; if the process dies after it, the version file
/// already names what is on disk.
pub fn install(
    bundled_dir: &Path,
    binary_bytes: &[u8],
    artifact: &Artifact,
    version: &str,
    upstream_tag: Option<&str>,
) -> Result<(), UpdateError> {
    let binary = bundled_dir.join(paths::TOSU_BINARY);
    let staged = stage_bytes(&binary, binary_bytes, &artifact.sha256, "tosu")?;

    let notice = notice_text(version, upstream_tag, &artifact.url);
    std::fs::write(bundled_dir.join(NOTICE_FILE), notice)?;
    std::fs::write(bundled_dir.join(VERSION_FILE), format!("{version}\n"))?;

    staged.install_to(&binary)
}

/// The §T-3 notice: upstream project, author, license, exact bundled version
/// and where the binary came from, plus the replacement rights LGPL-3.0 grants
/// the user — which the `$OSUPAD_TOSU_PATH` override is what satisfies.
pub fn notice_text(version: &str, upstream_tag: Option<&str>, url: &str) -> String {
    let tag = upstream_tag.unwrap_or(version);
    format!(
        "tosu\n\
         ====\n\n\
         This copy of tosu is redistributed with OPad.\n\n\
         Project:   tosu (https://github.com/tosuapp/tosu)\n\
         Author:    Mikhail Babynichev and the tosu contributors\n\
         License:   GNU Lesser General Public License v3.0\n\
         Version:   {version}\n\
         Upstream:  {tag}\n\
         Obtained:  {url}\n\n\
         The full licence text is in LICENSE next to this file.\n\n\
         Corresponding source for this exact version is published alongside the\n\
         OPad release this binary came from, under the upstream tag above.\n\n\
         LGPL-3.0 gives you the right to replace this component with your own\n\
         build. OPad supports that directly: set $OSUPAD_TOSU_PATH, or point\n\
         the app at your own tosu in Settings. A tosu found that way is used as\n\
         is and is never updated or overwritten by OPad.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ArtifactKind, Component};
    use crate::verify::sha256_bytes;
    use std::collections::BTreeMap;

    fn manifest(version: &str) -> ReleaseManifest {
        let mut components = BTreeMap::new();
        components.insert(
            TOSU.to_string(),
            Component {
                version: version.to_string(),
                upstream_tag: Some(format!("v{version}")),
                notes: None,
                artifacts: vec![Artifact {
                    target: "linux-x86_64".to_string(),
                    kind: ArtifactKind::Binary,
                    url: "https://example.invalid/tosu".to_string(),
                    sha256: "00".to_string(),
                    size: None,
                }],
            },
        );
        ReleaseManifest {
            schema: 1,
            generated: "2026-09-16T00:00:00Z".to_string(),
            components,
        }
    }

    #[test]
    fn a_matching_version_does_nothing() {
        let action = plan(
            &manifest("4.1.0"),
            Some("4.1.0"),
            InstallOrigin::User,
            "linux-x86_64",
            Ok(()),
        )
        .unwrap();
        assert_eq!(
            action,
            TosuAction::UpToDate {
                version: "4.1.0".into()
            }
        );
    }

    #[test]
    fn a_package_manager_install_is_told_but_never_touched() {
        // §T-2: pacman/apt/dnf own that binary.
        for origin in [
            InstallOrigin::Deb,
            InstallOrigin::Rpm,
            InstallOrigin::Aur,
            InstallOrigin::Unknown,
        ] {
            let action = plan(
                &manifest("4.2.0"),
                Some("4.1.0"),
                origin,
                "linux-x86_64",
                Ok(()),
            )
            .unwrap();
            assert_eq!(
                action,
                TosuAction::NotifyOnly {
                    installed: "4.1.0".into(),
                    available: "4.2.0".into()
                },
                "{origin:?}"
            );
        }
    }

    #[test]
    fn an_update_waits_for_the_map_to_end() {
        let action = plan(
            &manifest("4.2.0"),
            Some("4.1.0"),
            InstallOrigin::User,
            "linux-x86_64",
            Err(DeferReason::Playing),
        )
        .unwrap();
        assert_eq!(
            action,
            TosuAction::Defer {
                reason: DeferReason::Playing
            }
        );
    }

    #[test]
    fn notifying_happens_even_mid_map_because_it_writes_nothing() {
        let action = plan(
            &manifest("4.2.0"),
            Some("4.1.0"),
            InstallOrigin::Deb,
            "linux-x86_64",
            Err(DeferReason::Playing),
        )
        .unwrap();
        assert!(matches!(action, TosuAction::NotifyOnly { .. }));
    }

    #[test]
    fn a_newer_version_on_an_install_we_own_is_installed() {
        let action = plan(
            &manifest("4.2.0"),
            Some("4.1.0"),
            InstallOrigin::User,
            "linux-x86_64",
            Ok(()),
        )
        .unwrap();
        match action {
            TosuAction::Install { from, to, artifact } => {
                assert_eq!(from, "4.1.0");
                assert_eq!(to, "4.2.0");
                assert_eq!(artifact.url, "https://example.invalid/tosu");
            }
            other => panic!("expected an install, got {other:?}"),
        }
    }

    #[test]
    fn a_first_install_with_no_recorded_version_still_installs() {
        let action = plan(
            &manifest("4.2.0"),
            None,
            InstallOrigin::Windows,
            "linux-x86_64",
            Ok(()),
        )
        .unwrap();
        assert!(matches!(action, TosuAction::Install { .. }));
    }

    #[test]
    fn a_platform_with_no_artifact_is_an_error_not_a_silent_skip() {
        let err = plan(
            &manifest("4.2.0"),
            Some("4.1.0"),
            InstallOrigin::User,
            "windows-aarch64",
            Ok(()),
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::NoArtifact { .. }));
    }

    #[test]
    fn installing_writes_the_binary_version_and_notice_together() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"#!/bin/sh\necho tosu\n";
        let artifact = Artifact {
            target: "linux-x86_64".into(),
            kind: ArtifactKind::Binary,
            url: "https://example.invalid/tosu-4.2.0".into(),
            sha256: sha256_bytes(bytes),
            size: None,
        };

        install(dir.path(), bytes, &artifact, "4.2.0", Some("v4.2.0")).unwrap();

        assert_eq!(
            std::fs::read(dir.path().join(paths::TOSU_BINARY)).unwrap(),
            bytes
        );
        assert_eq!(installed_version(dir.path()).as_deref(), Some("4.2.0"));

        let notice = std::fs::read_to_string(dir.path().join(NOTICE_FILE)).unwrap();
        assert!(notice.contains("4.2.0"));
        assert!(notice.contains("v4.2.0"));
        assert!(notice.contains("Lesser General Public License v3.0"));
        assert!(notice.contains("Mikhail Babynichev"));
        assert!(notice.contains("OSUPAD_TOSU_PATH"));
        assert!(notice.contains("https://example.invalid/tosu-4.2.0"));
    }

    #[test]
    fn a_corrupted_download_leaves_the_previous_tosu_running() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join(paths::TOSU_BINARY);
        std::fs::write(&binary, b"the working tosu").unwrap();
        std::fs::write(dir.path().join(VERSION_FILE), "4.1.0\n").unwrap();

        let artifact = Artifact {
            target: "linux-x86_64".into(),
            kind: ArtifactKind::Binary,
            url: "https://example.invalid/tosu".into(),
            sha256: sha256_bytes(b"what we were promised"),
            size: None,
        };
        let err = install(dir.path(), b"what we got", &artifact, "4.2.0", None).unwrap_err();
        assert!(matches!(err, UpdateError::HashMismatch { .. }));

        // "Never leave the directory without a working binary."
        assert_eq!(std::fs::read(&binary).unwrap(), b"the working tosu");
        assert_eq!(installed_version(dir.path()).as_deref(), Some("4.1.0"));
    }

    #[test]
    fn no_recorded_version_reads_as_none_not_as_an_empty_version() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(installed_version(dir.path()), None);
        std::fs::write(dir.path().join(VERSION_FILE), "  \n").unwrap();
        assert_eq!(installed_version(dir.path()), None);
    }
}
