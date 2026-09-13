//! Only one GUI runs at a time. Launching it again (app menu, launcher) asks the running
//! instance to show its window, like Discord, instead of starting a second copy.

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
    iced::stream::channel(4, |mut output: iced::futures::channel::mpsc::Sender<Option<String>>| async move {
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
    })
}
