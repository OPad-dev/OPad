use futures_util::StreamExt;
use osupad_model::GameplayTelemetry;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum TosuError {
    #[error("WebSocket connection error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("JSON decode error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct TosuManager {
    endpoint: String,
    telemetry_tx: broadcast::Sender<GameplayTelemetry>,
}

impl TosuManager {
    pub fn new(endpoint: String) -> (Self, broadcast::Receiver<GameplayTelemetry>) {
        let (tx, rx) = broadcast::channel(32);
        (
            Self {
                endpoint,
                telemetry_tx: tx,
            },
            rx,
        )
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GameplayTelemetry> {
        self.telemetry_tx.subscribe()
    }

    /// Spawns the background tosu worker loop with auto-reconnect
    pub fn start(self) {
        tokio::spawn(async move {
            loop {
                info!("Connecting to tosu WebSocket at {}", self.endpoint);
                match connect_async(&self.endpoint).await {
                    Ok((mut stream, _)) => {
                        info!("Connected to tosu WebSocket successfully");
                        while let Some(msg_result) = stream.next().await {
                            match msg_result {
                                Ok(msg) => {
                                    if msg.is_text() {
                                        if let Ok(text) = msg.to_text() {
                                            if let Some(telemetry) = parse_tosu_v2_json(text) {
                                                let _ = self.telemetry_tx.send(telemetry);
                                            }
                                        }
                                    } else if msg.is_close() {
                                        warn!("tosu WebSocket received close frame");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    warn!("Error reading tosu WebSocket frame: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("tosu WebSocket connection failed (osu! not running?): {}", e);
                    }
                }

                // Wait before reconnecting
                tokio::time::sleep(Duration::from_millis(2500)).await;
            }
        });
    }
}

/// Parses the tosu v2 JSON payload defensively (§15)
pub fn parse_tosu_v2_json(json_str: &str) -> Option<GameplayTelemetry> {
    let root: serde_json::Value = serde_json::from_str(json_str).ok()?;

    // tosu state: 2 = PLAYING
    let state_num = root
        .pointer("/state/number")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let is_playing = state_num == 2;

    let title = root
        .pointer("/menu/bm/metadata/title")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Title")
        .to_string();

    let artist = root
        .pointer("/menu/bm/metadata/artist")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Artist")
        .to_string();

    let current_pp = root
        .pointer("/gameplay/pp/current")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;

    // Calculate progress ratio if available
    let current_time = root
        .pointer("/menu/bm/time/current")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let total_time = root
        .pointer("/menu/bm/time/full")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let progress_ratio = if total_time > 0.0 {
        ((current_time / total_time) as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(GameplayTelemetry {
        is_playing,
        title,
        artist,
        current_pp,
        progress_ratio,
        map_presses_k1: 0,
        map_presses_k2: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tosu_json() {
        let sample = r#"{
            "state": { "number": 2 },
            "menu": {
                "bm": {
                    "metadata": { "title": "Freedom Dive", "artist": "xi" },
                    "time": { "current": 50000, "full": 100000 }
                }
            },
            "gameplay": {
                "pp": { "current": 286.4 }
            }
        }"#;

        let parsed = parse_tosu_v2_json(sample).expect("valid parse");
        assert!(parsed.is_playing);
        assert_eq!(parsed.title, "Freedom Dive");
        assert_eq!(parsed.artist, "xi");
        assert!((parsed.current_pp - 286.4).abs() < 0.01);
        assert!((parsed.progress_ratio - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_idle_state() {
        let sample = r#"{
            "state": { "number": 1 },
            "menu": { "bm": { "metadata": { "title": "Main Menu" } } }
        }"#;

        let parsed = parse_tosu_v2_json(sample).expect("valid parse");
        assert!(!parsed.is_playing);
        assert_eq!(parsed.title, "Main Menu");
    }
}
