//! The signed release manifest (§U-0.3).
//!
//! One document lists every artifact the app may download, with a SHA-256 for
//! each, and is signed with minisign/ed25519. The signature is what makes the
//! hashes worth anything: without it an attacker who can serve the manifest
//! can serve matching hashes too.
//!
//! tosu's artifacts are in here as well, even though tosu is upstream's
//! project and §U-1 tracks upstream's own releases. We cannot sign their
//! binaries, and §U-0.3 says HTTPS and "it came from GitHub" are not enough on
//! their own — so a tosu version becomes installable only once it has been
//! recorded here and this manifest re-signed. Keeping the "within a day of a
//! new stable release" promise in §U-1 is therefore a release-automation job:
//! regenerate and re-sign this manifest when upstream publishes, not only when
//! osu!pad itself ships.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Component keys. Strings rather than an enum so an older build ignores a
/// component a newer manifest adds instead of failing to parse the whole file.
pub const APP: &str = "app";
pub const TOSU: &str = "tosu";
pub const FIRMWARE: &str = "firmware";

pub const SUPPORTED_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub schema: u32,
    /// RFC 3339, so a stale manifest served from a cache is recognisable
    pub generated: String,
    #[serde(default)]
    pub components: BTreeMap<String, Component>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub version: String,
    /// The upstream tag this was built from, for the §T-3 source offer
    #[serde(default)]
    pub upstream_tag: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
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
        "schema": 1,
        "generated": "2026-09-16T12:00:00Z",
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

    #[test]
    fn current_target_is_known_for_this_build() {
        assert_ne!(current_target(), "unsupported");
    }
}
