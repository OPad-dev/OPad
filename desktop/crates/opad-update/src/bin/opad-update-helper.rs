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
//! hands that copy to apt, dnf or pacman. It reads only files that belong to
//! the user who called it (`read_callers_file`), so it cannot be pointed at
//! one of root's. The running OPad keeps running;
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
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
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
        let owner = match caller_uid(std::env::var("PKEXEC_UID").ok().as_deref()) {
            Ok(owner) => owner,
            Err(e) => {
                say(e);
                return 2;
            }
        };
        let read = |path: &Path, max: u64| read_callers_file(path, max, owner);
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
            opad_update::verify::test_key_switch_on(),
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

    /// Whose files the helper may read: the user pkexec ran it for
    /// (`PKEXEC_UID`, which pkexec sets itself after clearing the caller's
    /// environment). `None` when root started the helper directly: nothing is
    /// lent to anyone then.
    fn caller_uid(pkexec_uid: Option<&str>) -> Result<Option<u32>, String> {
        match pkexec_uid {
            None => Ok(None),
            Some(uid) => uid
                .parse()
                .map(Some)
                .map_err(|_| format!("PKEXEC_UID is not a user id: {uid:?}")),
        }
    }

    /// One of the files the caller named, read as root without lending the
    /// caller root's eyes.
    ///
    /// The helper used to check the path and then read it: any path, so a
    /// caller could name one of root's files and learn whether it exists, its
    /// size (the "larger than" refusal) and whether its SHA-256 is a given
    /// one, and could swap a link in between the check and the read. Now the
    /// file is opened once, never through a link, and everything is checked
    /// on that open file: it must be a regular file owned by `owner`, which
    /// the caller could read anyway. Every refusal up to that point is the
    /// same sentence, so it says nothing about a file that is someone else's.
    fn read_callers_file(path: &Path, max: u64, owner: Option<u32>) -> Result<Vec<u8>, String> {
        use rustix::fs::OFlags;
        let refused = || {
            format!(
                "{} is not a regular file that belongs to the calling user",
                path.display()
            )
        };
        // NONBLOCK: opening a FIFO or a device must not hang the helper
        let flags = OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(flags.bits() as i32)
            .open(path)
            .map_err(|_| refused())?;
        let meta = file.metadata().map_err(|_| refused())?;
        if !meta.file_type().is_file() || owner.is_some_and(|uid| meta.uid() != uid) {
            return Err(refused());
        }
        // The caller's own file from here on. The size is checked on what is
        // read, not on what the file said a moment ago.
        let mut bytes = Vec::new();
        file.take(max + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if bytes.len() as u64 > max {
            return Err(format!("{} is larger than {max} bytes", path.display()));
        }
        Ok(bytes)
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
        use super::{caller_uid, parse, read_callers_file};

        #[test]
        fn only_the_callers_own_regular_files_are_read() {
            let me = rustix::process::getuid().as_raw();
            let dir = tempfile::tempdir().unwrap();
            let mine = dir.path().join("feed.json");
            std::fs::write(&mine, b"12345").unwrap();
            assert_eq!(read_callers_file(&mine, 5, Some(me)).unwrap(), b"12345");
            // Root starting the helper itself reads what it names
            assert_eq!(read_callers_file(&mine, 5, None).unwrap(), b"12345");

            // Someone else's file, a link, a directory and a missing file are
            // all refused in the same words: nothing is learnt about them
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&mine, &link).unwrap();
            let refusals = [
                read_callers_file(&mine, 5, Some(me + 1)),
                read_callers_file(&link, 5, Some(me)),
                read_callers_file(dir.path(), 5, Some(me)),
                read_callers_file(&dir.path().join("missing"), 5, Some(me)),
            ];
            for (refusal, path) in refusals.iter().zip([
                &mine,
                &link,
                &dir.path().to_path_buf(),
                &dir.path().join("missing"),
            ]) {
                assert_eq!(
                    refusal.as_ref().unwrap_err(),
                    &format!(
                        "{} is not a regular file that belongs to the calling user",
                        path.display()
                    )
                );
            }

            // Too big is said only of a file that is the caller's
            let e = read_callers_file(&mine, 4, Some(me)).unwrap_err();
            assert!(e.contains("larger than 4 bytes"), "{e}");
            let e = read_callers_file(&mine, 4, Some(me + 1)).unwrap_err();
            assert!(!e.contains("larger"), "{e}");
        }

        #[test]
        fn the_caller_is_the_user_pkexec_ran_the_helper_for() {
            assert_eq!(caller_uid(Some("1000")), Ok(Some(1000)));
            assert_eq!(caller_uid(None), Ok(None));
            assert!(caller_uid(Some("")).is_err());
            assert!(caller_uid(Some("root")).is_err());
        }

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
