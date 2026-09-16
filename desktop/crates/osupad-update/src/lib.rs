//! Shared machinery for the three updaters: tosu (§U-1), the app (§U-2) and
//! the firmware (§U-3).
//!
//! **An updater is a remote code execution channel into the user's machine**
//! (§U-0.2). It runs unattended and repeatedly, which makes it the highest-risk
//! component in the product — higher than the installer. Everything here exists
//! to make the dangerous parts hard to skip:
//!
//! - Nothing is trusted because it came over HTTPS or because it came from
//!   GitHub. The release manifest is signed (§U-0.3) and every artifact is
//!   checked against a hash inside that signed manifest before it is written
//!   anywhere an execute bit could reach it.
//! - Nothing is applied while the user is playing (§U-0.1, P1-3).
//! - Nothing is replaced except by an atomic rename onto a fsynced temporary
//!   file, so a kill at any moment leaves the previous working version (§U-0.5).
//! - Checks are daily with ETag caching (§U-0.6); the unauthenticated GitHub
//!   API allows 60 requests an hour per IP, and a naive poll across many users
//!   looks like abuse.

pub mod download;
pub mod gate;
/// Off without the `net` feature, which is how the platform-independent half
/// of this crate is type-checked for Windows from a Linux host: reqwest's TLS
/// stack needs an MSVC toolchain to *build*, though not to work.
#[cfg(feature = "net")]
pub mod http;
pub mod manifest;
pub mod schedule;
pub mod verify;

pub use gate::{may_update_now, DeferReason};
pub use manifest::{Artifact, ArtifactKind, Component, ReleaseManifest, APP, FIRMWARE, TOSU};
pub use schedule::CheckSchedule;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Malformed release manifest: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    Http(String),
    /// The build has no signing key, so nothing can be trusted and nothing is
    /// applied. Failing closed is the only safe direction here.
    #[error("This build has no update signing key compiled in; updates are disabled")]
    NoPublicKey,
    #[error("The release manifest signature is not valid: {0}")]
    BadSignature(String),
    #[error("{artifact}: expected SHA-256 {expected}, got {actual}")]
    HashMismatch {
        artifact: String,
        expected: String,
        actual: String,
    },
    #[error("The release manifest is schema v{found}, this build understands v{supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("No {component} artifact published for {target}")]
    NoArtifact {
        component: String,
        target: &'static str,
    },
    #[error("Update deferred: {0}")]
    Deferred(DeferReason),
}
