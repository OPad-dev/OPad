//! Whether osu! should get the pad's volume swipes (`HostStatus.osu_active`).
//! The pad then sends Alt+wheel (a plain wheel during a map) instead of the
//! system volume keys.
//!
//! - Windows: osu! is the foreground window (stable and lazer are both
//!   `osu!.exe`), so a swipe never sends Alt+wheel to another program.
//! - Linux: osu!lazer is running. Wayland has no portable way to ask which
//!   window has focus, so a swipe while another window is in front sends that
//!   window Alt+wheel.

use std::time::Duration;
use tokio::sync::watch;

/// A swipe right after switching to osu! should already reach it, and one
/// check costs well under a millisecond
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Checks on a thread of its own and reports changes. Starts as `false`.
pub fn watch() -> watch::Receiver<bool> {
    let (tx, rx) = watch::channel(false);
    let spawned = std::thread::Builder::new()
        .name("osu-window".into())
        .spawn(move || {
            while !tx.is_closed() {
                let active = osu_active();
                tx.send_if_modified(|current| {
                    let changed = *current != active;
                    *current = active;
                    changed
                });
                std::thread::sleep(POLL_INTERVAL);
            }
        });
    if let Err(e) = spawned {
        tracing::warn!(
            "Could not watch for osu!: {}; volume swipes set the system volume",
            e
        );
    }
    rx
}

/// osu!lazer's process name (`/proc/<pid>/comm`, newline-terminated): its
/// launcher binary is `osu!`, in the AppImage and in distro packages alike
pub fn is_lazer_comm(comm: &[u8]) -> bool {
    comm.strip_suffix(b"\n").unwrap_or(comm) == b"osu!"
}

/// The executable path of the foreground window's process is osu!'s
pub fn is_osu_exe(path: &str) -> bool {
    path.rsplit(['\\', '/'])
        .next()
        .is_some_and(|file| file.eq_ignore_ascii_case("osu!.exe"))
}

#[cfg(target_os = "linux")]
fn osu_active() -> bool {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return false;
    };
    dir.flatten().any(|entry| {
        let is_pid = entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit()));
        is_pid && std::fs::read(entry.path().join("comm")).is_ok_and(|c| is_lazer_comm(&c))
    })
}

#[cfg(windows)]
fn osu_active() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    // SAFETY: plain Win32 calls; the process handle is closed on every path
    // and the buffer length passed is the buffer's
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return false;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);
        ok != 0 && is_osu_exe(&String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn osu_active() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazer_is_found_by_its_process_name() {
        assert!(is_lazer_comm(b"osu!\n"));
        assert!(is_lazer_comm(b"osu!"));
        assert!(!is_lazer_comm(b"tosu\n"));
        assert!(!is_lazer_comm(b"osu!.exe\n"));
        assert!(!is_lazer_comm(b"osu\n"));
    }

    #[test]
    fn osu_exe_is_matched_by_file_name() {
        assert!(is_osu_exe(
            r"C:\Users\me\AppData\Local\osulazer\current\osu!.exe"
        ));
        assert!(is_osu_exe(r"D:\Games\osu!\OSU!.EXE"));
        assert!(!is_osu_exe(r"C:\tosu\tosu.exe"));
        assert!(!is_osu_exe(r"C:\Games\osu!\osu!.exe.bak"));
        assert!(!is_osu_exe(""));
    }
}
