//! One-shot requests to osupad-daemon.

use osupad_ipc::{get_socket_path, send_request, IpcRequest, IpcResponse};
use tokio::net::UnixStream;

pub async fn request(request: IpcRequest) -> Result<IpcResponse, String> {
    let mut stream = UnixStream::connect(get_socket_path()).await.map_err(|e| format!("daemon unreachable: {}", e))?;
    send_request(&mut stream, &request).await.map_err(|e| e.to_string())
}
