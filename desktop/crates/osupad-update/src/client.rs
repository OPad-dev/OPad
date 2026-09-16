//! Fetching the signed manifest and the artifacts it names (§U-0.3, §U-0.6).
//!
//! The order here is the whole security model and is not rearrangeable:
//! fetch the manifest bytes, verify the detached signature **over those exact
//! bytes**, only then parse them, and only then trust a hash out of the result.
//! Parsing first and verifying a re-serialised copy would verify something the
//! server never sent.

use crate::http::{Fetched, Http, MAX_ARTIFACT_BYTES, MAX_MANIFEST_BYTES};
use crate::manifest::{ReleaseManifest, SUPPORTED_SCHEMA};
use crate::schedule::CheckSchedule;
use crate::verify::{check_hash, signing_configured, verify_manifest_signature};
use crate::{Artifact, UpdateError};
use std::time::SystemTime;

/// Where the signed manifest is published. The repository has no remote yet
/// (A.1), so this is the address the release process must publish to, not one
/// that resolves today. `$OSUPAD_MANIFEST_URL` overrides it for testing
/// against a local copy.
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/GFerreiroS/osupad/releases/latest/download/osupad-manifest.json";

pub struct UpdateClient {
    http: Http,
    manifest_url: String,
}

impl UpdateClient {
    pub fn new() -> Result<Self, UpdateError> {
        Ok(Self {
            http: Http::new()?,
            manifest_url: std::env::var("OSUPAD_MANIFEST_URL")
                .unwrap_or_else(|_| DEFAULT_MANIFEST_URL.to_string()),
        })
    }

    pub fn manifest_url(&self) -> &str {
        &self.manifest_url
    }

    /// The detached minisign signature lives beside the manifest
    fn signature_url(&self) -> String {
        format!("{}.minisig", self.manifest_url)
    }

    /// Returns `None` when the server confirmed our cached copy is current.
    ///
    /// Updates `schedule` either way, so a failure backs off and a success
    /// records the ETag the next conditional GET will send.
    pub async fn fetch_manifest(
        &self,
        schedule: &mut CheckSchedule,
        now: SystemTime,
    ) -> Result<Option<ReleaseManifest>, UpdateError> {
        if !signing_configured() {
            // Fail closed before touching the network: with no key there is
            // nothing a download could be checked against.
            schedule.record_failure(now, UpdateError::NoPublicKey.to_string());
            return Err(UpdateError::NoPublicKey);
        }

        let result = self.fetch_manifest_inner(schedule.etag.as_deref()).await;
        match result {
            Ok(None) => {
                schedule.record_success(now, None);
                Ok(None)
            }
            Ok(Some((manifest, etag))) => {
                schedule.record_success(now, etag);
                Ok(Some(manifest))
            }
            Err(e) => {
                schedule.record_failure(now, e.to_string());
                Err(e)
            }
        }
    }

    async fn fetch_manifest_inner(
        &self,
        etag: Option<&str>,
    ) -> Result<Option<(ReleaseManifest, Option<String>)>, UpdateError> {
        let fetched = self
            .http
            .get(&self.manifest_url, etag, MAX_MANIFEST_BYTES)
            .await?;
        let (bytes, etag) = match fetched {
            Fetched::NotModified => return Ok(None),
            Fetched::Body { bytes, etag } => (bytes, etag),
        };

        // The signature is never conditionally fetched: it must match the
        // bytes we just received, not whatever a cache thought was current.
        let signature = match self.http.get(&self.signature_url(), None, 4096).await? {
            Fetched::Body { bytes, .. } => String::from_utf8(bytes)
                .map_err(|e| UpdateError::BadSignature(format!("signature is not text: {e}")))?,
            Fetched::NotModified => {
                return Err(UpdateError::BadSignature(
                    "signature endpoint answered 304 to an unconditional request".to_string(),
                ))
            }
        };

        verify_manifest_signature(&bytes, &signature)?;

        // Only now are these bytes trustworthy enough to parse.
        let manifest: ReleaseManifest = serde_json::from_slice(&bytes)?;
        if manifest.schema != SUPPORTED_SCHEMA {
            return Err(UpdateError::UnsupportedSchema {
                found: manifest.schema,
                supported: SUPPORTED_SCHEMA,
            });
        }
        Ok(Some((manifest, etag)))
    }

    /// Downloads one artifact and checks it against the hash in the verified
    /// manifest. The bytes are never written anywhere the caller has not
    /// chosen, and a mismatch returns before they are handed back.
    pub async fn fetch_artifact(&self, artifact: &Artifact) -> Result<Vec<u8>, UpdateError> {
        let bytes = match self
            .http
            .get(&artifact.url, None, MAX_ARTIFACT_BYTES)
            .await?
        {
            Fetched::Body { bytes, .. } => bytes,
            Fetched::NotModified => {
                return Err(UpdateError::Http(
                    "server answered 304 to an unconditional artifact request".to_string(),
                ))
            }
        };

        if let Some(expected) = artifact.size {
            if bytes.len() as u64 != expected {
                return Err(UpdateError::Http(format!(
                    "{}: expected {} bytes, got {}",
                    artifact.url,
                    expected,
                    bytes.len()
                )));
            }
        }

        // Hash in a temporary file so one code path checks every artifact.
        let dir = tempdir_for_check()?;
        let path = dir.join("artifact");
        std::fs::write(&path, &bytes)?;
        let verdict = check_hash(&artifact.url, &path, &artifact.sha256);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
        verdict?;

        Ok(bytes)
    }
}

fn tempdir_for_check() -> Result<std::path::PathBuf, UpdateError> {
    let dir = std::env::temp_dir().join(format!("osupad-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_sits_beside_the_manifest() {
        let client = UpdateClient {
            http: Http::new().unwrap(),
            manifest_url: "https://example.invalid/m.json".to_string(),
        };
        assert_eq!(
            client.signature_url(),
            "https://example.invalid/m.json.minisig"
        );
    }

    #[test]
    fn the_default_manifest_url_is_https() {
        assert!(DEFAULT_MANIFEST_URL.starts_with("https://"));
    }

    #[tokio::test]
    async fn an_unreachable_manifest_backs_off_instead_of_hanging() {
        // This build has a signing key compiled in (§U-0.3), so the fail-closed
        // branch above no longer short-circuits and the request is really made.
        // That "no key ⇒ no network" branch is now unreachable from a test
        // without injecting the key; `verify::tests` covers the gate itself,
        // and the branch here is two lines above the network call.
        assert!(signing_configured());

        let client = UpdateClient {
            http: Http::new().unwrap(),
            // Refused immediately; nothing leaves the machine
            manifest_url: "https://127.0.0.1:1/never".to_string(),
        };
        let mut schedule = CheckSchedule::default();
        let err = client
            .fetch_manifest(&mut schedule, SystemTime::UNIX_EPOCH)
            .await
            .unwrap_err();
        assert!(matches!(err, UpdateError::Http(_)), "got {err:?}");
        assert_eq!(schedule.consecutive_failures, 1);
    }
}
