use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use opad_device::flash::{self, APP_PARTITION_OFFSET};
use opad_ipc::{send_request, IpcRequest, IpcResponse, IpcStream, TapStatsSnapshot};
use opad_model::board_test;
use opad_model::result_anim;
use opad_model::tap_rate::{self, TapChannel};
use opad_model::JsonBackup;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// How a command ended, as the process exit code. 0 is success and 2 is
/// clap's own, for a command line it could not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fault {
    /// Anything that went wrong and is not one of the others
    Failed = 1,
    /// The OPad service (opad-daemon) is not running or did not answer
    ServiceUnavailable = 3,
    /// The service or the pad said no: mid-map, an invalid value, a pad that
    /// is waiting for a decision
    Rejected = 4,
    /// The command needs a pad and none is connected
    NoPad = 5,
}

impl Fault {
    /// The `kind` of a `--json` error
    fn name(self) -> &'static str {
        match self {
            Fault::Failed => "failed",
            Fault::ServiceUnavailable => "service_unavailable",
            Fault::Rejected => "rejected",
            Fault::NoPad => "no_pad",
        }
    }
}

/// An error that carries its exit code
#[derive(Debug)]
struct Failure {
    fault: Fault,
    message: String,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

fn rejected(message: impl Into<String>) -> anyhow::Error {
    Failure {
        fault: Fault::Rejected,
        message: message.into(),
    }
    .into()
}

fn fault_of(error: &anyhow::Error) -> Fault {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<Failure>())
        .map_or(Fault::Failed, |f| f.fault)
}

/// Why a command that needs the pad was refused. Without a pad that is the
/// reason whatever words the service used, so the service is asked instead
/// of its message being read.
async fn refusal(stream: &mut IpcStream, fault: Fault, message: String) -> anyhow::Error {
    let no_pad = matches!(
        send_request(stream, &IpcRequest::GetStatus).await,
        Ok(IpcResponse::Status {
            device_connected: false,
            ..
        })
    );
    Failure {
        fault: if no_pad { Fault::NoPad } else { fault },
        message,
    }
    .into()
}

/// `--json`: stdout carries one JSON result and nothing else
static JSON: AtomicBool = AtomicBool::new(false);

fn json_mode() -> bool {
    JSON.load(Ordering::Relaxed)
}

/// A line for a person. With `--json` it goes to stderr, so stdout stays JSON.
macro_rules! say {
    ($($arg:tt)*) => {
        if json_mode() {
            eprintln!($($arg)*)
        } else {
            println!($($arg)*)
        }
    };
}

/// `say!` without the line end
macro_rules! say_inline {
    ($($arg:tt)*) => {
        if json_mode() {
            eprint!($($arg)*)
        } else {
            print!($($arg)*)
        }
    };
}

/// The result of a command, for `--json`. Says nothing without it.
fn emit(value: serde_json::Value) {
    if json_mode() {
        println!("{}", value);
    }
}

/// A reply as JSON without the wrapper that names its kind
fn unwrapped(reply: &impl serde::Serialize) -> serde_json::Value {
    match serde_json::to_value(reply) {
        Ok(serde_json::Value::Object(map)) if map.len() == 1 => {
            map.into_iter().next().map(|(_, v)| v).unwrap_or_default()
        }
        Ok(value) => value,
        Err(_) => serde_json::Value::Null,
    }
}

#[derive(Parser)]
#[command(name = "opadctl", about = "OPad from the command line")]
struct Cli {
    #[arg(
        long,
        global = true,
        help = "Print the result as JSON on stdout; progress and questions go to stderr"
    )]
    json: bool,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the pad, its counters and the OPad service at a glance
    Status,
    /// Check this computer's setup (USB permissions, osu! memory access) and
    /// say how to fix what is missing. It changes nothing.
    #[command(alias = "setup")]
    Doctor,
    /// The lifetime press counters
    Counters {
        #[command(subcommand)]
        action: CountersCmd,
    },
    /// Save the pad's settings and counters to a file, or put them back
    Backup {
        #[command(subcommand)]
        action: BackupCmd,
    },
    /// Show your tap rate in KPM (keys per minute, not the map's BPM):
    /// the current or last attempt and the history
    Stats {
        #[arg(
            long,
            value_name = "DAYS",
            help = "Set the history period first, in days (0 = all time). It is kept for the \
                    app and the pad too."
        )]
        period: Option<u32>,
    },
    /// Show key-press-to-HID latency measured on the device
    Latency {
        #[command(subcommand)]
        action: Option<LatencyCmd>,
        #[arg(long, hide = true)]
        reset: bool,
    },
    /// Show the log of the pad, the OPad service and tosu
    #[command(alias = "monitor")]
    Logs(LogsArgs),
    /// Updates of OPad, tosu and the pad's firmware. With nothing after it,
    /// shows their status.
    Update {
        #[command(subcommand)]
        action: Option<UpdateCmd>,
        #[arg(long, hide = true, value_name = "CHANNEL")]
        channel: Option<String>,
        #[arg(long, hide = true)]
        check: bool,
        #[arg(long, hide = true)]
        install: bool,
    },
    /// The pad itself: restart it, test its boards, flash or recover it
    Pad {
        #[command(subcommand)]
        action: PadCmd,
    },
    // These two are also under `pad`. They stay here under the names they
    // always had, because the instructions for a pad that does not start have
    // to work with whichever opadctl is installed.
    /// Flash new firmware binary onto the ESP32-S3 using espflash (same as
    /// `pad flash`)
    Flash(FlashArgs),
    /// Recover a pad that no longer shows up as OPad: replug it, and its next
    /// boot is caught over USB and flashed with the full set. No buttons
    /// (same as `pad recover`)
    Recover(RecoverArgs),

    // The names these had before the commands were grouped. They keep working
    // and are left out of --help.
    /// Same as `counters sync`
    #[command(hide = true)]
    Sync,
    /// Same as `counters reset`
    #[command(hide = true)]
    Reset {
        #[arg(long, help = "Confirm destructive reset of lifetime key counters")]
        yes: bool,
    },
    /// Same as `backup export`
    #[command(hide = true)]
    Export(ExportArgs),
    /// Same as `backup import`
    #[command(hide = true)]
    Import(ImportArgs),
    /// Same as `update install firmware`
    #[command(hide = true)]
    FirmwareUpdate {
        #[arg(long, help = "Consent without the interactive prompt")]
        yes: bool,
    },
    /// Same as `pad restart`
    #[command(hide = true)]
    Restart,
    /// Same as `pad test`
    #[command(hide = true)]
    BoardTest(TestArgs),
    /// Same as `pad animation`
    #[command(hide = true)]
    Animation(AnimationArgs),
    /// Same as `pad bootloader`
    #[command(hide = true)]
    Bootloader(BootloaderArgs),
    /// Hidden command: trigger freaky 67 easter egg on the pad
    #[command(hide = true)]
    Freaky67,
    /// Hidden command: trigger freaky 67 easter egg on the pad
    #[command(hide = true)]
    EasterEgg,
}

#[derive(Subcommand)]
enum CountersCmd {
    /// Bring the pad's counters, this PC's counters and the pad's clock into line now
    Sync,
    /// Set the lifetime key counters back to zero, for good
    Reset {
        #[arg(long, help = "Confirm destructive reset of lifetime key counters")]
        yes: bool,
    },
}

#[derive(Subcommand)]
enum BackupCmd {
    /// Export device configuration and lifetime counters to a JSON file
    Export(ExportArgs),
    /// Import configuration and counters from a validated JSON backup
    Import(ImportArgs),
}

#[derive(Subcommand)]
enum LatencyCmd {
    /// Clear the collected samples (e.g. right before playing a map)
    Reset,
}

#[derive(Subcommand)]
enum UpdateCmd {
    /// Show what is installed and what is available, as the app's Updates
    /// tab does
    Status,
    /// Check the update feed now, wait for the answer and show it
    Check,
    /// Install the available update of one part
    Install {
        #[arg(value_enum, help = "What to update")]
        part: Part,
        #[arg(
            long,
            help = "Firmware only: consent without the interactive prompt. The firmware is \
                    still only flashed because you said so."
        )]
        yes: bool,
    },
    /// Show the update channel, or follow another one from now on
    Channel {
        #[arg(help = "stable, or beta for release candidates")]
        channel: Option<String>,
    },
    /// Turn one part's automatic updates on or off
    Auto {
        #[arg(value_enum, help = "Whose automatic updates")]
        part: Part,
        #[arg(value_enum)]
        state: Switch,
    },
}

/// The three things that update on their own
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Part {
    /// OPad itself: the app, the service and opadctl
    App,
    /// tosu, the program that reads osu!
    Tosu,
    /// The firmware on the pad
    Firmware,
}

impl Part {
    fn component(self) -> opad_ipc::UpdateComponent {
        match self {
            Part::App => opad_ipc::UpdateComponent::App,
            Part::Tosu => opad_ipc::UpdateComponent::Tosu,
            Part::Firmware => opad_ipc::UpdateComponent::Firmware,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Part::App => "OPad",
            Part::Tosu => "tosu",
            Part::Firmware => "the pad's firmware",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum Switch {
    On,
    Off,
}

#[derive(Subcommand)]
enum PadCmd {
    /// Restart the pad, e.g. after swapping its input module. It saves its
    /// counters and restarts once no map is being played
    Restart,
    /// Test the carrier and input module PCBs: module ID, cable direction,
    /// key lines, connector shorts, and a Hall Effect V3 module's ADC and
    /// sensors. Release both keys while it runs.
    Test(TestArgs),
    /// Play a result animation on the pad, as osu!'s results screen would
    Animation(AnimationArgs),
    /// Flash new firmware binary onto the ESP32-S3 using espflash
    Flash(FlashArgs),
    /// Recover a pad that no longer shows up as OPad: replug it, and its next
    /// boot is caught over USB and flashed with the full set. No buttons.
    Recover(RecoverArgs),
    /// Reboot device into ROM download bootloader mode hands-free over USB
    Bootloader(BootloaderArgs),
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct ExportArgs {
    #[arg(help = "Path to write backup JSON file")]
    file: PathBuf,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct ImportArgs {
    #[arg(help = "Path to JSON backup file")]
    file: PathBuf,
    #[arg(long, help = "Skip preview and confirmation prompt")]
    yes: bool,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct LogsArgs {
    #[arg(short, long, default_value_t = 50)]
    limit: usize,
    #[arg(short, long, help = "Follow log stream in real time")]
    follow: bool,
    #[arg(
        long,
        value_enum,
        ignore_case = true,
        help = "Only this severity and above"
    )]
    level: Option<opad_model::LogLevel>,
    #[arg(long, value_enum, ignore_case = true, help = "Only this source")]
    source: Option<opad_model::LogSource>,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct TestArgs {
    #[arg(
        long,
        help = "If the keys are on the wrong pins for the carrier, set Key 1 = GPIO10 and \
                Key 2 = GPIO7, then test again"
    )]
    fix_pins: bool,
    #[arg(
        long,
        help = "Hall Effect V3: then watch the sensors live while you hold a magnet over \
                each key in turn (up to 60 s, Ctrl-C to stop)"
    )]
    magnet: bool,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct AnimationArgs {
    #[arg(help = "pop, drop or burst (or an animation id); none stops the one playing")]
    name: String,
    #[arg(default_value = "S", help = "The grade it shows: SS, S, A, B, C or D")]
    grade: String,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct FlashArgs {
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
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct RecoverArgs {
    #[arg(help = "An ESP-IDF build directory or an unpacked release (as for flash --full)")]
    firmware: PathBuf,
    #[arg(
        long,
        default_value_t = 300,
        help = "How long to wait for the replug, in seconds"
    )]
    timeout: u64,
}

#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
struct BootloaderArgs {
    #[arg(long, help = "Explicit serial port (default: auto-detect)")]
    port: Option<String>,
}

/// What a command line asks for, whichever of its names was typed
#[derive(Debug, Clone, PartialEq, Eq)]
enum Action {
    Status,
    Doctor,
    Sync,
    Reset {
        yes: bool,
    },
    Export(ExportArgs),
    Import(ImportArgs),
    Stats {
        period: Option<u32>,
    },
    Latency {
        reset: bool,
    },
    Logs(LogsArgs),
    UpdateStatus,
    UpdateCheck,
    UpdateInstall {
        part: Part,
        yes: bool,
    },
    UpdateChannel {
        channel: Option<String>,
    },
    UpdateAuto {
        part: Part,
        on: bool,
    },
    /// `update --channel --check --install`, in that order, then the status
    UpdateFlags {
        channel: Option<String>,
        check: bool,
        install: bool,
    },
    Restart,
    BoardTest(TestArgs),
    Animation(AnimationArgs),
    Flash(FlashArgs),
    Recover(RecoverArgs),
    Bootloader(BootloaderArgs),
    EasterEgg,
}

impl From<Cmd> for Action {
    fn from(cmd: Cmd) -> Self {
        match cmd {
            Cmd::Status => Action::Status,
            Cmd::Doctor => Action::Doctor,
            Cmd::Counters {
                action: CountersCmd::Sync,
            }
            | Cmd::Sync => Action::Sync,
            Cmd::Counters {
                action: CountersCmd::Reset { yes },
            }
            | Cmd::Reset { yes } => Action::Reset { yes },
            Cmd::Backup {
                action: BackupCmd::Export(args),
            }
            | Cmd::Export(args) => Action::Export(args),
            Cmd::Backup {
                action: BackupCmd::Import(args),
            }
            | Cmd::Import(args) => Action::Import(args),
            Cmd::Stats { period } => Action::Stats { period },
            Cmd::Latency { action, reset } => Action::Latency {
                reset: reset || action.is_some(),
            },
            Cmd::Logs(args) => Action::Logs(args),
            Cmd::Update {
                action: Some(action),
                ..
            } => match action {
                UpdateCmd::Status => Action::UpdateStatus,
                UpdateCmd::Check => Action::UpdateCheck,
                UpdateCmd::Install { part, yes } => Action::UpdateInstall { part, yes },
                UpdateCmd::Channel { channel } => Action::UpdateChannel { channel },
                UpdateCmd::Auto { part, state } => Action::UpdateAuto {
                    part,
                    on: state == Switch::On,
                },
            },
            Cmd::Update {
                action: None,
                channel: None,
                check: false,
                install: false,
            } => Action::UpdateStatus,
            Cmd::Update {
                action: None,
                channel,
                check,
                install,
            } => Action::UpdateFlags {
                channel,
                check,
                install,
            },
            Cmd::FirmwareUpdate { yes } => Action::UpdateInstall {
                part: Part::Firmware,
                yes,
            },
            Cmd::Pad { action } => match action {
                PadCmd::Restart => Action::Restart,
                PadCmd::Test(args) => Action::BoardTest(args),
                PadCmd::Animation(args) => Action::Animation(args),
                PadCmd::Flash(args) => Action::Flash(args),
                PadCmd::Recover(args) => Action::Recover(args),
                PadCmd::Bootloader(args) => Action::Bootloader(args),
            },
            Cmd::Restart => Action::Restart,
            Cmd::BoardTest(args) => Action::BoardTest(args),
            Cmd::Animation(args) => Action::Animation(args),
            Cmd::Flash(args) => Action::Flash(args),
            Cmd::Recover(args) => Action::Recover(args),
            Cmd::Bootloader(args) => Action::Bootloader(args),
            Cmd::Freaky67 | Cmd::EasterEgg => Action::EasterEgg,
        }
    }
}

async fn run_board_test(stream: &mut IpcStream) -> Result<board_test::BoardTestReport> {
    // An older daemon cannot parse the request and drops the connection
    let resp = send_request(stream, &IpcRequest::RunBoardTest)
        .await
        .context("The daemon did not answer; it may predate the board test")?;
    match resp {
        IpcResponse::BoardTest(report) => Ok(report),
        IpcResponse::Error(e) => Err(refusal(stream, Fault::Failed, e).await),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

fn format_board_test(
    report: &board_test::BoardTestReport,
    checks: &[board_test::BoardCheck],
) -> String {
    use board_test::CheckStatus;
    let mut out = String::from("=== Board test (carrier + input module) ===\n");
    let mut group = None;
    for c in checks {
        if group != Some(c.group) {
            group = Some(c.group);
            out += &format!("\n[{}]\n", c.group.label());
        }
        let mark = match c.status {
            CheckStatus::Pass => "✓",
            CheckStatus::Warn => "!",
            CheckStatus::Fail => "✗",
            CheckStatus::Skip => "-",
        };
        out += &format!("{mark} {:<18} {}\n", c.name, c.detail);
    }
    if report.ran {
        let mv = |v: Option<u32>| v.map_or("-".to_string(), |v| format!("{v} mV"));
        out += &format!(
            "\nMeasured: module {} (boot: {}), ID {} / {} loaded, reverse probe {}, keys GPIO{} / GPIO{}{}\n",
            report.module.label(),
            report.boot_module.label(),
            mv(report.id_mv),
            mv(report.id_loaded_mv),
            mv(report.reverse_probe_mv),
            report.key1_gpio,
            report.key2_gpio,
            if report.cable_reversed { ", cable turned over" } else { "" }
        );
        if let Some(a) = &report.he_adc {
            let hex = |v: Option<u8>| v.map_or("-".to_string(), |v| format!("0x{v:02X}"));
            out += &format!(
                "ADC: status {} / {} / {} (reset / cleared / end), walk {}/{} wrong bits 0x{:02X}, cal {}\n",
                hex(a.status_after_reset),
                hex(a.status_after_clear),
                hex(a.status_end),
                a.walk_mismatches,
                a.walk_writes,
                a.walk_wrong_bits,
                if a.cal_done { "done" } else { "not done" }
            );
            for (i, c) in a.channels.iter().enumerate() {
                out += &format!(
                    "Key {} sensor: AIN{} {:.3} V ({:.1} codes), min {} max {}, noise {:.2} rms\n",
                    i + 1,
                    c.ain,
                    c.volts(),
                    c.mean(),
                    c.min,
                    c.max,
                    c.noise()
                );
            }
        }
    }
    out += &format!(
        "Result: {}\n",
        board_test::overall(checks).label().to_uppercase()
    );
    out
}

/// Run the board test over and over while the person moves a magnet over each
/// Hall Effect V3 key, printing the sensors live
async fn magnet_test(stream: &mut IpcStream) -> Result<board_test::MagnetTest> {
    use std::io::Write;
    let mut test = board_test::MagnetTest::new();
    say!(
        "\nMagnet test: hold a magnet over key 1, then key 2, then key 3 (a key's own magnet \
         or any small magnet). Each sensor must move {:.0} codes on its own.",
        board_test::MAGNET_MOVE_CODES
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while tokio::time::Instant::now() < deadline {
        let report = run_board_test(stream).await?;
        test.update(&report);
        let Some(adc) = report
            .he_adc
            .as_ref()
            .filter(|a| a.ran && !a.channels.is_empty())
        else {
            bail!("the pad returned no sensor readings: is a Hall Effect V3 module connected?");
        };
        let base = test.baseline().unwrap_or(&[]);
        let line: Vec<String> = adc
            .channels
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let d = c.mean() - base.get(i).copied().unwrap_or(c.mean());
                let mark = match test.key(i) {
                    board_test::MagnetKey::Waiting { .. } => " ",
                    board_test::MagnetKey::Seen { .. } => "✓",
                    board_test::MagnetKey::Crosstalk { .. } => "!",
                };
                format!("{mark} K{} {:.3} V ({:+5.0})", i + 1, c.volts(), d)
            })
            .collect();
        say_inline!("\r{}   ", line.join("   "));
        std::io::stdout().flush().ok();
        if test.done() {
            break;
        }
    }
    say!("\n");
    Ok(test)
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    JSON.store(cli.json, Ordering::Relaxed);
    match run(cli.command.into()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let fault = fault_of(&error);
            if json_mode() {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": false,
                        "kind": fault.name(),
                        "error": format!("{error:#}"),
                    })
                );
            } else {
                eprintln!("Error: {error:#}");
            }
            std::process::ExitCode::from(fault as u8)
        }
    }
}

async fn run(action: Action) -> Result<()> {
    if action == Action::Doctor {
        return run_doctor();
    }

    // Flashing has to work on a machine where the daemon will not start: that is
    // the exact situation docs/recovery.md is written for, and it is what
    // scripts/flash_board.sh used to handle by stopping the systemd unit. Every
    // other command is meaningless without the daemon and still fails loudly.
    let flashing = matches!(
        action,
        Action::Flash(_) | Action::Bootloader(_) | Action::Recover(_)
    );
    let mut stream = match opad_ipc::connect_and_handshake().await {
        Ok((s, _)) => Some(s),
        Err(e) => {
            if !flashing {
                return Err(Failure {
                    fault: Fault::ServiceUnavailable,
                    message: format!(
                        "Failed to connect to opad-daemon. Is the daemon running? ({e:#})"
                    ),
                }
                .into());
            }
            // Not necessarily "not running": a version-mismatched handshake
            // fails here too, and that daemon still holds the port. Say what
            // actually happened so a Windows sharing-violation later reads as
            // a consequence rather than a mystery.
            say!("Could not reach opad-daemon ({e:#}); flashing without it.");
            say!("If a daemon is in fact running, stop it before flashing on Windows.");
            None
        }
    };

    match action {
        Action::Status => {
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::GetStatus).await?;
            if json_mode() {
                emit(unwrapped(&resp));
                return Ok(());
            }
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
                he_calibration,
                key3,
                ..
            } = resp
            {
                say!("=== OPad Status ===");
                say!("Daemon Mode:      {:?}", mode);
                say!(
                    "ESP32 Device:     {}",
                    if device_connected {
                        "Connected"
                    } else {
                        "Disconnected"
                    }
                );
                say!("Counters Source:  {:?}", counters_source);
                if let Some(info) = &device_info {
                    say!("Device ID:        {}", info.device_id);
                    say!("Board Profile:    {}", info.board_profile);
                    say!("Firmware Version: {}", info.firmware_version);
                    // §U-3a. Old firmware does not report it at all, and that
                    // is worth seeing: it means the pad is still on the
                    // single-app table and needs a serial reflash.
                    say!(
                        "Running Slot:     {}",
                        info.running_partition
                            .as_deref()
                            .unwrap_or("unknown (firmware predates the OTA layout)")
                    );
                    // HE lines only for a pad running an HE module; an MX
                    // pad's firmware is merely able to (he_input)
                    if info.he_calibration {
                        say!("Input Module:     Hall Effect");
                    }
                }
                let he_active = device_info.as_ref().is_some_and(|i| i.he_calibration);
                if let Some(cal) = he_calibration.as_ref().filter(|_| he_active) {
                    let yes = |b: bool| if b { "yes" } else { "no" };
                    if key3.is_some() {
                        say!(
                            "HE Calibrated:    K1: {}, K2: {}, K3: {}",
                            yes(cal.key1_calibrated),
                            yes(cal.key2_calibrated),
                            yes(cal.key3_calibrated)
                        );
                    } else {
                        say!(
                            "HE Calibrated:    K1: {}, K2: {}",
                            yes(cal.key1_calibrated),
                            yes(cal.key2_calibrated)
                        );
                    }
                }
                say!(
                    "Key 1 (K1):       {} (Total: {} presses)",
                    config.key1_char(),
                    counters.lifetime_key1
                );
                say!(
                    "Key 2 (K2):       {} (Total: {} presses)",
                    config.key2_char(),
                    counters.lifetime_key2
                );
                if let Some(k3) = key3 {
                    say!(
                        "Key 3 (K3):       {} (Total: {} presses)",
                        config.key3_char(),
                        k3.lifetime
                    );
                    say!(
                        "Game Keys:        {}",
                        opad_model::game_keys_names(config.game_keys)
                    );
                }
                // The game keys' presses (keys 1 and 2 unless others are picked)
                let total: u64 = [
                    counters.lifetime_key1,
                    counters.lifetime_key2,
                    key3.map_or(0, |k| k.lifetime),
                ]
                .iter()
                .enumerate()
                .filter(|(k, _)| config.is_game_key(*k as u8 + 1))
                .map(|(_, v)| v)
                .sum();
                say!("Total Presses:    {}", total);
                say!("Generation:       {}", counters.counter_generation);
                say!(
                    "Last Sync:        {}",
                    last_sync_time.as_deref().unwrap_or("Never")
                );
                // Written automatically 20 s after a session settles into
                // IDLE; see docs/recovery.md §5.
                say!(
                    "Last Backup:      {}",
                    last_backup.as_deref().unwrap_or("Never")
                );
                if let Some(err) = last_sync_error {
                    say!("Last Sync Error:  ⚠ {}", err);
                }
                if let Some(err) = storage_error {
                    say!("Database Error:   ⚠ {}", err);
                }
                // §W3-3. Without this the CLI shows a connected pad that
                // simply never syncs, with nothing saying why — the prompt
                // used to exist only in the GUI, so a headless install had no
                // way to even see that a pad belonged to someone else.
                if let Some(t) = &pending_takeover {
                    say!(
                        "Ownership:        ⚠ This pad belongs to another OPad install. \
                         Counter sync is paused until you decide."
                    );
                    say!(
                        "                    pad: {} / {}   this PC: {} / {}",
                        t.device_key1,
                        t.device_key2,
                        t.pc_key1,
                        t.pc_key2
                    );
                    say!(
                        "                    Take it over or leave it alone in the app; \
                         see docs/recovery.md §7 to unbind it entirely."
                    );
                }
                if let Some(old_id) = pending_replacement {
                    say!("Replacement:      ⚠ New pad detected (previous: {}). Run GUI to restore or adopt.", old_id);
                }
                if let Some(incompat) = incompatible {
                    let ours = opad_protocol::DEVICE_PROTOCOL_VERSION;
                    let fix = if incompat.protocol_version > ours {
                        "Update OPad (`opadctl update install app`)."
                    } else {
                        "Update the pad's firmware (`opadctl update install firmware`)."
                    };
                    say!(
                        "Incompatible:     ⚠ The pad (firmware {}) speaks protocol {}, this app {}. It works as a keyboard and nothing else until then. {}",
                        incompat.firmware_version, incompat.protocol_version, ours, fix
                    );
                }
                say!(
                    "tosu (osu!):      {}",
                    if tosu_connected { "Active" } else { "Offline" }
                );
                if let Some(l) = latency {
                    say!(
                        "Key Latency:      p50 {}µs, p99.9 {}µs, max {}µs ({} samples)",
                        l.p50_us,
                        l.p999_us,
                        l.max_us,
                        l.samples
                    );
                }
                say!(
                    "Key Pins:         K1 GPIO{}, K2 GPIO{}",
                    config.key1_gpio,
                    config.key2_gpio
                );
                say!("Debounce Lockout: {} µs", config.debounce_us);
                use opad_model::swipe::SwipeDir;
                say!(
                    "Swipes:           up {}, down {}, left {}, right {}",
                    config.swipe(SwipeDir::Up).describe(),
                    config.swipe(SwipeDir::Down).describe(),
                    config.swipe(SwipeDir::Left).describe(),
                    config.swipe(SwipeDir::Right).describe()
                );
                say!("Brightness:       {}%", config.brightness);
                say!("Sleep Timeout:    {}s", config.display_sleep_seconds);
            }
        }

        Action::Sync => {
            say!("Requesting safe counter & clock synchronization...");
            let conn = daemon(&mut stream)?;
            match send_request(conn, &IpcRequest::ForceSync).await? {
                IpcResponse::SyncCompleted {
                    success: true,
                    counters,
                } => {
                    say!("✓ Sync succeeded!");
                    say!("  K1: {}", counters.lifetime_key1);
                    say!("  K2: {}", counters.lifetime_key2);
                    emit(serde_json::json!({ "ok": true, "counters": counters }));
                }
                IpcResponse::OperationRejected { reason } => {
                    return Err(refusal(conn, Fault::Rejected, reason).await);
                }
                IpcResponse::Error(e) => return Err(refusal(conn, Fault::Failed, e).await),
                other => bail!("Sync failed: {:?}", other),
            }
        }

        Action::Reset { yes } => {
            if !yes {
                bail!(
                    "Resetting counters is permanent! Pass --yes to confirm: \
                     opadctl counters reset --yes"
                );
            }
            let resp = send_request(
                daemon(&mut stream)?,
                &IpcRequest::ResetCounters { confirm: yes },
            )
            .await?;
            match resp {
                IpcResponse::CountersReset { counters } => {
                    say!(
                        "✓ Counters reset successfully (new generation: {})",
                        counters.counter_generation
                    );
                    emit(serde_json::json!({ "ok": true, "counters": counters }));
                }
                IpcResponse::OperationRejected { reason } => return Err(rejected(reason)),
                IpcResponse::Error(e) => bail!("{}", e),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }

        Action::Export(ExportArgs { file }) => {
            let resp = send_request(daemon(&mut stream)?, &IpcRequest::ExportBackup).await?;
            if let IpcResponse::BackupExported(backup) = resp {
                let json_data = serde_json::to_string_pretty(&backup)?;
                std::fs::write(&file, json_data)
                    .with_context(|| format!("Failed to write to file {}", file.display()))?;
                say!("✓ Backup exported to {}", file.display());
                emit(serde_json::json!({ "ok": true, "file": file }));
            } else {
                bail!("Unexpected response from daemon: {:?}", resp);
            }
        }

        Action::Import(ImportArgs { file, yes }) => {
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
                    say!("=== Import Preview ===");
                    if let Some(cur) = current {
                        say!("Current:  Device ID: {}", cur.device_id);
                        say!(
                            "          Key 1: {} ({} presses)",
                            cur.config.key1_char(),
                            cur.lifetime_key1
                        );
                        say!(
                            "          Key 2: {} ({} presses)",
                            cur.config.key2_char(),
                            cur.lifetime_key2
                        );
                        if let Some(k3) = cur.lifetime_key3 {
                            say!(
                                "          Key 3: {} ({} presses)",
                                cur.config.key3_char(),
                                k3
                            );
                            say!(
                                "          Game keys: {}",
                                opad_model::game_keys_names(cur.config.game_keys)
                            );
                        }
                    }
                    say!("Incoming: Device ID: {}", incoming.device.device_id);
                    say!(
                        "          Key 1: {} ({} presses)",
                        incoming.config.key1,
                        incoming.stats.lifetime_key1
                    );
                    say!(
                        "          Key 2: {} ({} presses)",
                        incoming.config.key2,
                        incoming.stats.lifetime_key2
                    );
                    if let Some(k3) = &incoming.config.key3 {
                        match incoming.stats.lifetime_key3 {
                            Some(n) => say!("          Key 3: {} ({} presses)", k3, n),
                            None => say!("          Key 3: {} (count kept)", k3),
                        }
                    }
                    if let Some(game) = incoming.config.game_keys {
                        say!("          Game keys: {}", opad_model::game_keys_names(game));
                    }

                    if !device_id_matches {
                        say!("\n  ⚠ Note: Backup device ID does not match current pad.");
                    }
                    if is_counter_rollback {
                        say!("  ⚠ Warning: Counter rollback detected.");
                    }
                    if !warnings.is_empty() {
                        say!("\nWarnings:");
                        for w in &warnings {
                            say!("  ⚠ {}", w);
                        }
                    }

                    if !yes
                        && !confirm(
                            "\nProceed with import? [y/N]: ",
                            "Confirmation required to import backup. Run with --yes in non-interactive mode.",
                        )?
                    {
                        say!("Import cancelled.");
                        emit(serde_json::json!({ "ok": true, "imported": false }));
                        return Ok(());
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
                            say!(
                                "✓ Backup imported and synchronized successfully (generation: {})!",
                                counters.counter_generation
                            );
                            emit(serde_json::json!({ "ok": true, "counters": counters }));
                        }
                        IpcResponse::OperationRejected { reason } => {
                            return Err(rejected(reason));
                        }
                        IpcResponse::Error(e) => bail!("{}", e),
                        other => bail!("Unexpected response from daemon: {:?}", other),
                    }
                }
                IpcResponse::OperationRejected { reason } => {
                    return Err(rejected(format!("Preview rejected: {reason}")));
                }
                IpcResponse::Error(e) => bail!("Preview error: {}", e),
                other => {
                    bail!("Unexpected preview response: {:?}", other);
                }
            }
        }

        Action::Logs(LogsArgs {
            limit,
            follow,
            level,
            source,
        }) => {
            let mut since_seq = None;
            let mut first_batch = true;

            loop {
                // The daemon filters before applying `limit`; the filter is
                // repeated here for an older daemon that ignores it
                let resp = send_request(
                    daemon(&mut stream)?,
                    &IpcRequest::GetLogEntries {
                        since_seq,
                        limit,
                        level,
                        source,
                    },
                )
                .await?;
                if let IpcResponse::LogEntries { entries, .. } = resp {
                    // From the last entry received, never the daemon's latest:
                    // a burst over `limit` leaves the rest for the next poll
                    since_seq = next_since_seq(since_seq, &entries);
                    let shown: Vec<_> = entries
                        .iter()
                        .filter(|e| level.is_none_or(|l| e.level >= l))
                        .filter(|e| source.is_none_or(|s| e.source == s))
                        .collect();
                    if first_batch && !follow && !json_mode() {
                        println!("=== OPad Monitor (Last {} entries) ===", shown.len());
                    }
                    for entry in shown {
                        if json_mode() {
                            // One entry a line, so a follow can be read as it comes
                            println!("{}", serde_json::to_string(entry)?);
                        } else {
                            println!("{}", entry.format_line());
                        }
                    }
                }
                first_batch = false;

                if !follow {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        Action::Flash(FlashArgs {
            firmware,
            port,
            full,
        }) => {
            let mut images = resolve_flash_set(&firmware, full)?;
            // The app image is always the last of the set, and it is the one
            // whose header has to match this board
            let (_, app_image) = images.last().cloned().expect("flash set is never empty");
            flash::check_esp32s3_image_file(&app_image)
                .map_err(|e| anyhow::anyhow!("{}: {}", app_image.display(), e))?;

            // An app update goes beside the running image, which stays
            // there to fall back to (see flash::plan_app_update)
            let mut target_slot = None;
            if !full {
                let running = running_slot(stream.as_mut()).await;
                let plan =
                    flash::plan_app_update(&app_image, running.as_deref(), &std::env::temp_dir())?;
                images = plan.images;
                target_slot = plan.target_slot;
                match target_slot {
                    Some(slot) => say!(
                        "Writing slot {slot}; {} stays as the fallback if the new firmware never comes up on USB",
                        running.as_deref().unwrap_or("?")
                    ),
                    None => say!(
                        "Running slot unknown: writing ota_0 and making it the slot that boots, with no fallback"
                    ),
                }
            }

            for (offset, path) in &images {
                say!("  {:#08x}  {}", offset, path.display());
            }

            let app_port = prepare_flash(stream.as_mut(), port).await?;
            let interrupt = resume_on_interrupt(stream.is_some());
            let result = flash::flash(&images, app_port.as_deref(), None, &|m| say!("{m}")).await;
            // Always hand the port back to the daemon. After a failure the pad
            // is not coming back as the app, so resume without the 15 s
            // verification wait and show the real error straight away.
            if result.is_err() {
                resume_device(stream.as_mut()).await;
                release(&interrupt);
            }
            result?;
            let finish_resp = finish_flash(stream.as_mut()).await;
            release(&interrupt);

            if let Some(slot) = target_slot {
                // A rejected image restarts ~20 s after the host gives up on
                // it, so the verification above may have timed out on it
                return report_slot_after_flash(stream.as_mut(), slot, finish_resp).await;
            }

            match finish_resp {
                Some(IpcResponse::FlashFinished {
                    firmware_version,
                    protocol_version,
                    compatible,
                }) => print_flash_success(&firmware_version, protocol_version, compatible),
                // The same outcome as the no-daemon branch below, so the same
                // exit status: a script must not read a pad that never came
                // back as success
                Some(IpcResponse::Error(e)) => bail!(
                    "Firmware was written, but post-flash verification failed: {}. \
                     See docs/recovery.md.",
                    e
                ),
                // No daemon to verify through, so watch the pad re-enumerate
                // as the app ourselves — the last step flash_board.sh did
                None => {
                    match flash::wait_for_port(
                        opad_device::find_target_port,
                        Duration::from_secs(15),
                    )
                    .await
                    {
                        Some(p) => {
                            say!("✓ Flash succeeded! The pad came back on {}", p);
                            emit(serde_json::json!({ "ok": true, "port": p }));
                        }
                        None => bail!(
                            "Firmware was written, but the pad did not come back as the OPad app \
                         within 15s. Run `opadctl recover` and replug it; see docs/recovery.md."
                        ),
                    }
                }
                Some(_) => {}
            }
        }

        Action::Recover(RecoverArgs { firmware, timeout }) => {
            let images = resolve_flash_set(&firmware, true)?;
            let (_, app_image) = images.last().expect("flash set is never empty");
            flash::check_esp32s3_image_file(app_image)
                .map_err(|e| anyhow::anyhow!("{}: {}", app_image.display(), e))?;
            for (offset, path) in &images {
                say!("  {:#08x}  {}", offset, path.display());
            }
            say!("Unplug the pad's USB cable and plug it back in (waiting {timeout} s)...");
            let result = tokio::task::spawn_blocking(move || {
                flash::recover(&images, Duration::from_secs(timeout), &|m| say!("{m}"))
            })
            .await?;
            result?;
            match flash::wait_for_port(opad_device::find_target_port, Duration::from_secs(15)).await {
                Some(p) => {
                    say!("✓ Recovered: the pad came back as OPad on {p}");
                    emit(serde_json::json!({ "ok": true, "port": p }));
                }
                None => bail!("The firmware was written, but the pad did not come back as the OPad app within 15 s"),
            }
        }

        Action::Bootloader(BootloaderArgs { port }) => {
            let app_port = prepare_flash(stream.as_mut(), port).await?;
            let interrupt = resume_on_interrupt(stream.is_some());
            let result = flash::enter_bootloader(app_port.as_deref(), None).await;
            // The daemon only opens the app port (303a:8415), so resuming now cannot
            // interfere with the bootloader; it reconnects once the app is flashed.
            // No FinishFlash: the pad is meant to stay in the bootloader, so a
            // wait for it to come back as the app could only time out.
            resume_device(stream.as_mut()).await;
            release(&interrupt);
            let boot_port = result?;
            say!("✓ Device is in ROM download mode on {}", boot_port);
            emit(serde_json::json!({ "ok": true, "port": boot_port }));
        }

        Action::UpdateStatus => {
            let conn = daemon(&mut stream)?;
            show_update_status(conn).await?;
        }

        Action::UpdateCheck => {
            let conn = daemon(&mut stream)?;
            check_for_updates(conn).await?;
            show_update_status(conn).await?;
        }

        Action::UpdateChannel { channel } => {
            let conn = daemon(&mut stream)?;
            match channel {
                Some(channel) => {
                    set_update_channel(conn, channel.clone()).await?;
                    say!("✓ Following the {channel} channel; checking it now");
                    emit(serde_json::json!({ "ok": true, "channel": channel }));
                }
                None => {
                    let channel = match send_request(conn, &IpcRequest::GetUpdateStatus).await? {
                        IpcResponse::UpdateStatus { channel, .. } => channel,
                        IpcResponse::Error(e) => bail!("{}", e),
                        other => bail!("Unexpected response from daemon: {:?}", other),
                    };
                    say!("Channel:          {channel}");
                    emit(serde_json::json!({ "channel": channel }));
                }
            }
        }

        Action::UpdateAuto { part, on } => {
            let request = IpcRequest::SetUpdateEnabled {
                component: part.component(),
                enabled: on,
            };
            match send_request(daemon(&mut stream)?, &request).await? {
                IpcResponse::ConfigUpdated { .. } => {
                    say!(
                        "✓ Automatic updates of {} are {}",
                        part.label(),
                        if on { "on" } else { "off" }
                    );
                    emit(serde_json::json!({ "ok": true, "automatic": on }));
                }
                IpcResponse::OperationRejected { reason }
                | IpcResponse::OperationDeferred { reason } => return Err(rejected(reason)),
                IpcResponse::Error(e) => bail!("{}", e),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }

        Action::UpdateInstall { part, yes } => {
            let conn = daemon(&mut stream)?;
            match part {
                Part::Firmware => install_firmware(conn, yes).await?,
                Part::App | Part::Tosu => install_update(conn, part).await?,
            }
        }

        Action::UpdateFlags {
            channel,
            check,
            install,
        } => {
            let conn = daemon(&mut stream)?;
            if let Some(channel) = channel {
                set_update_channel(conn, channel).await?;
            }
            if check {
                check_for_updates(conn).await?;
            }
            if install {
                // The status is for a person here; with --json the install's
                // own result is the one answer
                if !json_mode() {
                    show_update_status(conn).await?;
                }
                install_update(conn, Part::App).await?;
            } else {
                show_update_status(conn).await?;
            }
        }

        Action::Latency { reset } => {
            let conn = daemon(&mut stream)?;
            if reset {
                match send_request(conn, &IpcRequest::ResetLatencyStats).await? {
                    IpcResponse::Error(e) => return Err(refusal(conn, Fault::Failed, e).await),
                    _ => {
                        say!("✓ Latency statistics reset");
                        emit(serde_json::json!({ "ok": true }));
                    }
                }
            } else {
                match send_request(conn, &IpcRequest::GetStatus).await? {
                    IpcResponse::Status {
                        latency: Some(l), ..
                    } if json_mode() => emit(serde_json::to_value(l)?),
                    IpcResponse::Status {
                        latency: Some(l), ..
                    } => {
                        say!("=== Key edge -> HID submit latency (device-side) ===");
                        say!("Samples:          {}", l.samples);
                        say!("p50:              {} µs", l.p50_us);
                        say!("p99:              {} µs", l.p99_us);
                        say!("p99.9:            {} µs", l.p999_us);
                        say!("max:              {} µs", l.max_us);
                        say!(
                            "Deferred reports: {} (waited for the next USB poll, then sent)",
                            l.deferred_reports
                        );
                    }
                    IpcResponse::Status {
                        latency: None,
                        device_connected: false,
                        ..
                    } => {
                        return Err(Failure {
                            fault: Fault::NoPad,
                            message: "No latency data: the pad is not connected".into(),
                        }
                        .into());
                    }
                    IpcResponse::Status { latency: None, .. } => {
                        say!("No latency data yet (device not connected, or old firmware)");
                        emit(serde_json::Value::Null);
                    }
                    other => bail!("Unexpected response from daemon: {:?}", other),
                }
            }
        }

        Action::Stats { period } => {
            let request = match period {
                Some(days) => {
                    tap_rate::validate_history_days(days).map_err(anyhow::Error::msg)?;
                    IpcRequest::SetTapHistoryPeriod { days }
                }
                None => IpcRequest::GetTapStats,
            };
            // An older daemon cannot parse the request and drops the connection
            let resp = send_request(daemon(&mut stream)?, &request)
                .await
                .context("The daemon did not answer; it may predate KPM statistics")?;
            match resp {
                IpcResponse::TapStats(snapshot) if json_mode() => {
                    emit(serde_json::to_value(&snapshot)?)
                }
                IpcResponse::TapStats(snapshot) => print!("{}", format_tap_stats(&snapshot)),
                IpcResponse::Error(e) => bail!("{}", e),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }

        Action::BoardTest(TestArgs { fix_pins, magnet }) => {
            let stream = daemon(&mut stream)?;
            let mut report = run_board_test(stream).await?;
            let mut checks = board_test::evaluate(&report);
            let needs_pins = checks
                .iter()
                .any(|c| c.fix == Some(board_test::BoardFix::UseCarrierKeyPins));
            if fix_pins && needs_pins {
                let mut config = match send_request(stream, &IpcRequest::GetStatus).await? {
                    IpcResponse::Status { config, .. } => config,
                    other => bail!("Unexpected response from daemon: {:?}", other),
                };
                config.key1_gpio = board_test::CARRIER_KEY1_GPIO;
                config.key2_gpio = board_test::CARRIER_KEY2_GPIO;
                match send_request(stream, &IpcRequest::UpdateConfig(config)).await? {
                    IpcResponse::ConfigUpdated { .. } => {
                        say!("✓ Keys moved to GPIO10 / GPIO7; testing again\n")
                    }
                    IpcResponse::OperationRejected { reason } => {
                        return Err(rejected(format!("Could not set the key pins: {reason}")));
                    }
                    IpcResponse::Error(e) => bail!("Could not set the key pins: {}", e),
                    other => bail!("Unexpected response from daemon: {:?}", other),
                }
                // The pad moves the pins once both keys are released
                tokio::time::sleep(Duration::from_millis(300)).await;
                report = run_board_test(stream).await?;
                checks = board_test::evaluate(&report);
            }
            say_inline!("{}", format_board_test(&report, &checks));
            if magnet {
                let test = magnet_test(stream).await?;
                for c in test.checks() {
                    say!(
                        "{} {:<18} {}",
                        c.status.label().to_uppercase(),
                        c.name,
                        c.detail
                    );
                }
                checks.extend(test.checks());
            }
            let result = board_test::overall(&checks);
            emit(serde_json::json!({
                "ok": result != board_test::CheckStatus::Fail,
                "result": result,
                "report": report,
                "checks": checks,
            }));
            if result == board_test::CheckStatus::Fail {
                if !fix_pins && needs_pins {
                    say!("Run `opadctl pad test --fix-pins` to set the key pins.");
                }
                // The result above already says so
                if json_mode() {
                    std::process::exit(Fault::Failed as i32);
                }
                bail!("the board test found problems")
            }
        }

        Action::Animation(AnimationArgs { name, grade }) => {
            let animation = match name.to_ascii_lowercase().as_str() {
                "none" => result_anim::NONE,
                other => match result_anim::BUILTINS
                    .iter()
                    .find(|(_, n)| n.eq_ignore_ascii_case(other))
                {
                    Some((id, _)) => *id,
                    None => other.parse().map_err(|_| {
                        anyhow::anyhow!("Unknown animation {name:?}: pop, drop, burst or none")
                    })?,
                },
            };
            let req = IpcRequest::PlayAnimation {
                animation,
                grade: grade.to_ascii_uppercase(),
            };
            let conn = daemon(&mut stream)?;
            match send_request(conn, &req).await? {
                IpcResponse::AnimationPlayed => {
                    say!("Playing {} on the pad", result_anim::name(animation));
                    emit(serde_json::json!({ "ok": true }));
                }
                IpcResponse::Error(e) => return Err(refusal(conn, Fault::Failed, e).await),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }

        Action::Restart => {
            let conn = daemon(&mut stream)?;
            match send_request(conn, &IpcRequest::RestartPad).await? {
                IpcResponse::PadRestarting => {
                    say!("The pad is restarting; it reconnects in a few seconds");
                    emit(serde_json::json!({ "ok": true }));
                }
                IpcResponse::Error(e) => return Err(refusal(conn, Fault::Failed, e).await),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }

        Action::Doctor => unreachable!(),
        Action::EasterEgg => {
            let conn = daemon(&mut stream)?;
            match send_request(conn, &IpcRequest::TriggerEasterEgg).await? {
                IpcResponse::EasterEggTriggered => {
                    say!("🐱👅 Freaky 67 easter egg triggered on device!");
                    emit(serde_json::json!({ "ok": true }));
                }
                IpcResponse::Error(e) => return Err(refusal(conn, Fault::Failed, e).await),
                other => bail!("Unexpected response from daemon: {:?}", other),
            }
        }
    }

    Ok(())
}

/// `update channel <name>`. Changing the channel starts a check of the new one.
async fn set_update_channel(conn: &mut IpcStream, channel: String) -> Result<()> {
    match send_request(conn, &IpcRequest::SetUpdateChannel { channel }).await? {
        IpcResponse::UpdateCheckStarted => Ok(()),
        IpcResponse::OperationRejected { reason } | IpcResponse::OperationDeferred { reason } => {
            Err(rejected(reason))
        }
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

/// `update check`: ask for a check of the feed and wait until it has an answer
async fn check_for_updates(conn: &mut IpcStream) -> Result<()> {
    let before = match send_request(conn, &IpcRequest::GetUpdateStatus).await? {
        IpcResponse::UpdateStatus { last_check, .. } => last_check,
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    };
    match send_request(conn, &IpcRequest::CheckForUpdates).await? {
        IpcResponse::UpdateCheckStarted => {}
        IpcResponse::OperationDeferred { reason } => return Err(rejected(reason)),
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
    // The check runs in the daemon's update worker: done once it
    // is not running and has a newer answer or an error
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let IpcResponse::UpdateStatus {
            last_check,
            last_error,
            checking,
            ..
        } = send_request(conn, &IpcRequest::GetUpdateStatus).await?
        {
            if !checking && (last_check != before || last_error.is_some()) {
                return Ok(());
            }
        }
        if std::time::Instant::now() > deadline {
            bail!("The update check did not finish within 120 s");
        }
    }
}

/// What the three updaters know: the service's update status and the pad's
/// firmware offer, which is None when the service could not give one
async fn update_status(
    conn: &mut IpcStream,
) -> Result<(IpcResponse, Option<opad_ipc::FirmwareOffer>)> {
    let status = send_request(conn, &IpcRequest::GetUpdateStatus).await?;
    if !matches!(status, IpcResponse::UpdateStatus { .. }) {
        bail!("Unexpected response from daemon: {:?}", status);
    }
    let offer = match send_request(conn, &IpcRequest::GetFirmwareUpdate).await {
        Ok(IpcResponse::FirmwareUpdateOffer(offer)) => Some(offer),
        _ => None,
    };
    Ok((status, offer))
}

/// `update status --json`: the update status with the offer under `firmware`
fn update_json(status: &IpcResponse, offer: &Option<opad_ipc::FirmwareOffer>) -> serde_json::Value {
    let mut value = unwrapped(status);
    if let Some(fields) = value.as_object_mut() {
        fields.insert(
            "firmware".into(),
            serde_json::to_value(offer).unwrap_or_default(),
        );
    }
    value
}

/// One updater's lines in `update status`
fn say_component(update: &opad_ipc::ComponentUpdate) {
    say!(
        "Installed:        {}",
        update.installed.as_deref().unwrap_or("unknown")
    );
    let offer = match (&update.available, update.notify_only, update.with_app) {
        (None, _, _) => "none, up to date".to_string(),
        (Some(v), _, true) => format!("{} (comes with OPad's own update)", v),
        (Some(v), true, _) => format!("{} (this install cannot update itself)", v),
        (Some(v), false, _) if update.ready_to_install => format!("{} (ready to install)", v),
        (Some(v), false, _) => v.clone(),
    };
    say!("Available:        {}", offer);
    say!(
        "Automatic:        {}",
        if update.enabled { "on" } else { "off" }
    );
}

/// `update status`: OPad, tosu and the pad's firmware, as the app's Updates
/// tab shows them
async fn show_update_status(conn: &mut IpcStream) -> Result<()> {
    let (status, offer) = update_status(conn).await?;
    if json_mode() {
        emit(update_json(&status, &offer));
        return Ok(());
    }
    let IpcResponse::UpdateStatus {
        app,
        tosu,
        last_check,
        last_error,
        restart_required,
        pending_version,
        channel,
        last_result,
        ..
    } = status
    else {
        bail!("Unexpected response from daemon: {:?}", status);
    };
    println!("=== OPad Updates ===");
    println!("Channel:          {}", channel);
    say_component(&app);
    println!(
        "Last check:       {}",
        last_check.as_deref().unwrap_or("never")
    );
    if let Some(e) = &last_error {
        println!("Last error:       {}", e);
    }
    if let Some(r) = &last_result {
        println!("Last install:     {}", r);
    }
    if restart_required {
        match &pending_version {
            Some(v) => println!("Installed:        {v}, runs the next time OPad starts"),
            None => println!("Restart OPad to run the installed version."),
        }
    }

    println!("\n=== tosu ===");
    say_component(&tosu);

    println!("\n=== Pad firmware ===");
    match offer {
        Some(offer) => {
            println!(
                "Installed:        {}",
                offer.installed.as_deref().unwrap_or("unknown (no pad?)")
            );
            println!(
                "Available:        {}",
                offer.available.as_deref().unwrap_or("nothing newer")
            );
            println!(
                "Automatic:        {}",
                if offer.auto {
                    format!(
                        "on, once the pad has gone {} min unused",
                        offer.auto_after_minutes
                    )
                } else {
                    "off".to_string()
                }
            );
            if offer.available.is_some() {
                for blocker in &offer.blockers {
                    println!("  ⚠ {}", blocker);
                }
            }
        }
        None => println!("Installed:        unknown (the service gave no answer)"),
    }
    Ok(())
}

/// `update install app` or `tosu`
async fn install_update(conn: &mut IpcStream, part: Part) -> Result<()> {
    let update = match send_request(conn, &IpcRequest::GetUpdateStatus).await? {
        IpcResponse::UpdateStatus { app, tosu, .. } => {
            if part == Part::App {
                app
            } else {
                tosu
            }
        }
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    };
    // OPad's own update is downloaded first and installed from disk. tosu's
    // is fetched by the install itself, so it only needs to exist.
    let ready = match part {
        Part::App => update.ready_to_install,
        _ => update.available.is_some() && !update.with_app,
    };
    if !ready {
        return Err(rejected(format!(
            "There is no update of {} ready to install (run opadctl update check)",
            part.label()
        )));
    }
    let request = IpcRequest::InstallUpdate {
        component: part.component(),
    };
    match send_request(conn, &request).await? {
        IpcResponse::UpdateStarted { .. } => {
            let version = update.available.as_deref().unwrap_or("?");
            match part {
                Part::App => say!("✓ Installing OPad {version}; OPad restarts when it is done"),
                _ => say!("✓ Updating {} to {version}", part.label()),
            }
            emit(serde_json::json!({ "ok": true, "version": update.available }));
            Ok(())
        }
        IpcResponse::OperationRejected { reason } | IpcResponse::OperationDeferred { reason } => {
            Err(rejected(reason))
        }
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

/// `update install firmware`: flash the pad from the signed release manifest
/// (§U-3b)
async fn install_firmware(conn: &mut IpcStream, yes: bool) -> Result<()> {
    let offer = match send_request(conn, &IpcRequest::GetFirmwareUpdate).await? {
        IpcResponse::FirmwareUpdateOffer(o) => o,
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    };

    say!("=== OPad Firmware ===");
    say!(
        "Installed:        {}",
        offer.installed.as_deref().unwrap_or("unknown (no pad?)")
    );
    say!(
        "Running Slot:     {}",
        offer
            .running_partition
            .as_deref()
            .unwrap_or("unknown (firmware predates the OTA layout)")
    );

    for blocker in &offer.blockers {
        say!("  ⚠ {}", blocker);
    }

    let Some(available) = offer.available.as_deref() else {
        say!("Available:        nothing newer");
        emit(serde_json::json!({ "ok": true, "updated": false, "installed": offer.installed }));
        return Ok(());
    };
    say!("Available:        {}", available);
    if let Some(notes) = &offer.notes {
        say!("Notes:            {}", notes);
    }
    if !offer.blockers.is_empty() {
        let message = format!(
            "The pad cannot be flashed right now: {}",
            offer.blockers.join(" ")
        );
        return Err(refusal(conn, Fault::Rejected, message).await);
    }

    // §U-3b: explicit consent every time. --yes is the person saying so
    // in a script; it is not a way of skipping the decision.
    if let Some(text) = &offer.consent_text {
        say!();
        say!("{}", text);
    }
    if !yes
        && !confirm(
            "\nFlash the pad now? [y/N]: ",
            "A firmware update needs confirmation. Re-run with --yes.",
        )?
    {
        say!("Cancelled. Nothing was written to the pad.");
        emit(serde_json::json!({ "ok": true, "updated": false, "installed": offer.installed }));
        return Ok(());
    }

    say!("Flashing. Do not unplug the pad.");
    let request = IpcRequest::InstallFirmwareUpdate { confirm: true };
    match send_request(conn, &request).await? {
        IpcResponse::FirmwareUpdateFinished {
            from,
            to,
            running_partition,
            protocol_version,
            compatible,
            ..
        } => {
            say!("✓ Firmware updated from {} to {}", from, to);
            say!(
                "  Running Slot:     {}",
                running_partition.as_deref().unwrap_or("unknown")
            );
            if !compatible {
                say!("  ⚠ WARNING: the pad reports protocol version {} which this host does not speak!", protocol_version);
            }
            emit(serde_json::json!({
                "ok": true,
                "updated": true,
                "from": from,
                "to": to,
                "running_slot": running_partition,
                "compatible": compatible,
            }));
            Ok(())
        }
        IpcResponse::OperationRejected { reason } => {
            Err(refusal(conn, Fault::Rejected, format!("Rejected: {reason}")).await)
        }
        IpcResponse::Error(e) => bail!("{}", e),
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

fn fmt_ppm(ppm: Option<f64>) -> String {
    match ppm.filter(|p| p.is_finite()) {
        Some(p) => format!("{:.0}", p),
        None => "-".into(),
    }
}

/// `opadctl stats`: the live or last attempt, then the history
fn format_tap_stats(s: &TapStatsSnapshot) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let (attempt, live) = match (&s.current, &s.last) {
        (Some(a), _) => (Some(a), true),
        (None, Some(a)) => (Some(a), false),
        (None, None) => (None, false),
    };
    match attempt {
        Some(a) => {
            let _ = writeln!(
                out,
                "=== {} (KPM) ===",
                if live {
                    "Current attempt, live"
                } else {
                    "Last attempt"
                }
            );
            let map = &a.beatmap;
            if !map.title.is_empty() {
                let mut line = if map.artist.is_empty() {
                    map.title.clone()
                } else {
                    format!("{} - {}", map.artist, map.title)
                };
                if !map.difficulty.is_empty() {
                    let _ = write!(line, " [{}]", map.difficulty);
                }
                if let Some(mods) = map.mods.as_deref().filter(|m| !m.is_empty()) {
                    let _ = write!(line, " +{mods}");
                }
                let _ = writeln!(out, "Map:      {line}");
            }
            if let Some(end) = a.ended_at {
                let _ = writeln!(out, "Ended:    {}", end.format("%Y-%m-%d %H:%M UTC"));
            }
            if a.replay {
                let _ = writeln!(out, "Replay:   shown only, not saved to the history");
            }
            let _ = writeln!(
                out,
                "AVG:      {} KPM (every press over the song time, breaks included)",
                fmt_ppm(a.song.ppm())
            );
            let _ = writeln!(
                out,
                "PEAK:     {} KPM (fastest 7 presses in a row); best 10 s: {} KPM",
                fmt_ppm(a.combined.peak_ppm),
                fmt_ppm(a.song.peak_ppm)
            );
            let _ = writeln!(
                out,
                "{:<10}{:>9}{:>9}{:>9}{:>10}",
                "", "Current", "Tap avg", "Peak", "Presses"
            );
            for ch in TapChannel::ALL {
                let c = a.channel(ch);
                let _ = writeln!(
                    out,
                    "{:<10}{:>9}{:>9}{:>9}{:>10}",
                    ch.label(),
                    fmt_ppm(c.current_ppm),
                    fmt_ppm(c.average_ppm),
                    fmt_ppm(c.peak_ppm),
                    c.presses
                );
            }
        }
        None => {
            let _ = writeln!(out, "=== Attempt (KPM) ===");
            let _ = writeln!(out, "No attempt yet: play a map with tosu running.");
        }
    }

    let _ = writeln!(
        out,
        "\n=== History: {} ===",
        tap_rate::history_period_label(s.period_days)
    );
    match &s.history {
        Some(h) if h.attempts > 0 => {
            let _ = writeln!(out, "Attempts: {}", h.attempts);
            let _ = writeln!(out, "AVG:      {} KPM", fmt_ppm(h.song_ppm));
            let _ = writeln!(
                out,
                "PEAK:     {} KPM; best 10 s: {} KPM",
                fmt_ppm(h.combined.best_peak_ppm),
                fmt_ppm(h.best_song_peak_ppm)
            );
            let _ = writeln!(
                out,
                "{:<10}{:>9}{:>10}{:>11}{:>10}",
                "", "Tap avg", "Avg peak", "Best peak", "Presses"
            );
            for ch in TapChannel::ALL {
                let c = h.channel(ch);
                let _ = writeln!(
                    out,
                    "{:<10}{:>9}{:>10}{:>11}{:>10}",
                    ch.label(),
                    fmt_ppm(c.average_ppm),
                    fmt_ppm(c.average_peak_ppm),
                    fmt_ppm(c.best_peak_ppm),
                    c.total_presses
                );
            }
        }
        Some(_) => {
            let _ = writeln!(out, "No attempts in this period yet.");
        }
        None => {
            let _ = writeln!(out, "History not available (storage not ready).");
        }
    }
    if s.unsaved_attempts > 0 {
        let _ = writeln!(
            out,
            "{} attempt(s) will be saved when play stops.",
            s.unsaved_attempts
        );
    }
    out
}

/// The `monitor` cursor after a reply: the last entry received. The daemon
/// returns the first `limit` entries after it, so jumping to its latest seq
/// would drop the rest of a burst; with nothing received it stays put.
fn next_since_seq(since_seq: Option<u64>, entries: &[opad_model::LogEntry]) -> Option<u64> {
    entries.last().map(|e| e.seq).or(since_seq)
}

/// The daemon connection, for the commands that cannot work without one.
/// Asks a y/N question on the terminal; true only for "y"/"yes". Without a
/// terminal to ask on, fails with `non_tty_msg` rather than guessing.
fn confirm(prompt: &str, non_tty_msg: &str) -> Result<bool> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        bail!("{}", non_tty_msg);
    }
    say_inline!("{}", prompt);
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

fn daemon(stream: &mut Option<IpcStream>) -> Result<&mut IpcStream> {
    stream.as_mut().ok_or_else(|| {
        Failure {
            fault: Fault::ServiceUnavailable,
            message: "Failed to connect to opad-daemon. Is the daemon running?".into(),
        }
        .into()
    })
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
    say!("Asking opad-daemon to release the serial port...");
    match send_request(stream, &IpcRequest::PrepareFlash).await? {
        IpcResponse::ReadyForFlash { port } => Ok(explicit_port
            .or(port)
            .or_else(opad_device::find_target_port)),
        IpcResponse::OperationRejected { reason } => {
            Err(rejected(format!("Rejected by daemon: {reason}")))
        }
        other => bail!("Unexpected response from daemon: {:?}", other),
    }
}

/// The slot the pad runs from, as the daemon last heard it. `None` without a
/// daemon, a pad, or firmware that reports it.
async fn running_slot(stream: Option<&mut IpcStream>) -> Option<String> {
    match send_request(stream?, &IpcRequest::GetStatus).await.ok()? {
        IpcResponse::Status {
            device_connected: true,
            device_info: Some(info),
            ..
        } => info.running_partition,
        _ => None,
    }
}

fn print_flash_success(firmware_version: &str, protocol_version: u32, compatible: bool) {
    say!("✓ Flash succeeded!");
    say!("  Firmware Version: {}", firmware_version);
    say!("  Protocol Version: {}", protocol_version);
    emit(serde_json::json!({
        "ok": true,
        "firmware_version": firmware_version,
        "protocol_version": protocol_version,
        "compatible": compatible,
    }));
    if !compatible {
        say!(
            "  ⚠ WARNING: Device reported protocol version {} which is incompatible with host!",
            protocol_version
        );
    }
}

/// How long a rejected image takes to give up and fall back, with margin:
/// IMAGE_CONFIRM_TIMEOUT_US in firmware/main/app_main.c, then a reboot
const ROLLBACK_WAIT: Duration = Duration::from_secs(45);

/// After an A/B flash: success only if the pad came back running the slot
/// that was written. Back on the old slot means it rejected the new image.
async fn report_slot_after_flash(
    mut stream: Option<&mut IpcStream>,
    target: &str,
    finish: Option<IpcResponse>,
) -> Result<()> {
    if let Some(IpcResponse::FlashFinished {
        firmware_version,
        protocol_version,
        compatible,
    }) = &finish
    {
        if running_slot(stream.as_deref_mut()).await.as_deref() == Some(target) {
            print_flash_success(firmware_version, *protocol_version, *compatible);
            say!("  Running Slot:     {target}");
            return Ok(());
        }
    }
    say!("Waiting to see which image the pad settles on...");
    let deadline = tokio::time::Instant::now() + ROLLBACK_WAIT;
    while tokio::time::Instant::now() < deadline {
        match running_slot(stream.as_deref_mut()).await {
            Some(slot) if slot == target => {
                say!("✓ Flash succeeded! Running Slot: {target}");
                emit(serde_json::json!({ "ok": true, "running_slot": target }));
                return Ok(());
            }
            Some(slot) => bail!(
                "The pad did not accept the new firmware (it never came up as a working USB \
                 device) and went back to the previous one on {slot}. It works as before; \
                 nothing else is needed."
            ),
            None => tokio::time::sleep(Duration::from_secs(1)).await,
        }
    }
    bail!(
        "Firmware was written, but the pad did not come back within {}s. Run `opadctl \
         recover` and replug it; see docs/recovery.md.",
        ROLLBACK_WAIT.as_secs()
    )
}

/// Hand the port back to the daemon and let it verify what is now running.
/// `None` means there was no daemon to hand it back to.
async fn finish_flash(stream: Option<&mut IpcStream>) -> Option<IpcResponse> {
    let stream = stream?;
    say!("Resuming daemon device communication and verifying new firmware...");
    send_request(stream, &IpcRequest::FinishFlash).await.ok()
}

/// Hand the port back without verifying: the pad is not expected back as the
/// app (left in the bootloader, or the flash failed). An older daemon does not
/// know ResumeDevice and closes the connection; FinishFlash on a new one
/// resumes it too, and its reply (the verification) is not waited for.
async fn resume_device(stream: Option<&mut IpcStream>) {
    let Some(stream) = stream else {
        return;
    };
    match send_request(stream, &IpcRequest::ResumeDevice).await {
        Ok(IpcResponse::DeviceResumed) => {}
        Ok(other) => say!("⚠ Unexpected response from daemon to ResumeDevice: {other:?}"),
        Err(_) => resume_with_finish_flash().await,
    }
}

async fn resume_with_finish_flash() {
    match opad_ipc::connect_and_handshake().await {
        Ok((mut stream, _)) => {
            // The daemon resumes as soon as it reads FinishFlash; the rest of
            // the reply is the reconnect wait this caller does not want
            let _ = tokio::time::timeout(
                Duration::from_secs(2),
                send_request(&mut stream, &IpcRequest::FinishFlash),
            )
            .await;
        }
        Err(e) => say!(
            "⚠ Could not hand the pad back to opad-daemon ({e}); restart the daemon if the pad stays disconnected"
        ),
    }
}

/// While the daemon has the pad paused for us: on Ctrl-C, hand it back
/// before exiting, instead of leaving it paused. (A newer daemon also resumes
/// by itself when this process's connection drops; this covers older ones and
/// says what is happening.) Clear the returned flag once the pad is handed
/// back. The watcher stays installed, because a SIGINT handler outlives its
/// task: Ctrl-C keeps exiting the process afterwards, just without a resume.
fn resume_on_interrupt(daemon_paused: bool) -> Option<Arc<AtomicBool>> {
    if !daemon_paused {
        return None;
    }
    let paused = Arc::new(AtomicBool::new(true));
    let watching = paused.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            if watching.load(Ordering::SeqCst) {
                eprintln!("Interrupted; handing the pad back to opad-daemon...");
                match opad_ipc::connect_and_handshake().await {
                    Ok((mut stream, _)) => resume_device(Some(&mut stream)).await,
                    Err(e) => eprintln!(
                        "⚠ Could not reach opad-daemon ({e}); restart it if the pad stays disconnected"
                    ),
                }
            }
            std::process::exit(130);
        }
    });
    Some(paused)
}

/// The pad is back with the daemon: Ctrl-C no longer needs to resume it
fn release(interrupt: &Option<Arc<AtomicBool>>) {
    if let Some(paused) = interrupt {
        paused.store(false, Ordering::SeqCst);
    }
}

/// Which images to write, and where. `full` is the recovery flash that
/// `scripts/flash_board.sh` used to perform: bootloader, partition table and a
/// fresh `otadata` as well as the app, and the asset packs
/// (`opad-assets.bin`, `opad-user.bin`) when they are beside it.
///
/// `path` is the app image, or — with `--full` — an ESP-IDF build directory.
fn resolve_flash_set(path: &Path, full: bool) -> Result<Vec<(u32, PathBuf)>> {
    if !path.exists() {
        bail!("No such file or directory: {}", path.display());
    }

    let (build_dir, app_image) = if path.is_dir() {
        (path.to_path_buf(), path.join("opad-firmware.bin"))
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
    // .bin files a firmware release ships (release-firmware.yml).
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
    // Asset packs (scripts/build_asset_pack.py) beside the app image are
    // written too; the table above is the one that has their partitions
    for part in [flash::ASSETS_PARTITION, flash::USER_PARTITION] {
        let pack = build_dir.join(part.file_name);
        if !pack.exists() {
            continue;
        }
        let size = std::fs::metadata(&pack)
            .with_context(|| format!("reading {}", pack.display()))?
            .len();
        if size > u64::from(part.size) {
            bail!(
                "{} is {} bytes; its partition holds {}",
                pack.display(),
                size,
                part.size
            );
        }
        images.push((part.offset, pack));
    }
    images.push((APP_PARTITION_OFFSET, app_image));
    Ok(images)
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Where a hand install goes (the admin's directory, which overrides the
/// package's).
const UDEV_RULE_PATH: &str = "/etc/udev/rules.d/70-opad.rules";
/// Every place the rule may already live: the admin's directory, then where
/// the .deb/.rpm/AppImage/`make install` put it. udev reads both.
const UDEV_RULE_PATHS: [&str; 2] = [UDEV_RULE_PATH, "/usr/lib/udev/rules.d/70-opad.rules"];

#[derive(Debug, PartialEq, Eq)]
enum UdevRuleState {
    /// An installed copy is byte-for-byte the rule this build ships.
    Installed(&'static str),
    /// A copy exists but none matches (older package, hand edit).
    Differs,
    /// Nothing installed anywhere udev looks.
    Missing,
}

fn udev_rule_state(
    installed: impl IntoIterator<Item = (&'static str, Option<String>)>,
    embedded: &str,
) -> UdevRuleState {
    let mut found_any = false;
    for (path, content) in installed {
        if let Some(current) = content {
            if current.trim_end() == embedded.trim_end() {
                return UdevRuleState::Installed(path);
            }
            found_any = true;
        }
    }
    if found_any {
        UdevRuleState::Differs
    } else {
        UdevRuleState::Missing
    }
}

/// The install command is built from the embedded rule, so it works from any
/// directory — the packaged binary has no `packaging/` tree next to it.
fn print_udev_install_hint(udev_rule: &str, lead: &str) {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        say!("This is the AppImage; it can install the rule itself:");
        say!(
            "  \"{}\" install-udev",
            std::path::Path::new(&appimage).display()
        );
        return;
    }
    say!("{lead}");
    say!("  sudo tee {UDEV_RULE_PATH} >/dev/null <<'EOF'");
    say_inline!("{}", udev_rule);
    if !udev_rule.ends_with('\n') {
        say!();
    }
    say!("EOF");
    say!("  sudo udevadm control --reload-rules && sudo udevadm trigger");
}

/// `doctor`: what this computer still needs for the pad's serial port and for
/// tosu to read osu!. It reports and changes nothing, so it always succeeds;
/// `--json` carries `ok: false` when something is missing.
fn run_doctor() -> Result<()> {
    // udev is Linux's; on Windows the pad binds to inbox drivers with no setup
    // step at all, so saying nothing would read as "something is missing".
    if !cfg!(target_os = "linux") {
        say!("=== OPad Setup ===");
        say!("Nothing to do on this platform: the pad uses the inbox USB");
        say!("drivers, so HID and the CDC port work with no setup step.");
        emit(serde_json::json!({ "ok": true, "checks": [] }));
        return Ok(());
    }

    say!("=== OPad Linux Setup ===");
    // The same file the packages install, so the two cannot drift apart
    let udev_rule = include_str!("../../../packaging/linux/udev/70-opad.rules");

    let installed = UDEV_RULE_PATHS
        .iter()
        .map(|p| (*p, std::fs::read_to_string(p).ok()));
    let udev = udev_rule_state(installed, udev_rule);
    let udev_json = match &udev {
        UdevRuleState::Installed(path) => serde_json::json!({
            "name": "udev_rule", "ok": true, "state": "installed", "path": path,
        }),
        UdevRuleState::Differs => serde_json::json!({
            "name": "udev_rule", "ok": false, "state": "differs",
        }),
        UdevRuleState::Missing => serde_json::json!({
            "name": "udev_rule", "ok": false, "state": "missing",
        }),
    };
    match udev {
        UdevRuleState::Installed(path) => {
            say!("✓ udev rule for non-root CDC access is installed ({path})");
        }
        UdevRuleState::Differs => {
            say!("⚠ an installed udev rule differs from the rule this build ships.");
            print_udev_install_hint(udev_rule, "To update it, run:");
        }
        UdevRuleState::Missing => {
            // Packages and `make install` ship the rule; only a source build
            // installed per-user (make install-user) lands here.
            say!("udev rule for non-root CDC access is not installed.");
            print_udev_install_hint(udev_rule, "To install it, run:");
        }
    }

    // Yama: can tosu read osu!'s memory at all?
    let ptrace = opad_tosu::ptrace_access();
    let ptrace_json = match &ptrace {
        opad_tosu::PtraceAccess::Blocked { scope, fix } => serde_json::json!({
            "name": "osu_memory_access", "ok": false, "state": "blocked",
            "ptrace_scope": scope.to_string(), "fix": fix.to_string(),
        }),
        opad_tosu::PtraceAccess::Unknown { scope, fix } => serde_json::json!({
            "name": "osu_memory_access", "ok": true, "state": "unknown",
            "ptrace_scope": scope.to_string(), "fix": fix.to_string(),
        }),
        opad_tosu::PtraceAccess::Capable { scope } => serde_json::json!({
            "name": "osu_memory_access", "ok": true, "state": "capable",
            "ptrace_scope": scope.to_string(),
        }),
        opad_tosu::PtraceAccess::Unrestricted => serde_json::json!({
            "name": "osu_memory_access", "ok": true, "state": "unrestricted",
        }),
    };
    match ptrace {
        opad_tosu::PtraceAccess::Blocked { scope, fix } => {
            say!();
            say!("⚠ kernel.yama.ptrace_scope is {scope}: tosu cannot read osu!'s memory,");
            say!("  so the pad shows no live gameplay data. To fix:");
            say!("    {fix}");
            say!("  See docs/troubleshooting.md §2.");
        }
        opad_tosu::PtraceAccess::Unknown { scope, fix } => {
            say!();
            say!("kernel.yama.ptrace_scope is {scope} and `getcap` is not installed, so");
            say!("whether tosu may read osu!'s memory is unknown. If the pad shows no live data:");
            say!("    {fix}");
        }
        opad_tosu::PtraceAccess::Capable { scope } => {
            say!("✓ Yama ptrace_scope {scope}, and tosu has cap_sys_ptrace");
        }
        opad_tosu::PtraceAccess::Unrestricted => {}
    }

    say!("Setup check completed.");
    let ok = udev_json["ok"] == true && ptrace_json["ok"] == true;
    emit(serde_json::json!({ "ok": ok, "checks": [udev_json, ptrace_json] }));
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
    fn the_udev_rule_check_compares_against_the_embedded_rule() {
        let rule = "SUBSYSTEM==\"tty\", ATTRS{idVendor}==\"303a\", MODE=\"0666\"\n";
        let etc = "/etc/udev/rules.d/70-opad.rules";
        let lib = "/usr/lib/udev/rules.d/70-opad.rules";
        let state = |e: Option<&str>, l: Option<&str>| {
            udev_rule_state(
                [(etc, e.map(str::to_owned)), (lib, l.map(str::to_owned))],
                rule,
            )
        };
        assert_eq!(state(None, None), UdevRuleState::Missing);
        // A package install (usr/lib only) counts as installed
        assert_eq!(state(None, Some(rule)), UdevRuleState::Installed(lib));
        assert_eq!(state(Some(rule), None), UdevRuleState::Installed(etc));
        // A trailing newline difference is not a real difference
        assert_eq!(
            state(Some(rule.trim_end()), None),
            UdevRuleState::Installed(etc)
        );
        // An old copy in one place does not hide a current one in the other
        assert_eq!(
            state(Some("# old rule\n"), Some(rule)),
            UdevRuleState::Installed(lib)
        );
        assert_eq!(state(Some("# old rule\n"), None), UdevRuleState::Differs);
    }

    #[test]
    fn the_monitor_cursor_follows_the_last_entry_received() {
        use opad_model::{LogEntry, LogLevel, LogSource};
        let entry = |seq| LogEntry::new(seq, LogSource::Host, LogLevel::Info, "t", "m");
        // A burst over `limit`: 51.. must still be fetched next time
        let burst: Vec<_> = (1..=50).map(entry).collect();
        assert_eq!(next_since_seq(None, &burst), Some(50));
        assert_eq!(next_since_seq(Some(50), &[]), Some(50));
        assert_eq!(next_since_seq(None, &[]), None);
        // A single entry is not fetched again
        assert_eq!(next_since_seq(None, &[entry(1)]), Some(1));
    }

    #[test]
    fn stats_prints_the_attempt_and_history() {
        use opad_model::tap_rate::{
            AttemptStats, BeatmapRef, ChannelHistory, ChannelStats, TapHistory,
        };
        let empty = format_tap_stats(&TapStatsSnapshot {
            period_days: 30,
            ..Default::default()
        });
        assert!(empty.contains("No attempt yet"), "{empty}");
        assert!(empty.contains("History: Last 30 days"), "{empty}");
        assert!(empty.contains("storage not ready"), "{empty}");

        let ch = |avg: f64, presses| ChannelStats {
            current_ppm: None,
            average_ppm: Some(avg),
            peak_ppm: Some(avg + 20.4),
            presses,
            ..Default::default()
        };
        let snapshot = TapStatsSnapshot {
            current: None,
            last: Some(AttemptStats {
                started_at: "2026-09-29T12:00:00Z".parse().unwrap(),
                ended_at: Some("2026-09-29T12:03:10Z".parse().unwrap()),
                beatmap: BeatmapRef {
                    title: "Blue Zenith".into(),
                    artist: "xi".into(),
                    difficulty: "FOUR DIMENSIONS".into(),
                    ..Default::default()
                },
                k1: ch(100.2, 50),
                k2: ch(99.8, 50),
                combined: ch(200.0, 100),
                song: tap_rate::SongRate {
                    presses: 100,
                    ms: 40_000.0,
                    peak_ppm: Some(212.0),
                },
                replay: false,
            }),
            history: Some(TapHistory {
                period_days: 0,
                attempts: 3,
                combined: ChannelHistory {
                    average_ppm: Some(190.4),
                    best_peak_ppm: Some(251.6),
                    total_presses: 3000,
                    ..Default::default()
                },
                ..Default::default()
            }),
            period_days: 0,
            unsaved_attempts: 2,
        };
        let out = format_tap_stats(&snapshot);
        assert!(out.contains("Last attempt"), "{out}");
        assert!(out.contains("Ended:    2026-09-29 12:03 UTC"), "{out}");
        assert!(out.contains("xi - Blue Zenith [FOUR DIMENSIONS]"), "{out}");
        assert!(out.contains("History: All time"), "{out}");
        assert!(out.contains("Attempts: 3"), "{out}");
        assert!(out.contains("AVG:      150 KPM"), "{out}");
        assert!(
            out.contains("PEAK:     220 KPM (fastest 7 presses in a row); best 10 s: 212 KPM"),
            "{out}"
        );
        assert!(out.contains("PEAK:     252 KPM; best 10 s: - KPM"), "{out}");
        let combined = out.lines().rfind(|l| l.starts_with("Combined")).unwrap();
        assert!(
            combined.contains("190") && combined.contains("252"),
            "{out}"
        );
        assert!(out.contains("2 attempt(s) will be saved"), "{out}");
        assert!(!out.to_lowercase().contains("bpm"), "{out}");
    }

    fn action(args: &[&str]) -> Action {
        use clap::Parser;
        let line = [&["opadctl"], args].concat();
        match Cli::try_parse_from(&line) {
            Ok(cli) => cli.command.into(),
            Err(e) => panic!("{line:?}: {e}"),
        }
    }

    #[test]
    fn every_old_command_name_still_means_the_same() {
        for (old, new) in [
            (&["setup"][..], &["doctor"][..]),
            (&["sync"], &["counters", "sync"]),
            (&["reset", "--yes"], &["counters", "reset", "--yes"]),
            (&["reset"], &["counters", "reset"]),
            (&["export", "b.json"], &["backup", "export", "b.json"]),
            (
                &["import", "b.json", "--yes"],
                &["backup", "import", "b.json", "--yes"],
            ),
            (&["latency", "--reset"], &["latency", "reset"]),
            (
                &["monitor", "--follow", "--level", "warning"],
                &["logs", "--follow", "--level", "warning"],
            ),
            (&["update"], &["update", "status"]),
            (
                &["firmware-update", "--yes"],
                &["update", "install", "firmware", "--yes"],
            ),
            (&["restart"], &["pad", "restart"]),
            (
                &["board-test", "--fix-pins", "--magnet"],
                &["pad", "test", "--fix-pins", "--magnet"],
            ),
            (
                &["animation", "pop", "SS"],
                &["pad", "animation", "pop", "SS"],
            ),
            (
                &["flash", "build", "--full", "--port", "/dev/ttyACM1"],
                &["pad", "flash", "build", "--full", "--port", "/dev/ttyACM1"],
            ),
            (
                &["recover", "build", "--timeout", "60"],
                &["pad", "recover", "build", "--timeout", "60"],
            ),
            (&["bootloader"], &["pad", "bootloader"]),
        ] {
            assert_eq!(action(old), action(new), "{old:?}");
        }
    }

    #[test]
    fn the_old_update_flags_still_run_in_their_order() {
        assert_eq!(
            action(&["update", "--channel", "beta", "--check", "--install"]),
            Action::UpdateFlags {
                channel: Some("beta".into()),
                check: true,
                install: true,
            }
        );
        assert_eq!(action(&["update", "check"]), Action::UpdateCheck);
        assert_eq!(
            action(&["update", "channel"]),
            Action::UpdateChannel { channel: None }
        );
        assert_eq!(
            action(&["update", "auto", "tosu", "off"]),
            Action::UpdateAuto {
                part: Part::Tosu,
                on: false,
            }
        );
        assert_eq!(
            action(&["update", "install", "app"]),
            Action::UpdateInstall {
                part: Part::App,
                yes: false,
            }
        );
    }

    #[test]
    fn json_is_accepted_before_and_after_the_command() {
        use clap::Parser;
        for line in [
            &["opadctl", "--json", "status"][..],
            &["opadctl", "status", "--json"],
            &["opadctl", "pad", "restart", "--json"],
            &["opadctl", "sync", "--json"],
        ] {
            assert!(Cli::try_parse_from(line).unwrap().json, "{line:?}");
        }
        assert!(!Cli::try_parse_from(["opadctl", "status"]).unwrap().json);
    }

    #[test]
    fn the_old_names_are_left_out_of_help() {
        use clap::CommandFactory;
        let help = Cli::command().render_help().to_string();
        for shown in [
            "doctor", "counters", "backup", "logs", "update", "pad", "flash", "recover",
        ] {
            assert!(
                help.contains(&format!("\n  {shown} ")),
                "{shown} is missing"
            );
        }
        for hidden in [
            "sync",
            "firmware-update",
            "board-test",
            "bootloader",
            "monitor",
        ] {
            assert!(!help.contains(&format!("\n  {hidden} ")), "{hidden} shows");
        }
    }

    #[test]
    fn each_kind_of_failure_has_its_own_exit_code() {
        let plain = anyhow::anyhow!("the pad said something odd");
        assert_eq!(fault_of(&plain) as u8, 1);
        assert_eq!(fault_of(&rejected("mid-map")) as u8, 4);
        // The code survives the context a caller adds on top
        let wrapped = rejected("mid-map").context("could not sync");
        assert_eq!(fault_of(&wrapped), Fault::Rejected);
        let mut none = None;
        let Err(unavailable) = daemon(&mut none) else {
            panic!("there is no connection");
        };
        assert_eq!(fault_of(&unavailable) as u8, 3);
        assert_eq!(Fault::NoPad as u8, 5);
        assert_eq!(Fault::NoPad.name(), "no_pad");
    }

    #[test]
    fn a_json_reply_loses_the_wrapper_that_names_its_kind() {
        let reply = IpcResponse::OperationRejected {
            reason: "mid-map".into(),
        };
        assert_eq!(
            unwrapped(&reply),
            serde_json::json!({ "reason": "mid-map" })
        );
    }

    #[test]
    fn stats_period_is_optional() {
        use clap::Parser;
        assert!(Cli::try_parse_from(["opadctl", "stats"]).is_ok());
        assert!(Cli::try_parse_from(["opadctl", "stats", "--period", "0"]).is_ok());
        assert!(Cli::try_parse_from(["opadctl", "stats", "--period", "week"]).is_err());
    }

    #[test]
    fn log_filters_reject_unknown_values() {
        use clap::Parser;
        let parse = |args: &[&str]| Cli::try_parse_from([&["opadctl", "monitor"], args].concat());
        assert!(parse(&["--level", "err"]).is_err());
        assert!(parse(&["--source", "firmware"]).is_err());
        for ok in [
            &["--level", "error"][..],
            &["--level", "WARNING"],
            &["--source", "daemon"],
            &["--source", "device"],
            &["--source", "gui"],
            &["--source", "tosu"],
        ] {
            assert!(parse(ok).is_ok(), "{ok:?}");
        }
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
        // A firmware release (release-firmware.yml): no bootloader/ or
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
    fn full_flash_writes_asset_packs_found_beside_the_app() {
        let dir = std::env::temp_dir().join(format!("opadctl-packs-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bootloader")).unwrap();
        std::fs::create_dir_all(dir.join("partition_table")).unwrap();
        for f in [
            "bootloader/bootloader.bin",
            "partition_table/partition-table.bin",
            "ota_data_initial.bin",
            "opad-firmware.bin",
            "opad-user.bin",
        ] {
            std::fs::write(dir.join(f), [0xE9; 32]).unwrap();
        }

        let set = resolve_flash_set(&dir, true).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        // No opad-assets.bin here: only the user pack, and the app stays last
        assert_eq!(
            set.iter().map(|(o, _)| *o).collect::<Vec<_>>(),
            vec![0x0, 0x8000, 0xf000, 0x930000, 0x20000]
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
