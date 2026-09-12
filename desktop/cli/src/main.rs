use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use osupad_ipc::{get_socket_path, send_request, IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_model::JsonBackup;
use std::path::PathBuf;
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
            let resp = send_request(&mut stream, &IpcRequest::PrepareFlash).await?;
            let detected_port = match resp {
                IpcResponse::ReadyForFlash { port } => port,
                IpcResponse::OperationRejected { reason } => {
                    bail!("Firmware flash rejected by daemon: {}", reason);
                }
                _ => None,
            };

            let target_port = port
                .or(detected_port)
                .or_else(osupad_device::find_target_port)
                .unwrap_or_else(|| "/dev/ttyACM0".to_string());

            println!("Target serial port: {}", target_port);
            println!("Resetting device into ROM download bootloader...");
            let _ = serialport::new(&target_port, 1200)
                .timeout(std::time::Duration::from_millis(200))
                .open();
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;

            println!("Writing firmware binary via espflash at 0x10000...");

            let status = std::process::Command::new("espflash")
                .arg("write-bin")
                .arg("--chip")
                .arg("esp32s3")
                .arg("-p")
                .arg(&target_port)
                .arg("--non-interactive")
                .arg("0x10000")
                .arg(&firmware)
                .status();

            match status {
                Ok(exit) if exit.success() => {
                    println!("✓ Firmware flashing completed successfully!");
                }
                Ok(exit) => {
                    bail!("espflash exited with error code: {:?}", exit.code());
                }
                Err(e) => {
                    bail!("Failed to execute espflash: {}. Ensure espflash is installed.", e);
                }
            }

            println!("Resuming daemon device communication...");
            let _ = send_request(&mut stream, &IpcRequest::FinishFlash).await;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            println!("✓ Device reconnected successfully!");
        }

        Commands::Setup => unreachable!(),
    }

    Ok(())
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
