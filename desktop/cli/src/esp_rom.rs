//! Minimal ESP serial bootloader (SLIP) client, just enough to leave download mode.
//!
//! The firmware enters the ROM bootloader by setting RTC_CNTL_FORCE_DOWNLOAD_BOOT,
//! which survives resets. espflash 4.x refuses to watchdog-reset an ESP32-S3 while
//! that bit is set and its RTS hard reset does not reach the chip over
//! USB-Serial-JTAG, so the board would stay in the bootloader after flashing.
//! Here we do what esptool does: clear the bit, then trigger an RTC watchdog reset.

use anyhow::{bail, Context, Result};
use std::time::{Duration, Instant};

const CMD_WRITE_REG: u8 = 0x09;

const RTC_CNTL_OPTION1_REG: u32 = 0x6000_812C;
const RTC_CNTL_WDTCONFIG0_REG: u32 = 0x6000_8098;
const RTC_CNTL_WDTCONFIG1_REG: u32 = 0x6000_809C;
const RTC_CNTL_WDTWPROTECT_REG: u32 = 0x6000_80B0;
const RTC_CNTL_WDT_WKEY: u32 = 0x50D8_3AA1;
/// Enable | stage0 = RTC reset (clears all RTC state) | chip reset enable | reset width
const RTC_WDT_CONFIG0_RESET_RTC: u32 = (1 << 31) | (5 << 28) | (1 << 8) | 2;

/// Reboot an ESP32-S3 sitting in the ROM bootloader / flasher stub into its flashed app.
pub fn reset_to_app(port_path: &str) -> Result<()> {
    let mut port = open_with_retry(port_path, Duration::from_secs(3))?;

    write_reg(&mut *port, RTC_CNTL_OPTION1_REG, 0, true)
        .context("Failed to clear FORCE_DOWNLOAD_BOOT")?;
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
    write_reg(&mut *port, RTC_CNTL_WDTWPROTECT_REG, 0, false)?;
    Ok(())
}

/// Windows serial handles are exclusive (§W1-3) and the OS can take a moment to
/// release one after espflash exits, so a flash that succeeded must not fail at
/// the last step on a port that is about to become free. Linux is forgiving
/// here; retrying costs nothing on either.
fn open_with_retry(port_path: &str, timeout: Duration) -> Result<Box<dyn serialport::SerialPort>> {
    let deadline = Instant::now() + timeout;
    loop {
        match serialport::new(port_path, 115_200)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(port) => return Ok(port),
            Err(e) if Instant::now() >= deadline => {
                return Err(e).with_context(|| format!("Failed to open {}", port_path));
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
) -> Result<()> {
    let mut data = Vec::with_capacity(16);
    for word in [addr, value, 0xFFFF_FFFF, 0] {
        data.extend_from_slice(&word.to_le_bytes());
    }
    let mut packet = vec![0x00, CMD_WRITE_REG];
    packet.extend_from_slice(&(data.len() as u16).to_le_bytes());
    packet.extend_from_slice(&0u32.to_le_bytes()); // checksum is only used for data commands
    packet.extend_from_slice(&data);

    let _ = port.clear(serialport::ClearBuffer::Input);
    port.write_all(&slip_encode(&packet))?;
    port.flush()?;
    if !expect_reply {
        return Ok(());
    }

    let reply = read_slip_frame(port, Duration::from_secs(1))?;
    // Response: direction(0x01) cmd size(u16) value(u32) data... ending in status(0 = ok)
    if reply.len() < 10 || reply[0] != 0x01 || reply[1] != CMD_WRITE_REG {
        bail!(
            "Unexpected bootloader reply to WRITE_REG {:#010x}: {:02x?}",
            addr,
            reply
        );
    }
    if reply[8] != 0 {
        bail!(
            "Bootloader rejected WRITE_REG {:#010x} (status {:#04x})",
            addr,
            reply[8]
        );
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

fn read_slip_frame(port: &mut dyn serialport::SerialPort, timeout: Duration) -> Result<Vec<u8>> {
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
            Err(e) => return Err(e.into()),
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
    bail!("Timed out waiting for bootloader reply")
}
