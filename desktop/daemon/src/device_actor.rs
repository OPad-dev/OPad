//! The one task that talks to the pad on the runtime loop's behalf.
//!
//! Runtime actions used to be sent from a fresh tokio task each, so two
//! back-to-back actions (a config then a layout, two telemetry diffs) could
//! reach the pad in either order. Here they queue on one ordered channel and
//! are sent one at a time, each awaited before the next.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use opad_device::{DeviceError, DeviceManager};
use opad_layout::{Layout, Screen};
use opad_model::ui_source::SourceValue;
use opad_model::DeviceConfig;
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::sync::DeviceLink;

/// Room for a burst of runtime actions; the runtime loop awaits when it is full
pub const QUEUE_DEPTH: usize = 256;

/// A full device queue usually drains within a few USB frames
const BUSY_RETRIES: u32 = 5;
const BUSY_BACKOFF: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceCommand {
    TimeSync,
    ClaimOwnership(Vec<u8>),
    Config(DeviceConfig),
    HostStatus {
        tosu_connected: bool,
        is_playing: bool,
        play_id: u32,
    },
    DataUpdate(Vec<(u8, SourceValue)>),
    Layout(Screen, Layout),
    RequestStatus,
}

async fn send_once<D: DeviceLink>(dm: &D, cmd: &DeviceCommand) -> Result<(), DeviceError> {
    match cmd {
        DeviceCommand::TimeSync => dm.send_time_sync().await,
        DeviceCommand::ClaimOwnership(owner) => dm.claim_ownership(owner).await,
        DeviceCommand::Config(cfg) => dm.send_config(cfg).await,
        DeviceCommand::HostStatus {
            tosu_connected,
            is_playing,
            play_id,
        } => {
            dm.send_host_status(*tosu_connected, *is_playing, *play_id)
                .await
        }
        DeviceCommand::DataUpdate(changes) => dm.send_data_update(changes).await,
        DeviceCommand::Layout(screen, layout) => dm.send_layout(*screen, layout).await,
        DeviceCommand::RequestStatus => dm.request_status().await,
    }
}

/// Sends one command. A telemetry diff that meets a full queue is not retried:
/// it is stale by the next flush, so `resend_data` asks the runtime loop for a
/// full snapshot instead of silently losing the diff. Everything else is
/// retried briefly.
pub async fn execute<D: DeviceLink>(dm: &D, cmd: &DeviceCommand, resend_data: &AtomicBool) {
    let mut attempt = 0;
    loop {
        match send_once(dm, cmd).await {
            Ok(()) => return,
            Err(DeviceError::Busy) if matches!(cmd, DeviceCommand::DataUpdate(_)) => {
                debug!("Device queue full; next telemetry flush resends everything");
                resend_data.store(true, Ordering::SeqCst);
                return;
            }
            Err(DeviceError::Busy) if attempt < BUSY_RETRIES => {
                attempt += 1;
                tokio::time::sleep(BUSY_BACKOFF).await;
            }
            Err(DeviceError::NotConnected) => return,
            Err(e) => {
                warn!("Could not send {} to the pad: {}", command_name(cmd), e);
                return;
            }
        }
    }
}

fn command_name(cmd: &DeviceCommand) -> &'static str {
    match cmd {
        DeviceCommand::TimeSync => "time sync",
        DeviceCommand::ClaimOwnership(_) => "ownership claim",
        DeviceCommand::Config(_) => "configuration",
        DeviceCommand::HostStatus { .. } => "host status",
        DeviceCommand::DataUpdate(_) => "telemetry",
        DeviceCommand::Layout(..) => "layout",
        DeviceCommand::RequestStatus => "status request",
    }
}

/// Starts the actor. `resend_data` is set when telemetry was dropped.
pub fn spawn(dm: Arc<DeviceManager>, resend_data: Arc<AtomicBool>) -> mpsc::Sender<DeviceCommand> {
    let (tx, mut rx) = mpsc::channel::<DeviceCommand>(QUEUE_DEPTH);
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            execute(&*dm, &cmd, &resend_data).await;
        }
    });
    tx
}

#[cfg(test)]
mod tests {
    use super::*;
    use opad_device::DeviceEvent;
    use opad_model::CounterState;
    use parking_lot::Mutex;
    use tokio::sync::broadcast;

    /// Records what was sent; answers Busy while `busy` is set
    #[derive(Default)]
    struct Recorder {
        sent: Mutex<Vec<&'static str>>,
        busy: AtomicBool,
    }

    impl Recorder {
        fn record(&self, what: &'static str) -> Result<(), DeviceError> {
            if self.busy.load(Ordering::SeqCst) {
                return Err(DeviceError::Busy);
            }
            self.sent.lock().push(what);
            Ok(())
        }
    }

    impl DeviceLink for Recorder {
        async fn send_time_sync(&self) -> Result<(), DeviceError> {
            self.record("time")
        }
        async fn send_config(&self, _: &DeviceConfig) -> Result<(), DeviceError> {
            self.record("config")
        }
        async fn send_layout(&self, _: Screen, _: &Layout) -> Result<(), DeviceError> {
            self.record("layout")
        }
        async fn reset_layout(&self, _: Screen) -> Result<(), DeviceError> {
            self.record("reset_layout")
        }
        async fn send_host_status(&self, _: bool, _: bool, _: u32) -> Result<(), DeviceError> {
            self.record("host_status")
        }
        async fn send_data_update(&self, _: &[(u8, SourceValue)]) -> Result<(), DeviceError> {
            self.record("data")
        }
        async fn send_counter_sync(&self, _: &CounterState, _: bool) -> Result<u32, DeviceError> {
            self.record("counters").map(|_| 1)
        }
        async fn claim_ownership(&self, _: &[u8]) -> Result<(), DeviceError> {
            self.record("claim")
        }
        async fn request_status(&self) -> Result<(), DeviceError> {
            self.record("status")
        }
        async fn request_logs(&self) -> Result<(), DeviceError> {
            self.record("logs")
        }
        async fn reset_latency_stats(&self) -> Result<(), DeviceError> {
            self.record("latency")
        }
        async fn send_detect_pin(&self, _: u32, _: u32, _: u32) -> Result<(), DeviceError> {
            self.record("detect")
        }
        fn subscribe(&self) -> broadcast::Receiver<DeviceEvent> {
            broadcast::channel(1).1
        }
        async fn pause_and_release(&self, _: Duration) -> bool {
            true
        }
        fn resume(&self) {}
        fn is_connected(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn commands_reach_the_pad_in_the_order_they_were_queued() {
        let dm = Recorder::default();
        let resend = AtomicBool::new(false);
        for cmd in [
            DeviceCommand::Config(DeviceConfig::default()),
            DeviceCommand::DataUpdate(vec![(1, SourceValue::Number(1.0))]),
            DeviceCommand::DataUpdate(vec![(1, SourceValue::Number(2.0))]),
            DeviceCommand::RequestStatus,
        ] {
            execute(&dm, &cmd, &resend).await;
        }
        assert_eq!(*dm.sent.lock(), vec!["config", "data", "data", "status"]);
        assert!(!resend.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn a_telemetry_diff_that_meets_a_full_queue_asks_for_a_full_resend() {
        let dm = Recorder::default();
        dm.busy.store(true, Ordering::SeqCst);
        let resend = AtomicBool::new(false);
        execute(
            &dm,
            &DeviceCommand::DataUpdate(vec![(1, SourceValue::Number(1.0))]),
            &resend,
        )
        .await;
        assert!(resend.load(Ordering::SeqCst));
        assert!(dm.sent.lock().is_empty());
    }
}
