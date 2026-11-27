//! The daemon's answers as they change, over one connection that stays open
//! (`IpcRequest::Subscribe`), in place of asking again on a timer.
//!
//! The stream makes one attempt and then stays quiet: the app keeps its
//! timers running until `Event::Live`, takes them up again on `Event::Lost`,
//! and starts a new stream once the daemon answers a request again. A daemon
//! that predates `Subscribe` is reported once as `Event::Unsupported`, and
//! the app goes on asking as it always has.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use iced::futures::channel::mpsc::Sender;
use iced::futures::SinkExt;
use opad_ipc::{IpcResponse, Watch};

#[derive(Debug, Clone)]
pub enum Event {
    /// Subscribed: every watch's answer follows, and the timers can stop
    Live,
    Answer(Box<IpcResponse>),
    /// The connection could not be made or has ended
    Lost(String),
    /// The daemon closed the connection on `Subscribe`: it predates it
    Unsupported,
}

/// What one stream watches. A stream is told apart by this and by `attempt`:
/// when either changes, the old connection is dropped and a new one made,
/// which also sends every answer afresh.
pub struct Feed {
    pub attempt: u32,
    pub watches: Vec<Watch>,
}

impl Hash for Feed {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.attempt.hash(state);
        // The watches as the daemon is sent them
        serde_json::to_string(&self.watches)
            .unwrap_or_default()
            .hash(state);
    }
}

pub fn stream(feed: &Feed) -> impl futures_util::Stream<Item = Event> {
    let watches = feed.watches.clone();
    iced::stream::channel(64, async move |output: Sender<Event>| {
        run(opad_ipc::get_socket_path(), watches, output).await;
        // Never ends by itself: an ended stream would not be started again
        // while its `Feed` stays the same
        std::future::pending::<()>().await;
    })
}

async fn run(socket: PathBuf, watches: Vec<Watch>, mut output: Sender<Event>) {
    let last = follow(&socket, watches, &mut output).await;
    let _ = output.send(last).await;
}

/// Subscribes and passes the answers on until the connection ends. Returns
/// what ended it.
async fn follow(socket: &Path, watches: Vec<Watch>, output: &mut Sender<Event>) -> Event {
    let mut stream = match opad_ipc::connect_and_handshake_at(socket).await {
        Ok((stream, _)) => stream,
        Err(e) => return Event::Lost(format!("daemon unreachable: {e}")),
    };
    match opad_ipc::subscribe(&mut stream, watches).await {
        Ok(true) => {}
        Ok(false) => return Event::Unsupported,
        // Refused: this daemon does not watch something this app asks for
        Err(opad_ipc::IpcError::Protocol(reason)) => {
            tracing::warn!("The daemon refused the subscription: {reason}");
            return Event::Unsupported;
        }
        Err(e) => return Event::Lost(e.to_string()),
    }
    if output.send(Event::Live).await.is_err() {
        return Event::Lost("the app stopped listening".to_string());
    }
    loop {
        match opad_ipc::read_response(&mut stream).await {
            Ok(resp) => {
                if output.send(Event::Answer(Box::new(resp))).await.is_err() {
                    return Event::Lost("the app stopped listening".to_string());
                }
            }
            Err(e) => return Event::Lost(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::StreamExt;
    use opad_ipc::{IpcRequest, IPC_PROTOCOL_VERSION};

    fn socket(name: &str) -> PathBuf {
        #[cfg(unix)]
        {
            std::env::temp_dir()
                .join(format!("opad-feed-{}-{}", std::process::id(), name))
                .join("daemon.sock")
        }
        #[cfg(windows)]
        {
            PathBuf::from(format!(
                r"\\.\pipe\opad-feed-{}-{}",
                std::process::id(),
                name
            ))
        }
    }

    /// Removes what `create_listener` made for `socket`. A named pipe goes
    /// with its last instance.
    fn cleanup(socket: &Path) {
        #[cfg(unix)]
        if let Some(dir) = socket.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
        #[cfg(windows)]
        let _ = socket;
    }

    fn status_watch() -> Vec<Watch> {
        vec![Watch {
            request: IpcRequest::GetStatus,
            every_ms: 100,
        }]
    }

    /// Accepts one client and answers its handshake
    async fn accept(listener: &opad_ipc::IpcListener) -> opad_ipc::IpcServerStream {
        let mut client = listener.accept().await.unwrap();
        let hello = opad_ipc::read_request(&mut client).await.unwrap();
        assert!(matches!(hello, IpcRequest::Handshake { .. }), "{hello:?}");
        let ack = IpcResponse::HandshakeAck {
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            daemon_protocol: IPC_PROTOCOL_VERSION,
            device_connected: false,
        };
        opad_ipc::send_response(&mut client, &ack).await.unwrap();
        client
    }

    fn events(socket: PathBuf) -> iced::futures::channel::mpsc::Receiver<Event> {
        let (output, events) = iced::futures::channel::mpsc::channel(64);
        tokio::spawn(run(socket, status_watch(), output));
        events
    }

    #[tokio::test]
    async fn answers_are_passed_on_until_the_daemon_goes_away() {
        let socket = socket("live");
        let listener = opad_ipc::create_listener(&socket).unwrap();
        let mut events = events(socket.clone());

        let mut client = accept(&listener).await;
        let asked = opad_ipc::read_request(&mut client).await.unwrap();
        assert!(
            matches!(&asked, IpcRequest::Subscribe { watches } if watches.len() == 1),
            "{asked:?}"
        );
        opad_ipc::send_response(&mut client, &IpcResponse::Subscribed)
            .await
            .unwrap();
        assert!(matches!(events.next().await, Some(Event::Live)));

        for _ in 0..2 {
            opad_ipc::send_response(&mut client, &IpcResponse::UpdateCheckStarted)
                .await
                .unwrap();
            let got = events.next().await;
            assert!(
                matches!(&got, Some(Event::Answer(a)) if matches!(**a, IpcResponse::UpdateCheckStarted)),
                "{got:?}"
            );
        }

        drop(client);
        assert!(matches!(events.next().await, Some(Event::Lost(_))));
        // One attempt per stream: nothing follows
        assert!(events.next().await.is_none());
        cleanup(&socket);
    }

    #[tokio::test]
    async fn a_daemon_that_predates_subscribe_is_reported_as_such() {
        let socket = socket("older");
        let listener = opad_ipc::create_listener(&socket).unwrap();
        let mut events = events(socket.clone());

        let mut client = accept(&listener).await;
        // It reads the request, cannot parse it, and closes the connection
        let mut frame = [0u8; 4];
        tokio::io::AsyncReadExt::read_exact(&mut client, &mut frame)
            .await
            .unwrap();
        drop(client);
        assert!(matches!(events.next().await, Some(Event::Unsupported)));
        cleanup(&socket);
    }

    #[tokio::test]
    async fn no_daemon_is_reported_as_lost() {
        let mut events = events(socket("nobody"));
        let got = events.next().await;
        assert!(
            matches!(&got, Some(Event::Lost(e)) if e.contains("unreachable")),
            "{got:?}"
        );
    }
}
