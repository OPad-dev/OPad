//! Where osu!pad keeps its files, on every platform (§W0-4).
//!
//! Every path the app reads or writes is resolved here, so the uninstaller has
//! one list to work from (§W2-3) and no code falls back to a relative path. A
//! login-launched daemon inherits an unpredictable working directory — on
//! Windows often `C:\Windows\System32` — so a bare relative path is never a
//! safe default. When a directory cannot be resolved these functions fail
//! loudly instead of guessing.
//!
//! | | Linux | Windows |
//! |---|---|---|
//! | data | `$XDG_DATA_HOME/osupad`, else `~/.local/share/osupad` | `%APPDATA%\osupad` |
//! | state (logs) | `$XDG_STATE_HOME/osupad`, else `~/.local/state/osupad` | `%APPDATA%\osupad` |
//! | installed private files | `$(PREFIX)/lib/osupad` | the install directory |

use std::path::PathBuf;
use thiserror::Error;

/// The binary name tosu ships under on this platform
pub const TOSU_BINARY: &str = if cfg!(windows) { "tosu.exe" } else { "tosu" };

#[derive(Debug, Error)]
pub enum PathError {
    #[error("Could not resolve this user's {kind} directory. On Linux set $XDG_DATA_HOME or $HOME; on Windows %APPDATA% must be set.")]
    Unresolved { kind: &'static str },
    #[error("Could not locate the running executable: {0}")]
    Executable(#[from] std::io::Error),
    #[error("The running executable has no parent directory")]
    NoInstallDirectory,
}

/// Per-user data: the SQLite database and anything else that must survive.
///
/// `$XDG_DATA_HOME` is honoured first on Linux, so installs that predate this
/// module keep using the database they already have — there is no migration.
pub fn data_dir() -> Result<PathBuf, PathError> {
    dirs::data_dir()
        .map(|d| d.join("osupad"))
        .ok_or(PathError::Unresolved { kind: "data" })
}

/// Per-user state: logs, and anything else that is useful but disposable.
pub fn state_dir() -> Result<PathBuf, PathError> {
    match dirs::state_dir() {
        Some(d) => Ok(d.join("osupad")),
        // Windows and macOS have no XDG-style state directory; logs live with
        // the data rather than in a second place the uninstaller must know.
        None => data_dir(),
    }
}

/// The SQLite database holding counters, config, layouts and install identity
pub fn database_path() -> Result<PathBuf, PathError> {
    Ok(data_dir()?.join("osupad.db"))
}

/// stdout/stderr of the supervised tosu process
pub fn tosu_log_path() -> Result<PathBuf, PathError> {
    Ok(state_dir()?.join("tosu.log"))
}

/// stdout/stderr of a daemon the GUI launched itself
pub fn daemon_log_path() -> Result<PathBuf, PathError> {
    Ok(state_dir()?.join("daemon.log"))
}

/// The app's private installed files — the bundled tosu (§T-2) and the
/// install-origin marker (§U-2a) — resolved relative to the running binary.
///
/// `/usr/bin/osupad-daemon` gives `/usr/lib/osupad`, `~/.local/bin/…` gives
/// `~/.local/lib/osupad`, so one rule covers every prefix the Makefile and the
/// distro packages can be installed under. Windows has no such split: the
/// installer puts everything in one directory.
pub fn install_lib_dir() -> Result<PathBuf, PathError> {
    let exe = std::env::current_exe()?;
    let bin_dir = exe.parent().ok_or(PathError::NoInstallDirectory)?;

    if cfg!(windows) {
        Ok(bin_dir.to_path_buf())
    } else {
        let prefix = bin_dir.parent().ok_or(PathError::NoInstallDirectory)?;
        Ok(prefix.join("lib").join("osupad"))
    }
}

/// Where the bundled tosu lives (§T-2)
pub fn bundled_tosu_dir() -> Result<PathBuf, PathError> {
    Ok(install_lib_dir()?.join("tosu"))
}

/// The bundled tosu binary itself
pub fn bundled_tosu_binary() -> Result<PathBuf, PathError> {
    Ok(bundled_tosu_dir()?.join(TOSU_BINARY))
}

/// The marker naming the packaging path this install came from (§U-2a)
pub fn install_origin_path() -> Result<PathBuf, PathError> {
    Ok(install_lib_dir()?.join("install-origin"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_and_state_live_under_one_osupad_directory() {
        let data = data_dir().expect("data_dir");
        let state = state_dir().expect("state_dir");
        assert_eq!(data.file_name().unwrap(), "osupad");
        assert_eq!(state.file_name().unwrap(), "osupad");
        assert!(database_path().unwrap().starts_with(&data));
        assert!(tosu_log_path().unwrap().starts_with(&state));
        assert!(daemon_log_path().unwrap().starts_with(&state));
    }

    #[test]
    fn nothing_resolves_to_a_relative_path() {
        // A login-launched daemon has no useful working directory, so every
        // path must be absolute or the call must have failed.
        for p in [
            database_path().unwrap(),
            tosu_log_path().unwrap(),
            daemon_log_path().unwrap(),
            install_lib_dir().unwrap(),
            bundled_tosu_binary().unwrap(),
            install_origin_path().unwrap(),
        ] {
            assert!(p.is_absolute(), "{} is not absolute", p.display());
        }
    }

    #[test]
    fn xdg_data_home_still_wins_on_linux() {
        // Existing Linux installs must not be migrated out from under them.
        if cfg!(target_os = "linux") {
            if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
                assert!(data_dir().unwrap().starts_with(PathBuf::from(xdg)));
            }
        }
    }

    #[test]
    fn the_bundled_tosu_sits_beside_the_origin_marker() {
        let lib = install_lib_dir().unwrap();
        assert!(bundled_tosu_dir().unwrap().starts_with(&lib));
        assert_eq!(
            install_origin_path().unwrap().parent().unwrap(),
            lib.as_path()
        );
        assert_eq!(
            bundled_tosu_binary().unwrap().file_name().unwrap(),
            TOSU_BINARY
        );
    }
}
