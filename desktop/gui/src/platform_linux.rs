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
            return Ok(PathBuf::from(home).join(".config").join("systemd").join("user"));
        }
    }
    Err("Neither $XDG_CONFIG_HOME nor $HOME environment variable is set".to_string())
}

pub fn install_systemd_user_service(daemon_bin: &Path) -> Result<(), String> {
    let service_dir = get_user_systemd_dir()?;
    std::fs::create_dir_all(&service_dir).map_err(|e| format!("Failed to create directory {}: {}", service_dir.display(), e))?;

    let abs_bin = std::fs::canonicalize(daemon_bin).unwrap_or_else(|_| daemon_bin.to_path_buf());

    let unit_content = format!(
        r#"[Unit]
Description=osu!pad background management daemon
After=network.target

[Service]
Type=simple
ExecStart={}
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
"#,
        abs_bin.display()
    );

    let unit_file = service_dir.join("osupad.service");
    std::fs::write(&unit_file, unit_content)
        .map_err(|e| format!("Failed to write {}: {}", unit_file.display(), e))?;

    // Reload user systemd daemon
    let reload_status = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .map_err(|e| format!("Failed to run systemctl --user daemon-reload: {}", e))?;

    if !reload_status.success() {
        return Err(format!("systemctl --user daemon-reload exited with error code: {:?}", reload_status.code()));
    }

    // Enable and start service
    let enable_status = Command::new("systemctl")
        .args(["--user", "enable", "--now", "osupad.service"])
        .status()
        .map_err(|e| format!("Failed to run systemctl --user enable --now osupad.service: {}", e))?;

    if !enable_status.success() {
        return Err(format!("systemctl --user enable --now osupad.service exited with error code: {:?}", enable_status.code()));
    }

    Ok(())
}
