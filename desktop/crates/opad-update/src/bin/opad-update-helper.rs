//! The update helper: installs an OPad update for a package install (.deb,
//! .rpm, Arch opad-bin) as root, through pkexec.
//!
//!   pkexec /usr/lib/opad/opad-update-helper --feed F --signature S --package P
//!
//! The packages' polkit action (`dev.opad.update`) lets the user at the
//! computer run it without a password, so it installs nothing but a newer
//! OPad release that the signed update feed names (`opad_update::helper`
//! says exactly what is checked). The daemon downloads the package and
//! passes it with the feed it verified; the helper reads all three as root,
//! checks them itself, copies the package where only root can write, and
//! hands that copy to apt, dnf or pacman. The running OPad keeps running;
//! the package's files are the ones that start next time.
//!
//! Exit status: 0 installed, 2 bad arguments or not root, 3 refused (the
//! package is not an approved update), 4 the package manager failed.

#[cfg(not(unix))]
fn main() {
    eprintln!("opad-update-helper: package installs only (Linux)");
    std::process::exit(2);
}

#[cfg(unix)]
fn main() {
    std::process::exit(unix::run());
}

#[cfg(unix)]
mod unix {
    use opad_update::helper::{
        approve, install_command, package_file_name, package_manager, MAX_FEED_BYTES,
        MAX_PACKAGE_BYTES, MAX_SIGNATURE_BYTES,
    };
    use opad_update::manifest::current_target;
    use opad_update::InstallOrigin;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};

    /// Written by the package itself, so root-owned: how OPad was installed
    const ORIGIN_MARKER: &str = "/usr/lib/opad/install-origin";
    /// Where the approved package is copied before installing
    const CACHE_DIR: &str = "/var/cache/opad-update";

    fn say(msg: impl std::fmt::Display) {
        eprintln!("opad-update-helper: {msg}");
    }

    pub fn run() -> i32 {
        if !rustix::process::geteuid().is_root() {
            say("must run as root (pkexec)");
            return 2;
        }
        let args: Vec<String> = std::env::args().skip(1).collect();
        let (feed, signature, package) = match parse(&args) {
            Some(paths) => paths,
            None => {
                say("usage: opad-update-helper --feed FILE --signature FILE --package FILE");
                return 2;
            }
        };

        let Some(pm) = package_manager(InstallOrigin::read_from(Path::new(ORIGIN_MARKER))) else {
            say(format!(
                "{ORIGIN_MARKER} does not name a package install; nothing to do"
            ));
            return 3;
        };
        let read = |path: &Path, max: u64| -> Result<Vec<u8>, String> {
            // A regular file, not a link or a device, and not unreasonably big
            let meta =
                std::fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
            if !meta.file_type().is_file() {
                return Err(format!("{} is not a regular file", path.display()));
            }
            if meta.len() > max {
                return Err(format!("{} is larger than {max} bytes", path.display()));
            }
            std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
        };
        let inputs = read(&feed, MAX_FEED_BYTES).and_then(|f| {
            let s = read(&signature, MAX_SIGNATURE_BYTES)?;
            let p = read(&package, MAX_PACKAGE_BYTES)?;
            Ok((f, s, p))
        });
        let (feed_bytes, signature, package_bytes) = match inputs {
            Ok(v) => v,
            Err(e) => {
                say(e);
                return 3;
            }
        };
        let signature = String::from_utf8_lossy(&signature);

        let approved = match approve(
            &feed_bytes,
            &signature,
            &package_bytes,
            pm,
            env!("CARGO_PKG_VERSION"),
            current_target(),
            chrono::Utc::now(),
        ) {
            Ok(a) => a,
            Err(e) => {
                say(format!("refused: {e}"));
                return 3;
            }
        };
        say(format!(
            "installing OPad {} over {} ({})",
            approved.version,
            env!("CARGO_PKG_VERSION"),
            approved.artifact.url
        ));

        // The checked bytes, where no one but root can swap them before the
        // package manager reads them
        let copy = match write_private(&package_bytes, package_file_name(pm)) {
            Ok(p) => p,
            Err(e) => {
                say(format!("cannot stage the package in {CACHE_DIR}: {e}"));
                return 4;
            }
        };
        let cmd = install_command(pm, &copy);
        let status = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("LANG", "C.UTF-8")
            .env("DEBIAN_FRONTEND", "noninteractive")
            .status();
        let _ = std::fs::remove_dir_all(copy.parent().unwrap_or(Path::new(CACHE_DIR)));
        match status {
            Ok(s) if s.success() => {
                say(format!("OPad {} installed", approved.version));
                0
            }
            Ok(s) => {
                say(format!("{} exited with {s}", cmd[0]));
                4
            }
            Err(e) => {
                say(format!("cannot run {}: {e}", cmd[0]));
                4
            }
        }
    }

    /// `--feed F --signature S --package P`, each exactly once, nothing else
    fn parse(args: &[String]) -> Option<(PathBuf, PathBuf, PathBuf)> {
        if args.len() != 6 {
            return None;
        }
        let (mut feed, mut sig, mut pkg) = (None, None, None);
        for pair in args.chunks(2) {
            let slot = match pair[0].as_str() {
                "--feed" => &mut feed,
                "--signature" => &mut sig,
                "--package" => &mut pkg,
                _ => return None,
            };
            if slot.is_some() || !pair[1].starts_with('/') {
                return None;
            }
            *slot = Some(PathBuf::from(&pair[1]));
        }
        Some((feed?, sig?, pkg?))
    }

    /// A new directory of this run's own under CACHE_DIR, root-only, holding
    /// the package
    fn write_private(bytes: &[u8], name: &str) -> std::io::Result<PathBuf> {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(CACHE_DIR)?;
        std::fs::set_permissions(CACHE_DIR, std::fs::Permissions::from_mode(0o700))?;
        let dir = Path::new(CACHE_DIR).join(format!("run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        let path = dir.join(name);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&path)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(path)
    }

    #[cfg(test)]
    mod tests {
        use super::parse;

        fn a(v: &[&str]) -> Vec<String> {
            v.iter().map(|s| s.to_string()).collect()
        }

        #[test]
        fn only_the_three_absolute_paths_are_accepted() {
            assert!(parse(&a(&[
                "--feed",
                "/f",
                "--signature",
                "/s",
                "--package",
                "/p"
            ]))
            .is_some());
            assert!(parse(&a(&[
                "--package",
                "/p",
                "--feed",
                "/f",
                "--signature",
                "/s"
            ]))
            .is_some());
            assert!(parse(&a(&["--feed", "f", "--signature", "/s", "--package", "/p"])).is_none());
            assert!(parse(&a(&["--feed", "/f", "--feed", "/s", "--package", "/p"])).is_none());
            assert!(parse(&a(&["--feed", "/f", "--signature", "/s", "--exec", "/p"])).is_none());
            assert!(parse(&a(&["--feed", "/f", "--signature", "/s"])).is_none());
        }
    }
}
