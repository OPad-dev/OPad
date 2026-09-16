mod esp_rom;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use osupad_ipc::{send_request, IpcRequest, IpcResponse, IpcStream, IPC_PROTOCOL_VERSION};
use osupad_model::JsonBackup;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
        #[arg(long, help = "Skip preview and confirmation prompt")]
        yes: bool,
    },
    /// Stream live device and daemon diagnostic events
    Monitor {
        #[arg(short, long, default_value_t = 50)]
        limit: usize,
        #[arg(short, long, help = "Follow log stream in real time")]
        follow: bool,
        #[arg(long, help = "Filter by severity level (debug, info, warn, error)")]
        level: Option<String>,
        #[arg(long, help = "Filter by source (host, esp)")]
        source: Option<String>,
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
        #[arg(
            long,
            help = "Clear the collected samples (e.g. right before playing a map)"
        )]
        reset: bool,
    },
    /// Perform initial system setup (udev permissions & directories)
    Setup,
}

/// Where the app image lives, i.e. `ota_0` in `firmware/partitions.csv`
/// (§U-3a). It was `0x10000` under the old single-app table; a pad flashed at
/// the old offset with the new table will not boot.
const APP_PARTITION_OFFSET: u32 = 0x20000;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Setup = cli.command {
        return run_setup();
    }

    let (mut stream, _) = osupad_ipc::connect_and_handshake()
        .await
        .with_context(|| "Failed to connect to osupad-daemon. Is the daemon running?")?;

    match cli.command {
        Commands::Status => {
            let resp = send_request(&mut stream, &IpcRequest::GetStatus).await?;
            if let IpcResponse::Status {
                mode,
                device_connected,
                device_info,
                counters,
                counters_source,
                config,
                last_sync_time,
                last_sync_error,
                storage_error,
                tosu_connected,
                latency,
                pending_replacement,
                incompatible,
                ..
            } = resp
            {
                println!("=== osu!pad Status ===");
                println!("Daemon Mode:      {:?}", mode);
                println!(
                    "ESP32 Device:     {}",
                    if device_connected {
                        "Connected"
                    } else {
                        "Disconnected"
                    }
                );
                println!("Counters Source:  {:?}", counters_source);
                if let Some(info) = device_info {
                    println!("Device ID:        {}", info.device_id);
                    println!("Board Profile:    {}", info.board_profile);
                    println!("Firmware Version: {}", info.firmware_version);
                    // §U-3a. Old firmware does not report it at all, and that
                    // is worth seeing: it means the pad is still on the
                    // single-app table and needs a serial reflash.
                    println!(
                        "Running Slot:     {}",
                        info.running_partition
                            .as_deref()
                            .unwrap_or("unknown (firmware predates the OTA layout)")
                    );
                }
                println!(
                    "Key 1 (K1):       {} (Total: {} presses)",
                    config.key1_char(),
                    counters.lifetime_key1
                );
                println!(
                    "Key 2 (K2):       {} (Total: {} presses)",
                    config.key2_char(),
                    counters.lifetime_key2
                );
                println!("Total Presses:    {}", counters.total_lifetime_presses());
                println!("Generation:       {}", counters.counter_generation);
                println!(
                    "Last Sync:        {}",
                    last_sync_time.as_deref().unwrap_or("Never")
                );
                if let Some(err) = last_sync_error {
                    println!("Last Sync Error:  ⚠ {}", err);
                }
                if let Some(err) = storage_error {
                    println!("Database Error:   ⚠ {}", err);
                }
                if let Some(old_id) = pending_replacement {
                    println!("Replacement:      ⚠ New pad detected (previous: {}). Run GUI to restore or adopt.", old_id);
                }
                if let Some(incompat) = incompatible {
                    println!("Incompatible:     ⚠ Device protocol {} incompatible with daemon protocol {}. Update firmware or host.", incompat.protocol_version, IPC_PROTOCOL_VERSION);
                }
                println!(
                    "tosu (osu!lazer): {}",
                    if tosu_connected { "Active" } else { "Offline" }
                );
                if let Some(l) = latency {
                    println!(
                        "Key Latency:      p50 {}µs, p99.9 {}µs, max {}µs ({} samples)",
                        l.p50_us, l.p999_us, l.max_us, l.samples
                    );
                }
                println!(
                    "Key Pins:         K1 GPIO{}, K2 GPIO{}",
                    config.key1_gpio, config.key2_gpio
                );
                println!("Debounce Lockout: {} µs", config.debounce_us);
                println!("Brightness:       {}%", config.brightness);
                println!("Sleep Timeout:    {}s", config.display_sleep_seconds);
            }
        }

        Commands::Sync => {
            println!("Requesting safe counter & clock synchronization...");
            let resp = send_request(&mut stream, &IpcRequest::ForceSync).await?;
            match resp {
                IpcResponse::SyncCompleted {
                    success: true,
                    counters,
                } => {
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
                bail!(
                    "Resetting counters is permanent! Pass --yes to confirm: osupadctl reset --yes"
                );
            }
            let resp =
                send_request(&mut stream, &IpcRequest::ResetCounters { confirm: yes }).await?;
            match resp {
                IpcResponse::CountersReset { counters } => {
                    println!(
                        "✓ Counters reset successfully (new generation: {})",
                        counters.counter_generation
                    );
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

        Commands::Import { file, yes } => {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("Failed to read file {}", file.display()))?;
            let backup: JsonBackup =
                serde_json::from_str(&text).context("Failed to parse JSON backup file")?;

            backup
                .validate()
                .map_err(|e| anyhow::anyhow!("Validation error: {}", e))?;

            // Preview via IPC (§P1-5)
            let preview_resp =
                send_request(&mut stream, &IpcRequest::PreviewImport(backup.clone())).await?;
            match preview_resp {
                IpcResponse::ImportPreview {
                    current,
                    incoming,
                    device_id_matches,
                    is_counter_rollback,
                    warnings,
                } => {
                    println!("=== Import Preview ===");
                    if let Some(cur) = current {
                        println!("Current:  Device ID: {}", cur.device_id);
                        println!(
                            "          Key 1: {} ({} presses)",
                            cur.config.key1_char(),
                            cur.lifetime_key1
                        );
                        println!(
                            "          Key 2: {} ({} presses)",
                            cur.config.key2_char(),
                            cur.lifetime_key2
                        );
                    }
                    println!("Incoming: Device ID: {}", incoming.device.device_id);
                    println!(
                        "          Key 1: {} ({} presses)",
                        incoming.config.key1, incoming.stats.lifetime_key1
                    );
                    println!(
                        "          Key 2: {} ({} presses)",
                        incoming.config.key2, incoming.stats.lifetime_key2
                    );

                    if !device_id_matches {
                        println!("\n  ⚠ Note: Backup device ID does not match current pad.");
                    }
                    if is_counter_rollback {
                        println!("  ⚠ Warning: Counter rollback detected.");
                    }
                    if !warnings.is_empty() {
                        println!("\nWarnings:");
                        for w in &warnings {
                            println!("  ⚠ {}", w);
                        }
                    }

                    if !yes {
                        use std::io::IsTerminal;
                        if !std::io::stdin().is_terminal() {
                            bail!("Confirmation required to import backup. Run with --yes in non-interactive mode.");
                        }
                        print!("\nProceed with import? [y/N]: ");
                        std::io::stdout().flush()?;
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        let trimmed = input.trim().to_lowercase();
                        if trimmed != "y" && trimmed != "yes" {
                            println!("Import cancelled.");
                            return Ok(());
                        }
                    }

                    let resp = send_request(
                        &mut stream,
                        &IpcRequest::ImportBackup {
                            backup,
                            confirm: true,
                        },
                    )
                    .await?;
                    match resp {
                        IpcResponse::BackupImported {
                            success: true,
                            counters,
                            ..
                        } => {
                            println!(
                                "✓ Backup imported and synchronized successfully (generation: {})!",
                                counters.counter_generation
                            );
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
                IpcResponse::OperationRejected { reason } => {
                    println!("✗ Preview rejected: {}", reason);
                }
                IpcResponse::Error(e) => {
                    println!("✗ Preview error: {}", e);
                }
                other => {
                    bail!("Unexpected preview response: {:?}", other);
                }
            }
        }

        Commands::Monitor {
            limit,
            follow,
            level,
            source,
        } => {
            let filter_level = level
                .as_deref()
                .and_then(|l| match l.to_lowercase().as_str() {
                    "debug" => Some(osupad_model::LogLevel::Debug),
                    "info" => Some(osupad_model::LogLevel::Info),
                    "warn" | "warning" => Some(osupad_model::LogLevel::Warn),
                    "error" => Some(osupad_model::LogLevel::Error),
                    _ => None,
                });
            let filter_source = source
                .as_deref()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "host" => Some(osupad_model::LogSource::Host),
                    "esp" | "device" => Some(osupad_model::LogSource::Esp),
                    _ => None,
                });

            let mut since_seq = None;
            let mut first_batch = true;

            loop {
                let resp =
                    send_request(&mut stream, &IpcRequest::GetLogEntries { since_seq, limit })
                        .await?;
                if let IpcResponse::LogEntries {
                    entries,
                    latest_seq,
                } = resp
                {
                    if first_batch && !follow {
                        println!("=== osu!pad Monitor (Last {} entries) ===", entries.len());
                    }
                    for entry in &entries {
                        if let Some(fl) = filter_level {
                            if entry.level < fl {
                                continue;
                            }
                        }
                        if let Some(fs) = filter_source {
                            if entry.source != fs {
                                continue;
                            }
                        }
                        println!("{}", entry.format_line());
                    }
                    if latest_seq > 0 {
                        since_seq = Some(latest_seq);
                    }
                }
                first_batch = false;

                if !follow {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        Commands::Flash { firmware, port } => {
            if !firmware.exists() {
                bail!(
                    "Firmware binary file does not exist: {}",
                    firmware.display()
                );
            }
            validate_esp32s3_image(&firmware)?;

            println!("Coordinating with osupad-daemon for firmware flashing...");
            let app_port = prepare_flash(&mut stream, port).await?;
            // Always hand the port back to the daemon, even if flashing failed
            let result = flash_firmware(&firmware, app_port.as_deref()).await;
            println!("Resuming daemon device communication and verifying new firmware...");
            let finish_resp = send_request(&mut stream, &IpcRequest::FinishFlash).await?;
            result?;

            match finish_resp {
                IpcResponse::FlashFinished {
                    firmware_version,
                    protocol_version,
                    compatible,
                } => {
                    println!("✓ Flash succeeded!");
                    println!("  Firmware Version: {}", firmware_version);
                    println!("  Protocol Version: {}", protocol_version);
                    if !compatible {
                        println!("  ⚠ WARNING: Device reported protocol version {} which is incompatible with host!", protocol_version);
                    }
                }
                IpcResponse::Error(e) => {
                    println!("⚠ Post-flash verification warning: {}", e);
                }
                _ => {}
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
                    IpcResponse::Status {
                        latency: Some(l), ..
                    } => {
                        println!("=== Key edge -> HID submit latency (device-side) ===");
                        println!("Samples:          {}", l.samples);
                        println!("p50:              {} µs", l.p50_us);
                        println!("p99:              {} µs", l.p99_us);
                        println!("p99.9:            {} µs", l.p999_us);
                        println!("max:              {} µs", l.max_us);
                        println!(
                            "Deferred reports: {} (waited for the next USB poll, then sent)",
                            l.deferred_reports
                        );
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
async fn prepare_flash(
    stream: &mut IpcStream,
    explicit_port: Option<String>,
) -> Result<Option<String>> {
    match send_request(stream, &IpcRequest::PrepareFlash).await? {
        IpcResponse::ReadyForFlash { port } => Ok(explicit_port
            .or(port)
            .or_else(osupad_device::find_target_port)),
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
    sp.write_all(b"BOOTLOADER\n")
        .context("Failed to send bootloader trigger")?;
    let _ = sp.flush();
    drop(sp);

    wait_for_port(osupad_device::find_bootloader_port, Duration::from_secs(6))
        .await
        .context("Device did not re-enumerate as the ROM bootloader (303a:1001) within 6s")
}

async fn flash_firmware(firmware: &Path, app_port: Option<&str>) -> Result<()> {
    let boot_port = enter_bootloader(app_port).await?;
    println!(
        "Writing firmware binary via espflash at {:#x} on {}...",
        APP_PARTITION_OFFSET, boot_port
    );

    let status = std::process::Command::new("espflash")
        .args(["write-bin", "--chip", "esp32s3", "-p", &boot_port])
        // Already in download mode. Stay in the stub afterwards: espflash cannot reset an
        // ESP32-S3 out of forced download mode, esp_rom::reset_to_app does that below
        .args([
            "--before",
            "no-reset",
            "--after",
            "no-reset-no-stub",
            "--non-interactive",
        ])
        .arg(format!("{:#x}", APP_PARTITION_OFFSET))
        .arg(firmware)
        .status()
        .context("Failed to execute espflash. Ensure espflash is installed.")?;
    if !status.success() {
        bail!("espflash exited with error code: {:?}", status.code());
    }
    println!("✓ Firmware written, rebooting into application...");
    esp_rom::reset_to_app(&boot_port).context("Firmware written, but failed to reboot the device")
}

async fn open_port_with_retry(
    path: &str,
    baud: u32,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match serialport::new(path, baud)
            .timeout(Duration::from_millis(300))
            .open()
        {
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
    // udev is Linux's; on Windows the pad binds to inbox drivers with no setup
    // step at all, so saying nothing would read as "something is missing".
    if !cfg!(target_os = "linux") {
        println!("=== osu!pad Setup ===");
        println!("Nothing to do on this platform: the pad uses the inbox USB");
        println!("drivers, so HID and the CDC port work with no setup step.");
        return Ok(());
    }

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
        println!(
            "  sudo cp packaging/linux/udev/99-osupad.rules {}",
            target_path
        );
        println!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
    }

    println!("Setup check completed.");
    Ok(())
}

/// Validates that a file is an ESP32-S3 app image (§32)
/// Magic byte must be 0xE9, and chip ID at offset 12..13 must be 0x0009 (ESP32-S3).
fn validate_esp32s3_image(path: &Path) -> Result<()> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("Failed to open firmware image: {}", path.display()))?;
    let mut header = [0u8; 16];
    let n = f
        .read(&mut header)
        .context("Failed to read firmware image header")?;
    if n < 16 {
        bail!("Firmware file is too small to be a valid ESP32 image (less than 16 bytes)");
    }
    if header[0] != 0xE9 {
        bail!(
            "Invalid image magic byte: 0x{:02X} (expected 0xE9 for ESP image)",
            header[0]
        );
    }
    let chip_id = u16::from_le_bytes([header[12], header[13]]);
    const ESP32S3_CHIP_ID: u16 = 0x0009;
    if chip_id != ESP32S3_CHIP_ID {
        bail!(
            "Firmware binary is built for chip ID 0x{:04X}, but osu!pad requires ESP32-S3 (chip ID 0x{:04X})",
            chip_id,
            ESP32S3_CHIP_ID
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_test_file(name: &str, content: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("osupadctl-test-{}-{}", std::process::id(), name));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn test_validate_valid_esp32s3_image() {
        let mut header = [0u8; 32];
        header[0] = 0xE9; // Magic byte
        header[12] = 0x09; // ESP32-S3 chip ID low byte
        header[13] = 0x00; // ESP32-S3 chip ID high byte
        let path = make_test_file("valid.bin", &header);

        let res = validate_esp32s3_image(&path);
        let _ = std::fs::remove_file(&path);
        assert!(res.is_ok());
    }

    #[test]
    fn test_validate_wrong_magic_rejected() {
        let mut header = [0u8; 32];
        header[0] = 0xAA; // Wrong magic
        header[12] = 0x09;
        let path = make_test_file("wrong_magic.bin", &header);

        let err = validate_esp32s3_image(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains("Invalid image magic byte"));
    }

    #[test]
    fn test_validate_wrong_chip_rejected() {
        let mut header = [0u8; 32];
        header[0] = 0xE9;
        header[12] = 0x05; // ESP32-C3 chip ID
        let path = make_test_file("esp32c3.bin", &header);

        let err = validate_esp32s3_image(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains("chip ID 0x0005"));
    }

    #[test]
    fn test_validate_short_file_rejected() {
        let path = make_test_file("short.bin", &[0xE9, 0x01, 0x02]);

        let err = validate_esp32s3_image(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(err.to_string().contains("too small"));
    }

    #[test]
    fn test_validate_real_build_if_present() {
        let path = std::path::Path::new("../../firmware/build/osupad-firmware.bin");
        if path.exists() {
            assert!(validate_esp32s3_image(path).is_ok());
        }
    }
}
