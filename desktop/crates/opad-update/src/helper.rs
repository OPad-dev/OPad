//! What the update helper (`opad-update-helper`, run as root through pkexec)
//! checks before it installs anything.
//!
//! The packages' polkit action lets the user at the computer run the helper
//! without a password, so the helper is a door into root. It opens for one
//! thing only: a newer OPad release that the signed update feed names. It
//! trusts nothing the caller says: the feed's signature is checked against
//! the feed key compiled into the helper (the public test key only on a
//! machine whose root created `verify::TEST_KEY_SWITCH`), the package's SHA-256 against the one
//! that feed gives for this kind of install, and the version against the one
//! installed (the helper's own: the package installs both together), so a
//! genuine but older feed cannot downgrade OPad either.
//!
//! Pure, with no IO, so every refusal is tested; `bin/opad-update-helper.rs`
//! does the file handling and runs the package manager.

use crate::manifest::{check_feed, Artifact, ReleaseManifest, APP, BETA, STABLE, SUPPORTED_SCHEMA};
use crate::origin::{ApplyPolicy, InstallOrigin, PackageManager};
use crate::verify::{sha256_bytes, verify_manifest_signature_with};
use crate::version::is_newer;
use crate::UpdateError;
use chrono::{DateTime, Utc};

/// The largest feed and signature the helper reads (a feed is a few KB)
pub const MAX_FEED_BYTES: u64 = 1024 * 1024;
pub const MAX_SIGNATURE_BYTES: u64 = 4096;
/// The largest package (a release's packages are 30-40 MB)
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;

/// A package the helper may install
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approved {
    pub version: String,
    pub artifact: Artifact,
}

/// The package manager for the origin the package installed (the root-owned
/// `install-origin` marker), or `None` for an install the helper does not
/// update
pub fn package_manager(origin: InstallOrigin) -> Option<PackageManager> {
    match origin.apply_policy() {
        ApplyPolicy::Helper(pm) => Some(pm),
        _ => None,
    }
}

/// Whether `package` may be installed over `installed`, given the feed bytes
/// and signature the daemon passed along. `allow_test` trusts the test key
/// too, which only root can ask for (`verify::test_key_switch_on`).
#[allow(clippy::too_many_arguments)]
pub fn approve(
    feed_bytes: &[u8],
    signature: &str,
    package: &[u8],
    pm: PackageManager,
    installed: &str,
    target: &str,
    now: DateTime<Utc>,
    allow_test: bool,
) -> Result<Approved, UpdateError> {
    verify_manifest_signature_with(feed_bytes, signature, allow_test)?;
    let feed: ReleaseManifest = serde_json::from_slice(feed_bytes)?;
    if feed.schema != SUPPORTED_SCHEMA {
        return Err(UpdateError::UnsupportedSchema {
            found: feed.schema,
            supported: SUPPORTED_SCHEMA,
        });
    }
    // Either channel's feed will do (the user picks the channel), as long as
    // it is current. Replays of an older feed are covered by the version
    // check below.
    if feed.channel != STABLE && feed.channel != BETA {
        return Err(UpdateError::Feed(format!(
            "{:?} is not an update channel",
            feed.channel
        )));
    }
    check_feed(&feed, &feed.channel, now, None)?;

    let app = feed
        .component(APP)
        .ok_or_else(|| UpdateError::Feed("the feed has no OPad release".to_string()))?;
    if !is_newer(&app.version, installed) {
        return Err(UpdateError::Feed(format!(
            "the feed's OPad {} is not newer than the installed {installed}",
            app.version
        )));
    }

    let (target, kind) =
        crate::app::wanted_artifact(ApplyPolicy::Helper(pm), InstallOrigin::Unknown, target)
            .expect("a helper policy always names an artifact");
    let artifact = feed
        .artifact_for(APP, &target, Some(kind))
        .ok_or(UpdateError::NoArtifact {
            component: APP.to_string(),
            target: "this kind of install",
        })?;
    if let Some(size) = artifact.size {
        if package.len() as u64 != size {
            return Err(UpdateError::Feed(format!(
                "the package is {} bytes, the feed says {size}",
                package.len()
            )));
        }
    }
    let actual = sha256_bytes(package);
    if !actual.eq_ignore_ascii_case(&artifact.sha256) {
        return Err(UpdateError::HashMismatch {
            artifact: artifact.url.clone(),
            expected: artifact.sha256.clone(),
            actual,
        });
    }
    Ok(Approved {
        version: app.version.clone(),
        artifact: artifact.clone(),
    })
}

/// The package manager's command line for an approved package, at a path
/// only root can write. Fixed arguments: nothing from the feed reaches them.
pub fn install_command(pm: PackageManager, package: &std::path::Path) -> Vec<String> {
    let path = package.display().to_string();
    let v = |args: &[&str]| args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let mut cmd = match pm {
        // A fresh boot may have unattended-upgrades holding the dpkg lock
        PackageManager::Apt => v(&[
            "/usr/bin/apt-get",
            "-o",
            "DPkg::Lock::Timeout=600",
            "install",
            "-y",
        ]),
        PackageManager::Dnf => v(&["/usr/bin/dnf", "install", "-y"]),
        PackageManager::Pacman => v(&["/usr/bin/pacman", "-U", "--noconfirm"]),
    };
    cmd.push(path);
    cmd
}

/// The file name the approved package is copied to before installing:
/// apt and pacman go by the extension
pub fn package_file_name(pm: PackageManager) -> &'static str {
    match pm {
        PackageManager::Apt => "opad-update.deb",
        PackageManager::Dnf => "opad-update.rpm",
        PackageManager::Pacman => "opad-update.pkg.tar.zst",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PACMAN_TARGET;

    /// A beta feed offering OPad 1.0.0-rc.7, signed with the committed test
    /// key (`minisign -S -s test-key/opad-test.key`), in tests/fixtures
    const FEED: &[u8] = include_bytes!("../tests/fixtures/helper-feed.json");
    const SIG: &str = include_str!("../tests/fixtures/helper-feed.json.minisig");
    /// The files the feed names: their SHA-256 is in it
    const DEB: &[u8] = b"pretend .deb for the helper tests\n";
    const PKG: &[u8] = b"pretend .pkg.tar.zst for the helper tests\n";

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-12-01T00:00:00Z")
            .unwrap()
            .into()
    }

    fn approve_deb(
        feed: &[u8],
        sig: &str,
        package: &[u8],
        installed: &str,
    ) -> Result<Approved, UpdateError> {
        approve(
            feed,
            sig,
            package,
            PackageManager::Apt,
            installed,
            "linux-x86_64",
            now(),
            true,
        )
    }

    #[test]
    fn a_newer_release_the_signed_feed_names_is_approved() {
        let ok = approve_deb(FEED, SIG, DEB, "1.0.0-rc.6").unwrap();
        assert_eq!(ok.version, "1.0.0-rc.7");
        let ok = approve(
            FEED,
            SIG,
            PKG,
            PackageManager::Pacman,
            "1.0.0-rc.6",
            "linux-x86_64",
            now(),
            true,
        )
        .unwrap();
        assert_eq!(ok.artifact.target, PACMAN_TARGET);
    }

    #[test]
    fn a_package_the_feed_does_not_name_is_refused() {
        let err = approve_deb(FEED, SIG, b"something else", "1.0.0-rc.6").unwrap_err();
        assert!(matches!(err, UpdateError::HashMismatch { .. }), "{err}");
        // The right bytes for another kind of install are not this one's
        let err = approve_deb(FEED, SIG, PKG, "1.0.0-rc.6").unwrap_err();
        assert!(matches!(err, UpdateError::HashMismatch { .. }), "{err}");
    }

    #[test]
    fn a_feed_with_a_changed_byte_or_another_signature_is_refused() {
        let mut tampered = FEED.to_vec();
        let at = tampered.iter().position(|&b| b == b'7').unwrap();
        tampered[at] = b'9';
        assert!(matches!(
            approve_deb(&tampered, SIG, DEB, "1.0.0-rc.6").unwrap_err(),
            UpdateError::BadSignature(_)
        ));
        assert!(approve_deb(FEED, "not a signature", DEB, "1.0.0-rc.6").is_err());
    }

    #[test]
    fn the_public_test_key_is_refused_unless_root_allowed_it() {
        let err = approve(
            FEED,
            SIG,
            DEB,
            PackageManager::Apt,
            "1.0.0-rc.6",
            "linux-x86_64",
            now(),
            false,
        )
        .unwrap_err();
        assert!(matches!(err, UpdateError::BadSignature(_)), "{err}");
    }

    #[test]
    fn the_same_or_an_older_release_is_refused() {
        for installed in ["1.0.0-rc.7", "1.0.0", "1.1.0"] {
            let err = approve_deb(FEED, SIG, DEB, installed).unwrap_err();
            assert!(err.to_string().contains("not newer"), "{installed}: {err}");
        }
    }

    #[test]
    fn an_expired_feed_is_refused() {
        let later: DateTime<Utc> = DateTime::parse_from_rfc3339("2027-06-01T00:00:00Z")
            .unwrap()
            .into();
        let err = approve(
            FEED,
            SIG,
            DEB,
            PackageManager::Apt,
            "1.0.0-rc.6",
            "linux-x86_64",
            later,
            true,
        )
        .unwrap_err();
        assert!(err.to_string().contains("expired"), "{err}");
    }

    #[test]
    fn only_package_installs_have_a_helper() {
        assert_eq!(
            package_manager(InstallOrigin::Deb),
            Some(PackageManager::Apt)
        );
        assert_eq!(
            package_manager(InstallOrigin::Rpm),
            Some(PackageManager::Dnf)
        );
        assert_eq!(
            package_manager(InstallOrigin::Aur),
            Some(PackageManager::Pacman)
        );
        for origin in [
            InstallOrigin::Windows,
            InstallOrigin::AppImage,
            InstallOrigin::User,
            InstallOrigin::Source,
            InstallOrigin::Unknown,
        ] {
            assert_eq!(package_manager(origin), None, "{origin:?}");
        }
    }

    #[test]
    fn the_package_manager_gets_fixed_arguments_and_the_root_owned_copy() {
        let p = std::path::Path::new("/var/cache/opad-update/opad-update.deb");
        assert_eq!(
            install_command(PackageManager::Apt, p),
            [
                "/usr/bin/apt-get",
                "-o",
                "DPkg::Lock::Timeout=600",
                "install",
                "-y",
                p.to_str().unwrap()
            ]
        );
        assert_eq!(
            install_command(PackageManager::Pacman, p)[..3],
            ["/usr/bin/pacman", "-U", "--noconfirm"]
        );
        assert_eq!(
            install_command(PackageManager::Dnf, p)[..3],
            ["/usr/bin/dnf", "install", "-y"]
        );
    }
}
