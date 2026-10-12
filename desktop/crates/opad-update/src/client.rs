//! Fetching the signed feed and the files it names
//! (docs/specs/release-and-updates.md §5).
//!
//! The order here is the whole security model and is not rearrangeable:
//! fetch the feed bytes, verify the detached signature **over those exact
//! bytes**, only then parse them, check the feed is the one asked for, still
//! current and not older than one already seen, and only then trust a hash out
//! of it. Parsing first and verifying a re-serialised copy would verify
//! something the server never sent.

use crate::http::{Fetched, Http, MAX_ARTIFACT_BYTES, MAX_MANIFEST_BYTES};
use crate::manifest::{check_feed, ReleaseManifest, SUPPORTED_SCHEMA};
use crate::schedule::CheckSchedule;
use crate::verify::{check_hash_bytes, signing_configured, verify_manifest_signature};
use crate::{Artifact, UpdateError};
use std::time::SystemTime;

/// Where the feeds are published (GitHub Pages, deployed by update-feed.yml):
/// `<base><channel>.json` and its `.minisig`. `$OPAD_MANIFEST_URL` replaces
/// the whole feed URL, for testing against `make feed-local`.
pub const DEFAULT_FEED_BASE: &str = "https://opad-dev.github.io/OPad/updates/";

/// The feed URL for a channel, honouring `$OPAD_MANIFEST_URL`
pub fn feed_url(channel: &str) -> String {
    std::env::var("OPAD_MANIFEST_URL")
        .or_else(|_| std::env::var("OSUPAD_MANIFEST_URL"))
        .unwrap_or_else(|_| format!("{DEFAULT_FEED_BASE}{channel}.json"))
}

pub struct UpdateClient {
    http: Http,
    manifest_url: String,
    channel: String,
}

impl UpdateClient {
    /// A client for one channel's feed
    pub fn new(channel: &str) -> Result<Self, UpdateError> {
        Ok(Self {
            http: Http::new()?,
            manifest_url: feed_url(channel),
            channel: channel.to_string(),
        })
    }

    /// A client that only downloads files a verified feed named
    pub fn for_artifacts() -> Result<Self, UpdateError> {
        Self::new(crate::manifest::STABLE)
    }

    pub fn manifest_url(&self) -> &str {
        &self.manifest_url
    }

    /// The detached minisign signature lives beside the feed
    fn signature_url(&self) -> String {
        format!("{}.minisig", self.manifest_url)
    }

    /// Returns `None` when the server confirmed our cached copy is current.
    ///
    /// `have_cached` says whether the caller still holds the feed the stored
    /// ETag belongs to. Only then is the request conditional: the ETag
    /// outlives a daemon restart but the feed does not, and a 304 to a caller
    /// with nothing cached would leave it with no feed at all.
    ///
    /// Updates `schedule` either way, so a failure backs off and a success
    /// records the ETag the next conditional GET will send and the feed's
    /// serial, below which no feed is accepted again.
    pub async fn fetch_manifest(
        &self,
        schedule: &mut CheckSchedule,
        now: SystemTime,
        have_cached: bool,
    ) -> Result<Option<ReleaseManifest>, UpdateError> {
        if !signing_configured() {
            // Fail closed before touching the network: with no key there is
            // nothing a download could be checked against.
            schedule.record_failure(now, UpdateError::NoPublicKey.to_string());
            return Err(UpdateError::NoPublicKey);
        }

        let etag = conditional_etag(schedule, &self.channel, have_cached).map(str::to_string);
        let result = match self.fetch_manifest_inner(etag.as_deref()).await {
            Ok(Some((feed, etag))) => {
                let seen = schedule.serials.get(&self.channel).copied();
                check_feed(&feed, &self.channel, now.into(), seen).map(|()| Some((feed, etag)))
            }
            other => other,
        };
        match result {
            Ok(None) => {
                schedule.record_success(now, None);
                Ok(None)
            }
            Ok(Some((feed, etag))) => {
                schedule.channel = Some(self.channel.clone());
                schedule.record_success(now, etag);
                let serial = schedule.serials.entry(self.channel.clone()).or_default();
                *serial = (*serial).max(feed.serial);
                Ok(Some(feed))
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

        check_hash_bytes(&artifact.url, &bytes, &artifact.sha256)?;
        Ok(bytes)
    }
}

/// The ETag to send as `If-None-Match`, if the request should be conditional
fn conditional_etag<'a>(
    schedule: &'a CheckSchedule,
    channel: &str,
    have_cached: bool,
) -> Option<&'a str> {
    schedule.etag_for(channel).filter(|_| have_cached)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_sits_beside_the_manifest() {
        let client = UpdateClient {
            http: Http::new().unwrap(),
            manifest_url: "https://example.invalid/m.json".to_string(),
            channel: "stable".to_string(),
        };
        assert_eq!(
            client.signature_url(),
            "https://example.invalid/m.json.minisig"
        );
    }

    #[test]
    fn the_etag_is_only_sent_while_its_manifest_is_held() {
        let mut schedule = CheckSchedule::default();
        assert_eq!(conditional_etag(&schedule, "stable", true), None);
        schedule.switch_channel("stable");
        schedule.record_success(SystemTime::UNIX_EPOCH, Some("\"v1\"".into()));
        assert_eq!(conditional_etag(&schedule, "stable", true), Some("\"v1\""));
        // Another channel is another file
        assert_eq!(conditional_etag(&schedule, "beta", true), None);
        // A restarted daemon has the persisted ETag but no manifest: a 304
        // would leave it with nothing, so it must fetch in full
        let restarted: CheckSchedule =
            serde_json::from_str(&serde_json::to_string(&schedule).unwrap()).unwrap();
        assert_eq!(conditional_etag(&restarted, "stable", false), None);
    }

    #[test]
    fn the_default_feed_is_https_and_per_channel() {
        assert!(DEFAULT_FEED_BASE.starts_with("https://"));
        if std::env::var_os("OPAD_MANIFEST_URL").is_none()
            && std::env::var_os("OSUPAD_MANIFEST_URL").is_none()
        {
            assert!(feed_url("beta").ends_with("/updates/beta.json"));
        }
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
            channel: "stable".to_string(),
        };
        let mut schedule = CheckSchedule::default();
        let err = client
            .fetch_manifest(&mut schedule, SystemTime::UNIX_EPOCH, false)
            .await
            .unwrap_err();
        assert!(matches!(err, UpdateError::Http(_)), "got {err:?}");
        assert_eq!(schedule.consecutive_failures, 1);
    }
}
