//! `opad-manifest` — builds and signs the release manifest (§U-0.3).
//!
//! This is the release-side half of `opad_update::verify`. It walks a `dist`
//! directory, records every artifact with its SHA-256, writes
//! `opad-manifest.json`, and produces the detached
//! `opad-manifest.json.minisig` beside it — the two files
//! `opad_update::client` fetches.
//!
//! It builds the manifest by serialising the very `ReleaseManifest` the client
//! deserialises, rather than by writing JSON by hand, so the two cannot drift.
//! After signing it re-verifies the result against the *compiled-in*
//! `MANIFEST_PUBLIC_KEY`, which makes it impossible to publish a manifest
//! this build of the app cannot read.
//!
//! # The secret key
//!
//! Lives at `~/.config/opad/opad-manifest.key`, outside the repository,
//! and is never committed, printed or logged. Only this tool reads it, and
//! only through the `minisign` CLI — the key bytes never enter this process.
//!
//! **⚠ The current key is unencrypted** (it was cut with `minisign -G -W`
//! because `minisign -G` demands a passphrase interactively and no agent can
//! supply one). Stated plainly: *this key signs a remote code execution
//! channel into every user's machine*. Anyone who reads that one file can
//! publish a manifest the app will trust and install. It is acceptable only
//! while the repository is private, unpublished and has no users. **Re-cut it
//! with a passphrase before the repository is published** — the public half in
//! `verify.rs` changes with it, so this is a code change, not just a key swap.
//!
//! # Usage
//!
//! ```text
//! opad-manifest --dist dist --base-url https://example.invalid/download \
//!                 [--app-version 1.0.0-rc] [--firmware-version 1.0.0] \
//!                 [--tosu-version 4.26.2 --tosu-tag v4.26.2 \
//!                  --tosu linux-x86_64=/path/to/tosu] \
//!                 [--secret-key PATH] [--out PATH] [--no-sign]
//! ```

use opad_update::firmware::FIRMWARE_TARGET;
use opad_update::manifest::{
    Artifact, ArtifactKind, Component, ReleaseManifest, APP, FIRMWARE, SUPPORTED_SCHEMA, TOSU,
};
use opad_update::verify::{sha256_file, verify_manifest_signature};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST_NAME: &str = "opad-manifest.json";

fn main() {
    if let Err(e) = run() {
        eprintln!("opad-manifest: {e}");
        std::process::exit(1);
    }
}

struct Args {
    dist: PathBuf,
    base_url: String,
    app_version: String,
    firmware_version: Option<String>,
    tosu_version: Option<String>,
    tosu_tag: Option<String>,
    tosu: Vec<(String, PathBuf)>,
    secret_key: PathBuf,
    out: Option<PathBuf>,
    sign: bool,
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    let manifest = build_manifest(&args)?;
    if manifest.components.is_empty() {
        return Err(format!(
            "no artifacts found in {} — nothing to sign",
            args.dist.display()
        ));
    }

    let out = args
        .out
        .clone()
        .unwrap_or_else(|| args.dist.join(MANIFEST_NAME));
    // Pretty-printed with a trailing newline: the signature covers these exact
    // bytes, so the file must be written once and never reformatted.
    let mut bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    std::fs::write(&out, &bytes).map_err(|e| format!("writing {}: {e}", out.display()))?;

    for (name, component) in &manifest.components {
        println!(
            "  {name} {}: {} artifact(s)",
            component.version,
            component.artifacts.len()
        );
        for a in &component.artifacts {
            println!("    {:<16} {:?}  {}", a.target, a.kind, a.sha256);
        }
    }
    println!("manifest: {}", out.display());

    if !args.sign {
        println!("--no-sign: stopping before the signature");
        return Ok(());
    }
    sign(&out, &args.secret_key)?;

    // The point of re-verifying: a manifest this build cannot read is worse
    // than no manifest, because it fails at the user rather than here.
    let sig_path = signature_path(&out);
    let signature = std::fs::read_to_string(&sig_path)
        .map_err(|e| format!("reading {}: {e}", sig_path.display()))?;
    verify_manifest_signature(&bytes, &signature).map_err(|e| {
        format!("the signature just written does not verify against MANIFEST_PUBLIC_KEY: {e}")
    })?;

    println!("signature: {}", sig_path.display());
    println!("verified against the public key compiled into this build");
    Ok(())
}

fn signature_path(manifest: &Path) -> PathBuf {
    let mut s = manifest.as_os_str().to_os_string();
    s.push(".minisig");
    PathBuf::from(s)
}

/// Shells out to `minisign` rather than signing in-process, so the secret key
/// is only ever handled by the tool that owns that job.
fn sign(manifest: &Path, secret_key: &Path) -> Result<(), String> {
    if !secret_key.exists() {
        return Err(format!(
            "no secret key at {} — see the module docs; it is deliberately outside the repository",
            secret_key.display()
        ));
    }
    let status = Command::new("minisign")
        .arg("-S")
        .arg("-s")
        .arg(secret_key)
        .arg("-m")
        .arg(manifest)
        .arg("-x")
        .arg(signature_path(manifest))
        .arg("-t")
        .arg(format!("OPad release manifest {}", now_rfc3339()))
        .status()
        .map_err(|e| format!("could not run minisign (is it installed?): {e}"))?;
    if !status.success() {
        return Err("minisign refused to sign the manifest".to_string());
    }
    Ok(())
}

fn build_manifest(args: &Args) -> Result<ReleaseManifest, String> {
    let mut components: BTreeMap<String, Component> = BTreeMap::new();

    let app = collect(&args.dist, &args.base_url, app_artifact)?;
    if !app.is_empty() {
        components.insert(
            APP.to_string(),
            Component {
                version: args.app_version.clone(),
                upstream_tag: None,
                notes: None,
                artifacts: app,
            },
        );
    }

    let fw = collect(&args.dist, &args.base_url, firmware_artifact)?;
    if !fw.is_empty() {
        let version = args.firmware_version.clone().ok_or_else(|| {
            "a firmware image is present but --firmware-version was not given; \
             it is compared against what the pad reports, so it must not be guessed"
                .to_string()
        })?;
        components.insert(
            FIRMWARE.to_string(),
            Component {
                version,
                upstream_tag: None,
                notes: None,
                artifacts: fw,
            },
        );
    }

    if !args.tosu.is_empty() {
        let version = args
            .tosu_version
            .clone()
            .ok_or_else(|| "--tosu needs --tosu-version".to_string())?;
        let mut artifacts = Vec::new();
        for (target, path) in &args.tosu {
            let name = file_name(path)?;
            artifacts.push(artifact_at(
                target,
                ArtifactKind::Binary,
                path,
                &name,
                &args.base_url,
            )?);
        }
        components.insert(
            TOSU.to_string(),
            Component {
                version,
                upstream_tag: args.tosu_tag.clone(),
                // §T-3: the bundled binary is upstream's, relicensed to nobody
                notes: Some("Bundled tosu, LGPL-3.0; see licenses/tosu/NOTICE".to_string()),
                artifacts,
            },
        );
    }

    Ok(ReleaseManifest {
        schema: SUPPORTED_SCHEMA,
        generated: now_rfc3339(),
        components,
    })
}

/// Which artifacts a file name maps to, as `(target, kind)`.
///
/// Split out from the filesystem walk so the naming rules are unit-testable
/// without building a release.
fn app_artifact(name: &str) -> Option<(String, ArtifactKind)> {
    let lower = name.to_ascii_lowercase();
    if (lower.starts_with("opad-setup") || lower.starts_with("osupad-setup-"))
        && lower.ends_with(".exe")
    {
        return Some(("windows-x86_64".into(), ArtifactKind::Installer));
    }
    if lower.ends_with(".deb") {
        return Some(("linux-x86_64".into(), ArtifactKind::Deb));
    }
    if lower.ends_with(".rpm") {
        return Some(("linux-x86_64".into(), ArtifactKind::Rpm));
    }
    if (lower.starts_with("opad-linux-x86_64-") || lower.starts_with("osupad-linux-x86_64-"))
        && lower.ends_with(".tar.gz")
    {
        return Some(("linux-x86_64".into(), ArtifactKind::Archive));
    }
    // What an AppImage install (DirectReplace) swaps itself for
    if lower.ends_with(".appimage") {
        return Some(("linux-x86_64".into(), ArtifactKind::Binary));
    }
    None
}

fn firmware_artifact(name: &str) -> Option<(String, ArtifactKind)> {
    // The app image only. bootloader.bin, partition-table.bin and
    // ota_data_initial.bin are recovery-flash files a person writes by hand
    // (§W3-4); §U-3b never sends them over the wire.
    (name == "opad-firmware.bin" || name == "osupad-firmware.bin")
        .then(|| (FIRMWARE_TARGET.to_string(), ArtifactKind::Firmware))
}

fn collect(
    dist: &Path,
    base_url: &str,
    classify: fn(&str) -> Option<(String, ArtifactKind)>,
) -> Result<Vec<Artifact>, String> {
    let mut entries: Vec<_> = std::fs::read_dir(dist)
        .map_err(|e| format!("reading {}: {e}", dist.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    let mut out = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((target, kind)) = classify(&name) else {
            continue;
        };
        out.push(artifact_at(&target, kind, &entry.path(), &name, base_url)?);
    }
    Ok(out)
}

fn artifact_at(
    target: &str,
    kind: ArtifactKind,
    path: &Path,
    name: &str,
    base_url: &str,
) -> Result<Artifact, String> {
    let sha256 = sha256_file(path).map_err(|e| format!("hashing {}: {e}", path.display()))?;
    let size = std::fs::metadata(path).map(|m| m.len()).ok();
    Ok(Artifact {
        target: target.to_string(),
        kind,
        url: format!("{}/{}", base_url.trim_end_matches('/'), name),
        sha256,
        size,
    })
}

fn file_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} has no file name", path.display()))
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn parse_args() -> Result<Args, String> {
    let mut dist = None;
    let mut base_url = None;
    let mut app_version = env!("CARGO_PKG_VERSION").to_string();
    let mut firmware_version = None;
    let mut tosu_version = None;
    let mut tosu_tag = None;
    let mut tosu = Vec::new();
    let mut secret_key = None;
    let mut out = None;
    let mut sign = true;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| -> Result<String, String> {
            it.next().ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "--dist" => dist = Some(PathBuf::from(value("--dist")?)),
            "--base-url" => base_url = Some(value("--base-url")?),
            "--app-version" => app_version = value("--app-version")?,
            "--firmware-version" => firmware_version = Some(value("--firmware-version")?),
            "--tosu-version" => tosu_version = Some(value("--tosu-version")?),
            "--tosu-tag" => tosu_tag = Some(value("--tosu-tag")?),
            "--tosu" => {
                let spec = value("--tosu")?;
                let (target, path) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--tosu wants <target>=<path>, got {spec:?}"))?;
                tosu.push((target.to_string(), PathBuf::from(path)));
            }
            "--secret-key" => secret_key = Some(PathBuf::from(value("--secret-key")?)),
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--no-sign" => sign = false,
            "-h" | "--help" => {
                println!("{}", HELP);
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other:?}\n\n{HELP}")),
        }
    }

    Ok(Args {
        dist: dist.ok_or_else(|| format!("--dist is required\n\n{HELP}"))?,
        base_url: base_url.ok_or_else(|| format!("--base-url is required\n\n{HELP}"))?,
        app_version,
        firmware_version,
        tosu_version,
        tosu_tag,
        tosu,
        secret_key: secret_key.unwrap_or_else(default_secret_key),
        out,
        sign,
    })
}

const HELP: &str = "\
opad-manifest — build and sign the release manifest (§U-0.3)

  --dist <dir>              directory of built artifacts (required)
  --base-url <url>          URL prefix the artifacts are published under (required)
  --app-version <v>         default: this build's workspace version
  --firmware-version <v>    required when dist holds opad-firmware.bin
  --tosu-version <v>        version of the bundled tosu
  --tosu-tag <tag>          upstream tag, for the §T-3 source offer
  --tosu <target>=<path>    a tosu binary to record; repeatable
  --secret-key <path>       default: <config dir>/opad-manifest.key
  --out <path>              default: <dist>/opad-manifest.json
  --no-sign                 write the manifest but do not sign it
";

/// `~/.config/opad/opad-manifest.key`, honouring `$XDG_CONFIG_HOME`.
///
/// Resolved here rather than in `opad_model::paths`: that module is the
/// uninstaller's list of everything the *app* touches, and the signing key is
/// emphatically not part of the app — it never ships and nothing installed on
/// a user's machine may know where it lives.
fn default_secret_key() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("opad").join("opad-manifest.key")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_artifacts_are_classified_by_name() {
        assert_eq!(
            app_artifact("opad-setup.exe"),
            Some(("windows-x86_64".into(), ArtifactKind::Installer))
        );
        assert_eq!(
            app_artifact("opad-setup-1.0.0.exe"),
            Some(("windows-x86_64".into(), ArtifactKind::Installer))
        );
        assert_eq!(
            app_artifact("osupad-setup-1.0.0-rc.exe"),
            Some(("windows-x86_64".into(), ArtifactKind::Installer))
        );
        assert_eq!(
            app_artifact("opad_1.0.0_amd64.deb"),
            Some(("linux-x86_64".into(), ArtifactKind::Deb))
        );
        assert_eq!(
            app_artifact("osupad_1.0.0-rc_amd64.deb"),
            Some(("linux-x86_64".into(), ArtifactKind::Deb))
        );
        assert_eq!(
            app_artifact("opad-1.0.0-1.x86_64.rpm"),
            Some(("linux-x86_64".into(), ArtifactKind::Rpm))
        );
        assert_eq!(
            app_artifact("osupad-1.0.0_rc-1.x86_64.rpm"),
            Some(("linux-x86_64".into(), ArtifactKind::Rpm))
        );
        assert_eq!(
            app_artifact("opad-linux-x86_64-1.0.0.tar.gz"),
            Some(("linux-x86_64".into(), ArtifactKind::Archive))
        );
        assert_eq!(
            app_artifact("osupad-linux-x86_64-1.0.0-rc.tar.gz"),
            Some(("linux-x86_64".into(), ArtifactKind::Archive))
        );
        assert_eq!(
            app_artifact("opad-x86_64.AppImage"),
            Some(("linux-x86_64".into(), ArtifactKind::Binary))
        );
        assert_eq!(
            app_artifact("OPAD-X86_64.APPIMAGE"),
            Some(("linux-x86_64".into(), ArtifactKind::Binary))
        );
        // Not app artifacts, and in particular not silently classified as one
        assert_eq!(app_artifact("SHA256SUMS"), None);
        assert_eq!(app_artifact("osupad-firmware.bin"), None);
        assert_eq!(app_artifact("opad-firmware.bin"), None);
        assert_eq!(app_artifact("opad-daemon"), None);
    }

    #[test]
    fn only_the_app_image_is_a_firmware_artifact() {
        assert_eq!(
            firmware_artifact("opad-firmware.bin"),
            Some((FIRMWARE_TARGET.to_string(), ArtifactKind::Firmware))
        );
        assert_eq!(
            firmware_artifact("osupad-firmware.bin"),
            Some((FIRMWARE_TARGET.to_string(), ArtifactKind::Firmware))
        );
        // A recovery flash writes these by hand; an updater must never fetch
        // and write them, so they must not appear in the manifest at all.
        for name in [
            "bootloader.bin",
            "partition-table.bin",
            "ota_data_initial.bin",
        ] {
            assert_eq!(firmware_artifact(name), None, "{name} must not be offered");
        }
    }

    #[test]
    fn a_built_manifest_round_trips_and_finds_its_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let dist = dir.path();
        std::fs::write(dist.join("opad-firmware.bin"), b"firmware bytes").unwrap();
        std::fs::write(dist.join("osupad-setup-1.0.0-rc.exe"), b"installer bytes").unwrap();
        std::fs::write(dist.join("SHA256SUMS"), b"ignored").unwrap();

        let args = Args {
            dist: dist.to_path_buf(),
            base_url: "https://example.invalid/download/".into(),
            app_version: "1.0.0-rc".into(),
            firmware_version: Some("1.0.0".into()),
            tosu_version: None,
            tosu_tag: None,
            tosu: Vec::new(),
            secret_key: PathBuf::new(),
            out: None,
            sign: false,
        };
        let manifest = build_manifest(&args).unwrap();

        // Serialise and parse back through the client's own type
        let json = serde_json::to_vec_pretty(&manifest).unwrap();
        let parsed: ReleaseManifest = serde_json::from_slice(&json).unwrap();
        assert_eq!(parsed.schema, SUPPORTED_SCHEMA);

        let fw = parsed
            .artifact_for(FIRMWARE, FIRMWARE_TARGET, Some(ArtifactKind::Firmware))
            .expect("the firmware artifact must be reachable the way U-3b looks it up");
        assert_eq!(
            fw.sha256,
            opad_update::verify::sha256_bytes(b"firmware bytes")
        );
        assert_eq!(fw.size, Some(14));
        // The trailing slash on base_url must not double up
        assert_eq!(fw.url, "https://example.invalid/download/opad-firmware.bin");

        let app = parsed
            .artifact_for(APP, "windows-x86_64", Some(ArtifactKind::Installer))
            .expect("the installer must be reachable");
        assert_eq!(
            app.sha256,
            opad_update::verify::sha256_bytes(b"installer bytes")
        );
        assert!(parsed.component(TOSU).is_none());
    }

    #[test]
    fn a_firmware_image_without_a_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("opad-firmware.bin"), b"x").unwrap();
        let args = Args {
            dist: dir.path().to_path_buf(),
            base_url: "https://example.invalid".into(),
            app_version: "1.0.0-rc".into(),
            firmware_version: None,
            tosu_version: None,
            tosu_tag: None,
            tosu: Vec::new(),
            secret_key: PathBuf::new(),
            out: None,
            sign: false,
        };
        assert!(build_manifest(&args)
            .unwrap_err()
            .contains("--firmware-version"));
    }

    #[test]
    fn the_signature_sits_beside_the_manifest() {
        assert_eq!(
            signature_path(Path::new("/tmp/dist/opad-manifest.json")),
            PathBuf::from("/tmp/dist/opad-manifest.json.minisig")
        );
    }
}
