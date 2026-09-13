use osupad_ipc::{connect_and_handshake, send_request, IpcRequest, IpcResponse};

pub async fn request(request: IpcRequest) -> Result<IpcResponse, String> {
    let (mut stream, _) = connect_and_handshake()
        .await
        .map_err(|e| format!("daemon unreachable: {}", e))?;
    send_request(&mut stream, &request)
        .await
        .map_err(|e| e.to_string())
}
