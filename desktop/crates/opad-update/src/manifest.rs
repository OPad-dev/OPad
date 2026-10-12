//! The signed update feed (docs/specs/release-and-updates.md §5).
//!
//! One document per channel (`stable.json`, `beta.json`) lists the current
//! version of each component (the app, the firmware, tosu) and every file the
//! app may download, with a SHA-256 for each, and is signed with
//! minisign/ed25519. The signature is what makes the hashes worth anything:
//! without it an attacker who can serve the feed can serve matching hashes too.
//!
//! Each component releases on its own track and its files stay where that
//! release put them; the feed only points at them. tosu is in here although it
//! is upstream's project: we cannot sign their binaries, and HTTPS or "it came
//! from GitHub" is not enough on its own, so a tosu build becomes installable
//! only once our own release has published it and this feed names its hash.
//!
//! Beyond the signature, three fields stop a genuine but old feed being used
//! against an install (§5.2): `channel` (a beta feed served as stable),
//! `serial` (an older feed replayed to hide an update or force a downgrade)
//! and `expires` (a feed frozen in the past). [`check_feed`] enforces them.

use crate::UpdateError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Component keys. Strings rather than an enum so an older build ignores a
/// component a newer feed adds instead of failing to parse the whole file.
pub const APP: &str = "app";
pub const TOSU: &str = "tosu";
pub const FIRMWARE: &str = "firmware";

/// Update channels (§9.1)
pub const STABLE: &str = "stable";
pub const BETA: &str = "beta";

/// Schema 2 is the per-channel feed. Schema 1 was one manifest per release;
/// no build that read it was ever published.
pub const SUPPORTED_SCHEMA: u32 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub schema: u32,
    /// Which feed this is; must be the one the client asked for
    #[serde(default)]
    pub channel: String,
    /// Grows with every publish (the generation time in Unix seconds). A
    /// client never accepts a lower serial than one it already accepted.
    #[serde(default)]
    pub serial: u64,
    /// RFC 3339, so a stale feed served from a cache is recognisable
    pub generated: String,
    /// RFC 3339. After this the feed is refused, so a feed someone stopped
    /// from being republished is noticed instead of reading as "up to date".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default)]
    pub components: BTreeMap<String, Component>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub version: String,
    /// Our build revision of an upstream version (tosu); 0 when there is none
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision: u32,
    /// The release tag the files belong to (`v1.2.0`, `firmware-v1.2.0`,
    /// `tosu-v4.26.2-r1`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The upstream tag this was built from, for the §T-3 source offer
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// The release page, for "what's new"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
    /// Firmware only: the oldest app that can drive it. Normally absent,
    /// because every app works with every firmware (AGENTS.md §1.3); §3.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_app: Option<String>,
    /// Stable channel only: the share of installs offered this version (§9.2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout: Option<Rollout>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rollout {
    /// 0 pauses the rollout, 100 offers it to everyone
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// `current_target()` form, e.g. `linux-x86_64`
    pub target: String,
    pub kind: ArtifactKind,
    pub url: String,
    /// Lowercase hex SHA-256 of the file at `url`
    pub sha256: String,
    #[serde(default)]
    pub size: Option<u64>,
    /// Firmware only: the board the image is for, as the pad reports it in
    /// `HelloAck.board_profile`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// Everything about a verified feed's bytes that the signature does not
/// cover by itself: that it is the feed asked for, still current, and not
/// older than one already seen. Call it after the signature check and the
/// schema check, before using anything in the feed.
pub fn check_feed(
    feed: &ReleaseManifest,
    channel: &str,
    now: DateTime<Utc>,
    highest_serial_seen: Option<u64>,
) -> Result<(), UpdateError> {
    if feed.channel != channel {
        return Err(UpdateError::Feed(format!(
            "this is the {:?} feed, but the {channel:?} feed was requested",
            feed.channel
        )));
    }
    let expires = feed
        .expires
        .as_deref()
        .ok_or_else(|| UpdateError::Feed("the feed has no expiry date".to_string()))?;
    let expires = DateTime::parse_from_rfc3339(expires)
        .map_err(|e| UpdateError::Feed(format!("unreadable expiry date {expires:?}: {e}")))?;
    if now > expires {
        return Err(UpdateError::Feed(format!(
            "the feed expired on {expires}; it should have been republished before then"
        )));
    }
    if let Some(seen) = highest_serial_seen {
        if feed.serial < seen {
            return Err(UpdateError::Feed(format!(
                "the feed is older than one already seen (serial {} < {seen}); refusing a replayed feed",
                feed.serial
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Windows installer, applied with `/SILENT /NORESTART`
    Installer,
    Deb,
    Rpm,
    /// Archive of replacement files, for `make install-user` and AppImage
    Archive,
    /// A single executable, such as the bundled tosu
    Binary,
    /// An ESP32-S3 app image
    Firmware,
}

/// What this build is, in the manifest's `target` vocabulary.
///
/// Host targets only. A firmware artifact is for a chip rather than for the
/// machine that flashes it, so it uses `firmware::FIRMWARE_TARGET` instead —
/// the same `.bin` is written from Windows and from Linux.
pub fn current_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("macos", "x86_64") => "macos-x86_64",
        ("macos", "aarch64") => "macos-aarch64",
        _ => "unsupported",
    }
}

impl ReleaseManifest {
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.components.get(name)
    }

    /// The artifact for this machine, optionally narrowed to one kind — a
    /// Linux release carries a `.deb` and an `.rpm` for the same target.
    pub fn artifact_for(
        &self,
        component: &str,
        target: &str,
        kind: Option<ArtifactKind>,
    ) -> Option<&Artifact> {
        self.component(component)?
            .artifacts
            .iter()
            .find(|a| a.target == target && kind.is_none_or(|k| a.kind == k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "schema": 2,
        "channel": "stable",
        "serial": 1789000000,
        "generated": "2026-09-16T12:00:00Z",
        "expires": "2026-10-16T12:00:00Z",
        "components": {
            "app": {
                "version": "1.0.0",
                "artifacts": [
                    {"target": "linux-x86_64", "kind": "deb", "url": "https://x/a.deb", "sha256": "aa"},
                    {"target": "linux-x86_64", "kind": "rpm", "url": "https://x/a.rpm", "sha256": "bb"},
                    {"target": "windows-x86_64", "kind": "installer", "url": "https://x/s.exe", "sha256": "cc"}
                ]
            },
            "tosu": {
                "version": "4.1.0",
                "upstream_tag": "v4.1.0",
                "artifacts": [
                    {"target": "linux-x86_64", "kind": "binary", "url": "https://x/tosu", "sha256": "dd", "size": 42}
                ]
            }
        }
    }"#;

    #[test]
    fn parses_and_selects_by_target_and_kind() {
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(m.schema, SUPPORTED_SCHEMA);
        assert_eq!(m.component(APP).unwrap().version, "1.0.0");

        let deb = m
            .artifact_for(APP, "linux-x86_64", Some(ArtifactKind::Deb))
            .unwrap();
        assert_eq!(deb.url, "https://x/a.deb");
        let rpm = m
            .artifact_for(APP, "linux-x86_64", Some(ArtifactKind::Rpm))
            .unwrap();
        assert_eq!(rpm.sha256, "bb");

        assert!(m.artifact_for(APP, "linux-aarch64", None).is_none());
        assert!(m
            .artifact_for(TOSU, "windows-x86_64", Some(ArtifactKind::Binary))
            .is_none());
    }

    #[test]
    fn an_unknown_component_does_not_break_parsing() {
        // A newer manifest must not stop an older build from updating itself.
        let json = SAMPLE.replace(r#""tosu": {"#, r#""something-new": {"#);
        let m: ReleaseManifest = serde_json::from_str(&json).unwrap();
        assert!(m.component(APP).is_some());
        assert!(m.component(TOSU).is_none());
    }

    #[test]
    fn every_artifact_url_is_https() {
        // Plain HTTP would let a network attacker choose what we download; the
        // signature would still catch it, but there is no reason to allow it.
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        for c in m.components.values() {
            for a in &c.artifacts {
                assert!(a.url.starts_with("https://"), "{}", a.url);
            }
        }
    }

    fn at(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_current_feed_for_the_right_channel_passes() {
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        check_feed(&m, STABLE, at("2026-09-20T00:00:00Z"), Some(1789000000)).unwrap();
        check_feed(&m, STABLE, at("2026-09-20T00:00:00Z"), None).unwrap();
    }

    #[test]
    fn a_feed_for_another_channel_is_refused() {
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        let err = check_feed(&m, BETA, at("2026-09-20T00:00:00Z"), None).unwrap_err();
        assert!(matches!(err, UpdateError::Feed(_)), "{err:?}");
    }

    #[test]
    fn an_expired_feed_is_refused() {
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        assert!(check_feed(&m, STABLE, at("2026-10-17T00:00:00Z"), None).is_err());
    }

    #[test]
    fn a_feed_without_an_expiry_is_refused() {
        let mut m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        m.expires = None;
        assert!(check_feed(&m, STABLE, at("2026-09-20T00:00:00Z"), None).is_err());
    }

    #[test]
    fn an_older_feed_than_one_seen_is_refused() {
        let m: ReleaseManifest = serde_json::from_str(SAMPLE).unwrap();
        assert!(check_feed(&m, STABLE, at("2026-09-20T00:00:00Z"), Some(1789000001)).is_err());
    }

    #[test]
    fn optional_fields_stay_out_of_the_serialised_feed() {
        let c = Component {
            version: "1.0.0".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(
            !json.contains("rollout") && !json.contains("min_app") && !json.contains("revision"),
            "{json}"
        );
    }

    #[test]
    fn current_target_is_known_for_this_build() {
        assert_ne!(current_target(), "unsupported");
    }
}
