mod esp_rom;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use osupad_ipc::{get_socket_path, send_request, IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_model::JsonBackup;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::UnixStream;

#[derive(Parser)]
#[command(name = "osupadctl", about = "osu!pad CLI management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show device and daemon status
    Status,
    /// Trigger manual counter reconciliation & time synchronization
    Sync,
    /// Reset lifetime key counters (requires deliberate confirmation)
    Reset {
        #[arg(long, help = "Confirm destructive reset of lifetime key counters")]
        yes: bool,
    },
    /// Export device configuration and lifetime counters to a JSON file
    Export {
        #[arg(help = "Path to write backup JSON file")]
        file: PathBuf,
    },
    /// Import configuration and counters from a validated JSON backup
    Import {
        #[arg(help = "Path to JSON backup file")]
        file: PathBuf,
    },
    /// Stream live device and daemon diagnostic events
    Monitor {
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
    },
    /// Flash new firmware binary onto the ESP32-S3 using espflash
    Flash {
        #[arg(help = "Path to firmware binary (.bin)")]
        firmware: PathBuf,
        #[arg(long, help = "Explicit serial port (default: auto-detect)")]
        port: Option<String>,
    },
    /// Reboot device into ROM download bootloader mode hands-free over USB
    Bootloader {
        #[arg(long, help = "Explicit serial port (default: auto-detect)")]
        port: Option<String>,
    },
    /// Show key-press-to-HID latency measured on the device
    Latency {
        #[arg(long, help = "Clear the collected samples (e.g. right before playing a map)")]
        reset: bool,
    },
    /// Perform initial system setup (udev permissions & directories)
    Setup,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Setup = cli.command {
        return run_setup();
    }

    let socket_path = get_socket_path();
    let mut stream = UnixStream::connect(&socket_path)
        .await
        .with_context(|| format!("Failed to connect to osupad-daemon at {}. Is the daemon running?", socket_path.display()))?;

    // Handshake
    let handshake_req = IpcRequest::Handshake {
        client_version: "1.0.0".to_string(),
        client_protocol: IPC_PROTOCOL_VERSION,
    };
    let _ = send_request(&mut stream, &handshake_req).await?;

    match cli.command {
        Commands::Status => {
            let resp = send_request(&mut stream, &IpcRequest::GetStatus).await?;
            if let IpcResponse::Status {
                mode,
                device_connected,
                device_info,
                counters,
                config,
                last_sync_time,
                tosu_connected,
                latency,
            } = resp
            {
                println!("=== osu!pad Status ===");
                println!("Daemon Mode:      {:?}", mode);
                println!("ESP32 Device:     {}", if device_connected { "Connected" } else { "Disconnected" });
                if let Some(info) = device_info {
                    println!("Device ID:        {}", info.device_id);
                    println!("Board Profile:    {}", info.board_profile);
                    println!("Firmware Version: {}", info.firmware_version);
                }
                println!("Key 1 (K1):       {} (Total: {} presses)", config.key1_char(), counters.lifetime_key1);
                println!("Key 2 (K2):       {} (Total: {} presses)", config.key2_char(), counters.lifetime_key2);
                println!("Total Presses:    {}", counters.total_lifetime_presses());
                println!("Generation:       {}", counters.counter_generation);
                println!("Last Sync:        {}", last_sync_time.as_deref().unwrap_or("Never"));
                println!("tosu (osu!lazer): {}", if tosu_connected { "Active" } else { "Offline" });
                if let Some(l) = latency {
                    println!("Key Latency:      p50 {}µs, p99.9 {}µs, max {}µs ({} samples)", l.p50_us, l.p999_us, l.max_us, l.samples);
                }
                println!("Debounce Lockout: {} µs", config.debounce_us);
                println!("Brightness:       {}%", config.brightness);
                println!("Sleep Timeout:    {}s", config.display_sleep_seconds);
            }
        }

        Commands::Sync => {
            println!("Requesting safe counter & clock synchronization...");
            let resp = send_request(&mut stream, &IpcRequest::ForceSync).await?;
            match resp {
                IpcResponse::SyncCompleted { success: true, counters } => {
                    println!("✓ Sync succeeded!");
                    println!("  K1: {}", counters.lifetime_key1);
                    println!("  K2: {}", counters.lifetime_key2);
                }
                IpcResponse::OperationRejected { reason } => {
                    println!("✗ Operation rejected: {}", reason);
                }
                _ => println!("✗ Sync failed: {:?}", resp),
            }
        }

        Commands::Reset { yes } => {
            if !yes {
                bail!("Resetting counters is permanent! Pass --yes to confirm: osupadctl reset --yes");
            }
            let resp = send_request(&mut stream, &IpcRequest::ResetCounters).await?;
            match resp {
                IpcResponse::CountersReset { counters } => {
                    println!("✓ Counters reset successfully (new generation: {})", counters.counter_generation);
                }
                IpcResponse::OperationRejected { reason } => {
                    println!("✗ Rejected: {}", reason);
                }
                _ => println!("✗ Failed: {:?}", resp),
            }
        }

        Commands::Export { file } => {
            let resp = send_request(&mut stream, &IpcRequest::ExportBackup).await?;
            if let IpcResponse::BackupExported(backup) = resp {
                let json_data = serde_json::to_string_pretty(&backup)?;
                std::fs::write(&file, json_data)
                    .with_context(|| format!("Failed to write to file {}", file.display()))?;
                println!("✓ Backup exported to {}", file.display());
            } else {
                bail!("Unexpected response from daemon: {:?}", resp);
            }
        }

        Commands::Import { file } => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file {}", file.display()))?;
            let backup: JsonBackup = serde_json::from_str(&text)
                .context("Failed to parse JSON backup file")?;

            backup.validate().map_err(|e| anyhow::anyhow!("Validation error: {}", e))?;

            println!("Backup Summary to Import:");
            println!("  Device ID: {}", backup.device.device_id);
            println!("  Key 1:     {} ({} presses)", backup.config.key1, backup.stats.lifetime_key1);
            println!("  Key 2:     {} ({} presses)", backup.config.key2, backup.stats.lifetime_key2);

            let resp = send_request(&mut stream, &IpcRequest::ImportBackup(backup)).await?;
            match resp {
                IpcResponse::BackupImported { success: true, .. } => {
                    println!("✓ Backup imported and synchronized with device successfully!");
                }
                IpcResponse::OperationRejected { reason } => {
                    println!("✗ Rejected: {}", reason);
                }
                IpcResponse::Error(e) => {
                    println!("✗ Error: {}", e);
                }
                _ => println!("✗ Failed: {:?}", resp),
            }
        }

        Commands::Monitor { limit } => {
            let resp = send_request(&mut stream, &IpcRequest::GetLogEntries { limit }).await?;
            if let IpcResponse::LogEntries(entries) = resp {
                println!("=== osu!pad Monitor (Last {} entries) ===", entries.len());
                for line in entries.iter().rev() {
                    println!("{}", line);
                }
            }
        }

        Commands::Flash { firmware, port } => {
            if !firmware.exists() {
                bail!("Firmware binary file does not exist: {}", firmware.display());
            }

            println!("Coordinating with osupad-daemon for firmware flashing...");
            let app_port = prepare_flash(&mut stream, port).await?;
            // Always hand the port back to the daemon, even if flashing failed
            let result = flash_firmware(&firmware, app_port.as_deref()).await;
            println!("Resuming daemon device communication...");
            let _ = send_request(&mut stream, &IpcRequest::FinishFlash).await;
            result?;

            println!("Waiting for osu!pad to boot the new firmware...");
            match wait_for_port(osupad_device::find_target_port, Duration::from_secs(10)).await {
                Some(p) => println!("✓ Firmware flashed; osu!pad is back on {}", p),
                None => bail!("Flash succeeded but the osu!pad app (303a:4001) did not come back within 10s"),
            }
        }

        Commands::Bootloader { port } => {
            println!("Coordinating with osupad-daemon...");
            let app_port = prepare_flash(&mut stream, port).await?;
            let result = enter_bootloader(app_port.as_deref()).await;
            // The daemon only opens the app port (303a:4001), so resuming now cannot
            // interfere with the bootloader; it reconnects once the app is flashed
            let _ = send_request(&mut stream, &IpcRequest::FinishFlash).await;
            let boot_port = result?;
            println!("✓ Device is in ROM download mode on {}", boot_port);
        }

        Commands::Latency { reset } => {
            if reset {
                match send_request(&mut stream, &IpcRequest::ResetLatencyStats).await? {
                    IpcResponse::Error(e) => bail!("{}", e),
                    _ => println!("✓ Latency statistics reset"),
                }
            } else {
                match send_request(&mut stream, &IpcRequest::GetStatus).await? {
                    IpcResponse::Status { latency: Some(l), .. } => {
                        println!("=== Key edge -> HID submit latency (device-side) ===");
                        println!("Samples:          {}", l.samples);
                        println!("p50:              {} µs", l.p50_us);
                        println!("p99:              {} µs", l.p99_us);
                        println!("p99.9:            {} µs", l.p999_us);
                        println!("max:              {} µs", l.max_us);
                        println!("Dropped reports:  {}", l.dropped_reports);
                    }
                    IpcResponse::Status { latency: None, .. } => {
                        println!("No latency data yet (device not connected, or old firmware)");
                    }
                    other => bail!("Unexpected response from daemon: {:?}", other),
                }
            }
        }

        Commands::Setup => unreachable!(),
    }

    Ok(())
}

/// Ask the daemon to release the serial port. Returns the app port to trigger,
/// or None if the device is already sitting in the ROM bootloader.
async fn prepare_flash(stream: &mut UnixStream, explicit_port: Option<String>) -> Result<Option<String>> {
    match send_request(stream, &IpcRequest::PrepareFlash).await? {
        IpcResponse::ReadyForFlash { port } => Ok(explicit_port.or(port).or_else(osupad_device::find_target_port)),
        IpcResponse::OperationRejected { reason } => bail!("Rejected by daemon: {}", reason),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

/// Reboot the running app into the ROM download bootloader and return the bootloader port.
async fn enter_bootloader(app_port: Option<&str>) -> Result<String> {
    if let Some(p) = osupad_device::find_bootloader_port() {
        return Ok(p);
    }
    let Some(app_port) = app_port else {
        bail!("osu!pad not found: neither the app (303a:4001) nor the ROM bootloader (303a:1001) is connected");
    };

    println!("Sending bootloader reboot trigger to {}...", app_port);
    // Firmware reacts to the "BOOTLOADER" command and to a 1200-baud touch; send both
    let mut sp = open_port_with_retry(app_port, 1200, Duration::from_secs(2)).await?;
    sp.write_all(b"BOOTLOADER\n").context("Failed to send bootloader trigger")?;
    let _ = sp.flush();
    drop(sp);

    wait_for_port(osupad_device::find_bootloader_port, Duration::from_secs(6))
        .await
        .context("Device did not re-enumerate as the ROM bootloader (303a:1001) within 6s")
}

async fn flash_firmware(firmware: &Path, app_port: Option<&str>) -> Result<()> {
    let boot_port = enter_bootloader(app_port).await?;
    println!("Writing firmware binary via espflash at 0x10000 on {}...", boot_port);

    let status = std::process::Command::new("espflash")
        .args(["write-bin", "--chip", "esp32s3", "-p", &boot_port])
        // Already in download mode. Stay in the stub afterwards: espflash cannot reset an
        // ESP32-S3 out of forced download mode, esp_rom::reset_to_app does that below
        .args(["--before", "no-reset", "--after", "no-reset-no-stub", "--non-interactive", "0x10000"])
        .arg(firmware)
        .status()
        .context("Failed to execute espflash. Ensure espflash is installed.")?;
    if !status.success() {
        bail!("espflash exited with error code: {:?}", status.code());
    }
    println!("✓ Firmware written, rebooting into application...");
    esp_rom::reset_to_app(&boot_port).context("Firmware written, but failed to reboot the device")

}

async fn open_port_with_retry(path: &str, baud: u32, timeout: Duration) -> Result<Box<dyn serialport::SerialPort>> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match serialport::new(path, baud).timeout(Duration::from_millis(300)).open() {
            Ok(sp) => return Ok(sp),
            Err(e) if tokio::time::Instant::now() >= deadline => {
                return Err(e).with_context(|| format!("Failed to open {}", path));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

async fn wait_for_port(find: fn() -> Option<String>, timeout: Duration) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if let Some(p) = find() {
            return Some(p);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn run_setup() -> Result<()> {
    println!("=== osu!pad Linux Setup ===");
    let udev_rule = r#"# /etc/udev/rules.d/99-osupad.rules
# Espressif ESP32-S3 USB JTAG / Serial / CDC
SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", MODE="0666", GROUP="uucp", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="303a", MODE="0666", GROUP="uucp", TAG+="uaccess"
"#;

    let target_path = "/etc/udev/rules.d/99-osupad.rules";
    println!("Recommended udev rule for non-root CDC access:");
    println!("{}", udev_rule);

    if std::path::Path::new("/etc/udev/rules.d").exists() {
        println!("To install this rule, run:");
        println!("  sudo cp packaging/linux/udev/99-osupad.rules {}", target_path);
        println!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
    }

    println!("Setup check completed.");
    Ok(())
}
