# Code review: `desktop/crates/opad-update`

- Date: 2026-09-23
- Command: `/code-review high desktop/crates/opad-update`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched this crate, so the review covers the crate as a whole plus its callers in `daemon/src/updater.rs`, `daemon/src/firmware_update.rs`, `scripts/release/build_release.sh` and `packaging/linux/appimage/build_appimage.sh`.
- Verification: findings 1–3 were confirmed by the reviewer against the daemon/release code; the rest were not re-verified.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `fetch_manifest` sends the persisted ETag with no cached manifest body — `fixed` (confirmed)

**File:** `desktop/crates/opad-update/src/client.rs:63`
**Category:** correctness

`fetch_manifest` sends the persisted ETag even when the caller holds no manifest body, so a 304 after a daemon restart leaves all three updaters with no manifest until upstream republishes.

**Failure scenario:** Daemon checks once, persists `CheckSchedule { etag: Some(..) }` to storage, is restarted (manifest lives only in the in-memory `SharedManifest`, which starts as `None`). Next due tick sends `If-None-Match` → server answers 304 → `Ok(None)` → `updater.rs` logs "unchanged" and keeps `manifest = None`. Firmware offers, app Install ("No verified release manifest yet"), and tosu updates are all dead until the manifest bytes change on GitHub.

**Suggested fix:** Only send `If-None-Match` when the caller has a cached manifest (e.g. pass `Option<&ReleaseManifest>` / a `have_cached` flag, or clear `schedule.etag` when none is held).

## 2. `tosu::install` writes VERSION before the binary rename — `fixed` (confirmed)

**File:** `desktop/crates/opad-update/src/tosu.rs:137`
**Category:** correctness

`install()` writes VERSION before the binary rename, so a failed `install_to` leaves VERSION naming a version that is not on disk and `plan()` reports `UpToDate` forever.

**Failure scenario:** Windows: supervisor pause does not fully release `tosu.exe` (or AV holds it) → `std::fs::rename` in `install_to` fails with a sharing violation → `install()` returns Err, but VERSION already says 4.2.0 while `tosu.exe` is 4.1.0. Next check: `installed_version() == component.version` → `TosuAction::UpToDate`; the update is never retried and status shows 4.2.0. The doc comment ("the binary goes in last... if anything fails before that") is false for the rename step itself.

**Suggested fix:** Write VERSION/NOTICE after `install_to` succeeds, or roll them back on Err.

## 3. `opad-manifest` has no rule for `*.AppImage`, so AppImage installs never update — `fixed` (confirmed)

**File:** `desktop/crates/opad-update/src/bin/opad-manifest.rs:248`
**Category:** correctness

`app_artifact()` has no rule for `*.AppImage`, yet the release ships `dist/opad-x86_64.AppImage` and `app::wanted_kind(DirectReplace, AppImage)` demands `ArtifactKind::Binary`.

**Failure scenario:** `build_release.sh` runs `opad-manifest` over `dist/` containing `opad-x86_64.AppImage`; it is skipped, so the app component has no `linux-x86_64/binary` artifact. On an AppImage install (marker `appimage`), `app::plan` returns `UpdateError::NoArtifact` on every daily check; `check_app` logs it to `last_error` and `s.app.available` stays None — the user is not even notified.

**Suggested fix:** Add an `.appimage` → (`linux-x86_64`, `Binary`) rule and a test.

## 4. `fetch_artifact` round-trips in-memory bytes through a predictable shared temp path to hash them — `fixed`

**File:** `desktop/crates/opad-update/src/client.rs:147`
**Category:** correctness / security

`fetch_artifact` hashes the in-memory bytes by round-tripping them through a predictable, attacker-preparable path in the shared temp dir instead of calling `sha256_bytes`.

**Failure scenario:** On Linux `/tmp` is shared. Another local user pre-creates `/tmp/opad-update-<pid>/` (pids are guessable/enumerable) with `artifact` as a symlink to a file the daemon's user can write; `create_dir_all` succeeds on the existing dir and `std::fs::write` follows the symlink, clobbering that file with up to 512 MiB of artifact bytes. Even without an attacker it is wasted I/O: the artifact is written+read here, then written+read again by `stage_bytes`.

**Suggested fix:** The bytes are already in memory — compare `sha256_bytes(&bytes)` against `artifact.sha256` (add a bytes variant of `check_hash`) and delete `tempdir_for_check`.

## 5. `tosu::plan` treats any version-string difference as "install" — `fixed`

**File:** `desktop/crates/opad-update/src/tosu.rs:71`
**Category:** correctness

`tosu::plan` treats any string difference from the manifest version as "install", ignoring `is_newer`, so a rolled-back manifest downgrades tosu and any cosmetic mismatch reinstalls it every day.

**Failure scenario:** Manifest tosu version is rolled back from 4.2.0 to 4.1.0 after a bad upstream release → every owned install (Windows/User/AppImage) silently downgrades — the exact scenario `version.rs` says must not happen. Separately, a VERSION file written as `v4.2.0` by a packager, or a manifest version `v4.2.0`, never equals `4.2.0`, so the binary is re-downloaded, tosu paused and swapped on every daily check.

**Suggested fix:** Use `is_newer` (or at least normalise both sides) like `app.rs` and `firmware.rs` do.

## 6. Pre-release identifiers compared as whole strings; `+` metadata treated as pre-release — `fixed`

**File:** `desktop/crates/opad-update/src/version.rs:37`
**Category:** correctness

Pre-release identifiers are compared as whole strings, so numeric suffixes sort lexicographically and `+` build metadata is treated as a pre-release.

**Failure scenario:** `is_newer("1.0.0-rc10", "1.0.0-rc9") == false` because `"rc10" < "rc9"` as strings: users on rc9 are never offered rc10 (the project is currently at 1.0.0-rc and iterating). `is_newer("1.0.0-rc.10", "1.0.0-rc.9")` is likewise false. And `split_once(['-','+'])` makes `"1.0.0+build"` rank below `"1.0.0"`, so a build-metadata-tagged install is offered a "downgrade".

**Suggested fix:** Split pre-release on `.`, compare numeric identifiers numerically, and drop everything after `+`.

## 7. Comment claims `require_https` is re-checked after redirects; it is not — `fixed`

**File:** `desktop/crates/opad-update/src/http.rs:43`
**Category:** correctness (documentation)

The comment claims `require_https` is re-checked on the final URL after redirects, but it is only called on the initial URL; only reqwest's `https_only` enforces the redirect case.

**Failure scenario:** A reader auditing the "HTTPS-only" guarantee relies on a check that does not exist; if `https_only(true)` is ever loosened (e.g. to allow a local plain-HTTP test manifest via `OPAD_MANIFEST_URL`), an https→http redirect would be followed silently.

**Suggested fix:** Either fix the comment or actually check `resp.url().scheme()` after `send()`.

## 8. Atomic-write sequence duplicated in `backup.rs`; VERSION/NOTICE not written atomically — `fixed`

**File:** `desktop/crates/opad-update/src/tosu.rs:136`
**Category:** reuse

NOTICE and VERSION are written with plain `fs::write` while `daemon/src/backup.rs` re-implements `stage_bytes`' temp+fsync+rename+dir-sync by hand; a shared hash-less atomic writer in `download.rs` would serve both and remove the duplicate.

**Failure scenario:** `backup.rs::write` copies the `.{name}.incoming` + `sync_all` + rename + parent-dir sync sequence verbatim (it even cites `stage_bytes`), so a future fix to one (e.g. a Windows rename quirk) will miss the other; meanwhile tosu's VERSION/NOTICE get none of that protection, and a crash mid-write leaves a truncated VERSION (`4.`) that reads as an unknown version.

**Suggested fix:** Extract `write_atomic(target, bytes)` in `download.rs`, have `stage_bytes` and `backup.rs` call it, and use it for VERSION/NOTICE.
