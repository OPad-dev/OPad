# osu!pad — Windows Portability & Architecture Guide

As specified in §5, §7, and Phase 10 of `osupad_technical_spec_v1.md`, the entire core application model, SQLite database schema, protocol schema (`osupad.proto`), JSON backup format, and ESP32-S3 firmware are platform-agnostic and require **zero redesign** to target Windows.

---

## 1. Hardware & OS Drivers (Driverless by Design)

On Windows 10 and Windows 11, osu!pad functions out of the box without any third-party driver installers (`.inf` / Zadig / WinUSB):

1. **HID Keyboard Interface:**
   - Handled natively by Microsoft's standard USB HID Class Driver (`kbdhid.sys` / `hidclass.sys`).
   - 1000 Hz USB Full Speed polling interval (`bInterval = 1`) is natively honored.
2. **CDC-ACM Serial Telemetry Interface:**
   - Automatically loaded by Microsoft's built-in USB Serial Driver (`usbser.sys`).
   - Automatically assigned a Virtual COM port name (e.g., `COM3`, `COM4`).
   - Accessible via standard Win32 `CreateFile` / `ReadFile` / `WriteFile` APIs.

---

## 2. Serial Port Discovery Abstraction

In `crates/osupad-device`, device discovery uses the `serialport` crate:

```rust
pub fn find_target_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for p in ports {
        if let SerialPortType::UsbPort(info) = p.port_type {
            if info.vid == ESPRESSIF_VID {
                // On Windows: returns "COM3", "COM4", etc.
                // On Linux: returns "/dev/ttyACM0"
                return Some(p.port_name);
            }
        }
    }
    None
}
```

Because `serialport` exposes identical interfaces across POSIX and Win32, serial communication logic is 100% shared between Linux and Windows.

---

## 3. IPC Architecture: Unix Sockets vs. Windows Named Pipes

The IPC framing format (`[len: 4 bytes LE][json payload: len bytes]`) and message enums (`IpcRequest` / `IpcResponse`) are identical. The transport layer adapts per OS:

- **Linux / macOS:** Unix Domain Socket at `$XDG_RUNTIME_DIR/osupad/daemon.sock` or `/tmp/osupad.sock`.
- **Windows:** Named Pipe at `\\.\pipe\osupad-ipc` using `tokio::net::windows::named_pipe`.

### Windows Named Pipe Implementation Pattern

```rust
#[cfg(windows)]
pub fn get_pipe_name() -> &'static str {
    r"\\.\pipe\osupad-ipc"
}

#[cfg(windows)]
pub async fn create_pipe_server() -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    tokio::net::windows::named_pipe::ServerOptions::new()
        .first_pipe_instance(true)
        .max_instances(16)
        .create(get_pipe_name())
}
```

---

## 4. Storage & Application Directories

In `crates/osupad-storage` and `daemon`:

- **Database Path:**
  - Linux: `~/.local/share/osupad/osupad.db`
  - Windows: `%APPDATA%\osupad\osupad.db` (via `dirs::data_dir()` or `%LOCALAPPDATA%`)
- **SQLite Engine:**
  - `rusqlite` with WAL mode works identically on NTFS and ext4/btrfs.

---

## 5. Startup at Login

On Windows, the background daemon can be launched automatically at login via:

1. **Registry Run Key:**
   - Key: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
   - Name: `osupad-daemon`
   - Value: `"C:\Program Files\osupad\osupad-daemon.exe"`
2. **Windows Task Scheduler:**
   - Trigger: At log on of any user
   - Action: Start a program `osupad-daemon.exe`
   - Settings: Run with highest privileges (optional, standard user permissions are sufficient for COM and Named Pipe access).

---

## 6. iced GUI & System Tray

- **GUI Framework:** `iced` 0.13 natively compiles to Windows using WGPU DirectX 12 / Vulkan backends.
- **System Tray:** The `tray-icon` crate natively supports the Windows Shell Notification Area (taskbar tray) via Win32 Shell_NotifyIcon APIs.
