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
const SERVICE_NAME: &str = "osupad-daemon.service";
const PACKAGED_UNIT: &str =
    include_str!("../../../packaging/linux/systemd-user/osupad-daemon.service");
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

    // Point ExecStart at the binary next to this GUI, which may not be ~/.local/bin
    let abs_bin = std::fs::canonicalize(daemon_bin).unwrap_or_else(|_| daemon_bin.to_path_buf());
    let unit_content: String = PACKAGED_UNIT
        .lines()
        .map(|line| {
            if line.starts_with("ExecStart=") {
                format!("ExecStart={}\n", abs_bin.display())
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
/// Otherwise launches it detached from the GUI with output in $XDG_STATE_HOME/osupad/daemon.log.
pub fn start_daemon(daemon_bin: &Path) -> Result<(), String> {
    if service_installed() {
        return systemctl_user(&["start", SERVICE_NAME]);
    }

    let log_dir = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(std::env::temp_dir)
        .join("osupad");
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
    Command::new(daemon_bin)
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(log_err)
        // Own process group: closing the GUI's terminal or session scope does not signal it
        .process_group(0)
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", daemon_bin.display(), e))?;
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

pub fn is_gui_autostart_enabled() -> bool {
    if let Ok(dir) = get_user_autostart_dir() {
        dir.join("osupad-gui.desktop").exists()
    } else {
        false
    }
}

pub fn set_gui_autostart_enabled(enabled: bool) -> Result<(), String> {
    let dir = get_user_autostart_dir()?;
    let path = dir.join("osupad-gui.desktop");
    if enabled {
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create directory {}: {}", dir.display(), e))?;
        let content = "[Desktop Entry]\nType=Application\nName=OPad Tray\nComment=OPad configuration and system tray applet\nExec=osupad-gui --tray\nIcon=input-keyboard\nTerminal=false\nCategories=Utility;HardwareSettings;\nX-GNOME-Autostart-enabled=true\n";
        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    } else if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}
