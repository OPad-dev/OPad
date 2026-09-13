use osupad_ipc::{create_listener, read_request, send_request, send_response, IpcRequest, IpcResponse, IPC_PROTOCOL_VERSION};
use osupad_model::{CounterState, DeviceConfig, DeviceInfo, JsonBackup, RuntimeMode};
use tokio::net::UnixStream;

#[tokio::test]
async fn test_ipc_roundtrip_requests() {
    let socket_path = std::env::temp_dir().join(format!("osupad-test-{}.sock", std::process::id()));
    let listener = create_listener(&socket_path).expect("create_listener");

    // Spawn mock server task
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                while let Ok(req) = read_request(&mut stream).await {
                    let resp = match req {
                        IpcRequest::Handshake { client_protocol, .. } => {
                            IpcResponse::HandshakeAck {
                                daemon_version: "1.0.0".to_string(),
                                daemon_protocol: client_protocol,
                                device_connected: true,
                            }
                        }
                        IpcRequest::GetStatus => {
                            IpcResponse::Status {
                                mode: RuntimeMode::Idle,
                                device_connected: true,
                                device_info: Some(DeviceInfo {
                                    device_id: "OSUPAD-TEST".to_string(),
                                    board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                                    firmware_version: "1.0.0".to_string(),
                                    protocol_version: 1,
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
                                config: DeviceConfig::default(),
                                last_sync_time: Some("2026-09-12T00:00:00Z".to_string()),
                                last_sync_error: None,
                                tosu_connected: false,
                                latency: None,
                                pending_replacement: None,
                                incompatible: None,
                            }
                        }
                        IpcRequest::PrepareFlash => {
                            IpcResponse::ReadyForFlash {
                                port: Some("/dev/ttyACM0".to_string()),
                            }
                        }
                        IpcRequest::FinishFlash => {
                            IpcResponse::HandshakeAck {
                                daemon_version: "1.0.0".to_string(),
                                daemon_protocol: IPC_PROTOCOL_VERSION,
                                device_connected: true,
                            }
                        }
                        IpcRequest::UpdateConfig(cfg) => {
                            IpcResponse::ConfigUpdated { config: cfg, deferred_persist: false }
                        }
                        IpcRequest::ExportBackup => {
                            let info = DeviceInfo {
                                device_id: "OSUPAD-TEST".to_string(),
                                board_profile: "waveshare_esp32s3_touch_lcd_2".to_string(),
                                firmware_version: "1.0.0".to_string(),
                                protocol_version: 1,
                            };
                            let counters = CounterState {
                                device_id: "OSUPAD-TEST".to_string(),
                                counter_generation: 1,
                                lifetime_key1: 100,
                                lifetime_key2: 200,
                                map_key1: 0,
                                map_key2: 0,
                            };
                            let backup = JsonBackup::new(&info, &counters, &DeviceConfig::default());
                            IpcResponse::BackupExported(backup)
                        }
                        _ => IpcResponse::Error("Unhandled".to_string()),
                    };
                    let _ = send_response(&mut stream, &resp).await;
                }
            });
        }
    });

    // Client connection
    let mut client = UnixStream::connect(&socket_path).await.expect("client connect");

    // 1. Handshake
    let hs_req = IpcRequest::Handshake {
        client_version: "1.0.0".to_string(),
        client_protocol: IPC_PROTOCOL_VERSION,
    };
    let hs_resp = send_request(&mut client, &hs_req).await.expect("handshake request");
    match hs_resp {
        IpcResponse::HandshakeAck { daemon_protocol, device_connected, .. } => {
            assert_eq!(daemon_protocol, IPC_PROTOCOL_VERSION);
            assert!(device_connected);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 2. GetStatus
    let status_resp = send_request(&mut client, &IpcRequest::GetStatus).await.expect("status request");
    match status_resp {
        IpcResponse::Status { mode, counters, .. } => {
            assert_eq!(mode, RuntimeMode::Idle);
            assert_eq!(counters.lifetime_key1, 42);
            assert_eq!(counters.lifetime_key2, 99);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 3. PrepareFlash & FinishFlash
    let prep_resp = send_request(&mut client, &IpcRequest::PrepareFlash).await.expect("prepare flash");
    match prep_resp {
        IpcResponse::ReadyForFlash { port } => {
            assert_eq!(port, Some("/dev/ttyACM0".to_string()));
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    let finish_resp = send_request(&mut client, &IpcRequest::FinishFlash).await.expect("finish flash");
    match finish_resp {
        IpcResponse::HandshakeAck { device_connected, .. } => {
            assert!(device_connected);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 4. UpdateConfig
    let mut custom_cfg = DeviceConfig::default();
    custom_cfg.debounce_us = 4500;
    custom_cfg.brightness = 90;
    let cfg_resp = send_request(&mut client, &IpcRequest::UpdateConfig(custom_cfg.clone())).await.expect("update config");
    match cfg_resp {
        IpcResponse::ConfigUpdated { config, .. } => {
            assert_eq!(config.debounce_us, 4500);
            assert_eq!(config.brightness, 90);
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    // 5. ExportBackup
    let export_resp = send_request(&mut client, &IpcRequest::ExportBackup).await.expect("export backup");
    match export_resp {
        IpcResponse::BackupExported(backup) => {
            assert_eq!(backup.device.device_id, "OSUPAD-TEST");
            assert_eq!(backup.stats.lifetime_key1, 100);
            assert_eq!(backup.stats.lifetime_key2, 200);
            assert!(backup.validate().is_ok());
        }
        other => panic!("Unexpected response: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket_path);
}

#[tokio::test]
async fn test_ipc_handshake_protocol_mismatch() {
    let socket_path = std::env::temp_dir().join(format!("osupad-hs-test-{}.sock", std::process::id()));
    let listener = create_listener(&socket_path).expect("create_listener");

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            if let Ok(req) = read_request(&mut stream).await {
                let resp = match req {
                    IpcRequest::Handshake { client_protocol, .. } => {
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

    let mut client = UnixStream::connect(&socket_path).await.expect("client connect");
    let hs_req = IpcRequest::Handshake {
        client_version: "1.0.0".to_string(),
        client_protocol: 999, // Mismatched protocol version
    };
    let hs_resp = send_request(&mut client, &hs_req).await.expect("handshake response");
    match hs_resp {
        IpcResponse::HandshakeRejected { daemon_protocol, .. } => {
            assert_eq!(daemon_protocol, IPC_PROTOCOL_VERSION);
        }
        other => panic!("Expected HandshakeRejected, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket_path);
}

#[tokio::test]
async fn test_connect_and_handshake_helper() {
    let socket_path = std::env::temp_dir().join(format!("osupad-helper-test-{}.sock", std::process::id()));
    let listener = create_listener(&socket_path).expect("create_listener");

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            if let Ok(req) = read_request(&mut stream).await {
                if let IpcRequest::Handshake { client_protocol, .. } = req {
                    if client_protocol == IPC_PROTOCOL_VERSION {
                        let _ = send_response(&mut stream, &IpcResponse::HandshakeAck {
                            daemon_version: "1.0.0".to_string(),
                            daemon_protocol: IPC_PROTOCOL_VERSION,
                            device_connected: true,
                        }).await;
                    }
                }
            }
        }
    });

    let res = osupad_ipc::connect_and_handshake_at(&socket_path).await;
    assert!(res.is_ok());
    let (_stream, ack) = res.unwrap();
    match ack {
        IpcResponse::HandshakeAck { device_connected, .. } => {
            assert!(device_connected);
        }
        other => panic!("Expected HandshakeAck, got: {:?}", other),
    }

    let _ = std::fs::remove_file(&socket_path);
}
