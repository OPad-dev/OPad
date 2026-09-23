//! Writing firmware to the pad over USB (§W1-3, §U-3b).
//!
//! This lives beside the port discovery rather than in the CLI because two
//! callers need exactly the same sequence: `opadctl flash`, and the daemon's
//! host-driven firmware update. Two implementations of "reboot the pad into
//! the ROM bootloader and write its app partition" is one too many — the
//! failure modes here are the ones that stop the pad being a keyboard.
//!
//! The order is always the same, and the app image is always written last:
//! nothing reboots the pad until every image is down, so an interrupted flash
//! leaves it in download mode rather than half-booted.
//!
//! **Never `erase-flash` from here.** That wipes NVS, which holds the lifetime
//! counters and the owner record (§U-3b). Erasing is a deliberate, documented,
//! manual act (`docs/recovery.md` §7), not something an updater does.

use crate::{select_bootloader_port, BootloaderMatch, PadLocation};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info};

/// Where the app image lives: `ota_0` in `firmware/partitions.csv` (§U-3a).
/// It was `0x10000` under the old single-app table, and a pad flashed at the
/// old offset with the new table does not boot.
pub const APP_PARTITION_OFFSET: u32 = 0x20000;

#[derive(Debug, Error)]
pub enum FlashError {
    #[error("OPad not found: neither the app (303a:4001) nor the ROM bootloader (303a:1001) is connected")]
    NoDevice,
    #[error("{0} could not be opened. On Windows the port is exclusive: close anything else using it (a serial monitor, another opad-daemon) first. ({1})")]
    PortBusy(String, String),
    #[error("The pad did not re-enumerate as the ROM bootloader (303a:1001) after every trigger. See docs/recovery.md for the manual BOOT+RESET sequence.")]
    NoBootloader,
    #[error("Several ESP32 ROM bootloader ports could be this pad ({0:?}); pass the right one with --port")]
    AmbiguousBootloader(Vec<String>),
    #[error("Could not run espflash ({0}). Install espflash, or flash by hand as docs/recovery.md describes.")]
    EspflashMissing(String),
    #[error("espflash failed writing {path} at {offset:#x} (exit {code:?}). The pad is still in download mode; see docs/recovery.md.")]
    Espflash {
        path: String,
        offset: u32,
        code: Option<i32>,
    },
    #[error("The firmware was written, but rebooting the pad into it failed: {0}. See docs/recovery.md.")]
    ResetFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// How to ask the running app to reboot into the ROM download bootloader.
///
/// The firmware accepts all three (`firmware/main/usb/usb_cdc.c`) and they are
/// tried in this order. More than one exists because which of them lands
/// depends on how the host's CDC driver orders `SET_LINE_CODING` against
/// `SET_CONTROL_LINE_STATE`, and Linux and Windows disagree: Linux asserts DTR
/// when the tty is opened, while on Windows serialport's DCB sets
/// `fDtrControl = Disable` and leaves DTR low unless it is set explicitly
/// (§W1-3). Every line state below is therefore set by hand rather than
/// inherited from the open, so the sequence means the same thing on both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootTrigger {
    /// The plain-text command, honoured between protocol frames. Baud-rate and
    /// line-state independent, so it behaves identically everywhere and is
    /// tried first.
    Command,
    /// 1200-baud touch: the firmware arms download mode on the line-coding
    /// change and fires when DTR drops, i.e. when the port is closed.
    BaudTouch,
}

/// The firmware ignores the esptool RTS/DTR pattern: serial probes such as
/// ModemManager produce it when opening any port.
pub const BOOT_TRIGGERS: [BootTrigger; 2] = [BootTrigger::Command, BootTrigger::BaudTouch];

fn pulse_trigger(path: &str, trigger: BootTrigger) -> Result<(), FlashError> {
    let baud = match trigger {
        BootTrigger::BaudTouch => 1200,
        _ => 115_200,
    };
    let mut sp = serialport::new(path, baud)
        .timeout(Duration::from_millis(300))
        .open()
        .map_err(|e| FlashError::PortBusy(path.to_string(), e.to_string()))?;

    match trigger {
        BootTrigger::Command => {
            let _ = sp.write_data_terminal_ready(true);
            let _ = sp.write_request_to_send(true);
            sp.write_all(b"BOOTLOADER\n")?;
            let _ = sp.flush();
        }
        BootTrigger::BaudTouch => {
            // Opening at 1200 baud is the whole trigger; dropping the handle
            // below clears DTR and fires it.
            let _ = sp.write_data_terminal_ready(true);
        }
    }
    drop(sp);
    Ok(())
}

fn is_bootloader_port(path: &str) -> bool {
    crate::bootloader_ports().iter().any(|b| b.name == path)
}

fn pick(pad: &PadLocation, preexisting: &[String]) -> Result<Option<String>, FlashError> {
    match select_bootloader_port(&crate::bootloader_ports(), pad, preexisting) {
        BootloaderMatch::Found(p) => Ok(Some(p)),
        BootloaderMatch::None => Ok(None),
        BootloaderMatch::Ambiguous(ports) => Err(FlashError::AmbiguousBootloader(ports)),
    }
}

/// Reboot the running app into the ROM download bootloader and return the port
/// it came back on — the port of *this* pad, matched by MAC or USB path, never
/// just any 303a:1001 port.
///
/// `app_port` may itself name a bootloader port (`--port` override), which is
/// then used as-is. With no app port, a pad already in the bootloader is used
/// only if it is the sole bootloader port.
pub async fn enter_bootloader(
    app_port: Option<&str>,
    device_id: Option<&str>,
) -> Result<String, FlashError> {
    let Some(app_port) = app_port else {
        return pick(&PadLocation::default(), &[])?.ok_or(FlashError::NoDevice);
    };
    if is_bootloader_port(app_port) {
        return Ok(app_port.to_string());
    }

    let pad = crate::locate_pad(app_port, device_id);
    debug!("Pad on {} is at {:?}", app_port, pad);
    // Ports already in download mode are someone else's unless they prove
    // otherwise by MAC or path
    let preexisting: Vec<String> = crate::bootloader_ports()
        .into_iter()
        .map(|b| b.name)
        .collect();

    info!("Rebooting {} into the ROM download bootloader", app_port);
    let mut last_err = None;
    let mut fired = false;
    for trigger in BOOT_TRIGGERS {
        // An earlier trigger may have landed after its window closed (on
        // Windows the first bootloader enumeration waits on a driver install)
        if fired {
            if let Some(p) = pick(&pad, &preexisting)? {
                return Ok(p);
            }
        }
        // The pad may still be re-enumerating from the previous attempt
        if let Err(e) = wait_until_openable(app_port, Duration::from_secs(2)).await {
            if fired {
                if let Some(p) = pick(&pad, &preexisting)? {
                    return Ok(p);
                }
            }
            last_err = Some(unusable_app_port_error(e, fired, port_present(app_port)));
            continue;
        }
        if let Err(e) = pulse_trigger(app_port, trigger) {
            last_err = Some(unusable_app_port_error(e, fired, port_present(app_port)));
            continue;
        }
        fired = true;
        // A trigger that went out and missed is a bootloader problem, whatever
        // stopped an earlier one
        last_err = None;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while tokio::time::Instant::now() < deadline {
            if let Some(p) = pick(&pad, &preexisting)? {
                return Ok(p);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        debug!("The {:?} trigger did not take", trigger);
    }
    if fired {
        if let Some(p) = pick(&pad, &preexisting)? {
            return Ok(p);
        }
    }

    Err(last_err.unwrap_or(FlashError::NoBootloader))
}

/// What to report when `app_port` could not be used for the next trigger.
/// After a trigger has fired, a vanished app port means the pad left the app,
/// most likely for download mode on a port not matched yet: "close anything
/// else using it" would send the user after the wrong problem.
fn unusable_app_port_error(err: FlashError, fired: bool, app_port_present: bool) -> FlashError {
    if fired && !app_port_present {
        FlashError::NoBootloader
    } else {
        err
    }
}

fn port_present(path: &str) -> bool {
    serialport::available_ports()
        .map(|ports| ports.iter().any(|p| p.port_name == path))
        // Unknown: keep whatever error the port itself gave
        .unwrap_or(true)
}

/// The espflash OPad ships (pinned in the Makefile's ESPFLASH_VERSION) before
/// any other: next to the binary on Windows, `<prefix>/lib/opad/bin` beside
/// `<prefix>/bin` on Linux (.deb, .rpm, make install and the AppImage's usr/).
fn resolve_espflash() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let mut candidates = vec![dir.join("espflash.exe"), dir.join("espflash")];
            if let Some(prefix) = dir.parent() {
                candidates.push(prefix.join("lib/opad/bin/espflash"));
            }
            if let Some(found) = candidates.into_iter().find(|c| c.is_file()) {
                return found;
            }
        }
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let candidate = PathBuf::from(userprofile)
            .join(".cargo")
            .join("bin")
            .join("espflash.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("espflash")
}

/// Write each image at its offset, in the order given.
///
/// Every invocation stays in the flasher stub (`--after no-reset-no-stub`), so
/// the caller decides when — and whether — the pad reboots.
pub fn write_images(
    images: &[(u32, PathBuf)],
    boot_port: &str,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<(), FlashError> {
    let espflash_cmd = resolve_espflash();
    for (offset, path) in images {
        progress(&format!(
            "Writing {} at {:#x} on {}...",
            path.display(),
            offset,
            boot_port
        ));
        let status = std::process::Command::new(&espflash_cmd)
            .args(["write-bin", "--chip", "esp32s3", "-p", boot_port])
            // Already in download mode, and every image after the first needs
            // the stub still there. espflash cannot reset an ESP32-S3 out of
            // forced download mode; reset_to_app does that at the end.
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
            .map_err(|e| FlashError::EspflashMissing(e.to_string()))?;
        if !status.success() {
            return Err(FlashError::Espflash {
                path: path.display().to_string(),
                offset: *offset,
                code: status.code(),
            });
        }
    }
    Ok(())
}

/// The whole sequence: into the bootloader, write everything, back into the app.
pub async fn flash(
    images: &[(u32, PathBuf)],
    app_port: Option<&str>,
    device_id: Option<&str>,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<(), FlashError> {
    let boot_port = enter_bootloader(app_port, device_id).await?;
    write_images(images, &boot_port, progress)?;
    progress("Firmware written, rebooting into the application...");
    reset_to_app(&boot_port)
}

/// Wait until the port can actually be opened. On Windows the handle is
/// exclusive, so this is where a daemon that has not let go yet shows up as a
/// clear wait rather than as a mysterious flash failure (§W1-3).
pub async fn wait_until_openable(path: &str, timeout: Duration) -> Result<(), FlashError> {
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
                return Err(FlashError::PortBusy(path.to_string(), e.to_string()));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
}

pub async fn wait_for_port(find: fn() -> Option<String>, timeout: Duration) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if let Some(p) = find() {
            return Some(p);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

// ---------------------------------------------------------------------------
// Minimal ESP serial bootloader (SLIP) client, just enough to leave download
// mode.
//
// The firmware enters the ROM bootloader by setting RTC_CNTL_FORCE_DOWNLOAD_BOOT,
// which survives resets. espflash 4.x refuses to watchdog-reset an ESP32-S3
// while that bit is set and its RTS hard reset does not reach the chip over
// USB-Serial-JTAG, so the board would stay in the bootloader after flashing.
// Here we do what esptool does: clear the bit, then trigger an RTC watchdog
// reset.
// ---------------------------------------------------------------------------

const CMD_WRITE_REG: u8 = 0x09;

const RTC_CNTL_OPTION1_REG: u32 = 0x6000_812C;
const RTC_CNTL_WDTCONFIG0_REG: u32 = 0x6000_8098;
const RTC_CNTL_WDTCONFIG1_REG: u32 = 0x6000_809C;
const RTC_CNTL_WDTWPROTECT_REG: u32 = 0x6000_80B0;
const RTC_CNTL_WDT_WKEY: u32 = 0x50D8_3AA1;
/// Enable | stage0 = RTC reset (clears all RTC state) | chip reset enable | reset width
const RTC_WDT_CONFIG0_RESET_RTC: u32 = (1 << 31) | (5 << 28) | (1 << 8) | 2;

/// Reboot an ESP32-S3 sitting in the ROM bootloader / flasher stub into its flashed app.
pub fn reset_to_app(port_path: &str) -> Result<(), FlashError> {
    let mut port = open_with_retry(port_path, Duration::from_secs(3))?;

    write_reg(&mut *port, RTC_CNTL_OPTION1_REG, 0, true)
        .map_err(|e| FlashError::ResetFailed(format!("clearing FORCE_DOWNLOAD_BOOT: {e}")))?;
    let rest = (|| {
        write_reg(
            &mut *port,
            RTC_CNTL_WDTWPROTECT_REG,
            RTC_CNTL_WDT_WKEY,
            true,
        )?;
        write_reg(&mut *port, RTC_CNTL_WDTCONFIG1_REG, 2000, true)?;
        write_reg(
            &mut *port,
            RTC_CNTL_WDTCONFIG0_REG,
            RTC_WDT_CONFIG0_RESET_RTC,
            true,
        )?;
        // The chip may reset before it answers the final write
        write_reg(&mut *port, RTC_CNTL_WDTWPROTECT_REG, 0, false)
    })();
    rest.map_err(|e| FlashError::ResetFailed(e.to_string()))
}

/// Windows serial handles are exclusive (§W1-3) and the OS can take a moment to
/// release one after espflash exits, so a flash that succeeded must not fail at
/// the last step on a port that is about to become free. Linux is forgiving
/// here; retrying costs nothing on either.
fn open_with_retry(
    port_path: &str,
    timeout: Duration,
) -> Result<Box<dyn serialport::SerialPort>, FlashError> {
    let deadline = Instant::now() + timeout;
    loop {
        match serialport::new(port_path, 115_200)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(port) => return Ok(port),
            Err(e) if Instant::now() >= deadline => {
                return Err(FlashError::PortBusy(port_path.to_string(), e.to_string()));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn write_reg(
    port: &mut dyn serialport::SerialPort,
    addr: u32,
    value: u32,
    expect_reply: bool,
) -> Result<(), String> {
    let mut data = Vec::with_capacity(16);
    for word in [addr, value, 0xFFFF_FFFF, 0] {
        data.extend_from_slice(&word.to_le_bytes());
    }
    let mut packet = vec![0x00, CMD_WRITE_REG];
    packet.extend_from_slice(&(data.len() as u16).to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes()); // checksum is only used for data commands
    packet.extend_from_slice(&data);

    let _ = port.clear(serialport::ClearBuffer::Input);
    port.write_all(&slip_encode(&packet))
        .map_err(|e| e.to_string())?;
    port.flush().map_err(|e| e.to_string())?;
    if !expect_reply {
        return Ok(());
    }

    let reply = read_slip_frame(port, Duration::from_secs(1))?;
    // Response: direction(0x01) cmd size(u16) value(u32) data... ending in status(0 = ok)
    if reply.len() < 10 || reply[0] != 0x01 || reply[1] != CMD_WRITE_REG {
        return Err(format!(
            "unexpected bootloader reply to WRITE_REG {:#010x}: {:02x?}",
            addr, reply
        ));
    }
    if reply[8] != 0 {
        return Err(format!(
            "bootloader rejected WRITE_REG {:#010x} (status {:#04x})",
            addr, reply[8]
        ));
    }
    Ok(())
}

fn slip_encode(packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packet.len() + 2);
    out.push(0xC0);
    for &b in packet {
        match b {
            0xC0 => out.extend_from_slice(&[0xDB, 0xDC]),
            0xDB => out.extend_from_slice(&[0xDB, 0xDD]),
            _ => out.push(b),
        }
    }
    out.push(0xC0);
    out
}

fn read_slip_frame(
    port: &mut dyn serialport::SerialPort,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + timeout;
    let mut frame = Vec::new();
    let mut in_frame = false;
    let mut escaped = false;
    let mut byte = [0u8; 1];
    while Instant::now() < deadline {
        match port.read(&mut byte) {
            Ok(1) => {}
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(e.to_string()),
        }
        match (byte[0], in_frame, escaped) {
            (0xC0, false, _) => in_frame = true,
            (0xC0, true, _) if frame.is_empty() => {} // back-to-back delimiters
            (0xC0, true, _) => return Ok(frame),
            (_, false, _) => {}
            (0xDB, true, false) => escaped = true,
            (0xDC, true, true) => {
                frame.push(0xC0);
                escaped = false;
            }
            (0xDD, true, true) => {
                frame.push(0xDB);
                escaped = false;
            }
            (b, true, _) => {
                frame.push(b);
                escaped = false;
            }
        }
    }
    Err("timed out waiting for a bootloader reply".to_string())
}

/// Is this an ESP32-S3 app image? (§32)
///
/// Magic byte 0xE9, and the chip ID at offset 12..13 is 0x0009. This is the
/// check that stops an ESP32-C3 build, or a `.deb` that arrived under the
/// wrong name, from being written to the pad's app partition.
pub fn check_esp32s3_image(header: &[u8]) -> Result<(), String> {
    const ESP32S3_CHIP_ID: u16 = 0x0009;
    if header.len() < 16 {
        return Err("too small to be a valid ESP32 image (less than 16 bytes)".to_string());
    }
    if header[0] != 0xE9 {
        return Err(format!(
            "invalid image magic byte: 0x{:02X} (expected 0xE9 for an ESP image)",
            header[0]
        ));
    }
    let chip_id = u16::from_le_bytes([header[12], header[13]]);
    if chip_id != ESP32S3_CHIP_ID {
        return Err(format!(
            "built for chip ID 0x{:04X}, but OPad requires ESP32-S3 (chip ID 0x{:04X})",
            chip_id, ESP32S3_CHIP_ID
        ));
    }
    Ok(())
}

/// The same check against a file on disk, reading only the header.
pub fn check_esp32s3_image_file(path: &Path) -> Result<(), String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
    let mut header = [0u8; 16];
    let n = file
        .read(&mut header)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    check_esp32s3_image(&header[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s3_header() -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = 0xE9;
        h[12] = 0x09;
        h
    }

    /// DD#6: a pad that left the app after a trigger is not blamed on a busy port
    #[test]
    fn a_vanished_app_port_after_a_trigger_is_a_bootloader_miss() {
        let busy = || FlashError::PortBusy("COM5".into(), "not found".into());
        assert!(matches!(
            unusable_app_port_error(busy(), true, false),
            FlashError::NoBootloader
        ));
        // Before any trigger, or with the port still there, it really is busy
        assert!(matches!(
            unusable_app_port_error(busy(), false, false),
            FlashError::PortBusy(..)
        ));
        assert!(matches!(
            unusable_app_port_error(busy(), true, true),
            FlashError::PortBusy(..)
        ));
    }

    #[test]
    fn a_real_esp32s3_image_header_is_accepted() {
        assert!(check_esp32s3_image(&s3_header()).is_ok());
    }

    #[test]
    fn another_chips_image_is_refused() {
        let mut h = s3_header();
        h[12] = 0x05; // ESP32-C3
        let err = check_esp32s3_image(&h).unwrap_err();
        assert!(err.contains("0x0005"), "{err}");
    }

    #[test]
    fn something_that_is_not_an_esp_image_at_all_is_refused() {
        let mut h = s3_header();
        h[0] = 0x7F; // ELF
        assert!(check_esp32s3_image(&h).is_err());
        assert!(check_esp32s3_image(&[0xE9, 0x01]).is_err());
    }

    #[test]
    fn the_app_offset_is_ota_0_not_the_old_single_app_one() {
        assert_eq!(APP_PARTITION_OFFSET, 0x20000);
    }

    #[test]
    fn slip_escapes_the_frame_delimiter_and_the_escape_byte() {
        assert_eq!(slip_encode(&[0xC0]), vec![0xC0, 0xDB, 0xDC, 0xC0]);
        assert_eq!(slip_encode(&[0xDB]), vec![0xC0, 0xDB, 0xDD, 0xC0]);
        assert_eq!(slip_encode(&[0x01, 0x02]), vec![0xC0, 0x01, 0x02, 0xC0]);
    }

    #[test]
    fn the_real_build_is_accepted_if_it_is_present() {
        let path = Path::new("../../../firmware/build/opad-firmware.bin");
        if path.exists() {
            assert_eq!(check_esp32s3_image_file(path), Ok(()));
        }
    }
}
