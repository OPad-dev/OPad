//! Only one GUI runs at a time. Launching it again (app menu, launcher) asks the running
//! instance to show its window, like Discord, instead of starting a second copy.

#[cfg(unix)]
mod platform {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    static LISTENER: OnceLock<UnixListener> = OnceLock::new();

    fn socket_path() -> PathBuf {
        opad_ipc::get_socket_path().with_file_name("gui.sock")
    }

    pub(super) enum Claim {
        /// This launch is the instance; show requests arrive on the listener
        Primary(Option<UnixListener>),
        /// Another instance answered and was asked to show its window
        Secondary,
    }

    /// Returns false if another instance is running (it has been asked to show its window).
    pub fn claim(page: Option<&str>) -> bool {
        match claim_at(&socket_path(), page) {
            Claim::Primary(listener) => {
                if let Some(listener) = listener {
                    let _ = LISTENER.set(listener);
                }
                true
            }
            Claim::Secondary => false,
        }
    }

    /// Binds first, so of two launches racing for the socket exactly one wins
    /// the bind and the other connects to it. The file is only removed when
    /// nothing answers on it (ECONNREFUSED: a crashed instance's leftover);
    /// removing it unconditionally let a second launch unlink the first's
    /// fresh socket and run as a second instance.
    pub(super) fn claim_at(path: &Path, page: Option<&str>) -> Claim {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        for attempt in 0..2 {
            match bind(path) {
                Ok(listener) => return Claim::Primary(Some(listener)),
                Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {}
                Err(e) => {
                    tracing::warn!("Single-instance socket unavailable: {e}");
                    return Claim::Primary(None);
                }
            }
            match UnixStream::connect(path) {
                Ok(mut stream) => {
                    let msg = match page {
                        Some(p) => format!("show {}\n", p),
                        None => "show\n".to_string(),
                    };
                    let _ = stream.write_all(msg.as_bytes());
                    return Claim::Secondary;
                }
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused && attempt == 0 => {
                    // Stale socket from a crashed instance
                    let _ = std::fs::remove_file(path);
                }
                Err(e) => {
                    tracing::warn!("Single-instance socket unavailable: {e}");
                    return Claim::Primary(None);
                }
            }
        }
        Claim::Primary(None)
    }

    fn bind(path: &Path) -> std::io::Result<UnixListener> {
        let listener = UnixListener::bind(path)?;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        Ok(listener)
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
                    let _ = output.send(super::show_target(&line)).await;
                }
            },
        )
    }
}

#[cfg(windows)]
mod platform {
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

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
    /// Show requests from the pipe thread, taken once by `show_requests`
    static REQUESTS: Mutex<Option<UnboundedReceiver<Option<String>>>> = Mutex::new(None);

    fn windows_pipe_name() -> String {
        let base = opad_ipc::get_socket_path().to_string_lossy().to_string();
        if base.contains("opad-ipc-") {
            base.replace("opad-ipc-", "opad-gui-")
        } else {
            r"\\.\pipe\opad-gui".to_string()
        }
    }

    /// How long a second launch keeps trying to reach the running instance's
    /// pipe: it can be between instances (ERROR_PIPE_BUSY) for a moment
    const HANDOFF_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

    /// Returns false if another instance is running (it has been asked to show its window).
    pub fn claim(page: Option<&str>) -> bool {
        let pipe_name = windows_pipe_name();
        let name_utf16: Vec<u16> = "Local\\opad-gui\0".encode_utf16().collect();
        let handle = unsafe {
            windows_sys::Win32::System::Threading::CreateMutexW(
                std::ptr::null(),
                1, // bInitialOwner = TRUE
                name_utf16.as_ptr(),
            )
        };

        if handle.is_null() {
            tracing::warn!("Failed to create single-instance mutex Local\\opad-gui");
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
            let msg = match page {
                Some(p) => format!("show {}\n", p),
                None => "show\n".to_string(),
            };
            if let Err(e) = hand_off(&pipe_name, msg.as_bytes()) {
                let text = format!(
                    "OPad is already running, but its window could not be shown ({}): {e}",
                    pipe_name
                );
                tracing::warn!("{text}");
                eprintln!("{text}");
            }
            return false;
        }

        let _ = MUTEX.set(MutexGuard(handle));
        // The first pipe instance exists before this returns, so a second
        // launch from here on always finds a listener
        match start_pipe_server(pipe_name) {
            Ok(requests) => *REQUESTS.lock().unwrap_or_else(|e| e.into_inner()) = Some(requests),
            Err(e) => {
                let text = format!("Single-instance named pipe unavailable: {e}");
                tracing::warn!("{text}");
                eprintln!("{text}");
            }
        }
        true
    }

    fn hand_off(pipe_name: &str, msg: &[u8]) -> std::io::Result<()> {
        let deadline = std::time::Instant::now() + HANDOFF_TIMEOUT;
        loop {
            match std::fs::OpenOptions::new().write(true).open(pipe_name) {
                Ok(mut file) => return file.write_all(msg),
                Err(e) if std::time::Instant::now() >= deadline => return Err(e),
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
    }

    /// Only this user (and SYSTEM) may open it, like the daemon's pipe: any
    /// process on the machine could otherwise send the running GUI requests
    fn create_pipe_instance(
        pipe_name: &str,
        first: bool,
    ) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
        opad_ipc::create_private_pipe(std::ffi::OsStr::new(pipe_name), first).map_err(|e| match e {
            opad_ipc::IpcError::Io(io) => io,
            other => std::io::Error::other(other.to_string()),
        })
    }

    /// Runs the pipe on a thread with its own runtime: iced's executor does
    /// not exist yet when `claim` runs, and a tokio pipe must be created
    /// inside the runtime that drives it. Returns once the first instance is
    /// listening, or with the error that prevented it.
    fn start_pipe_server(pipe_name: String) -> std::io::Result<UnboundedReceiver<Option<String>>> {
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (requests_tx, requests_rx) = tokio::sync::mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("opad-gui-single-instance".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                runtime.block_on(serve(pipe_name, ready_tx, requests_tx));
            })?;
        ready_rx
            .recv()
            .map_err(|_| std::io::Error::other("the single-instance pipe thread exited"))??;
        Ok(requests_rx)
    }

    async fn serve(
        pipe_name: String,
        ready: std::sync::mpsc::Sender<std::io::Result<()>>,
        requests: UnboundedSender<Option<String>>,
    ) {
        use tokio::io::AsyncBufReadExt;
        let mut server = match create_pipe_instance(&pipe_name, true) {
            Ok(s) => {
                let _ = ready.send(Ok(()));
                s
            }
            Err(e) => {
                let _ = ready.send(Err(e));
                return;
            }
        };
        loop {
            let connected = server.connect().await;
            // The next instance before reading, so the name is never left
            // without a listener while this client is served
            let next = create_pipe_instance(&pipe_name, false);
            if connected.is_ok() {
                let mut reader = tokio::io::BufReader::new(server);
                let mut line = String::new();
                let _ = reader.read_line(&mut line).await;
                if requests.send(super::show_target(&line)).is_err() {
                    return;
                }
            }
            server = match next {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to recreate single-instance pipe: {e}");
                    return;
                }
            };
        }
    }

    /// Emits once per "show" request from another launch with optional target page name
    pub fn show_requests() -> impl futures_util::Stream<Item = Option<String>> {
        iced::stream::channel(
            4,
            |mut output: iced::futures::channel::mpsc::Sender<Option<String>>| async move {
                use futures_util::SinkExt;
                let taken = REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).take();
                let Some(mut requests) = taken else {
                    return std::future::pending().await;
                };
                while let Some(target) = requests.recv().await {
                    let _ = output.send(target).await;
                }
            },
        )
    }
}

/// `"show logs\n"` → `Some("logs")`; a bare `"show"` → `None`
fn show_target(line: &str) -> Option<String> {
    line.trim()
        .strip_prefix("show")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub use platform::{claim, show_requests};

#[cfg(all(test, unix))]
mod tests {
    use super::platform::{claim_at, Claim};
    use std::io::Read;

    #[test]
    fn a_second_launch_hands_off_to_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gui.sock");
        let Claim::Primary(Some(listener)) = claim_at(&path, None) else {
            panic!("the first launch must own the socket");
        };
        assert!(matches!(claim_at(&path, Some("logs")), Claim::Secondary));
        let (mut stream, _) = listener.accept().unwrap();
        let mut msg = String::new();
        stream.read_to_string(&mut msg).unwrap();
        assert_eq!(super::show_target(&msg).as_deref(), Some("logs"));
    }

    #[test]
    fn a_stale_socket_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gui.sock");
        // A crashed instance: its socket file stays, nothing listens
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        assert!(matches!(claim_at(&path, None), Claim::Primary(Some(_))));
    }

    #[test]
    fn of_two_simultaneous_launches_exactly_one_is_the_instance() {
        for _ in 0..50 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("gui.sock");
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let launches: Vec<_> = (0..2)
                .map(|_| {
                    let (path, barrier) = (path.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        barrier.wait();
                        match claim_at(&path, None) {
                            // Kept alive so the other launch can reach it
                            Claim::Primary(listener) => (true, listener),
                            Claim::Secondary => (false, None),
                        }
                    })
                })
                .collect();
            let results: Vec<_> = launches.into_iter().map(|h| h.join().unwrap()).collect();
            let primaries = results.iter().filter(|(primary, _)| *primary).count();
            assert_eq!(primaries, 1);
        }
    }

    #[test]
    fn show_targets_are_parsed() {
        assert_eq!(super::show_target("show\n"), None);
        assert_eq!(
            super::show_target("show diagnostics\n").as_deref(),
            Some("diagnostics")
        );
        assert_eq!(super::show_target("hello"), None);
    }
}
