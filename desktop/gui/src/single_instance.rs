//! Only one GUI runs at a time. Launching it again (app menu, launcher) asks the running
//! instance to show its window, like Discord, instead of starting a second copy.

#[cfg(unix)]
mod platform {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::sync::OnceLock;

    static LISTENER: OnceLock<UnixListener> = OnceLock::new();

    fn socket_path() -> PathBuf {
        osupad_ipc::get_socket_path().with_file_name("gui.sock")
    }

    /// Returns false if another instance is running (it has been asked to show its window).
    pub fn claim(page: Option<&str>) -> bool {
        let path = socket_path();
        if let Ok(mut stream) = UnixStream::connect(&path) {
            let msg = if let Some(p) = page {
                format!("show {}\n", p)
            } else {
                "show\n".to_string()
            };
            let _ = stream.write_all(msg.as_bytes());
            return false;
        }
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        let _ = std::fs::remove_file(&path); // stale socket from a crashed instance
        match UnixListener::bind(&path) {
            Ok(listener) => {
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                let _ = LISTENER.set(listener);
            }
            Err(e) => tracing::warn!("Single-instance socket unavailable: {e}"),
        }
        true
    }

    /// Emits once per "show" request from another launch with optional target page name
    pub fn show_requests() -> impl futures_util::Stream<Item = Option<String>> {
        iced::stream::channel(
            4,
            |mut output: iced::futures::channel::mpsc::Sender<Option<String>>| async move {
                use futures_util::SinkExt;
                use tokio::io::AsyncBufReadExt;
                let Some(listener) = LISTENER.get().and_then(|l| l.try_clone().ok()) else {
                    return std::future::pending().await;
                };
                let _ = listener.set_nonblocking(true);
                let Ok(listener) = tokio::net::UnixListener::from_std(listener) else {
                    return std::future::pending().await;
                };
                while let Ok((stream, _)) = listener.accept().await {
                    let mut reader = tokio::io::BufReader::new(stream);
                    let mut line = String::new();
                    let _ = reader.read_line(&mut line).await;
                    let target = line
                        .trim()
                        .strip_prefix("show")
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let _ = output.send(target).await;
                }
            },
        )
    }
}

#[cfg(windows)]
mod platform {
    use std::io::Write;
    use std::sync::OnceLock;

    struct MutexGuard(windows_sys::Win32::Foundation::HANDLE);
    unsafe impl Send for MutexGuard {}
    unsafe impl Sync for MutexGuard {}

    impl Drop for MutexGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    windows_sys::Win32::Foundation::CloseHandle(self.0);
                }
            }
        }
    }

    static MUTEX: OnceLock<MutexGuard> = OnceLock::new();
    static PIPE_NAME: OnceLock<String> = OnceLock::new();

    fn windows_pipe_name() -> String {
        let base = osupad_ipc::get_socket_path().to_string_lossy().to_string();
        if base.contains("osupad-ipc-") {
            base.replace("osupad-ipc-", "osupad-gui-")
        } else {
            r"\\.\pipe\osupad-gui".to_string()
        }
    }

    /// Returns false if another instance is running (it has been asked to show its window).
    pub fn claim(page: Option<&str>) -> bool {
        let pipe_name = windows_pipe_name();
        let name_utf16: Vec<u16> = "Local\\osupad-gui\0".encode_utf16().collect();
        let handle = unsafe {
            windows_sys::Win32::System::Threading::CreateMutexW(
                std::ptr::null(),
                1, // bInitialOwner = TRUE
                name_utf16.as_ptr(),
            )
        };

        if handle.is_null() {
            tracing::warn!("Failed to create single-instance mutex Local\\osupad-gui");
            return true;
        }

        let already_exists = unsafe {
            windows_sys::Win32::Foundation::GetLastError()
                == windows_sys::Win32::Foundation::ERROR_ALREADY_EXISTS
        };

        if already_exists {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(handle);
            }
            // Another instance is running; connect to its pipe to hand off the show request
            if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&pipe_name) {
                let msg = if let Some(p) = page {
                    format!("show {}\n", p)
                } else {
                    "show\n".to_string()
                };
                let _ = file.write_all(msg.as_bytes());
            }
            return false;
        }

        let _ = MUTEX.set(MutexGuard(handle));
        let _ = PIPE_NAME.set(pipe_name);
        true
    }

    /// Emits once per "show" request from another launch with optional target page name
    pub fn show_requests() -> impl futures_util::Stream<Item = Option<String>> {
        iced::stream::channel(
            4,
            |mut output: iced::futures::channel::mpsc::Sender<Option<String>>| async move {
                use futures_util::SinkExt;
                use tokio::io::AsyncBufReadExt;
                let Some(pipe_name) = PIPE_NAME.get() else {
                    return std::future::pending().await;
                };

                let mut server = match tokio::net::windows::named_pipe::ServerOptions::new()
                    .first_pipe_instance(true)
                    .create(pipe_name)
                {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("Single-instance named pipe unavailable: {e}");
                        return std::future::pending().await;
                    }
                };

                loop {
                    if server.connect().await.is_ok() {
                        let mut reader = tokio::io::BufReader::new(server);
                        let mut line = String::new();
                        let _ = reader.read_line(&mut line).await;
                        let target = line
                            .trim()
                            .strip_prefix("show")
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty());
                        let _ = output.send(target).await;
                    }

                    server = match tokio::net::windows::named_pipe::ServerOptions::new()
                        .create(pipe_name)
                    {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("Failed to recreate single-instance pipe: {e}");
                            break;
                        }
                    };
                }
            },
        )
    }
}

pub use platform::{claim, show_requests};
