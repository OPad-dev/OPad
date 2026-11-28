//! Firmware update over the pad's own connection (`OtaBegin`, `OtaChunk`,
//! `OtaEnd` in `protocol/opad.proto`).
//!
//! The pad writes the image to the app slot it is not running while it stays
//! a keyboard and plays its update animation, checks it, and restarts into it
//! on trial, exactly as after [`crate::flash::plan_app_update`]: the bootloader
//! rolls back unless the new image configures USB. Only for a pad whose
//! HelloAck says `ota_chunk_max`; any other is flashed through the ROM
//! bootloader ([`crate::flash`]).

use std::time::Duration;

use opad_protocol::proto::{self, host_to_device::Payload, OtaStage};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::{DeviceError, DeviceEvent, DeviceManager};

/// `OtaBegin`: the pad erases the image's room first, a few seconds for 1 MB
const BEGIN_TIMEOUT: Duration = Duration::from_secs(30);
/// Pieces sent before the first of them is answered: the pad works on one
/// while the next is on its way, instead of waiting for each round trip
const IN_FLIGHT: usize = 2;
/// No answer to a piece for this long: it was lost on the way, so it is sent
/// again. A piece takes well under a second, with a 64 KB erase in front of it.
const RESEND_AFTER: Duration = Duration::from_secs(3);
/// Times in a row a piece may go again before the update is given up
const MAX_RETRIES: u32 = 5;
/// `OtaEnd`: the pad reads the whole image back to check it
const END_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum OtaError {
    #[error("The pad does not take firmware over USB; flash it through the ROM bootloader")]
    Unsupported,
    #[error("The pad refused the update: {0}")]
    Refused(String),
    #[error("The pad stopped answering during the update ({0})")]
    NoAnswer(&'static str),
    #[error("The pad went away during the update")]
    Disconnected,
    #[error(transparent)]
    Device(#[from] DeviceError),
}

/// What [`update_over_usb`] reports as it goes: bytes written, of all of them
pub type Progress<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// Send `image` to the connected pad and have it restart into it. On `Ok`
/// the pad has verified the image and is restarting; the caller waits for it
/// to come back and checks what it runs. On `Err` the pad keeps running the
/// image it had (it gave up, or was told to).
pub async fn update_over_usb(
    dm: &DeviceManager,
    image: &[u8],
    progress: Progress<'_>,
) -> Result<(), OtaError> {
    let chunk = dm.ota_chunk_max() as usize;
    if chunk == 0 {
        return Err(OtaError::Unsupported);
    }
    let mut events = dm.subscribe();
    let result = run(dm, &mut events, image, chunk, progress).await;
    if result.is_err() {
        // A pad still in the middle of it would otherwise wait 10 s to give up
        let _ = dm.send_ota(Payload::OtaAbort(true));
    }
    result
}

async fn run(
    dm: &DeviceManager,
    events: &mut broadcast::Receiver<DeviceEvent>,
    image: &[u8],
    chunk: usize,
    progress: Progress<'_>,
) -> Result<(), OtaError> {
    let sha256 = Sha256::digest(image).to_vec();
    send(
        dm,
        Payload::OtaBegin(proto::OtaBegin {
            image_size: image.len() as u32,
            sha256,
        }),
    )
    .await?;
    expect(events, OtaStage::Ready, None, BEGIN_TIMEOUT, "start").await?;

    // The pad answers each piece, in order, with what it has written so far.
    // A piece lost on the way (a frame that failed its CRC) makes it answer
    // the next one with RESEND, or, with every piece in flight lost, nothing:
    // either way the pieces go again from what it has.
    let total = image.len() as u32;
    let mut acked: u32 = 0;
    let mut next: u32 = 0;
    let mut in_flight = 0;
    let mut retries = 0;
    progress(0, image.len());
    while acked < total {
        while in_flight < IN_FLIGHT && next < total {
            let end = (next as usize + chunk).min(image.len());
            send(
                dm,
                Payload::OtaChunk(proto::OtaChunk {
                    offset: next,
                    data: image[next as usize..end].to_vec(),
                }),
            )
            .await?;
            next = end as u32;
            in_flight += 1;
        }
        match next_status(events, RESEND_AFTER).await? {
            Some(st) if st.stage == OtaStage::Written as i32 => {
                in_flight = in_flight.saturating_sub(1);
                if st.written > acked {
                    acked = st.written;
                    retries = 0;
                    progress(acked as usize, image.len());
                }
            }
            Some(st) if st.stage == OtaStage::Resend as i32 => {
                acked = acked.max(st.written);
                next = st.written;
                in_flight = 0;
                retries += 1;
            }
            Some(st) if st.stage == OtaStage::Error as i32 => {
                return Err(OtaError::Refused(st.message));
            }
            Some(_) => {}
            None => {
                next = acked;
                in_flight = 0;
                retries += 1;
            }
        }
        if retries > MAX_RETRIES {
            return Err(OtaError::NoAnswer("write"));
        }
    }

    send(dm, Payload::OtaEnd(proto::OtaEnd {})).await?;
    expect(events, OtaStage::Done, None, END_TIMEOUT, "check").await
}

/// The command queue to the pad can be briefly full
async fn send(dm: &DeviceManager, payload: Payload) -> Result<(), OtaError> {
    for _ in 0..50 {
        match dm.send_ota(payload.clone()) {
            Err(DeviceError::Busy) => tokio::time::sleep(Duration::from_millis(10)).await,
            other => return Ok(other?),
        }
    }
    Err(OtaError::Device(DeviceError::Busy))
}

/// The pad's next OtaStatus, or None after `timeout`
async fn next_status(
    events: &mut broadcast::Receiver<DeviceEvent>,
    timeout: Duration,
) -> Result<Option<proto::OtaStatus>, OtaError> {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DeviceEvent::OtaStatus(st)) => return Ok(st),
                Ok(DeviceEvent::Disconnected) | Err(broadcast::error::RecvError::Closed) => {
                    return Err(OtaError::Disconnected)
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(Ok(st)) => Ok(Some(st)),
        Ok(Err(e)) => Err(e),
        Err(_) => Ok(None),
    }
}

async fn expect(
    events: &mut broadcast::Receiver<DeviceEvent>,
    stage: OtaStage,
    written: Option<u32>,
    timeout: Duration,
    step: &'static str,
) -> Result<(), OtaError> {
    let wait = async {
        loop {
            match events.recv().await {
                Ok(DeviceEvent::OtaStatus(st)) => {
                    if st.stage == OtaStage::Error as i32 {
                        return Err(OtaError::Refused(st.message));
                    }
                    if st.stage == stage as i32 && written.is_none_or(|w| w == st.written) {
                        return Ok(());
                    }
                }
                Ok(DeviceEvent::Disconnected) | Err(broadcast::error::RecvError::Closed) => {
                    return Err(OtaError::Disconnected)
                }
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    };
    tokio::time::timeout(timeout, wait)
        .await
        .unwrap_or(Err(OtaError::NoAnswer(step)))
}
