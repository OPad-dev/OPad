//! Only one GUI runs at a time. Launching it again (app menu, launcher) asks the running
//! instance to show its window, like Discord, instead of starting a second copy.

use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::OnceLock;

static LISTENER: OnceLock<UnixListener> = OnceLock::new();

fn socket_path() -> PathBuf {
    osupad_ipc::get_socket_path().with_file_name("gui.sock")
}

/// Returns false if another instance is running (it has been asked to show its window).
pub fn claim() -> bool {
    let path = socket_path();
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show\n");
        return false;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::remove_file(&path); // stale socket from a crashed instance
    match UnixListener::bind(&path) {
        Ok(listener) => {
            let _ = LISTENER.set(listener);
        }
        Err(e) => tracing::warn!("Single-instance socket unavailable: {e}"),
    }
    true
}

/// Emits once per "show" request from another launch
pub fn show_requests() -> impl futures_util::Stream<Item = ()> {
    iced::stream::channel(4, |mut output: iced::futures::channel::mpsc::Sender<()>| async move {
        use futures_util::SinkExt;
        let Some(listener) = LISTENER.get().and_then(|l| l.try_clone().ok()) else {
            return std::future::pending().await;
        };
        let _ = listener.set_nonblocking(true);
        let Ok(listener) = tokio::net::UnixListener::from_std(listener) else {
            return std::future::pending().await;
        };
        while let Ok((_stream, _)) = listener.accept().await {
            let _ = output.send(()).await;
        }
    })
}
