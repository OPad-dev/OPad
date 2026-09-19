//! The one HTTP client the updaters share (§U-0.6).
//!
//! Conditional GETs with `If-None-Match` keep the daily check nearly free: a
//! 304 is cheap and, on the GitHub API, does not spend rate-limit budget.
//! Everything is HTTPS-only, with a size ceiling so a hostile or broken server
//! cannot make the daemon allocate without bound — the signature check comes
//! after the download, so the download itself has to be survivable.

use crate::UpdateError;
use std::time::Duration;

const USER_AGENT: &str = concat!("osupad/", env!("CARGO_PKG_VERSION"));
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// 1 MiB is generous for a manifest and small enough that a bad one is
/// harmless; artifacts have their own, larger ceiling.
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
/// 512 MiB: larger than any artifact we ship, small enough to bound the damage
pub const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug)]
pub enum Fetched {
    /// The server confirmed our cached copy is current
    NotModified,
    Body {
        bytes: Vec<u8>,
        etag: Option<String>,
    },
}

#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
}

impl Http {
    pub fn new() -> Result<Self, UpdateError> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            // Redirects are followed, but `require_https` below is re-checked
            // on the final URL, so a redirect cannot downgrade the transport.
            .https_only(true)
            .build()
            .map_err(|e| UpdateError::Http(e.to_string()))?;
        Ok(Self { client })
    }

    /// A conditional GET. `etag` is the value from the last successful fetch.
    pub async fn get(
        &self,
        url: &str,
        etag: Option<&str>,
        max_bytes: u64,
    ) -> Result<Fetched, UpdateError> {
        require_https(url)?;
        let mut req = self.client.get(url);
        if let Some(tag) = etag {
            req = req.header(reqwest::header::IF_NONE_MATCH, tag);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| UpdateError::Http(format!("GET {url}: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(Fetched::NotModified);
        }
        if !resp.status().is_success() {
            return Err(UpdateError::Http(format!(
                "GET {url}: server returned {}",
                resp.status()
            )));
        }

        // Refuse before reading a byte when the server declares an oversized body
        if let Some(len) = resp.content_length() {
            if len > max_bytes {
                return Err(UpdateError::Http(format!(
                    "GET {url}: {len} bytes exceeds the {max_bytes} byte limit"
                )));
            }
        }

        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // A server can lie about or omit Content-Length, so cap the read too.
        let bytes = read_capped(resp, max_bytes, url).await?;
        Ok(Fetched::Body { bytes, etag })
    }
}

async fn read_capped(
    resp: reqwest::Response,
    max_bytes: u64,
    url: &str,
) -> Result<Vec<u8>, UpdateError> {
    use futures_util::StreamExt;
    let mut out: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| UpdateError::Http(format!("GET {url}: {e}")))?;
        if out.len() as u64 + chunk.len() as u64 > max_bytes {
            return Err(UpdateError::Http(format!(
                "GET {url}: body exceeds the {max_bytes} byte limit"
            )));
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

/// Belt and braces alongside `https_only`: the check is cheap and it also
/// catches a manifest that names a plain-HTTP artifact URL.
pub fn require_https(url: &str) -> Result<(), UpdateError> {
    if url.starts_with("https://") {
        return Ok(());
    }
    Err(UpdateError::Http(format!(
        "{url} is not https; refusing to download over an unauthenticated transport"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_is_refused() {
        assert!(require_https("http://example.com/manifest.json").is_err());
        assert!(require_https("ftp://example.com/x").is_err());
        assert!(require_https("https://example.com/manifest.json").is_ok());
    }
}
