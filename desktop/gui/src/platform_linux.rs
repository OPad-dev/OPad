use std::path::{Path, PathBuf};
use std::process::Command;

pub fn get_user_systemd_dir() -> Result<PathBuf, String> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        if !config_home.trim().is_empty() {
            return Ok(PathBuf::from(config_home).join("systemd").join("user"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Ok(PathBuf::from(home)
                .join(".config")
                .join("systemd")
                .join("user"));
        }
    }
    Err("Neither $XDG_CONFIG_HOME nor $HOME environment variable is set".to_string())
}

/// The unit shipped in packaging/, so the GUI and install.sh manage the same service
const SERVICE_NAME: &str = "opad-daemon.service";
const PACKAGED_UNIT: &str =
    include_str!("../../../packaging/linux/systemd-user/opad-daemon.service");
/// Written by earlier GUI builds under another name; removed so two units never start two daemons
const LEGACY_SERVICE_NAME: &str = "osupad.service";

fn systemctl_user(args: &[&str]) -> Result<(), String> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .map_err(|e| format!("Failed to run systemctl --user {}: {}", args.join(" "), e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "systemctl --user {} exited with code {:?}",
            args.join(" "),
            status.code()
        ))
    }
}

/// True when a systemd user manager is reachable and knows the OPad unit
fn service_installed() -> bool {
    Command::new("systemctl")
        .args(["--user", "cat", SERVICE_NAME])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn install_systemd_user_service(daemon_bin: &Path) -> Result<(), String> {
    let service_dir = get_user_systemd_dir()?;
    std::fs::create_dir_all(&service_dir).map_err(|e| {
        format!(
            "Failed to create directory {}: {}",
            service_dir.display(),
            e
        )
    })?;

    let legacy_unit = service_dir.join(LEGACY_SERVICE_NAME);
    if legacy_unit.exists() {
        let _ = systemctl_user(&["disable", "--now", LEGACY_SERVICE_NAME]);
        let _ = std::fs::remove_file(&legacy_unit);
    }

    // Point ExecStart at the binary next to this GUI, or the running AppImage
    let exec_start = if let Ok(appimage) = std::env::var("APPIMAGE") {
        format!("\"{}\" daemon", appimage)
    } else {
        let abs_bin =
            std::fs::canonicalize(daemon_bin).unwrap_or_else(|_| daemon_bin.to_path_buf());
        format!("\"{}\"", abs_bin.display())
    };
    let unit_content: String = PACKAGED_UNIT
        .lines()
        .map(|line| {
            if line.starts_with("ExecStart=") {
                format!("ExecStart={}\n", exec_start)
            } else {
                format!("{}\n", line)
            }
        })
        .collect();

    let unit_file = service_dir.join(SERVICE_NAME);
    std::fs::write(&unit_file, unit_content)
        .map_err(|e| format!("Failed to write {}: {}", unit_file.display(), e))?;

    systemctl_user(&["daemon-reload"])?;
    systemctl_user(&["enable", "--now", SERVICE_NAME])
}

/// Starts the daemon through its user service when installed, so systemd supervises and logs it.
/// Otherwise launches it detached from the GUI with output in opad_model::paths::state_dir()/daemon.log.
pub fn start_daemon(daemon_bin: &Path) -> Result<(), String> {
    if service_installed() {
        return systemctl_user(&["start", SERVICE_NAME]);
    }

    let log_dir = opad_model::paths::state_dir()
        .map_err(|e| format!("Failed to resolve state directory: {}", e))?;
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| format!("Failed to create {}: {}", log_dir.display(), e))?;
    let log_path = log_dir.join("daemon.log");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("Failed to open {}: {}", log_path.display(), e))?;
    let log_err = log
        .try_clone()
        .map_err(|e| format!("Failed to open {}: {}", log_path.display(), e))?;

    use std::os::unix::process::CommandExt;
    let mut cmd = if let Ok(appimage) = std::env::var("APPIMAGE") {
        let mut c = Command::new(appimage);
        c.arg("daemon");
        c
    } else {
        Command::new(daemon_bin)
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log_err)
        // Own process group: closing the GUI's terminal or session scope does not signal it
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {}", e))?;
    Ok(())
}

pub fn get_user_autostart_dir() -> Result<PathBuf, String> {
    if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        if !config_home.trim().is_empty() {
            return Ok(PathBuf::from(config_home).join("autostart"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Ok(PathBuf::from(home).join(".config").join("autostart"));
        }
    }
    Err("Neither $XDG_CONFIG_HOME nor $HOME environment variable is set".to_string())
}

/// Installed by the .deb/.rpm and `make install`: starts the tray for every user
const SYSTEM_AUTOSTART: &str = "/etc/xdg/autostart/opad-gui.desktop";
const AUTOSTART_FILE: &str = "opad-gui.desktop";

/// Whether the GUI starts at login. Per the XDG autostart spec a user file of
/// the same name overrides the system one, and `Hidden=true` in it disables it.
fn autostart_enabled_in(user_file: &Path, system_file: &Path) -> bool {
    match std::fs::read_to_string(user_file) {
        Ok(content) => !content.lines().any(|l| l.trim() == "Hidden=true"),
        Err(_) => system_file.exists(),
    }
}

fn set_autostart_in(
    user_file: &Path,
    system_file: &Path,
    enabled: bool,
    exe: &Path,
) -> Result<(), String> {
    let content = if enabled {
        format!(
            "[Desktop Entry]\nType=Application\nName=OPad Tray\nComment=OPad configuration and system tray applet\nExec=\"{}\" --tray\nIcon=input-keyboard\nTerminal=false\nCategories=Utility;HardwareSettings;\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        )
    } else if system_file.exists() {
        // Deleting ours would let the system entry start it anyway
        "[Desktop Entry]\nType=Application\nName=OPad Tray\nHidden=true\n".to_string()
    } else {
        if user_file.exists() {
            std::fs::remove_file(user_file)
                .map_err(|e| format!("Failed to remove {}: {}", user_file.display(), e))?;
        }
        return Ok(());
    };
    if let Some(dir) = user_file.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
    }
    std::fs::write(user_file, content)
        .map_err(|e| format!("Failed to write {}: {}", user_file.display(), e))
}

pub fn is_gui_autostart_enabled() -> bool {
    get_user_autostart_dir()
        .map(|dir| autostart_enabled_in(&dir.join(AUTOSTART_FILE), Path::new(SYSTEM_AUTOSTART)))
        .unwrap_or(false)
}

pub fn set_gui_autostart_enabled(enabled: bool) -> Result<(), String> {
    let dir = get_user_autostart_dir()?;
    // The AppImage's own path: its mount point changes on every run
    let exe = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("opad-gui"));
    set_autostart_in(
        &dir.join(AUTOSTART_FILE),
        Path::new(SYSTEM_AUTOSTART),
        enabled,
        &exe,
    )
}

/// Whether the pad's udev rule is installed anywhere udev reads rules from
pub fn udev_rule_installed() -> bool {
    [
        "/etc/udev/rules.d/70-opad.rules",
        "/usr/lib/udev/rules.d/70-opad.rules",
        "/lib/udev/rules.d/70-opad.rules",
    ]
    .iter()
    .any(|p| Path::new(p).exists())
}

/// The AppImage installs nothing system-wide, so it offers to install the rule
/// itself (AppRun's `install-udev`, which asks for a password via pkexec).
/// None when not running as an AppImage or the rule is already there.
pub fn appimage_needing_udev_rule() -> Option<PathBuf> {
    let appimage = std::env::var_os("APPIMAGE").map(PathBuf::from)?;
    (!udev_rule_installed()).then_some(appimage)
}

pub fn install_udev_rule_from_appimage(appimage: &Path) -> Result<(), String> {
    let status = Command::new(appimage)
        .arg("install-udev")
        .status()
        .map_err(|e| format!("Failed to run {}: {}", appimage.display(), e))?;
    if status.success() && udev_rule_installed() {
        Ok(())
    } else {
        Err("The udev rule was not installed (cancelled, or pkexec is unavailable)".to_string())
    }
}

#[cfg(test)]
mod autostart_tests {
    use super::*;

    fn dirs(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("opad-autostart-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("etc")).unwrap();
        (
            root.join("user/opad-gui.desktop"),
            root.join("etc/opad-gui.desktop"),
            root,
        )
    }

    #[test]
    fn disabling_hides_a_system_wide_entry_instead_of_deleting_ours() {
        let (user, system, root) = dirs("system");
        std::fs::write(&system, "[Desktop Entry]\nExec=/usr/bin/opad-gui --tray\n").unwrap();
        assert!(
            autostart_enabled_in(&user, &system),
            "the system entry starts it"
        );

        set_autostart_in(&user, &system, false, Path::new("/usr/bin/opad-gui")).unwrap();
        assert!(!autostart_enabled_in(&user, &system));
        assert!(std::fs::read_to_string(&user)
            .unwrap()
            .contains("Hidden=true"));

        set_autostart_in(&user, &system, true, Path::new("/usr/bin/opad-gui")).unwrap();
        assert!(autostart_enabled_in(&user, &system));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn without_a_system_entry_disabling_removes_ours() {
        let (user, system, root) = dirs("user");
        let exe = Path::new("/opt/My Apps/OPad.AppImage");
        set_autostart_in(&user, &system, true, exe).unwrap();
        let content = std::fs::read_to_string(&user).unwrap();
        assert!(content.contains("Exec=\"/opt/My Apps/OPad.AppImage\" --tray"));
        assert!(autostart_enabled_in(&user, &system));

        set_autostart_in(&user, &system, false, exe).unwrap();
        assert!(!user.exists());
        assert!(!autostart_enabled_in(&user, &system));
        let _ = std::fs::remove_dir_all(root);
    }
}
