//! Windows counterpart of `platform_linux` (§W1-1).
//!
//! Windows has exactly one per-user autostart mechanism worth using, so both
//! the daemon and the tray go through `HKCU\…\CurrentVersion\Run`. It is
//! trivially inspectable by the user and removed by deleting two values, which
//! is what a clean uninstall needs (§W2-3).
//!
//! Deliberately not a Windows Service: the daemon is per-user, needs the user's
//! session for the tray and for IPC, and a service would add an uninstall
//! footprint for no benefit.
//!
//! **The installer must write byte-identical values** (§W2-1). R7 was caused by
//! the GUI writing a *second*, divergent systemd unit instead of the packaged
//! one; the same mistake is available here, so the exact strings live in
//! [`daemon_autostart_command`] and [`gui_autostart_command`] and nowhere else.

use osupad_model::paths;
use std::path::{Path, PathBuf};
use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

/// The per-user autostart key. Not `HKLM`: this install is per-user.
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const DAEMON_VALUE: &str = "osupad-daemon";
const GUI_VALUE: &str = "osupad-gui";

/// Detached, no console window — the analogue of `process_group(0)` on Linux.
/// A daemon started from a terminal must not die when that terminal closes.
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// The exact `Run` value for the daemon. The installer writes this same string.
pub fn daemon_autostart_command(daemon_bin: &Path) -> String {
    format!("\"{}\"", daemon_bin.display())
}

/// The exact `Run` value for the tray, mirroring `Exec=osupad-gui --tray` in
/// the Linux `.desktop` entry.
pub fn gui_autostart_command(gui_bin: &Path) -> String {
    format!("\"{}\" --tray", gui_bin.display())
}

fn run_key(access: u32) -> Result<RegKey, String> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, access)
        .map_err(|e| format!("Failed to open HKCU\\{}: {}", RUN_KEY, e))
}

fn read_value(name: &str) -> Option<String> {
    run_key(KEY_READ).ok()?.get_value::<String, _>(name).ok()
}

fn write_value(name: &str, value: &str) -> Result<(), String> {
    run_key(KEY_WRITE)?
        .set_value(name, &value.to_string())
        .map_err(|e| format!("Failed to write HKCU\\{}\\{}: {}", RUN_KEY, name, e))
}

fn delete_value(name: &str) -> Result<(), String> {
    let key = run_key(KEY_WRITE)?;
    match key.delete_value(name) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "Failed to remove HKCU\\{}\\{}: {}",
            RUN_KEY, name, e
        )),
    }
}

/// Absolute path to a sibling binary, so a `Run` value never depends on the
/// working directory a login shell happens to hand us.
fn absolute(bin: &Path) -> PathBuf {
    if bin.is_absolute() {
        return bin.to_path_buf();
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(bin)))
        .unwrap_or_else(|| bin.to_path_buf())
}

/// Registers the daemon to start at login, and starts it now.
///
/// The counterpart of `install_systemd_user_service`: one call gives the user a
/// daemon that is running and will keep coming back.
pub fn install_daemon_autostart(daemon_bin: &Path) -> Result<(), String> {
    let bin = absolute(daemon_bin);
    write_value(DAEMON_VALUE, &daemon_autostart_command(&bin))?;
    start_daemon(&bin)
}

/// True when the daemon is registered to start at login
pub fn daemon_autostart_installed() -> bool {
    read_value(DAEMON_VALUE).is_some()
}

/// Launches the daemon detached, with its output in the state directory.
///
/// Whether the daemon is *running* is never asked here: the GUI already knows
/// from its IPC poll, which is the check §W1-1 asks for — process enumeration
/// would find a daemon belonging to another user's session.
pub fn start_daemon(daemon_bin: &Path) -> Result<(), String> {
    let log_path =
        paths::daemon_log_path().map_err(|e| format!("Cannot resolve the daemon log path: {e}"))?;
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create {}: {}", dir.display(), e))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Failed to open {}: {}", log_path.display(), e))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("Failed to open {}: {}", log_path.display(), e))?;

    use std::os::windows::process::CommandExt;
    Command::new(daemon_bin)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log_err)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", daemon_bin.display(), e))?;
    Ok(())
}

pub fn is_gui_autostart_enabled() -> bool {
    read_value(GUI_VALUE).is_some()
}

pub fn set_gui_autostart_enabled(enabled: bool) -> Result<(), String> {
    if !enabled {
        return delete_value(GUI_VALUE);
    }
    let exe = std::env::current_exe()
        .map_err(|e| format!("Cannot locate the osu!pad executable: {}", e))?;
    write_value(GUI_VALUE, &gui_autostart_command(&exe))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_commands_are_quoted_and_match_the_installer() {
        let bin = Path::new(r"C:\Users\me\AppData\Local\Programs\osupad\osupad-daemon.exe");
        assert_eq!(
            daemon_autostart_command(bin),
            r#""C:\Users\me\AppData\Local\Programs\osupad\osupad-daemon.exe""#
        );
        let gui = Path::new(r"C:\Users\me\AppData\Local\Programs\osupad\osupad-gui.exe");
        assert_eq!(
            gui_autostart_command(gui),
            r#""C:\Users\me\AppData\Local\Programs\osupad\osupad-gui.exe" --tray"#
        );
    }
}
