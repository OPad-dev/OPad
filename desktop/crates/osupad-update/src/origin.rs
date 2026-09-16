//! Which packaging path this install came from (§U-2a).
//!
//! The updater must know which row of the §U-2 table it is in, and **must not
//! guess**. Probing the system (`dpkg -S`, `rpm -qf`, `pacman -Qo`, prefix
//! checks) is slower, needs those tools present, and gets ambiguous cases
//! wrong — a `.deb` a user unpacked by hand, for example.
//!
//! So each packaging path drops a one-word marker file, and this reads it.
//! **The contract for the packagers (§B-2, §L-3, §W2-1):** write
//! `$(PREFIX)/lib/osupad/install-origin` — on Windows, `install-origin` in the
//! install directory — containing exactly one of:
//!
//! `windows` · `deb` · `rpm` · `aur` · `appimage` · `user` · `source`
//!
//! A missing or unrecognised marker degrades to notify-only. Never guess and
//! then modify files.

use osupad_model::paths;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOrigin {
    /// The Inno Setup installer
    Windows,
    Deb,
    Rpm,
    Aur,
    AppImage,
    /// `make install-user`, into ~/.local
    User,
    /// `make install` into a system prefix, owned by no package manager
    Source,
    /// No marker, or one this build does not recognise
    Unknown,
}

/// How an app update is applied for this origin (§U-2)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyPolicy {
    /// Run the new signed installer `/SILENT /NORESTART`
    Installer,
    /// Hand the downloaded package to the system package manager, which costs
    /// one polkit prompt and keeps the package database honest
    PackageManager(PackageManager),
    /// Replace the files directly; the user already owns every one of them
    DirectReplace,
    /// Tell the user and change nothing on disk
    NotifyOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
}

impl InstallOrigin {
    pub fn parse(marker: &str) -> Self {
        match marker.trim().to_ascii_lowercase().as_str() {
            "windows" => Self::Windows,
            "deb" => Self::Deb,
            "rpm" => Self::Rpm,
            "aur" => Self::Aur,
            "appimage" => Self::AppImage,
            "user" => Self::User,
            "source" => Self::Source,
            _ => Self::Unknown,
        }
    }

    /// Reads the marker next to the running binary
    pub fn detect() -> Self {
        match paths::install_origin_path() {
            Ok(path) => Self::read_from(&path),
            Err(_) => Self::Unknown,
        }
    }

    pub fn read_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents),
            Err(_) => Self::Unknown,
        }
    }

    pub fn apply_policy(self) -> ApplyPolicy {
        match self {
            Self::Windows => ApplyPolicy::Installer,
            Self::Deb => ApplyPolicy::PackageManager(PackageManager::Apt),
            Self::Rpm => ApplyPolicy::PackageManager(PackageManager::Dnf),
            // pacman owns these files and yay/paru update them (§U-2)
            Self::Aur => ApplyPolicy::NotifyOnly,
            Self::AppImage | Self::User => ApplyPolicy::DirectReplace,
            // A system prefix nobody's package manager tracks: the daemon runs
            // as the user and has no business writing there.
            Self::Source => ApplyPolicy::NotifyOnly,
            Self::Unknown => ApplyPolicy::NotifyOnly,
        }
    }

    /// Whether we installed the bundled tosu into a place we own (§T-2).
    ///
    /// Everywhere else a package manager owns that binary, so replacing it
    /// would desynchronise the package database and the file would be reported
    /// as modified. Those installs report a new tosu; they never swap it.
    pub fn owns_bundled_tosu(self) -> bool {
        matches!(self, Self::Windows | Self::User | Self::AppImage)
    }

    /// The word a packager writes into the marker
    pub fn marker(self) -> Option<&'static str> {
        Some(match self {
            Self::Windows => "windows",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Aur => "aur",
            Self::AppImage => "appimage",
            Self::User => "user",
            Self::Source => "source",
            Self::Unknown => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [InstallOrigin; 7] = [
        InstallOrigin::Windows,
        InstallOrigin::Deb,
        InstallOrigin::Rpm,
        InstallOrigin::Aur,
        InstallOrigin::AppImage,
        InstallOrigin::User,
        InstallOrigin::Source,
    ];

    #[test]
    fn every_marker_round_trips() {
        for origin in ALL {
            let marker = origin.marker().unwrap();
            assert_eq!(InstallOrigin::parse(marker), origin);
        }
    }

    #[test]
    fn a_marker_file_is_read_tolerantly() {
        // Packagers write these with echo, printf, Inno's file section and a
        // PKGBUILD; a trailing newline or capitalisation is not a failure.
        for text in ["deb", "deb\n", " deb \n", "DEB\r\n"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("install-origin");
            std::fs::write(&path, text).unwrap();
            assert_eq!(
                InstallOrigin::read_from(&path),
                InstallOrigin::Deb,
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_missing_or_unknown_marker_never_modifies_files() {
        // §U-2a: "An install with no marker never attempts to apply an update."
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("install-origin");
        assert_eq!(InstallOrigin::read_from(&missing), InstallOrigin::Unknown);

        std::fs::write(&missing, "snap").unwrap();
        assert_eq!(InstallOrigin::read_from(&missing), InstallOrigin::Unknown);

        assert_eq!(
            InstallOrigin::Unknown.apply_policy(),
            ApplyPolicy::NotifyOnly
        );
        assert!(!InstallOrigin::Unknown.owns_bundled_tosu());
    }

    #[test]
    fn apply_policy_matches_the_u2_table() {
        assert_eq!(
            InstallOrigin::Windows.apply_policy(),
            ApplyPolicy::Installer
        );
        assert_eq!(
            InstallOrigin::Deb.apply_policy(),
            ApplyPolicy::PackageManager(PackageManager::Apt)
        );
        assert_eq!(
            InstallOrigin::Rpm.apply_policy(),
            ApplyPolicy::PackageManager(PackageManager::Dnf)
        );
        assert_eq!(InstallOrigin::Aur.apply_policy(), ApplyPolicy::NotifyOnly);
        assert_eq!(
            InstallOrigin::User.apply_policy(),
            ApplyPolicy::DirectReplace
        );
        assert_eq!(
            InstallOrigin::AppImage.apply_policy(),
            ApplyPolicy::DirectReplace
        );
    }

    #[test]
    fn package_manager_owned_installs_never_swap_the_bundled_tosu() {
        // §T-2: pacman/apt/dnf own that binary. Replacing it behind their back
        // makes `dpkg -V` / `rpm -V` / `pacman -Qkk` report a modified file.
        for origin in [
            InstallOrigin::Deb,
            InstallOrigin::Rpm,
            InstallOrigin::Aur,
            InstallOrigin::Source,
            InstallOrigin::Unknown,
        ] {
            assert!(!origin.owns_bundled_tosu(), "{origin:?}");
        }
        for origin in [
            InstallOrigin::Windows,
            InstallOrigin::User,
            InstallOrigin::AppImage,
        ] {
            assert!(origin.owns_bundled_tosu(), "{origin:?}");
        }
    }

    #[test]
    fn nothing_that_notifies_only_also_claims_to_own_tosu() {
        for origin in ALL {
            if origin.apply_policy() == ApplyPolicy::NotifyOnly {
                assert!(
                    !origin.owns_bundled_tosu(),
                    "{origin:?} would touch files it does not own"
                );
            }
        }
    }
}
