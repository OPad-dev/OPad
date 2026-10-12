//! `opad-feed` — builds the update feeds (docs/specs/release-and-updates.md §5).
//!
//! This is the publishing half of `opad_update::client`. It writes
//! `stable.json` and `beta.json` by serialising the very `ReleaseManifest` the
//! client deserialises, so the two cannot drift. It never signs: the workflow
//! signs the files with minisign afterwards, and `opad-feed verify` then checks
//! them against the keys compiled into this build, so a feed this build cannot
//! read is never published.
//!
//! ```text
//! opad-feed github --releases releases.json [--sums-dir DIR] [--policy updates/policy.json]
//!                  --out-dir site/updates [--expires-days 30]
//!     From the repository's published releases (`gh api --paginate
//!     repos/OWNER/REPO/releases`, as one array or as JSON lines): for each
//!     channel, the newest release of each track. File hashes come from the
//!     API's `digest`, or from DIR/<tag>.SHA256SUMS.
//!
//! opad-feed local --dist DIR --base-url URL --out-dir DIR
//!                 [--app-version V] [--firmware-version V]
//!                 [--tosu-version V --tosu-revision N]
//!     From files built locally (`make feed-local`): both channels name the
//!     same files.
//!
//! opad-feed verify --feed FILE --channel stable|beta
//!     FILE.minisig verifies against a trusted key, and the feed is current.
//! ```

use opad_update::firmware::{FIRMWARE_BOARD, FIRMWARE_TARGET};
use opad_update::manifest::{
    check_feed, Artifact, ArtifactKind, Component, ReleaseManifest, Rollout, APP, BETA, FIRMWARE,
    STABLE, SUPPORTED_SCHEMA, TOSU,
};
use opad_update::verify::{sha256_file, verify_manifest_signature};
use opad_update::version::is_newer;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const DEFAULT_EXPIRES_DAYS: i64 = 30;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("github") => github(&args[1..]),
        Some("local") => local(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("-h" | "--help") | None => {
            println!("{HELP}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command {other:?}\n\n{HELP}")),
    };
    if let Err(e) = result {
        eprintln!("opad-feed: {e}");
        std::process::exit(1);
    }
}

const HELP: &str = "\
opad-feed — build the update feeds (docs/specs/release-and-updates.md §5)

  github --releases FILE [--sums-dir DIR] [--policy FILE] --out-dir DIR [--expires-days N]
  local  --dist DIR --base-url URL --out-dir DIR [--app-version V]
         [--firmware-version V] [--tosu-version V --tosu-revision N] [--expires-days N]
  verify --feed FILE --channel stable|beta
";

/// `--flag value` pairs; every flag takes exactly one value
fn flags(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if !flag.starts_with("--") {
            return Err(format!("unexpected argument {flag:?}\n\n{HELP}"));
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        out.insert(flag.trim_start_matches("--").to_string(), value.clone());
    }
    Ok(out)
}

fn required<'a>(f: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str, String> {
    f.get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("--{name} is required\n\n{HELP}"))
}

fn expires_days(f: &BTreeMap<String, String>) -> Result<i64, String> {
    match f.get("expires-days") {
        Some(v) => v.parse().map_err(|e| format!("--expires-days {v:?}: {e}")),
        None => Ok(DEFAULT_EXPIRES_DAYS),
    }
}

// ---------------------------------------------------------------------------
// Release tracks and file names

/// Which track a tag belongs to, with its version and (tosu) revision
#[derive(Debug, Clone, PartialEq, Eq)]
struct TagInfo {
    component: &'static str,
    version: String,
    revision: u32,
}

fn parse_tag(tag: &str) -> Option<TagInfo> {
    if let Some(rest) = tag.strip_prefix("firmware-v") {
        return Some(TagInfo {
            component: FIRMWARE,
            version: rest.to_string(),
            revision: 0,
        });
    }
    if let Some(rest) = tag.strip_prefix("tosu-v") {
        let (version, revision) = rest.rsplit_once("-r")?;
        return Some(TagInfo {
            component: TOSU,
            version: version.to_string(),
            revision: revision.parse().ok()?,
        });
    }
    let version = tag.strip_prefix('v')?;
    version
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| TagInfo {
            component: APP,
            version: version.to_string(),
            revision: 0,
        })
}

/// `(target, kind)` for a file of a track, by its published name
fn classify(component: &str, name: &str) -> Option<(String, ArtifactKind)> {
    match component {
        APP => app_artifact(name),
        FIRMWARE => firmware_artifact(name),
        TOSU => tosu_artifact(name),
        _ => None,
    }
}

fn app_artifact(name: &str) -> Option<(String, ArtifactKind)> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("opad-setup") && lower.ends_with(".exe") {
        return Some(("windows-x86_64".into(), ArtifactKind::Installer));
    }
    if lower.starts_with("opad") && lower.ends_with(".deb") {
        return Some(("linux-x86_64".into(), ArtifactKind::Deb));
    }
    if lower.starts_with("opad") && lower.ends_with(".rpm") {
        return Some(("linux-x86_64".into(), ArtifactKind::Rpm));
    }
    if lower.starts_with("opad-linux-x86_64-") && lower.ends_with(".tar.gz") {
        return Some(("linux-x86_64".into(), ArtifactKind::Archive));
    }
    // What an AppImage install swaps itself for
    if lower.starts_with("opad") && lower.ends_with(".appimage") {
        return Some(("linux-x86_64".into(), ArtifactKind::Binary));
    }
    None
}

fn firmware_artifact(name: &str) -> Option<(String, ArtifactKind)> {
    // The app image only. bootloader.bin, partition-table.bin and
    // ota_data_initial.bin are recovery files a person writes by hand; the
    // updater never sends them to the pad (§7.4).
    (name == "opad-firmware.bin").then(|| (FIRMWARE_TARGET.to_string(), ArtifactKind::Firmware))
}

fn tosu_artifact(name: &str) -> Option<(String, ArtifactKind)> {
    match name {
        "tosu-linux-x86_64" => Some(("linux-x86_64".into(), ArtifactKind::Binary)),
        "tosu-windows-x86_64.exe" => Some(("windows-x86_64".into(), ArtifactKind::Binary)),
        _ => None,
    }
}

fn artifact(
    component: &str,
    name: &str,
    url: String,
    sha256: String,
    size: Option<u64>,
) -> Option<Artifact> {
    let (target, kind) = classify(component, name)?;
    Some(Artifact {
        target,
        kind,
        url,
        sha256,
        size,
        board: (component == FIRMWARE).then(|| FIRMWARE_BOARD.to_string()),
    })
}

// ---------------------------------------------------------------------------
// Policy: yanked versions, rollout, min_app (updates/policy.json)

#[derive(Debug, Default, Deserialize)]
struct Policy {
    /// "app 1.2.0", "firmware 1.1.0", "tosu 4.26.2-r1": never offered
    #[serde(default)]
    yanked: BTreeSet<String>,
    /// "app 1.3.0" → percent, stable only
    #[serde(default)]
    rollout: BTreeMap<String, u8>,
    /// "firmware 2.0.0" → the oldest app that can drive it
    #[serde(default)]
    min_app: BTreeMap<String, String>,
}

/// How a release is named in the policy file
fn policy_key(t: &TagInfo) -> String {
    if t.revision > 0 {
        format!("{} {}-r{}", t.component, t.version, t.revision)
    } else {
        format!("{} {}", t.component, t.version)
    }
}

fn load_policy(path: Option<&str>) -> Result<Policy, String> {
    let Some(path) = path else {
        return Ok(Policy::default());
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    let policy: Policy = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    for (key, percent) in &policy.rollout {
        if *percent > 100 {
            return Err(format!(
                "{path}: rollout for {key:?} is {percent} %, above 100"
            ));
        }
    }
    Ok(policy)
}

// ---------------------------------------------------------------------------
// github: from the published releases

#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
    /// "sha256:<hex>", which GitHub computes for every release asset
    #[serde(default)]
    digest: Option<String>,
}

/// The API's pages, as one JSON array, several, or one release per line
fn parse_releases(text: &str) -> Result<Vec<GhRelease>, String> {
    let mut out = Vec::new();
    for value in serde_json::Deserializer::from_str(text).into_iter::<serde_json::Value>() {
        let value = value.map_err(|e| format!("releases JSON: {e}"))?;
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    out.push(serde_json::from_value(item).map_err(|e| format!("a release: {e}"))?);
                }
            }
            other => {
                out.push(serde_json::from_value(other).map_err(|e| format!("a release: {e}"))?)
            }
        }
    }
    Ok(out)
}

/// `name → sha256` from a SHA256SUMS file (`<hex>  <name>` per line)
fn parse_sums(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let (hash, name) = l.split_once(char::is_whitespace)?;
            let name = name.trim().trim_start_matches('*');
            (hash.len() == 64).then(|| (name.to_string(), hash.to_ascii_lowercase()))
        })
        .collect()
}

fn github(args: &[String]) -> Result<(), String> {
    let f = flags(args)?;
    let releases_path = required(&f, "releases")?;
    let out_dir = PathBuf::from(required(&f, "out-dir")?);
    let sums_dir = f.get("sums-dir").map(PathBuf::from);
    let policy = load_policy(f.get("policy").map(String::as_str))?;
    let days = expires_days(&f)?;

    let text = std::fs::read_to_string(releases_path)
        .map_err(|e| format!("reading {releases_path}: {e}"))?;
    let releases = parse_releases(&text)?;
    let now = chrono::Utc::now();

    for channel in [STABLE, BETA] {
        let feed = build_github_feed(&releases, sums_dir.as_deref(), &policy, channel, now, days)?;
        write_feed(&out_dir, &feed)?;
    }
    Ok(())
}

fn build_github_feed(
    releases: &[GhRelease],
    sums_dir: Option<&Path>,
    policy: &Policy,
    channel: &str,
    now: chrono::DateTime<chrono::Utc>,
    expires_days: i64,
) -> Result<ReleaseManifest, String> {
    // The newest eligible release of each track
    let mut newest: BTreeMap<&'static str, (TagInfo, &GhRelease)> = BTreeMap::new();
    for release in releases {
        if release.draft || (channel == STABLE && release.prerelease) {
            continue;
        }
        let Some(info) = parse_tag(&release.tag_name) else {
            continue;
        };
        if policy.yanked.contains(&policy_key(&info)) {
            continue;
        }
        let replace = match newest.get(info.component) {
            None => true,
            Some((best, _)) => {
                is_newer(&info.version, &best.version)
                    || (!is_newer(&best.version, &info.version) && info.revision > best.revision)
            }
        };
        if replace {
            newest.insert(info.component, (info, release));
        }
    }

    let mut components = BTreeMap::new();
    for (name, (info, release)) in newest {
        let sums = match sums_dir {
            Some(dir) => {
                let path = dir.join(format!("{}.SHA256SUMS", release.tag_name));
                std::fs::read_to_string(&path)
                    .map(|t| parse_sums(&t))
                    .unwrap_or_default()
            }
            None => BTreeMap::new(),
        };
        let mut artifacts = Vec::new();
        for asset in &release.assets {
            if classify(name, &asset.name).is_none() {
                continue;
            }
            let sha256 = asset
                .digest
                .as_deref()
                .and_then(|d| d.strip_prefix("sha256:"))
                .map(str::to_ascii_lowercase)
                .or_else(|| sums.get(&asset.name).cloned())
                .ok_or_else(|| {
                    format!(
                        "{}: no SHA-256 for {} (no digest from the API and none in SHA256SUMS)",
                        release.tag_name, asset.name
                    )
                })?;
            artifacts.extend(artifact(
                name,
                &asset.name,
                asset.browser_download_url.clone(),
                sha256,
                asset.size,
            ));
        }
        if artifacts.is_empty() {
            return Err(format!(
                "{} has no files the updater can use",
                release.tag_name
            ));
        }
        let key = policy_key(&info);
        components.insert(
            name.to_string(),
            Component {
                version: info.version.clone(),
                revision: info.revision,
                tag: Some(release.tag_name.clone()),
                upstream_tag: (name == TOSU).then(|| format!("v{}", info.version)),
                notes: (name == TOSU)
                    .then(|| "Bundled tosu, LGPL-3.0; see licenses/tosu/NOTICE".to_string()),
                notes_url: release.html_url.clone(),
                min_app: policy.min_app.get(&key).cloned(),
                rollout: (channel == STABLE)
                    .then(|| policy.rollout.get(&key).map(|&percent| Rollout { percent }))
                    .flatten(),
                artifacts,
            },
        );
    }

    Ok(feed(channel, now, expires_days, components))
}

// ---------------------------------------------------------------------------
// local: from files built on this machine

fn local(args: &[String]) -> Result<(), String> {
    let f = flags(args)?;
    let dist = PathBuf::from(required(&f, "dist")?);
    let base_url = required(&f, "base-url")?.trim_end_matches('/').to_string();
    let out_dir = PathBuf::from(required(&f, "out-dir")?);
    let days = expires_days(&f)?;
    let app_version = f
        .get("app-version")
        .cloned()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    let mut entries: Vec<_> = std::fs::read_dir(&dist)
        .map_err(|e| format!("reading {}: {e}", dist.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut components: BTreeMap<String, Component> = BTreeMap::new();
    for (name, version, revision) in [
        (APP, Some(app_version), 0),
        (FIRMWARE, f.get("firmware-version").cloned(), 0),
        (
            TOSU,
            f.get("tosu-version").cloned(),
            f.get("tosu-revision")
                .map(|r| r.parse().map_err(|e| format!("--tosu-revision: {e}")))
                .transpose()?
                .unwrap_or(1),
        ),
    ] {
        let mut artifacts = Vec::new();
        for entry in &entries {
            let file = entry.file_name().to_string_lossy().into_owned();
            if classify(name, &file).is_none() {
                continue;
            }
            let path = entry.path();
            let sha256 =
                sha256_file(&path).map_err(|e| format!("hashing {}: {e}", path.display()))?;
            let size = std::fs::metadata(&path).map(|m| m.len()).ok();
            artifacts.extend(artifact(
                name,
                &file,
                format!("{base_url}/{file}"),
                sha256,
                size,
            ));
        }
        if artifacts.is_empty() {
            continue;
        }
        let version = version.ok_or_else(|| {
            format!("{} has {name} files but no --{name}-version; it is compared with what is installed, so it must not be guessed", dist.display())
        })?;
        components.insert(
            name.to_string(),
            Component {
                version,
                revision: if name == TOSU { revision } else { 0 },
                artifacts,
                ..Default::default()
            },
        );
    }
    if components.is_empty() {
        return Err(format!(
            "no files the updater can use in {}",
            dist.display()
        ));
    }

    let now = chrono::Utc::now();
    for channel in [STABLE, BETA] {
        write_feed(&out_dir, &feed(channel, now, days, components.clone()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// verify: what update-feed.yml runs after signing

fn verify(args: &[String]) -> Result<(), String> {
    let f = flags(args)?;
    let path = PathBuf::from(required(&f, "feed")?);
    let channel = required(&f, "channel")?;
    let bytes = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let sig_path = PathBuf::from(format!("{}.minisig", path.display()));
    let signature = std::fs::read_to_string(&sig_path)
        .map_err(|e| format!("reading {}: {e}", sig_path.display()))?;
    verify_manifest_signature(&bytes, &signature).map_err(|e| {
        format!(
            "{}: the signature does not verify against any key this build trusts: {e}",
            path.display()
        )
    })?;
    let feed: ReleaseManifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if feed.schema != SUPPORTED_SCHEMA {
        return Err(format!(
            "{}: schema {} is not {SUPPORTED_SCHEMA}",
            path.display(),
            feed.schema
        ));
    }
    check_feed(&feed, channel, chrono::Utc::now(), None)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}: signature and contents OK", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------

fn feed(
    channel: &str,
    now: chrono::DateTime<chrono::Utc>,
    expires_days: i64,
    components: BTreeMap<String, Component>,
) -> ReleaseManifest {
    let rfc3339 =
        |t: chrono::DateTime<chrono::Utc>| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    ReleaseManifest {
        schema: SUPPORTED_SCHEMA,
        channel: channel.to_string(),
        // Seconds since the epoch: grows with every publish without any state
        // to keep between runs
        serial: now.timestamp().max(0) as u64,
        generated: rfc3339(now),
        expires: Some(rfc3339(now + chrono::Duration::days(expires_days))),
        components,
    }
}

/// Pretty-printed with a trailing newline: the signature covers these exact
/// bytes, so the file is written once and never reformatted.
fn write_feed(out_dir: &Path, feed: &ReleaseManifest) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("creating {}: {e}", out_dir.display()))?;
    let path = out_dir.join(format!("{}.json", feed.channel));
    let mut bytes = serde_json::to_vec_pretty(feed).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    std::fs::write(&path, &bytes).map_err(|e| format!("writing {}: {e}", path.display()))?;
    println!(
        "{} (serial {}, expires {})",
        path.display(),
        feed.serial,
        feed.expires.as_deref().unwrap_or("-")
    );
    for (name, c) in &feed.components {
        let revision = if c.revision > 0 {
            format!("-r{}", c.revision)
        } else {
            String::new()
        };
        println!(
            "  {name} {}{revision}: {} file(s)",
            c.version,
            c.artifacts.len()
        );
        for a in &c.artifacts {
            println!("    {:<16} {:?}  {}", a.target, a.kind, a.sha256);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_map_to_their_track() {
        assert_eq!(parse_tag("v1.2.0").unwrap().component, APP);
        assert_eq!(parse_tag("v1.2.0-rc.1").unwrap().version, "1.2.0-rc.1");
        let fw = parse_tag("firmware-v1.0.0-rc.1").unwrap();
        assert_eq!(
            (fw.component, fw.version.as_str()),
            (FIRMWARE, "1.0.0-rc.1")
        );
        let tosu = parse_tag("tosu-v4.26.2-r3").unwrap();
        assert_eq!(
            (tosu.component, tosu.version.as_str(), tosu.revision),
            (TOSU, "4.26.2", 3)
        );
        assert_eq!(parse_tag("vNext"), None);
        assert_eq!(parse_tag("something-else"), None);
        assert_eq!(
            parse_tag("tosu-v4.26.2"),
            None,
            "a tosu tag always carries a revision"
        );
    }

    #[test]
    fn files_are_classified_per_track() {
        assert_eq!(
            classify(APP, "opad-setup-1.2.0.exe"),
            Some(("windows-x86_64".into(), ArtifactKind::Installer))
        );
        assert_eq!(
            classify(APP, "opad_1.2.0_amd64.deb"),
            Some(("linux-x86_64".into(), ArtifactKind::Deb))
        );
        assert_eq!(
            classify(APP, "opad-1.2.0.x86_64.rpm"),
            Some(("linux-x86_64".into(), ArtifactKind::Rpm))
        );
        assert_eq!(
            classify(APP, "opad-1.2.0-x86_64.AppImage"),
            Some(("linux-x86_64".into(), ArtifactKind::Binary))
        );
        assert_eq!(
            classify(APP, "opad-linux-x86_64-1.2.0.tar.gz"),
            Some(("linux-x86_64".into(), ArtifactKind::Archive))
        );
        for other in [
            "SHA256SUMS",
            "THIRD_PARTY_NOTICES.html",
            "tosu-4.26.2-source.tar.gz",
            "opad-firmware.bin",
        ] {
            assert_eq!(classify(APP, other), None, "{other}");
        }
        assert_eq!(
            classify(FIRMWARE, "opad-firmware.bin"),
            Some((FIRMWARE_TARGET.into(), ArtifactKind::Firmware))
        );
        for recovery in [
            "bootloader.bin",
            "partition-table.bin",
            "ota_data_initial.bin",
        ] {
            assert_eq!(
                classify(FIRMWARE, recovery),
                None,
                "{recovery} must not be offered"
            );
        }
        assert_eq!(
            classify(TOSU, "tosu-linux-x86_64"),
            Some(("linux-x86_64".into(), ArtifactKind::Binary))
        );
        assert_eq!(
            classify(TOSU, "tosu-windows-x86_64.exe"),
            Some(("windows-x86_64".into(), ArtifactKind::Binary))
        );
        assert_eq!(classify(TOSU, "tosu-NOTICE"), None);
    }

    fn release(tag: &str, prerelease: bool, files: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.to_string(),
            draft: false,
            prerelease,
            html_url: Some(format!(
                "https://github.com/OPad-dev/OPad/releases/tag/{tag}"
            )),
            assets: files
                .iter()
                .map(|name| GhAsset {
                    name: name.to_string(),
                    browser_download_url: format!(
                        "https://github.com/OPad-dev/OPad/releases/download/{tag}/{name}"
                    ),
                    size: Some(10),
                    digest: Some(format!("sha256:{}", "ab".repeat(32))),
                })
                .collect(),
        }
    }

    fn releases() -> Vec<GhRelease> {
        vec![
            release(
                "v1.0.0",
                false,
                &["opad-setup-1.0.0.exe", "opad_1.0.0_amd64.deb"],
            ),
            release("v1.1.0-rc.1", true, &["opad-setup-1.1.0-rc.1.exe"]),
            release(
                "firmware-v1.0.0",
                false,
                &["opad-firmware.bin", "bootloader.bin"],
            ),
            release(
                "tosu-v4.26.2-r1",
                false,
                &["tosu-linux-x86_64", "tosu-windows-x86_64.exe"],
            ),
            release("tosu-v4.26.2-r2", false, &["tosu-linux-x86_64"]),
        ]
    }

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-11-01T00:00:00Z")
            .unwrap()
            .into()
    }

    #[test]
    fn stable_takes_final_releases_and_beta_the_newest() {
        let policy = Policy::default();
        let stable = build_github_feed(&releases(), None, &policy, STABLE, now(), 30).unwrap();
        let beta = build_github_feed(&releases(), None, &policy, BETA, now(), 30).unwrap();
        assert_eq!(stable.component(APP).unwrap().version, "1.0.0");
        assert_eq!(beta.component(APP).unwrap().version, "1.1.0-rc.1");
        assert_eq!(stable.component(FIRMWARE).unwrap().version, "1.0.0");
        let tosu = stable.component(TOSU).unwrap();
        assert_eq!(
            (tosu.version.as_str(), tosu.revision),
            ("4.26.2", 2),
            "the higher revision wins"
        );
        assert_eq!(tosu.tag.as_deref(), Some("tosu-v4.26.2-r2"));
    }

    #[test]
    fn every_file_points_at_its_own_tag_and_firmware_names_its_board() {
        let feed =
            build_github_feed(&releases(), None, &Policy::default(), STABLE, now(), 30).unwrap();
        for (name, c) in &feed.components {
            let tag = c.tag.as_deref().unwrap();
            for a in &c.artifacts {
                assert!(
                    a.url.contains(&format!("/download/{tag}/")),
                    "{name}: {}",
                    a.url
                );
                assert!(!a.url.contains("/latest/"), "{name}: {}", a.url);
            }
        }
        let fw = &feed.component(FIRMWARE).unwrap().artifacts;
        assert_eq!(fw.len(), 1, "recovery files are not offered");
        assert_eq!(fw[0].board.as_deref(), Some(FIRMWARE_BOARD));
    }

    #[test]
    fn the_feed_is_fresh_and_checkable() {
        let feed =
            build_github_feed(&releases(), None, &Policy::default(), STABLE, now(), 30).unwrap();
        assert_eq!(feed.schema, SUPPORTED_SCHEMA);
        assert_eq!(feed.serial, now().timestamp() as u64);
        check_feed(&feed, STABLE, now(), Some(feed.serial)).unwrap();
        assert!(check_feed(&feed, STABLE, now() + chrono::Duration::days(31), None).is_err());
    }

    #[test]
    fn the_policy_yanks_rolls_out_and_sets_min_app() {
        let policy: Policy = serde_json::from_str(
            r#"{"yanked": ["app 1.0.0", "tosu 4.26.2-r2"],
                "rollout": {"firmware 1.0.0": 10},
                "min_app": {"firmware 1.0.0": "1.0.0"}}"#,
        )
        .unwrap();
        let stable = build_github_feed(&releases(), None, &policy, STABLE, now(), 30).unwrap();
        assert!(
            stable.component(APP).is_none(),
            "the only final app was yanked"
        );
        assert_eq!(stable.component(TOSU).unwrap().revision, 1);
        let fw = stable.component(FIRMWARE).unwrap();
        assert_eq!(fw.rollout, Some(Rollout { percent: 10 }));
        assert_eq!(fw.min_app.as_deref(), Some("1.0.0"));

        let beta = build_github_feed(&releases(), None, &policy, BETA, now(), 30).unwrap();
        assert_eq!(
            beta.component(FIRMWARE).unwrap().rollout,
            None,
            "beta ignores rollout"
        );
    }

    #[test]
    fn hashes_fall_back_to_sha256sums() {
        let mut r = releases();
        for rel in &mut r {
            for a in &mut rel.assets {
                a.digest = None;
            }
        }
        assert!(build_github_feed(&r, None, &Policy::default(), STABLE, now(), 30).is_err());

        let dir = tempfile::tempdir().unwrap();
        for rel in &r {
            let sums: String = rel
                .assets
                .iter()
                .map(|a| format!("{}  {}\n", "cd".repeat(32), a.name))
                .collect();
            std::fs::write(
                dir.path().join(format!("{}.SHA256SUMS", rel.tag_name)),
                sums,
            )
            .unwrap();
        }
        let feed =
            build_github_feed(&r, Some(dir.path()), &Policy::default(), STABLE, now(), 30).unwrap();
        assert_eq!(
            feed.component(APP).unwrap().artifacts[0].sha256,
            "cd".repeat(32)
        );
    }

    #[test]
    fn drafts_are_never_offered() {
        let mut r = releases();
        r[0].draft = true;
        let stable = build_github_feed(&r, None, &Policy::default(), STABLE, now(), 30).unwrap();
        assert!(stable.component(APP).is_none());
    }

    #[test]
    fn releases_parse_from_an_array_pages_or_lines() {
        let one = r#"{"tag_name":"v1.0.0","assets":[]}"#;
        assert_eq!(parse_releases(&format!("[{one},{one}]")).unwrap().len(), 2);
        assert_eq!(
            parse_releases(&format!("[{one}]\n[{one}]")).unwrap().len(),
            2
        );
        assert_eq!(parse_releases(&format!("{one}\n{one}\n")).unwrap().len(), 2);
    }

    #[test]
    fn a_policy_above_100_percent_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        std::fs::write(&path, r#"{"rollout": {"app 1.0.0": 150}}"#).unwrap();
        assert!(load_policy(Some(path.to_str().unwrap())).is_err());
    }
}
