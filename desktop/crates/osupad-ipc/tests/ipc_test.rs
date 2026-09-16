use osupad_ipc::{
    connect, create_listener, read_request, send_request, send_response, IpcRequest, IpcResponse,
    IPC_PROTOCOL_VERSION,
};
use osupad_model::{CounterState, DeviceConfig, DeviceInfo, JsonBackup, RuntimeMode};
use std::path::PathBuf;

/// A per-test transport address: a socket under a private directory on Unix,
/// a named pipe on Windows.
fn test_addr(name: &str) -> PathBuf {
    #[cfg(unix)]
    {
        std::env::temp_dir()
            .join(format!("osupad-test-{}-{}", std::process::id(), name))
            .join("daemon.sock")
    }
    #[cfg(windows)]
    {
        PathBuf::from(format!(
            r"\\.\pipe\osupad-test-{}-{}",
            std::process::id(),
            name
        ))
    }
}

/// Removes whatever `test_addr` created. A named pipe disappears with its
/// last instance, so this is a no-op on Windows.
fn cleanup_addr(addr: &std::path::Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(addr);
        if let Some(parent) = addr.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
    #[cfg(windows)]
    {
        let _ = addr;
    }
}

#[tokio::test]
async fn test_ipc_roundtrip_requests() {
    let socket_path = test_addr("roundtrip");
    let listener = create_listener(&socket_path).expect("create_listener");

    // Spawn mock server task
    tokio::spawn(async move {
        while let Ok(mut stream) = listener.accept().await {
            tokio::spawn(async move {
                while let Ok(req) = read_request(&mut stream).await {
                    let resp = match req {
                        IpcRequest::Handshake {
                            client_protocol, ..
                        } => IpcResponse::HandshakeAck {
                            daemon_version: "1.0.0".to_string(),
                            daemon_protocol: client_protocol,
                            device_connected: true,
                        },
                        IpcRequest::GetStatus => IpcResponse::Status {
                            mode: RuntimeMode::Idle,
                            device_connected: true,
                            device_info: Some(DeviceInfo {
                                device_id: "OSUPAD-TEST".to_string(),
                                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                                firmware_version: "1.0.0".to_string(),
                                protocol_version: 1,
                                running_partition: None,
                            }),
                            counters: CounterState {
                                device_id: "OSUPAD-TEST".to_string(),
                                counter_generation: 1,
                                lifetime_key1: 42,
                                lifetime_key2: 99,
                                map_key1: 0,
                                map_key2: 0,
                            },
                            counters_source: osupad_model::CounterSource::Device,
                            pc_counters: None,
                            esp_counters: None,
                            config: DeviceConfig::default(),
                            last_sync_time: Some("2026-09-12T00:00:00Z".to_string()),
                            last_sync_error: None,
                            storage_error: None,
                            tosu_connected: false,
                            latency: None,
                            pending_replacement: None,
                            pending_takeover: None,
                            incompatible: None,
                        },
                        IpcRequest::PrepareFlash => IpcResponse::ReadyForFlash {
                            port: Some("/dev/ttyACM0".to_string()),
                        },
                        IpcRequest::FinishFlash => IpcResponse::HandshakeAck {
                            daemon_version: "1.0.0".to_string(),
                            daemon_protocol: IPC_PROTOCOL_VERSION,
                            device_connected: true,
                        },
                        IpcRequest::UpdateConfig(cfg) => IpcResponse::ConfigUpdated {
                            config: cfg,
                            deferred_persist: false,
                        },
                        IpcRequest::ExportBackup => {
                            let info = DeviceInfo {
                                device_id: "OSUPAD-TEST".to_string(),
                                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                                firmware_version: "1.0.0".to_string(),
                                protocol_version: 1,
                                running_partition: None,
                            };
                            let counters = CounterState {
                                device_id: "OSUPAD-TEST".to_string(),
                                counter_generation: 1,
                                lifetime_key1: 100,
                                lifetime_key2: 200,
                                map_key1: 0,
                                map_key2: 0,
                            };
                            let backup =
                                JsonBackup::new(&info, &counters, &DeviceConfig::default());
                            IpcResponse::BackupExported(backup)
                        }
                        IpcRequest::RestoreDeviceFromPc { .. }
                        | IpcRequest::ImportPcFromDevice { .. } => IpcResponse::CountersRestored {
                            counters: CounterState {
                                device_id: "OSUPAD-TEST".to_string(),
                                counter_generation: 2,
                                lifetime_key1: 100,
                                lifetime_key2: 200,
                                map_key1: 0,
                                map_key2: 0,
                            },
                        },
                        _ => IpcResponse::Error("Unhandled".to_string()),
                    };
                    let _ = send_response(&mut stream, &resp).await;
                }
            });
        }
    });

    // Client connection
    let mut client = connect(&socket_path).await.expect("client connect");

    // 1. Handshake
    let hs_req = IpcRequest::Handshake {
        client_version: "1.0.0".to_string(),
        client_protocol: IPC_PROTOCOL_VERSION,
    };
    let hs_resp = send_request(&mut client, &hs_req)
        .await
        .expect("handshake request");
    match hs_resp {
        IpcResponse::HandshakeAck {
            daemon_protocol,
            device_connected,
            ..
        } => {
            assert_eq!(daemon_protocol, IPC_PROTOCOL_VERSION);
            assert!(device_connected);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 2. GetStatus
    let status_resp = send_request(&mut client, &IpcRequest::GetStatus)
        .await
        .expect("status request");
    match status_resp {
        IpcResponse::Status { mode, counters, .. } => {
            assert_eq!(mode, RuntimeMode::Idle);
            assert_eq!(counters.lifetime_key1, 42);
            assert_eq!(counters.lifetime_key2, 99);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 3. PrepareFlash & FinishFlash
    let prep_resp = send_request(&mut client, &IpcRequest::PrepareFlash)
        .await
        .expect("prepare flash");
    match prep_resp {
        IpcResponse::ReadyForFlash { port } => {
            assert_eq!(port, Some("/dev/ttyACM0".to_string()));
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    let finish_resp = send_request(&mut client, &IpcRequest::FinishFlash)
        .await
        .expect("finish flash");
    match finish_resp {
        IpcResponse::HandshakeAck {
            device_connected, ..
        } => {
            assert!(device_connected);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 4. UpdateConfig
    let custom_cfg = DeviceConfig {
        debounce_us: 4500,
        brightness: 90,
        ..Default::default()
    };
    let cfg_resp = send_request(&mut client, &IpcRequest::UpdateConfig(custom_cfg.clone()))
        .await
        .expect("update config");
    match cfg_resp {
        IpcResponse::ConfigUpdated { config, .. } => {
            assert_eq!(config.debounce_us, 4500);
            assert_eq!(config.brightness, 90);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 5. ExportBackup
    let export_resp = send_request(&mut client, &IpcRequest::ExportBackup)
        .await
        .expect("export backup");
    match export_resp {
        IpcResponse::BackupExported(backup) => {
            assert_eq!(backup.device.device_id, "OSUPAD-TEST");
            assert_eq!(backup.stats.lifetime_key1, 100);
            assert_eq!(backup.stats.lifetime_key2, 200);
            assert!(backup.validate().is_ok());
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    cleanup_addr(&socket_path);
}

#[tokio::test]
async fn test_ipc_handshake_protocol_mismatch() {
    let socket_path = test_addr("handshake");
    let listener = create_listener(&socket_path).expect("create_listener");

    tokio::spawn(async move {
        if let Ok(mut stream) = listener.accept().await {
            if let Ok(req) = read_request(&mut stream).await {
                let resp = match req {
                    IpcRequest::Handshake {
                        client_protocol, ..
                    } => {
                        if client_protocol != IPC_PROTOCOL_VERSION {
                            IpcResponse::HandshakeRejected {
                                daemon_protocol: IPC_PROTOCOL_VERSION,
                                reason: "Protocol version mismatch".to_string(),
                            }
                        } else {
                            IpcResponse::HandshakeAck {
                                daemon_version: "1.0.0".to_string(),
                                daemon_protocol: IPC_PROTOCOL_VERSION,
                                device_connected: false,
                            }
                        }
                    }
                    _ => IpcResponse::Error("Expected handshake".to_string()),
                };
                let _ = send_response(&mut stream, &resp).await;
            }
        }
    });

    let mut client = connect(&socket_path).await.expect("client connect");
    let hs_req = IpcRequest::Handshake {
        client_version: "1.0.0".to_string(),
        client_protocol: 999, // Mismatched protocol version
    };
    let hs_resp = send_request(&mut client, &hs_req)
        .await
        .expect("handshake response");
    match hs_resp {
        IpcResponse::HandshakeRejected {
            daemon_protocol, ..
        } => {
            assert_eq!(daemon_protocol, IPC_PROTOCOL_VERSION);
        }
        other => panic!("Expected HandshakeRejected, got: {:?}", other),
    }

    cleanup_addr(&socket_path);
}

#[tokio::test]
async fn test_connect_and_handshake_helper() {
    let socket_path = test_addr("helper");
    let listener = create_listener(&socket_path).expect("create_listener");

    tokio::spawn(async move {
        if let Ok(mut stream) = listener.accept().await {
            if let Ok(IpcRequest::Handshake {
                client_protocol, ..
            }) = read_request(&mut stream).await
            {
                if client_protocol == IPC_PROTOCOL_VERSION {
                    let _ = send_response(
                        &mut stream,
                        &IpcResponse::HandshakeAck {
                            daemon_version: "1.0.0".to_string(),
                            daemon_protocol: IPC_PROTOCOL_VERSION,
                            device_connected: true,
                        },
                    )
                    .await;
                }
            }
        }
    });

    let res = osupad_ipc::connect_and_handshake_at(&socket_path).await;
    assert!(res.is_ok());
    let (_stream, ack) = res.unwrap();
    match ack {
        IpcResponse::HandshakeAck {
            device_connected, ..
        } => {
            assert!(device_connected);
        }
        other => panic!("Expected HandshakeAck, got: {:?}", other),
    }

    cleanup_addr(&socket_path);
}

#[tokio::test]
async fn test_oversized_frame_does_not_allocate() {
    use tokio::io::AsyncWriteExt;

    let socket_path = test_addr("frame");
    let listener = create_listener(&socket_path).expect("create_listener");

    let server_task = tokio::spawn(async move {
        if let Ok(mut stream) = listener.accept().await {
            // Attempt to read request with 2 GiB header
            let err = read_request(&mut stream).await.unwrap_err();
            match err {
                osupad_ipc::IpcError::Protocol(msg) => {
                    assert!(msg.contains("exceeds"));
                }
                other => panic!("Expected Protocol error, got: {:?}", other),
            }
        }
    });

    let mut client = connect(&socket_path).await.expect("client connect");
    // Send 2 GiB frame header (2 * 1024 * 1024 * 1024)
    let fake_len = 2u32 * 1024 * 1024 * 1024;
    client
        .write_all(&fake_len.to_le_bytes())
        .await
        .expect("write header");

    server_task.await.unwrap();
    cleanup_addr(&socket_path);
}

#[cfg(unix)]
#[tokio::test]
async fn test_socket_and_dir_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let socket_path = test_addr("perm");
    let socket_dir = socket_path.parent().unwrap().to_path_buf();
    let listener = create_listener(&socket_path).expect("create_listener");

    let dir_meta = std::fs::metadata(&socket_dir).expect("dir metadata");
    let dir_mode = dir_meta.permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700, "Directory permissions should be 0700");

    let sock_meta = std::fs::metadata(&socket_path).expect("sock metadata");
    let sock_mode = sock_meta.permissions().mode() & 0o777;
    assert_eq!(sock_mode, 0o600, "Socket permissions should be 0600");

    drop(listener);
    cleanup_addr(&socket_path);
}

/// A live listener must refuse a second one on both transports: a daemon
/// answering on the socket on Unix, `first_pipe_instance` on Windows.
#[tokio::test]
async fn test_second_listener_refused() {
    let socket_path = test_addr("single");
    let listener_1 = create_listener(&socket_path).expect("first create_listener");

    match create_listener(&socket_path) {
        Err(osupad_ipc::IpcError::AlreadyRunning) => {}
        other => panic!("Expected AlreadyRunning, got {:?}", other),
    }

    drop(listener_1);
    cleanup_addr(&socket_path);
}

/// The pipe DACL must name the current user and SYSTEM and nobody else — the
/// Windows half of the 0700/0600 guarantee the Unix socket has (§W0-2). The
/// cross-account half ("another user cannot open the pipe") needs two logged-in
/// accounts and stays a W4-2 manual check.
#[cfg(windows)]
#[test]
fn test_pipe_dacl_is_restricted_to_the_current_user() {
    let sddl = osupad_ipc::pipe_security_sddl().expect("pipe_security_sddl");

    assert!(
        sddl.starts_with("D:P"),
        "the DACL must be protected against inheritance: {sddl}"
    );
    assert!(
        sddl.contains("(A;;GA;;;SY)"),
        "SYSTEM must keep full access: {sddl}"
    );
    assert!(
        sddl.contains("(A;;GA;;;S-1-"),
        "the current user's SID must be granted access: {sddl}"
    );
    assert_eq!(
        sddl.matches("(A;").count(),
        2,
        "only SYSTEM and the current user may appear: {sddl}"
    );
    // Well-known groups that would put the pipe back within everyone's reach
    for trustee in ["WD", "AU", "BU", "AN", "BA", "IU"] {
        assert!(
            !sddl.contains(&format!(";{})", trustee)),
            "{trustee} must not be granted access: {sddl}"
        );
    }
}

/// A socket file left behind by a dead daemon must be cleaned up and rebound.
/// Windows has no equivalent: the pipe name disappears with its last instance.
#[cfg(unix)]
#[tokio::test]
async fn test_stale_socket_cleanup() {
    let socket_path = test_addr("stale");
    let listener_1 = create_listener(&socket_path).expect("first create_listener");

    // Drop listener_1 so the socket file becomes stale
    drop(listener_1);

    let listener_2 = create_listener(&socket_path).expect("stale socket cleanup create_listener");
    drop(listener_2);

    cleanup_addr(&socket_path);
}
