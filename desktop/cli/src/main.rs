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
        #[arg(help = "Path to the app image (.bin), or an ESP-IDF build directory with --full")]
        firmware: PathBuf,
        #[arg(long, help = "Explicit serial port (default: auto-detect)")]
        port: Option<String>,
        #[arg(
            long,
            help = "Recovery flash: also write the bootloader, partition table and OTA data"
        )]
        full: bool,
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

    // Flashing has to work on a machine where the daemon will not start: that is
    // the exact situation docs/recovery.md is written for, and it is what
    // scripts/flash_board.sh used to handle by stopping the systemd unit. Every
    // other command is meaningless without the daemon and still fails loudly.
    let flashing = matches!(
        cli.command,
        Commands::Flash { .. } | Commands::Bootloader { .. }
    );
    let mut stream = match osupad_ipc::connect_and_handshake().await {
        Ok((s, _)) => Some(s),
        Err(e) => {
            if !flashing {
                return Err(e)
                    .context("Failed to connect to osupad-daemon. Is the daemon running?");
            }
            // Not necessarily "not running": a version-mismatched handshake
            // fails here too, and that daemon still holds the port. Say what
            // actually happened so a Windows sharing-violation later reads as
            // a consequence rather than a mystery.
            println!("Could not reach osupad-daemon ({e:#}); flashing without it.");
            println!("If a daemon is in fact running, stop it before flashing on Windows.");
            None
        }
    };

    match cli.command {
        Commands::Status => {
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::GetStatus).await?;
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
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::ForceSync).await?;
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
            let resp = send_request(
                daemon(&mut stream)?,
                &IpcRequest::ResetCounters { confirm: yes },
            )
            .await?;
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
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::ExportBackup).await?;
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
            let preview_resp = send_request(
                daemon(&mut stream)?,
                &IpcRequest::PreviewImport(backup.clone()),
            )
            .await?;
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
                        daemon(&mut stream)?,
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
                let resp = send_request(
                    daemon(&mut stream)?,
                    &IpcRequest::GetLogEntries { since_seq, limit },
                )
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

        Commands::Flash {
            firmware,
            port,
            full,
        } => {
            let images = resolve_flash_set(&firmware, full)?;
            // The app image is always written last, and it is the one whose
            // header has to match this board
            let (_, app_image) = images.last().expect("flash set is never empty");
            validate_esp32s3_image(app_image)?;

            for (offset, path) in &images {
                println!("  {:#08x}  {}", offset, path.display());
            }

            let app_port = prepare_flash(stream.as_mut(), port).await?;
            // Always hand the port back to the daemon, even if flashing failed
            let result = flash_images(&images, app_port.as_deref()).await;
            let finish_resp = finish_flash(stream.as_mut()).await;
            result?;

            match finish_resp {
                Some(IpcResponse::FlashFinished {
                    firmware_version,
                    protocol_version,
                    compatible,
                }) => {
                    println!("✓ Flash succeeded!");
                    println!("  Firmware Version: {}", firmware_version);
                    println!("  Protocol Version: {}", protocol_version);
                    if !compatible {
                        println!("  ⚠ WARNING: Device reported protocol version {} which is incompatible with host!", protocol_version);
                    }
                }
                Some(IpcResponse::Error(e)) => {
                    println!("⚠ Post-flash verification warning: {}", e);
                }
                // No daemon to verify through, so watch the pad re-enumerate
                // as the app ourselves — the last step flash_board.sh did
                None => {
                    match wait_for_port(osupad_device::find_target_port, Duration::from_secs(15))
                        .await
                    {
                        Some(p) => println!("✓ Flash succeeded! The pad came back on {}", p),
                        None => bail!(
                        "Firmware was written, but the pad did not come back as the osu!pad app \
                         within 15s. See docs/recovery.md."
                    ),
                    }
                }
                Some(_) => {}
            }
        }

        Commands::Bootloader { port } => {
            let app_port = prepare_flash(stream.as_mut(), port).await?;
            let result = enter_bootloader(app_port.as_deref()).await;
            // The daemon only opens the app port (303a:4001), so resuming now cannot
            // interfere with the bootloader; it reconnects once the app is flashed
            let _ = finish_flash(stream.as_mut()).await;
            let boot_port = result?;
            println!("✓ Device is in ROM download mode on {}", boot_port);
        }

        Commands::Latency { reset } => {
            if reset {
                match send_request(daemon(&mut stream)?, &IpcRequest::ResetLatencyStats).await? {
                    IpcResponse::Error(e) => bail!("{}", e),
                    _ => println!("✓ Latency statistics reset"),
                }
            } else {
                match send_request(daemon(&mut stream)?, &IpcRequest::GetStatus).await? {
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

/// The daemon connection, for the commands that cannot work without one.
fn daemon(stream: &mut Option<IpcStream>) -> Result<&mut IpcStream> {
    stream
        .as_mut()
        .context("Failed to connect to osupad-daemon. Is the daemon running?")
}

/// Ask the daemon to release the serial port. Returns the app port to trigger,
/// or None if the device is already sitting in the ROM bootloader.
///
/// This is the whole of §W1-3's "the daemon must release the COM port": a
/// Windows serial handle is exclusive (serialport opens with `share_mode = 0`),
/// so a daemon still holding it does not merely slow the flash down, it fails
/// it. The daemon drops the handle before it clears `is_port_open`, so once
/// `ReadyForFlash` comes back the port really is free.
async fn prepare_flash(
    stream: Option<&mut IpcStream>,
    explicit_port: Option<String>,
) -> Result<Option<String>> {
    let Some(stream) = stream else {
        return Ok(explicit_port.or_else(osupad_device::find_target_port));
    };
    println!("Asking osupad-daemon to release the serial port...");
    match send_request(stream, &IpcRequest::PrepareFlash).await? {
        IpcResponse::ReadyForFlash { port } => Ok(explicit_port
            .or(port)
            .or_else(osupad_device::find_target_port)),
        IpcResponse::OperationRejected { reason } => bail!("Rejected by daemon: {}", reason),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

/// Hand the port back to the daemon and let it verify what is now running.
/// `None` means there was no daemon to hand it back to.
async fn finish_flash(stream: Option<&mut IpcStream>) -> Option<IpcResponse> {
    let stream = stream?;
    println!("Resuming daemon device communication and verifying new firmware...");
    send_request(stream, &IpcRequest::FinishFlash).await.ok()
}

/// Which images to write, and where. `full` is the recovery flash that
/// `scripts/flash_board.sh` used to perform: bootloader, partition table and a
/// fresh `otadata` as well as the app.
///
/// `path` is the app image, or — with `--full` — an ESP-IDF build directory.
fn resolve_flash_set(path: &Path, full: bool) -> Result<Vec<(u32, PathBuf)>> {
    if !path.exists() {
        bail!("No such file or directory: {}", path.display());
    }

    let (build_dir, app_image) = if path.is_dir() {
        (path.to_path_buf(), path.join("osupad-firmware.bin"))
    } else {
        let parent = path.parent().unwrap_or(Path::new("."));
        (parent.to_path_buf(), path.to_path_buf())
    };

    if !app_image.exists() {
        bail!("App image not found: {}", app_image.display());
    }
    if !full {
        return Ok(vec![(APP_PARTITION_OFFSET, app_image)]);
    }

    // Offsets come from firmware/partitions.csv and the ESP-IDF defaults; a
    // full flash that skips the partition table would leave the pad reading the
    // app at the old single-app offset (§U-3a).
    // Two layouts have to work: an ESP-IDF build directory, and the flat set of
    // .bin files a release tarball ships (scripts/release/build_release.sh).
    // A recovery flash is usually done from a release, not from a build tree.
    let bootloader = first_existing(&[
        build_dir.join("bootloader").join("bootloader.bin"),
        build_dir.join("bootloader.bin"),
    ]);
    let table = first_existing(&[
        build_dir
            .join("partition_table")
            .join("partition-table.bin"),
        build_dir.join("partition-table.bin"),
    ]);
    let ota_data = first_existing(&[build_dir.join("ota_data_initial.bin")]);

    let (Some(bootloader), Some(table)) = (bootloader, table) else {
        bail!(
            "--full needs bootloader.bin and partition-table.bin next to the app image, \
             and {} has neither an ESP-IDF build layout nor a flat one. Build the \
             firmware with `idf.py build`, or point this at an unpacked release.",
            build_dir.display()
        );
    };

    let mut images = vec![(0x0, bootloader), (0x8000, table)];
    if let Some(ota_data) = ota_data {
        // Only present on the two-slot layout. Writing it points the bootloader
        // back at ota_0, which is where the app image below is going.
        images.push((0xf000, ota_data));
    }
    images.push((APP_PARTITION_OFFSET, app_image));
    Ok(images)
}

/// How to ask the running app to reboot into the ROM download bootloader.
///
/// The firmware accepts all three (`firmware/main/usb/usb_cdc.c`) and they are
/// tried in this order. More than one exists because which of them lands
/// depends on how the host's CDC driver orders `SET_LINE_CODING` against
/// `SET_CONTROL_LINE_STATE`, and Linux and Windows disagree: Linux asserts DTR
/// when the tty is opened, while on Windows serialport's DCB sets
/// `fDtrControl = Disable` and leaves DTR low unless it is set explicitly
/// (§W1-3). So every line state below is set by hand rather than inherited
/// from the open, which makes the sequence mean the same thing on both.
#[derive(Clone, Copy, Debug)]
enum BootTrigger {
    /// The plain-text command, honoured between protocol frames. Baud-rate and
    /// line-state independent, so it is the one that behaves identically
    /// everywhere and is tried first.
    Command,
    /// 1200-baud touch: the firmware arms download mode on the line-coding
    /// change and fires when DTR drops, i.e. when the port is closed.
    BaudTouch,
    /// The classic esptool pattern: RTS falls while DTR stays high.
    DtrRts,
}

fn pulse_trigger(path: &str, trigger: BootTrigger) -> Result<()> {
    let baud = match trigger {
        BootTrigger::BaudTouch => 1200,
        _ => 115_200,
    };
    let mut sp = serialport::new(path, baud)
        .timeout(Duration::from_millis(300))
        .open()
        .with_context(|| format!("Failed to open {}", path))?;

    match trigger {
        BootTrigger::Command => {
            let _ = sp.write_data_terminal_ready(true);
            let _ = sp.write_request_to_send(true);
            sp.write_all(b"BOOTLOADER\n")
                .context("Failed to send the bootloader command")?;
            let _ = sp.flush();
        }
        BootTrigger::BaudTouch => {
            // Opening at 1200 baud is the whole trigger; dropping the handle
            // below clears DTR and fires it.
            let _ = sp.write_data_terminal_ready(true);
        }
        BootTrigger::DtrRts => {
            let _ = sp.write_data_terminal_ready(true);
            let _ = sp.write_request_to_send(true);
            std::thread::sleep(Duration::from_millis(50));
            let _ = sp.write_request_to_send(false);
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    drop(sp);
    Ok(())
}

/// Reboot the running app into the ROM download bootloader and return the bootloader port.
async fn enter_bootloader(app_port: Option<&str>) -> Result<String> {
    if let Some(p) = osupad_device::find_bootloader_port() {
        return Ok(p);
    }
    let Some(app_port) = app_port else {
        bail!("osu!pad not found: neither the app (303a:4001) nor the ROM bootloader (303a:1001) is connected");
    };

    println!("Rebooting {} into the ROM download bootloader...", app_port);
    let mut last_err = None;
    for trigger in [
        BootTrigger::Command,
        BootTrigger::BaudTouch,
        BootTrigger::DtrRts,
    ] {
        // The pad may still be re-enumerating from the previous attempt
        match open_port_with_retry(app_port, Duration::from_secs(2)).await {
            Ok(()) => {}
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
        if let Err(e) = pulse_trigger(app_port, trigger) {
            last_err = Some(e);
            continue;
        }
        if let Some(p) =
            wait_for_port(osupad_device::find_bootloader_port, Duration::from_secs(3)).await
        {
            return Ok(p);
        }
        println!(
            "  {:?} trigger did not take, trying the next one...",
            trigger
        );
    }

    match last_err {
        Some(e) => Err(e).context(
            "Device did not re-enumerate as the ROM bootloader (303a:1001). See docs/recovery.md",
        ),
        None => bail!(
            "Device did not re-enumerate as the ROM bootloader (303a:1001) after all three \
             triggers. See docs/recovery.md for the manual BOOT+RESET sequence."
        ),
    }
}

/// Write every image in the set, then reboot into the app.
///
/// This replaces `scripts/flash_board.sh` (§W1-3): same sequence, no shell, and
/// the daemon is coordinated over IPC instead of by stopping a systemd unit,
/// which is the half that never existed on Windows.
async fn flash_images(images: &[(u32, PathBuf)], app_port: Option<&str>) -> Result<()> {
    let boot_port = enter_bootloader(app_port).await?;

    for (offset, path) in images {
        println!(
            "Writing {} at {:#x} on {}...",
            path.display(),
            offset,
            boot_port
        );
        let status = std::process::Command::new("espflash")
            .args(["write-bin", "--chip", "esp32s3", "-p", &boot_port])
            // Already in download mode, and every image after the first needs
            // the stub still there. espflash cannot reset an ESP32-S3 out of
            // forced download mode; esp_rom::reset_to_app does that at the end.
            .args([
                "--before",
                "no-reset",
                "--after",
                "no-reset-no-stub",
                "--non-interactive",
            ])
            .arg(format!("{:#x}", offset))
            .arg(path)
            .status()
            .context("Failed to execute espflash. Ensure espflash is installed.")?;
        if !status.success() {
            bail!(
                "espflash failed writing {} at {:#x} (exit {:?}). The pad is still in download \
                 mode; see docs/recovery.md.",
                path.display(),
                offset,
                status.code()
            );
        }
    }

    println!("✓ Firmware written, rebooting into application...");
    esp_rom::reset_to_app(&boot_port).context("Firmware written, but failed to reboot the device")
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Wait until the port can actually be opened. On Windows the handle is
/// exclusive, so this is where a daemon that has not let go yet shows up as a
/// clear wait rather than as a mysterious flash failure (§W1-3).
async fn open_port_with_retry(path: &str, timeout: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match serialport::new(path, 115_200)
            .timeout(Duration::from_millis(300))
            .open()
        {
            Ok(sp) => {
                drop(sp);
                return Ok(());
            }
            Err(e) if tokio::time::Instant::now() >= deadline => {
                return Err(e).with_context(|| {
                    format!(
                        "Failed to open {}. On Windows the port is exclusive: close anything \
                         else using it (a serial monitor, another osupad-daemon) first.",
                        path
                    )
                });
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
    fn app_only_flash_writes_one_image_at_the_ota_0_offset() {
        let path = make_test_file("app-only.bin", &[0xE9; 32]);
        let set = resolve_flash_set(&path, false).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].0, 0x20000, "the app lives in ota_0 now (§U-3a)");
    }

    #[test]
    fn full_flash_covers_the_bootloader_table_otadata_and_app_in_order() {
        let dir = std::env::temp_dir().join(format!("osupadctl-full-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bootloader")).unwrap();
        std::fs::create_dir_all(dir.join("partition_table")).unwrap();
        for f in [
            dir.join("bootloader/bootloader.bin"),
            dir.join("partition_table/partition-table.bin"),
            dir.join("ota_data_initial.bin"),
            dir.join("osupad-firmware.bin"),
        ] {
            std::fs::write(&f, [0xE9; 32]).unwrap();
        }

        let set = resolve_flash_set(&dir, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let offsets: Vec<u32> = set.iter().map(|(o, _)| *o).collect();
        // Ascending, and the app last: reset_to_app only runs once every image
        // is down, so a failure part-way leaves the pad in download mode
        assert_eq!(offsets, vec![0x0, 0x8000, 0xf000, 0x20000]);
    }

    #[test]
    fn full_flash_accepts_the_flat_layout_a_release_ships() {
        // dist/ from scripts/release/build_release.sh: no bootloader/ or
        // partition_table/ subdirectories, everything side by side.
        let dir = std::env::temp_dir().join(format!("osupadctl-flat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "bootloader.bin",
            "partition-table.bin",
            "ota_data_initial.bin",
            "osupad-firmware.bin",
        ] {
            std::fs::write(dir.join(f), [0xE9; 32]).unwrap();
        }

        let set = resolve_flash_set(&dir, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            set.iter().map(|(o, _)| *o).collect::<Vec<_>>(),
            vec![0x0, 0x8000, 0xf000, 0x20000]
        );
    }

    #[test]
    fn full_flash_without_a_build_directory_is_refused() {
        // A lone app image in a directory of its own: no bootloader, no table.
        let dir = std::env::temp_dir().join(format!("osupadctl-lonely-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("osupad-firmware.bin");
        std::fs::write(&path, [0xE9; 32]).unwrap();

        let err = resolve_flash_set(&path, true).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.to_string().contains("bootloader.bin"), "got: {}", err);
    }

    #[test]
    fn full_flash_tolerates_a_build_without_ota_data() {
        // A build of the old single-app layout has no ota_data_initial.bin.
        // Writing the other three is still the right recovery flash.
        let dir = std::env::temp_dir().join(format!("osupadctl-noota-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bootloader")).unwrap();
        std::fs::create_dir_all(dir.join("partition_table")).unwrap();
        for f in [
            dir.join("bootloader/bootloader.bin"),
            dir.join("partition_table/partition-table.bin"),
            dir.join("osupad-firmware.bin"),
        ] {
            std::fs::write(&f, [0xE9; 32]).unwrap();
        }

        let set = resolve_flash_set(&dir, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            set.iter().map(|(o, _)| *o).collect::<Vec<_>>(),
            vec![0x0, 0x8000, 0x20000]
        );
    }

    #[test]
    fn test_validate_real_build_if_present() {
        let path = std::path::Path::new("../../firmware/build/osupad-firmware.bin");
        if path.exists() {
            assert!(validate_esp32s3_image(path).is_ok());
        }
    }
}
