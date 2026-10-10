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
/// `ota_1`, the second slot in `firmware/partitions.csv`
pub const OTA_1_OFFSET: u32 = 0x220000;
/// `otadata`: two 4 KB sectors, one boot-selection entry each
pub const OTADATA_OFFSET: u32 = 0xf000;
const OTADATA_SIZE: usize = 0x2000;
const OTADATA_SECTOR: usize = 0x1000;
// esp_ota_img_states_t (esp_flash_partitions.h)
const OTA_IMG_NEW: u32 = 0x0;
const OTA_IMG_VALID: u32 = 0x2;

/// Where an app update goes, and what the pad should be running afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUpdatePlan {
    /// In write order: the app image first, then the boot selection, so an
    /// interrupted flash still boots the old image
    pub images: Vec<(u32, PathBuf)>,
    /// The slot the new image boots from (`ota_0`/`ota_1`). `None` for the
    /// old in-place write, where a rollback has nothing to go back to.
    pub target_slot: Option<&'static str>,
}

/// Plan an app update next to the running image rather than over it.
///
/// The image goes to the slot the pad is *not* running, and `otadata` is
/// rewritten so the bootloader tries it once (`NEW`) with the running slot
/// kept `VALID` behind it. The firmware only confirms the new image once the
/// host has configured USB (`confirm_image_once_usb_works` in
/// `firmware/main/app_main.c`); one that never becomes a keyboard, or crashes,
/// is marked aborted by the bootloader on the next boot and the old image
/// runs again. No button, no recovery flash.
///
/// With the running slot unknown (no daemon, or firmware too old to report
/// it) this falls back to writing `ota_0` in place, as before.
pub fn plan_app_update(
    app_image: &Path,
    running_slot: Option<&str>,
    scratch_dir: &Path,
) -> Result<AppUpdatePlan, FlashError> {
    let (old, new) = match running_slot {
        Some("ota_0") => (0u8, 1u8),
        Some("ota_1") => (1, 0),
        _ => {
            return Ok(AppUpdatePlan {
                images: vec![(APP_PARTITION_OFFSET, app_image.to_path_buf())],
                target_slot: None,
            })
        }
    };
    let otadata = scratch_dir.join("opad-otadata.bin");
    std::fs::write(&otadata, otadata_image(old, new))?;
    let offset = if new == 0 {
        APP_PARTITION_OFFSET
    } else {
        OTA_1_OFFSET
    };
    Ok(AppUpdatePlan {
        images: vec![(offset, app_image.to_path_buf()), (OTADATA_OFFSET, otadata)],
        target_slot: Some(if new == 0 { "ota_0" } else { "ota_1" }),
    })
}

/// The `otadata` partition selecting `new_slot` for one trial boot, with
/// `old_slot` valid to fall back to. Each sector holds an
/// `esp_ota_select_entry_t`: `ota_seq` (u32), `seq_label` (20 bytes),
/// `ota_state` (u32), `crc` (u32). The bootloader boots the valid entry with
/// the highest sequence, slot `(seq - 1) % 2`.
pub fn otadata_image(old_slot: u8, new_slot: u8) -> Vec<u8> {
    let mut out = vec![0xFFu8; OTADATA_SIZE];
    // Sequence numbers that map to each slot, the new one higher
    let old_seq = u32::from(old_slot) + 1;
    let new_seq = if new_slot as u32 + 1 > old_seq {
        new_slot as u32 + 1
    } else {
        new_slot as u32 + 3
    };
    for (sector, seq, state) in [(0, old_seq, OTA_IMG_VALID), (1, new_seq, OTA_IMG_NEW)] {
        let entry = &mut out[sector * OTADATA_SECTOR..sector * OTADATA_SECTOR + 32];
        entry[0..4].copy_from_slice(&seq.to_le_bytes());
        // seq_label stays 0xFF, as esp_ota_set_boot_partition leaves it
        entry[24..28].copy_from_slice(&state.to_le_bytes());
        entry[28..32].copy_from_slice(&otadata_crc(seq).to_le_bytes());
    }
    out
}

/// `esp_rom_crc32_le(UINT32_MAX, &ota_seq, 4)`, which is zlib's
/// `crc32(seq, 0xFFFFFFFF)` (IDF's `otatool.py` computes it that way)
fn otadata_crc(seq: u32) -> u32 {
    let mut crc = !0xFFFF_FFFFu32;
    for byte in seq.to_le_bytes() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[derive(Debug, Error)]
pub enum FlashError {
    #[error("OPad not found: neither the app (303a:4001) nor the ROM bootloader (303a:1001) is connected")]
    NoDevice,
    #[error("{0} could not be opened. On Windows the port is exclusive: close anything else using it (a serial monitor, another opad-daemon) first. ({1})")]
    PortBusy(String, String),
    #[error("The pad did not re-enumerate as the ROM bootloader (303a:1001). Run `opadctl recover` and replug the pad's USB cable; see docs/recovery.md.")]
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

// ---------------------------------------------------------------------------
// Recovery without buttons (`opadctl recover`).
//
// An app that boots but never answers on USB (a broken build, a stalled
// SET_CONFIGURATION) cannot be asked to enter the bootloader. But at every
// boot, before the app takes the USB PHY over, the ROM's USB-Serial-JTAG is on
// the bus as 303a:1001 for a fraction of a second, and its DTR/RTS lines can
// reset the chip into download mode, where it stays. So: wait for a boot (the
// person replugs the cable), catch that window, and flash the full set.
// ---------------------------------------------------------------------------

const RTC_CNTL_SWD_CONF_REG: u32 = 0x6000_80B4;
const RTC_CNTL_SWD_WPROTECT_REG: u32 = 0x6000_80B8;
const RTC_CNTL_SWD_WKEY: u32 = 0x8F1D_312A;
const RTC_CNTL_SWD_AUTO_FEED_EN: u32 = 1 << 31;

/// esptool's USB-Serial-JTAG reset into download mode, with 5 ms steps instead
/// of 100 ms: the hardware applies each line change at once, and the app takes
/// the port away ~200 ms after it appears.
fn usj_reset_to_download(path: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(400);
    let mut port = loop {
        match serialport::new(path, 115_200)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(p) => break p,
            Err(e) if Instant::now() >= deadline => return Err(e.to_string()),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    };
    let step = Duration::from_millis(5);
    let mut set = |rts: bool, dtr: bool| -> Result<(), String> {
        port.write_request_to_send(rts).map_err(|e| e.to_string())?;
        port.write_data_terminal_ready(dtr)
            .map_err(|e| e.to_string())
    };
    set(false, false)?;
    std::thread::sleep(step);
    // IO0 low
    set(false, true)?;
    std::thread::sleep(step);
    // Reset with IO0 still low: RTS first, so the lines pass through (1,1)
    // and not (0,0), which lets IO0 go before the reset (esptool's order)
    port.write_request_to_send(true)
        .map_err(|e| e.to_string())?;
    port.write_data_terminal_ready(false)
        .map_err(|e| e.to_string())?;
    port.write_request_to_send(true)
        .map_err(|e| e.to_string())?;
    std::thread::sleep(step);
    // The port drops as the chip resets; nothing left to do if it is gone
    let _ = port.write_request_to_send(false);
    Ok(())
}

/// 303a:1001 ports, read straight from sysfs on Linux: udev's database (what
/// serialport's enumeration reads) fills in too late for a window this short
#[cfg(target_os = "linux")]
fn rom_port_names() -> Vec<String> {
    let read = |p: std::path::PathBuf| std::fs::read_to_string(p).unwrap_or_default();
    let Ok(entries) = std::fs::read_dir("/sys/class/tty") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("ttyACM"))
        .filter_map(|e| {
            let iface = std::fs::canonicalize(e.path().join("device")).ok()?;
            let usb = iface.parent()?;
            (read(usb.join("idVendor")).trim() == "303a"
                && read(usb.join("idProduct")).trim() == "1001")
                .then(|| format!("/dev/{}", e.file_name().to_string_lossy()))
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn rom_port_names() -> Vec<String> {
    crate::bootloader_ports()
        .into_iter()
        .map(|p| p.name)
        .collect()
}

/// The first 303a:1001 port to appear (or one already there), polled fast
/// enough to catch the boot window.
fn wait_for_rom_port(timeout: Duration) -> Result<String, FlashError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut ports = rom_port_names();
        match ports.len() {
            0 => std::thread::sleep(Duration::from_millis(5)),
            1 => return Ok(ports.remove(0)),
            _ => return Err(FlashError::AmbiguousBootloader(ports)),
        }
    }
    Err(FlashError::NoBootloader)
}

/// Download mode reached through a reset from the bootloader still has the
/// RTC watchdog the second-stage bootloader armed: it would reset the chip
/// halfway through the write. Stop it, and let the super watchdog feed itself.
fn disable_watchdogs(port_path: &str) -> Result<(), FlashError> {
    let mut port = open_with_retry(port_path, Duration::from_secs(3))?;
    // A ROM fresh out of reset ignores every command until it has seen a SYNC
    rom_sync(&mut *port)
        .map_err(|e| FlashError::ResetFailed(format!("syncing with the ROM: {e}")))?;
    let steps = [
        (RTC_CNTL_WDTWPROTECT_REG, RTC_CNTL_WDT_WKEY),
        (RTC_CNTL_WDTCONFIG0_REG, 0),
        (RTC_CNTL_WDTWPROTECT_REG, 0),
        (RTC_CNTL_SWD_WPROTECT_REG, RTC_CNTL_SWD_WKEY),
        (RTC_CNTL_SWD_CONF_REG, RTC_CNTL_SWD_AUTO_FEED_EN),
        (RTC_CNTL_SWD_WPROTECT_REG, 0),
    ];
    for (addr, value) in steps {
        write_reg(&mut *port, addr, value, true)
            .map_err(|e| FlashError::ResetFailed(format!("stopping the watchdogs: {e}")))?;
    }
    Ok(())
}

const CMD_SYNC: u8 = 0x08;

/// The ROM loader's SYNC handshake. It answers each SYNC with several
/// replies; they are drained so the next command reads its own.
fn rom_sync(port: &mut dyn serialport::SerialPort) -> Result<(), String> {
    let mut data = vec![0x07, 0x07, 0x12, 0x20];
    data.extend_from_slice(&[0x55; 32]);
    let mut packet = vec![0x00, CMD_SYNC];
    packet.extend_from_slice(&(data.len() as u16).to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes());
    packet.extend_from_slice(&data);
    let frame = slip_encode(&packet);
    for _ in 0..10 {
        let _ = port.clear(serialport::ClearBuffer::Input);
        port.write_all(&frame).map_err(|e| e.to_string())?;
        port.flush().map_err(|e| e.to_string())?;
        if let Ok(reply) = read_slip_frame(port, Duration::from_millis(100)) {
            if reply.len() >= 2 && reply[0] == 0x01 && reply[1] == CMD_SYNC {
                while read_slip_frame(port, Duration::from_millis(50)).is_ok() {}
                return Ok(());
            }
        }
    }
    Err("no answer to SYNC".to_string())
}

/// Catch the pad's next boot, hold it in download mode, write `images` (the
/// full recovery set) and boot it. `progress` hears each step; the caller has
/// already told the person to replug the cable.
pub fn recover(
    images: &[(u32, PathBuf)],
    timeout: Duration,
    progress: &(dyn Fn(&str) + Sync),
) -> Result<(), FlashError> {
    let caught = wait_for_rom_port(timeout)?;
    usj_reset_to_download(&caught).map_err(|e| FlashError::PortBusy(caught.clone(), e))?;
    progress(&format!(
        "Caught the pad booting on {caught}, holding it in download mode..."
    ));
    // A USB-Serial-JTAG reset keeps the port on the bus: the same port is now
    // the ROM in download mode, once it has restarted
    std::thread::sleep(Duration::from_millis(300));
    let boot_port = wait_for_rom_port(Duration::from_secs(5))?;
    disable_watchdogs(&boot_port)?;
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

    fn entry(img: &[u8], sector: usize) -> (u32, u32, u32) {
        let e = &img[sector * OTADATA_SECTOR..];
        let word = |at: usize| u32::from_le_bytes(e[at..at + 4].try_into().unwrap());
        (word(0), word(24), word(28))
    }

    /// The bootloader's rule: the valid entry with the highest sequence wins,
    /// and boots slot (seq - 1) % 2
    fn booted_slot(img: &[u8]) -> u32 {
        let (a, b) = (entry(img, 0), entry(img, 1));
        let seq = if a.0 > b.0 { a.0 } else { b.0 };
        (seq - 1) % 2
    }

    #[test]
    fn otadata_crc_matches_idf_otatool() {
        // binascii.crc32(struct.pack('I', seq), 0xFFFFFFFF), as otatool.py does
        assert_eq!(otadata_crc(1), 0x4743_989a);
        assert_eq!(otadata_crc(2), 0x55f6_3774);
        assert_eq!(otadata_crc(3), 0xed4a_5011);
    }

    #[test]
    fn otadata_tries_the_new_slot_and_keeps_the_old_one_valid() {
        for (old, new) in [(0u8, 1u8), (1, 0)] {
            let img = otadata_image(old, new);
            assert_eq!(img.len(), OTADATA_SIZE);
            assert_eq!(booted_slot(&img), u32::from(new));
            let (old_seq, old_state, old_crc) = entry(&img, 0);
            let (new_seq, new_state, new_crc) = entry(&img, 1);
            assert_eq!((old_seq - 1) % 2, u32::from(old));
            assert_eq!(old_state, OTA_IMG_VALID);
            assert_eq!(new_state, OTA_IMG_NEW);
            assert_eq!(old_crc, otadata_crc(old_seq));
            assert_eq!(new_crc, otadata_crc(new_seq));
            // Aborting the new entry leaves the old one to boot
            assert!(new_seq > old_seq);
        }
    }

    #[test]
    fn an_update_goes_to_the_slot_not_running() {
        let dir = std::env::temp_dir().join(format!("opad-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let app = Path::new("app.bin");

        let plan = plan_app_update(app, Some("ota_0"), &dir).unwrap();
        assert_eq!(plan.target_slot, Some("ota_1"));
        assert_eq!(plan.images[0], (OTA_1_OFFSET, app.to_path_buf()));
        assert_eq!(plan.images[1].0, OTADATA_OFFSET);

        let plan = plan_app_update(app, Some("ota_1"), &dir).unwrap();
        assert_eq!(plan.target_slot, Some("ota_0"));
        assert_eq!(plan.images[0], (APP_PARTITION_OFFSET, app.to_path_buf()));

        // Unknown slot: the old in-place write, nothing to roll back to
        let plan = plan_app_update(app, None, &dir).unwrap();
        assert_eq!(plan.target_slot, None);
        assert_eq!(plan.images, vec![(APP_PARTITION_OFFSET, app.to_path_buf())]);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
