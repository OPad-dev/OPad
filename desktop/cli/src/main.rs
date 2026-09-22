use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use opad_device::flash::{self, APP_PARTITION_OFFSET};
use opad_ipc::{send_request, IpcRequest, IpcResponse, IpcStream, IPC_PROTOCOL_VERSION};
use opad_model::JsonBackup;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Parser)]
#[command(name = "opadctl", about = "OPad CLI management tool")]
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
        #[arg(
            long,
            help = "The pad's app port, or its ROM bootloader port when several are connected (default: auto-detect)"
        )]
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
    /// Update the pad's firmware from the signed release manifest (§U-3b)
    FirmwareUpdate {
        #[arg(
            long,
            help = "Consent without the interactive prompt. The firmware is still only flashed \
                    because you said so."
        )]
        yes: bool,
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
    /// Hidden command: trigger freaky 67 easter egg on the pad
    #[command(hide = true)]
    Freaky67,
    /// Hidden command: trigger freaky 67 easter egg on the pad
    #[command(hide = true)]
    EasterEgg,
}

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
    let mut stream = match opad_ipc::connect_and_handshake().await {
        Ok((s, _)) => Some(s),
        Err(e) => {
            if !flashing {
                return Err(e).context("Failed to connect to opad-daemon. Is the daemon running?");
            }
            // Not necessarily "not running": a version-mismatched handshake
            // fails here too, and that daemon still holds the port. Say what
            // actually happened so a Windows sharing-violation later reads as
            // a consequence rather than a mystery.
            println!("Could not reach opad-daemon ({e:#}); flashing without it.");
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
                last_backup,
                pending_takeover,
                ..
            } = resp
            {
                println!("=== OPad Status ===");
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
                // Written automatically 20 s after a session settles into
                // IDLE; see docs/recovery.md §5.
                println!(
                    "Last Backup:      {}",
                    last_backup.as_deref().unwrap_or("Never")
                );
                if let Some(err) = last_sync_error {
                    println!("Last Sync Error:  ⚠ {}", err);
                }
                if let Some(err) = storage_error {
                    println!("Database Error:   ⚠ {}", err);
                }
                // §W3-3. Without this the CLI shows a connected pad that
                // simply never syncs, with nothing saying why — the prompt
                // used to exist only in the GUI, so a headless install had no
                // way to even see that a pad belonged to someone else.
                if let Some(t) = &pending_takeover {
                    println!(
                        "Ownership:        ⚠ This pad belongs to another OPad install. \
                         Counter sync is paused until you decide."
                    );
                    println!(
                        "                    pad: {} / {}   this PC: {} / {}",
                        t.device_key1, t.device_key2, t.pc_key1, t.pc_key2
                    );
                    println!(
                        "                    Take it over or leave it alone in the app; \
                         see docs/recovery.md §7 to unbind it entirely."
                    );
                }
                if let Some(old_id) = pending_replacement {
                    println!("Replacement:      ⚠ New pad detected (previous: {}). Run GUI to restore or adopt.", old_id);
                }
                if let Some(incompat) = incompatible {
                    println!("Incompatible:     ⚠ Device protocol {} incompatible with daemon protocol {}. Update firmware or host.", incompat.protocol_version, IPC_PROTOCOL_VERSION);
                }
                println!(
                    "tosu (osu!):      {}",
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
                    "Resetting counters is permanent! Pass --yes to confirm: opadctl reset --yes"
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
                    "debug" => Some(opad_model::LogLevel::Debug),
                    "info" => Some(opad_model::LogLevel::Info),
                    "warn" | "warning" => Some(opad_model::LogLevel::Warn),
                    "error" => Some(opad_model::LogLevel::Error),
                    _ => None,
                });
            let filter_source = source
                .as_deref()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "host" | "daemon" => Some(opad_model::LogSource::Host),
                    "esp" | "device" => Some(opad_model::LogSource::Esp),
                    "program" | "app" | "gui" => Some(opad_model::LogSource::Program),
                    "tosu" => Some(opad_model::LogSource::Tosu),
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
                        println!("=== OPad Monitor (Last {} entries) ===", entries.len());
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
            flash::check_esp32s3_image_file(app_image)
                .map_err(|e| anyhow::anyhow!("{}: {}", app_image.display(), e))?;

            for (offset, path) in &images {
                println!("  {:#08x}  {}", offset, path.display());
            }

            let app_port = prepare_flash(stream.as_mut(), port).await?;
            // Always hand the port back to the daemon, even if flashing failed
            let result =
                flash::flash(&images, app_port.as_deref(), None, &|m| println!("{m}")).await;
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
                    match flash::wait_for_port(
                        opad_device::find_target_port,
                        Duration::from_secs(15),
                    )
                    .await
                    {
                        Some(p) => println!("✓ Flash succeeded! The pad came back on {}", p),
                        None => bail!(
                            "Firmware was written, but the pad did not come back as the OPad app \
                         within 15s. See docs/recovery.md."
                        ),
                    }
                }
                Some(_) => {}
            }
        }

        Commands::Bootloader { port } => {
            let app_port = prepare_flash(stream.as_mut(), port).await?;
            let result = flash::enter_bootloader(app_port.as_deref(), None).await;
            // The daemon only opens the app port (303a:4001), so resuming now cannot
            // interfere with the bootloader; it reconnects once the app is flashed
            let _ = finish_flash(stream.as_mut()).await;
            let boot_port = result?;
            println!("✓ Device is in ROM download mode on {}", boot_port);
        }

        Commands::FirmwareUpdate { yes } => {
            let offer =
                match send_request(daemon(&mut stream)?, &IpcRequest::GetFirmwareUpdate).await? {
                    IpcResponse::FirmwareUpdateOffer(o) => o,
                    IpcResponse::Error(e) => bail!("{}", e),
                    other => bail!("Unexpected response from daemon: {:?}", other),
                };

            println!("=== OPad Firmware ===");
            println!(
                "Installed:        {}",
                offer.installed.as_deref().unwrap_or("unknown (no pad?)")
            );
            println!(
                "Running Slot:     {}",
                offer
                    .running_partition
                    .as_deref()
                    .unwrap_or("unknown (firmware predates the OTA layout)")
            );

            for blocker in &offer.blockers {
                println!("  ⚠ {}", blocker);
            }

            let Some(available) = offer.available.as_deref() else {
                println!("Available:        nothing newer");
                return Ok(());
            };
            println!("Available:        {}", available);
            if let Some(notes) = &offer.notes {
                println!("Notes:            {}", notes);
            }
            if !offer.blockers.is_empty() {
                bail!("The pad cannot be flashed right now; see the warnings above.");
            }

            // §U-3b: explicit consent every time. --yes is the person saying so
            // in a script; it is not a way of skipping the decision.
            if let Some(text) = &offer.consent_text {
                println!();
                println!("{}", text);
            }
            if !yes {
                use std::io::IsTerminal;
                if !std::io::stdin().is_terminal() {
                    bail!("A firmware update needs confirmation. Re-run with --yes.");
                }
                print!("\nFlash the pad now? [y/N]: ");
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let answer = input.trim().to_lowercase();
                if answer != "y" && answer != "yes" {
                    println!("Cancelled. Nothing was written to the pad.");
                    return Ok(());
                }
            }

            println!("Flashing. Do not unplug the pad.");
            match send_request(
                daemon(&mut stream)?,
                &IpcRequest::InstallFirmwareUpdate { confirm: true },
            )
            .await?
            {
                IpcResponse::FirmwareUpdateFinished {
                    from,
                    to,
                    running_partition,
                    protocol_version,
                    compatible,
                    ..
                } => {
                    println!("✓ Firmware updated from {} to {}", from, to);
                    println!(
                        "  Running Slot:     {}",
                        running_partition.as_deref().unwrap_or("unknown")
                    );
                    if !compatible {
                        println!("  ⚠ WARNING: the pad reports protocol version {} which this host does not speak!", protocol_version);
                    }
                }
                IpcResponse::OperationRejected { reason } => {
                    bail!("Rejected: {}", reason);
                }
                IpcResponse::Error(e) => bail!("{}", e),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
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
        Commands::Freaky67 | Commands::EasterEgg => {
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::TriggerEasterEgg).await?;
            match resp {
                IpcResponse::EasterEggTriggered => {
                    println!("🐱👅 Freaky 67 easter egg triggered on device!");
                }
                IpcResponse::Error(e) => bail!("{}", e),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }
    }

    Ok(())
}

/// The daemon connection, for the commands that cannot work without one.
fn daemon(stream: &mut Option<IpcStream>) -> Result<&mut IpcStream> {
    stream
        .as_mut()
        .context("Failed to connect to opad-daemon. Is the daemon running?")
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
        return Ok(explicit_port.or_else(opad_device::find_target_port));
    };
    println!("Asking opad-daemon to release the serial port...");
    match send_request(stream, &IpcRequest::PrepareFlash).await? {
        IpcResponse::ReadyForFlash { port } => Ok(explicit_port
            .or(port)
            .or_else(opad_device::find_target_port)),
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
        let opad_bin = path.join("opad-firmware.bin");
        let bin = if opad_bin.exists() {
            opad_bin
        } else {
            let legacy_bin = path.join("osupad-firmware.bin");
            if legacy_bin.exists() {
                legacy_bin
            } else {
                opad_bin
            }
        };
        (path.to_path_buf(), bin)
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

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

fn run_setup() -> Result<()> {
    // udev is Linux's; on Windows the pad binds to inbox drivers with no setup
    // step at all, so saying nothing would read as "something is missing".
    if !cfg!(target_os = "linux") {
        println!("=== OPad Setup ===");
        println!("Nothing to do on this platform: the pad uses the inbox USB");
        println!("drivers, so HID and the CDC port work with no setup step.");
        return Ok(());
    }

    println!("=== OPad Linux Setup ===");
    // The same file the packages install, so the two cannot drift apart
    let udev_rule = include_str!("../../../packaging/linux/udev/70-opad.rules");

    let target_path = "/etc/udev/rules.d/70-opad.rules";
    println!("Recommended udev rule for non-root CDC access:");
    println!("{}", udev_rule);

    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        println!("This is the AppImage; it can install the rule itself:");
        println!(
            "  \"{}\" install-udev",
            std::path::Path::new(&appimage).display()
        );
    } else if std::path::Path::new("/etc/udev/rules.d").exists() {
        println!("To install this rule, run:");
        println!(
            "  sudo cp packaging/linux/udev/70-opad.rules {}",
            target_path
        );
        println!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
    }

    // Yama: can tosu read osu!'s memory at all?
    match opad_tosu::ptrace_access() {
        opad_tosu::PtraceAccess::Blocked { scope, fix } => {
            println!();
            println!("⚠ kernel.yama.ptrace_scope is {scope}: tosu cannot read osu!'s memory,");
            println!("  so the pad shows no live gameplay data. To fix:");
            println!("    {fix}");
            println!("  See docs/troubleshooting.md §2.");
        }
        opad_tosu::PtraceAccess::Unknown { scope, fix } => {
            println!();
            println!("kernel.yama.ptrace_scope is {scope} and `getcap` is not installed, so");
            println!(
                "whether tosu may read osu!'s memory is unknown. If the pad shows no live data:"
            );
            println!("    {fix}");
        }
        opad_tosu::PtraceAccess::Capable { scope } => {
            println!("✓ Yama ptrace_scope {scope}, and tosu has cap_sys_ptrace");
        }
        opad_tosu::PtraceAccess::Unrestricted => {}
    }

    println!("Setup check completed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_test_file(name: &str, content: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("opadctl-test-{}-{}", std::process::id(), name));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
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
        let dir = std::env::temp_dir().join(format!("opadctl-full-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bootloader")).unwrap();
        std::fs::create_dir_all(dir.join("partition_table")).unwrap();
        for f in [
            dir.join("bootloader/bootloader.bin"),
            dir.join("partition_table/partition-table.bin"),
            dir.join("ota_data_initial.bin"),
            dir.join("opad-firmware.bin"),
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
        let dir = std::env::temp_dir().join(format!("opadctl-flat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in [
            "bootloader.bin",
            "partition-table.bin",
            "ota_data_initial.bin",
            "opad-firmware.bin",
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
        let dir = std::env::temp_dir().join(format!("opadctl-lonely-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("opad-firmware.bin");
        std::fs::write(&path, [0xE9; 32]).unwrap();

        let err = resolve_flash_set(&path, true).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.to_string().contains("bootloader.bin"), "got: {}", err);
    }

    #[test]
    fn full_flash_tolerates_a_build_without_ota_data() {
        // A build of the old single-app layout has no ota_data_initial.bin.
        // Writing the other three is still the right recovery flash.
        let dir = std::env::temp_dir().join(format!("opadctl-noota-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bootloader")).unwrap();
        std::fs::create_dir_all(dir.join("partition_table")).unwrap();
        for f in [
            dir.join("bootloader/bootloader.bin"),
            dir.join("partition_table/partition-table.bin"),
            dir.join("opad-firmware.bin"),
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
}
