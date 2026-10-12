# OPad — Releases, Update Feed and Updaters

**Status:** draft for review (2026-10-12). Nothing new here is implemented yet;
§2 and the "today" columns say what already exists.
**Amends:** `docs/specs/packaging-distribution.md` §7 L-0 (GitHub releases
only), §9 U (updates) and §4 W2-2 (code signing). Where the two disagree, this
document wins once it is accepted.

---

## 1. Goal

A release and update system that lasts for years: new app versions, new
firmware, a new tosu, a new board or a new distro are added to it, not
redesigned into it.

What a user should experience:

1. They install OPad once.
2. OPad keeps itself up to date with as little intervention as possible, from
   a button in the app.
3. OPad notices when the connected pad has newer firmware and offers it.
4. They update the pad from the app, over USB.
5. **A failed update never leaves the app or the pad unusable.** The pad stays
   a keyboard (AGENTS.md §1.2) and is never recovered with the BOOT or RESET
   buttons (§1.5).
6. Adding a version, a dependency, a distro or a board does not change the
   architecture.

**What this does not promise.** No design can guarantee the updater never needs
changing, or that a pad can never fail. This one makes failure rare, makes
every failure recoverable over USB, and keeps the contracts (feed format,
device protocol, install-origin markers) stable and versioned so they survive
several generations of app, firmware and hardware.

---

## 2. Architecture

```
            Git repository + GitHub Actions
       build · test · (sign) · publish · attest
                         │
          ┌──────────────┼───────────────┐
          ▼              ▼               ▼
     App releases   Firmware releases  tosu releases      ← GitHub Releases
       (v*)          (firmware-v*)      (tosu-v*)            immutable files
          └──────────────┬───────────────┘
                         ▼
         Signed update feed + apt/dnf repos               ← GitHub Pages
          stable.json · beta.json · dists/ · repodata/
                         │
                         ▼
          Common update engine (opad-update, in the daemon)
   fetch · verify signature · check freshness · pick what applies
   · compatibility · rollout · download · verify hash · stage
                         │
      ┌──────────────────┼─────────────────────┐
      ▼                  ▼                     ▼
 App installers      Firmware flasher       tosu updater
 (per install origin) (USB, A/B slots)       (atomic swap)
```

**The key idea:** one common engine decides *what* to update, *whether it is
allowed* and *whether the bytes are genuine*. Small per-platform installers
only know *how* to put verified bytes in place. No platform re-implements the
trust logic.

### The pieces

| Piece | Responsibility | Lives in | Today |
|---|---|---|---|
| **Release system** | Build, test, sign and publish each track | `.github/workflows/`, `scripts/release/` | one `release.yml` for everything |
| **Update feed** | Say which versions exist, for whom, and their hashes; signed | GitHub Pages | one `opad-manifest.json` per release (§5.1) |
| **Update engine** | Fetch, verify, decide, download, stage | `desktop/crates/opad-update/` | exists; schema 1 |
| **App installers** | Apply an app update the way its install origin requires | `opad-update/src/app.rs`, `desktop/daemon/src/updater.rs` | exists |
| **Firmware flasher** | Put a verified image on the pad without stranding it | `desktop/daemon/src/firmware_update.rs`, `desktop/crates/opad-device/src/flash.rs` | exists |
| **tosu manager** | Pin, build, ship and supervise the third-party tosu | `opad-update/src/tosu.rs`, `desktop/crates/opad-tosu/`, `Makefile` (`make tosu`) | exists, but no release ships a tosu file |

**Central rule:** OPad never installs anything just because a server says a
new version exists. Every update needs (1) a valid signature over the feed,
(2) a fresh feed, (3) a matching SHA-256 for the file, and (4) a
compatibility check against this machine and this pad.

---

## 3. Versions and compatibility

### 3.1 Everything has its own version

| Contract | Version today | Where | Changes when |
|---|---|---|---|
| App (daemon, GUI, `opadctl`) | `1.0.0-rc` | `desktop/Cargo.toml` | app release |
| Firmware | `1.0.0` | `firmware/CMakeLists.txt` (`PROJECT_VER`) | firmware release |
| App ↔ pad protocol | `PROTOCOL_VERSION_V1` | `protocol/osupad.proto` | only on an incompatible wire change |
| Daemon ↔ GUI IPC | `1` | `IPC_PROTOCOL_VERSION`, `desktop/crates/opad-ipc/src/lib.rs:21` | only on an incompatible IPC change |
| Update feed format | schema `1` → `2` | `SUPPORTED_SCHEMA`, `opad-update/src/manifest.rs` | only on an incompatible feed change |
| tosu | `4.26.2` | `TOSU_VERSION` (pin, AGENTS.md §5) | when we bump the pin |
| Bootloader, partition table | fixed | `firmware/partitions.csv` | never through an update (§7.4) |

The app and the firmware never share a version number. An app release does
not need a firmware release, and the other way round (AGENTS.md §1.3).

### 3.2 Compatibility is negotiated, not assumed

SemVer says how big a change is. It does not decide compatibility; the
protocol does. The pad already announces what it is and what it can do in
`HelloAck` (`protocol/osupad.proto:37`): `protocol_version`, `board_profile`,
`running_partition` and capability flags such as `board_test`, where a field
missing in older firmware reads as false and the host simply hides that
feature.

Rules, which the repository already follows and this document makes binding:

1. A compatible change **adds** a capability; it never removes or changes an
   old one.
2. An incompatible change gets a **new protocol version**, and both sides keep
   speaking the old one too (the pad answers in the framing the host used).
3. An unknown capability is **ignored** safely by both sides.
4. The firmware keeps the minimum needed to be flashed again (Hello, info,
   reboot to download) in **every** version.
5. The updater **refuses** combinations the feed does not allow (`min_app`,
   `board`, §5.2).

Because of rules 1–3, the feed's `min_app` for firmware is normally empty. It
exists for the one case where a firmware truly cannot be driven by an older
app: then that app shows "update OPad first" instead of offering the firmware.
That is a two-step migration (app, then firmware), never "update both
together".

---

## 4. Release system

### 4.1 Three release tracks

One repository, three tracks, each with its own tag prefix and
[SemVer](https://semver.org) version. This is the standard layout for a
monorepo with several products.

| Track | Tag | Release contains | GitHub "Latest" |
|---|---|---|---|
| **App** | `v1.2.0` | `opad-setup-1.2.0.exe`, `.deb`, `.rpm`, `.AppImage`, `.tar.gz`, `THIRD_PARTY_NOTICES.html`, `opad-bin` PKGBUILD + `.SRCINFO`, `SHA256SUMS` | **yes** |
| **Firmware** | `firmware-v1.1.0` | `opad-firmware.bin`, `bootloader.bin`, `partition-table.bin`, `ota_data_initial.bin`, notices, source bundle (§13), `SHA256SUMS` | no |
| **tosu** | `tosu-v4.26.2-r1` | `tosu-linux-x86_64`, `tosu-windows-x86_64.exe`, tosu `NOTICE`, `THIRD_PARTY_NOTICES.txt`, LGPL source archive, `SHA256SUMS` | no |

- The app keeps the plain `v` tag so GitHub's "Latest release" is always what
  users download.
- Pre-releases (`1.2.0-rc.1`, `firmware-v1.1.0-beta.1`) are marked
  pre-release and only reach the beta channel.
- tosu's `-r1` is our build revision of an upstream version. The feed carries
  `version` and `revision` separately; the tag is only a name.
- The app installers still **bundle** tosu, so a fresh install works offline.
  The tosu track only lets tosu move between app releases.

### 4.2 Versions and changelogs: release-please

[release-please](https://github.com/googleapis/release-please) in multi-component
mode, driven by the conventional commits this repository already uses
(AGENTS.md §3):

- It keeps one open PR per component ("chore: release app 1.2.0") with the
  version bump and a changelog from the commits since that component's last
  release.
- Merging the PR creates the tag and a draft release; the build workflow
  attaches the files and publishes.
- Components by path: **app** = `desktop/`, `packaging/`, `scripts/release/`;
  **firmware** = `firmware/`. A `protocol/` change regenerates files in both
  trees, so it appears in both changelogs.
- `fix` → patch, `feat` → minor, `!` / `BREAKING CHANGE` → major; `chore`,
  `docs`, `test`, `ci` do not release.
- tosu is released by hand when the pin is bumped (§8).

### 4.3 Workflows

| Workflow | Trigger | Does |
|---|---|---|
| `ci.yml` | push, PR | As today (format, clippy, tests on Linux and Windows, firmware build, protobuf drift, licences), plus current action versions, ESP-IDF caching, and a SemVer check of both version files |
| `release-please.yml` | push to `main` | Keeps release PRs current; on merge, tags and calls the build workflow |
| `release-app.yml` | from release-please, or manual | Linux job (glibc ≤ 2.31 packages, AppImage, tarball, clean-distro install tests: today's `release.yml` steps) and Windows job (app, `tosu.exe`, Inno installer); checks the tag matches `desktop/Cargo.toml`; signing steps (off until §19); publish; call the feed |
| `release-firmware.yml` | from release-please, or manual | ESP-IDF v5.5.2 build, source bundle, publish (not Latest), call the feed |
| `release-tosu.yml` | manual, with the revision | tosu for Linux (glibc ≤ 2.28, `make tosu`) and Windows, sources, notices, publish (not Latest), call the feed |
| `update-feed.yml` | called after every publish, or manual | Builds `stable.json`/`beta.json` and the apt/dnf repos from the published releases, **waits for your approval**, signs, deploys to Pages |

`release-verify.yml` and `sign_release.sh --tag` are folded into these; the
checks they do run before publishing instead of after.

Implementation notes: a release published with `GITHUB_TOKEN` does not fire
`release: published` for other workflows, so the release workflows call
`update-feed.yml` directly (`workflow_call`). release-please's draft-plus-tag
behaviour must be confirmed against its current version in phase 1.

### 4.4 Release pipeline

1. Format, lint, build, unit tests (`ci.yml` must be green on the commit).
2. Build every artifact of the track.
3. Install tests on clean systems: Ubuntu 22.04/24.04, Debian 12/13, Rocky 9,
   Fedora (`scripts/release/test_packages.sh`); glibc checks.
4. Third-party notices, SBOM.
5. Signing (Windows via SignPath; off until §19).
6. Build provenance attestations.
7. Publish (pre-release → beta only; final → stable).
8. Feed: rebuild, **approve**, sign, deploy.

Building, approving and signing are separate steps: no build job ever sees a
signing key.

---

## 5. Update feed and trust

### 5.1 Why the current manifest must change

Today the app reads `releases/latest/download/opad-manifest.json`
(`DEFAULT_MANIFEST_URL`, `opad-update/src/client.rs`). With three tracks,
"latest" is only ever the app release, so firmware and tosu releases would be
invisible, and a manifest copied into each release goes stale the moment
another track releases.

### 5.2 The feed

A **signed feed per channel**, made of static files on GitHub Pages, separate
from the releases (as Sparkle's appcast, Tauri's `latest.json` and
electron-updater's `latest.yml` work). No server runs code; any static host or
CDN can serve it.

```
https://opad-dev.github.io/OPad/updates/stable.json   (+ .minisig)
https://opad-dev.github.io/OPad/updates/beta.json     (+ .minisig)
https://opad-dev.github.io/OPad/apt/…   https://opad-dev.github.io/OPad/rpm/…
```

```json
{
  "schema": 2,
  "channel": "stable",
  "serial": 42,
  "generated": "2026-11-02T10:00:00Z",
  "expires":   "2026-12-02T10:00:00Z",
  "components": {
    "app": {
      "version": "1.2.0",
      "tag": "v1.2.0",
      "notes_url": "https://github.com/OPad-dev/OPad/releases/tag/v1.2.0",
      "rollout": { "percent": 100 },
      "artifacts": [
        { "target": "windows-x86_64", "kind": "installer",
          "url": "https://github.com/OPad-dev/OPad/releases/download/v1.2.0/opad-setup-1.2.0.exe",
          "sha256": "…", "size": 41234567 },
        { "target": "linux-x86_64", "kind": "deb", "url": "…/v1.2.0/opad_1.2.0_amd64.deb", "sha256": "…" }
      ]
    },
    "firmware": {
      "version": "1.1.0",
      "tag": "firmware-v1.1.0",
      "min_app": null,
      "rollout": { "percent": 10 },
      "artifacts": [
        { "target": "esp32s3", "board": "waveshare-esp32s3-touch-lcd-2",
          "kind": "firmware", "url": "…/firmware-v1.1.0/opad-firmware.bin", "sha256": "…" }
      ]
    },
    "tosu": {
      "version": "4.26.2", "revision": 1, "tag": "tosu-v4.26.2-r1",
      "upstream_tag": "v4.26.2",
      "artifacts": [ "…" ]
    }
  }
}
```

| Field | Protects against / enables |
|---|---|
| signature (`.minisig`) | a forged or modified feed (verified over the exact bytes before parsing, as `client.rs` does today) |
| `channel` | a beta feed served as stable |
| `serial` | **replay and downgrade**: the client stores the highest serial it accepted and rejects lower ones |
| `expires` | **freeze**: a feed stuck in the past shows "could not check", never "up to date"; the feed is republished at least monthly |
| per-tag `url` | files changing under a feed: release files are immutable, the feed never points at "latest" |
| `sha256`, `size` | a corrupted or swapped download (checked before the file can be executed; size limits as in `http.rs`) |
| `board` | firmware for the wrong board; compared with `board_profile` from `HelloAck` |
| `min_app` | firmware the installed app cannot drive (§3.2) |
| `rollout` | gradual release (§9.2) |

### 5.3 Keys

A scaled-down version of [TUF](https://theupdateframework.io)'s ideas, sized
for one maintainer:

| Key | Signs | Kept | Rotation |
|---|---|---|---|
| **Feed key** (minisign, ed25519, with passphrase) | `stable.json`, `beta.json` | GitHub secret in the `release` environment (requires your approval) + offline backup in a password manager | the app trusts a list of keys; a new key ships in an update signed with the old one (§5.4) |
| **Repository key** (GPG) | apt `InRelease`, dnf `repomd.xml` | same | published key file; distros handle rotation |
| **Windows code signing** | binaries and `opad-setup.exe` | SignPath Foundation's HSM | handled by SignPath |
| **Provenance** | every release file | none: GitHub OIDC (`actions/attest-build-provenance`) | — |
| **Test key** (minisign, no passphrase) | beta and local feeds until final signing | anywhere; not secret in practice | removed in the rotation release |

The full four-role TUF (root, targets, snapshot, timestamp) is the possible
next step if OPad ever has several maintainers or a commercial fleet; schema 2
leaves room for it (unknown fields are ignored, `schema` gates real changes).

### 5.4 No real signing until the end (owner decision, 2026-10-12)

Phases 1–5 sign nothing with a real key. Every signing step is built, but each
one is switched off until you set it up following §19; nothing in phases 1–5
asks you for a key, a passphrase or an account.

The updater still needs *some* signature, because it refuses unsigned feeds
by design (§2, central rule), and that refusal is worth testing rather than
switching off. So until §19:

- A **test key** is used. Both its halves are committed to the repository
  (`desktop/crates/opad-update/test-key/`), so it protects nothing: it
  only makes the feed well-formed so the real code path runs. It replaces the
  current key in `opad-update/src/verify.rs:34` in phase 3.
- Every release made this way is a **pre-release** on the **beta** feed only.
  Nothing reaches the stable feed until §19 is done.

When the real key exists (§19):

1. A release that trusts **both** keys is published, with its feed signed by
   the test key.
2. After it has been out long enough, a release trusting only the real key
   follows.

Rotating through an update signed with the old key is the standard way to
change an updater key without stranding installs. **Losing every trusted key
cannot be fixed remotely**: users would reinstall by hand. That is why the
real key has an offline backup.

---

## 6. App updater

### 6.1 Install origin decides how; the engine decides what

Each package writes an `install-origin` marker (`opad-update/src/origin.rs`;
written by `scripts/release/build_packages.sh`, the `Makefile`, the
PKGBUILDs and `packaging/windows/installer.iss`). The engine never guesses:
an unknown marker means notify only.

| Origin | *Install* does | Prompt |
|---|---|---|
| **Windows** | runs the new `opad-setup.exe /SILENT /NORESTART`; Inno closes GUI, daemon and tosu (`CloseApplications`, `installer.iss:72`) and starts the daemon (`[Run]`). The GUI entry is `skipifsilent`, so the installer gets a flag the updater passes to relaunch it. **The only update path on Windows.** | none (per-user install, `PrivilegesRequired=lowest`) |
| **deb, our apt repo configured** | upgrades from the repo through PackageKit (`pkcon update opad`), falling back to `pkexec apt-get install --only-upgrade opad` | one password prompt |
| **rpm, our dnf repo configured** (Fedora, Nobara, RHEL) | same through PackageKit, fallback `pkexec dnf upgrade opad` | one password prompt |
| **downloaded `.deb` / `.rpm`** | `pkexec apt-get install ./file.deb` / `pkexec dnf install ./file.rpm` (today) | one password prompt |
| **AppImage** | replaces the AppImage file (today) | none |
| **`make install-user` / tarball** | extracts over the prefix (today) | none |
| **AUR** | shows the version, notes and `yay -Syu` / `paru`; pacman owns the files | — |
| **unknown** | shows the version only | — |

**Never** write files a package manager owns (`/usr/bin/…`) directly: `dpkg -V`
would report them modified and the next upgrade would revert them
(packaging-distribution §U-2).

### 6.2 State machine

The states an update goes through. What is persisted (written only while
IDLE, P1-3) is the part a crash or a restart must not lose: an install marked
in progress before it is applied (`update.in_progress`) and how the last one
went (`update.last_result`), in `desktop/daemon/src/updater.rs`. A daemon that
starts with an install still marked in progress reports it from the version
it is now running: the new one means it worked (the Windows installer stops
the daemon on purpose), the old one means it was interrupted. Everything
else is recomputed from the feed at the next check.

```
 Idle ──check──▶ Checking ──▶ UpToDate
                    │
                    ▼
                Available ──(rollout/channel/min_app say no)──▶ Idle
                    │ user presses Install (or auto, if opted in)
                    ▼
               Downloading ──hash mismatch / too big──▶ Failed
                    │
                    ▼
                 Staged  (verified file in the state dir, not /tmp)
                    │ daemon IDLE
                    ▼
                Applying  (installer / package manager / file swap)
                    │                    │
                    ▼                    ▼
           RestartPending            Failed (running version untouched,
                    │                 update stays Available)
                    ▼
                  Done
```

In place: size limits, atomic writes with fsync and rename
(`opad-update/src/download.rs`), exponential backoff with a cap
(`schedule.rs`), IDLE-only checks and applies (`gate.rs`), staging outside
`/tmp`, one update at a time (the worker handles one command after another),
and the in-progress / last-result records above. **Not yet:** resumable
downloads (HTTP Range) and a free-space check before staging.

**No privilege escalation from the network.** The only privileged commands are
the fixed ones in the table above (`app::apply_command`); nothing in a feed can
add or change a command.

### 6.3 Automatic install

Opt-in, off by default, Windows and AppImage only, still IDLE-only and still
subject to rollout. Everywhere else a person presses *Install*. **Not built
yet** (open question 3): today every app update waits for *Install*.

---

## 7. Firmware updater

The most delicate part: it is the only update that can stop the pad being a
keyboard.

### 7.1 What exists

The pad has two app slots (`firmware/partitions.csv`: `ota_0`, `ota_1`).
`opad-device/src/flash.rs` writes the new image to the slot the pad is **not**
running, then `otadata` to select it for one trial boot. The new firmware must
configure USB with the host within 20 s (`IMAGE_CONFIRM_TIMEOUT_US`,
`firmware/main/app_main.c:30`) or it restarts and the bootloader rolls back
(`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE`, `firmware/sdkconfig.defaults:92`).
NVS (lifetime counters, owner) is never erased.

### 7.2 Flow

1. **Identify the pad:** `HelloAck` gives firmware version, protocol,
   `board_profile`, running slot and capabilities.
2. **Choose:** the feed's firmware entry for this channel, matching `board`,
   honouring `min_app` and rollout.
3. **Download and verify:** SHA-256 from the signed feed, then the ESP32-S3
   chip ID in the image header, then "newer than installed"
   (`firmware_update.rs`).
4. **Prepare:** explicit consent every time (`consent_text`, about
   `TYPICAL_OUTAGE_SECONDS` without a keyboard), daemon IDLE, counters synced
   to the PC, serial port released.
5. **Write:** reboot into the ROM bootloader over USB, write the inactive slot,
   then `otadata`. Never the bootloader, never the partition table, never
   `erase-flash`.
6. **Boot and confirm:** the pad boots the new slot; the firmware confirms
   itself only once the host has configured USB.
7. **Check:** the daemon waits for the pad (`RECONNECT_TIMEOUT`, 45 s) and
   requires it to report the version just written. If not, it says so loudly
   and links `docs/recovery.md`.

### 7.3 What happens when it goes wrong

| Incident | What happens | User action |
|---|---|---|
| Download corrupt or tampered | rejected before the pad is touched | none |
| Image for another chip or board | rejected (chip ID, `board`) before the pad is touched | none |
| USB unplugged while writing the image | `otadata` not written yet; the old slot still boots | replug |
| Power lost while writing `otadata` | two-sector `otadata` with sequence numbers and CRC: the bootloader boots the old slot or the new one on trial | replug |
| New firmware crashes or hangs | no USB configuration within 20 s → restart → rollback to the old slot | none |
| New firmware boots but USB never works | same rollback | none |
| OPad closes or the PC sleeps mid-flash | same as unplugging | replug; reopen OPad |
| Pad stuck in download mode / does not enumerate | the ROM bootloader is in mask ROM and cannot be erased | `opadctl recover`, then replug (`docs/recovery.md` §7.6–7.7). **Never** BOOT/RESET |
| Counters | synced before flashing, NVS never written by the flasher | none |

### 7.4 Firmware rules

- **The bootloader and partition table are never updated by the updater.**
  Changing them needs a full serial reflash; it is a recovery-time operation.
- **No Secure Boot and no anti-rollback eFuses.** Both are irreversible, and
  locking pads to our key would stop owners installing their own firmware,
  which GPL-3.0 requires for consumer devices. Integrity is checked by the
  host against the signed feed.
- **The pad never updates itself, and an app update never flashes it**
  (`updater.rs` refuses `UpdateComponent::Firmware`).

### 7.5 Possible evolution

Streaming the image over the normal CDC protocol into the inactive slot while
the app keeps running, so the keyboard stays alive during the transfer
(packaging-distribution §U-3c), with numbered, CRC-checked chunks and resume.
It needs firmware work, OTA writes only while IDLE (flash writes stall the
cache, AGENTS.md §1.1), and a latency re-test. Not in this plan's phases.

---

## 8. tosu (the third-party dependency)

tosu can stop being maintained, change its API, ship a vulnerability or stop
building. OPad is designed so that none of those breaks the product.

**Isolation (exists):** tosu runs as a separate, supervised process
(`desktop/crates/opad-tosu/`); OPad talks to it over its local WebSocket API.
That process boundary is OPad's adapter: if tosu's API changes, only
`opad-tosu` changes. Without tosu, the pad is still a full keyboard and every
touch is Quick Retry (AGENTS.md §1.4).

**Never "latest upstream":** OPad ships one pinned tosu version, built from
source by us (`make tosu`, glibc ≤ 2.28 on Linux), with its notices and LGPL
source. A new upstream version goes through:

```
new upstream tag
   → read the changelog and licence
   → bump TOSU_VERSION, build with release-tosu.yml
   → test against osu! (lazer and stable) with OPad
   → tosu-v…-r1 to beta → stable
```

**If tosu disappears:** keep shipping the last version its LGPL licence lets
us redistribute, fork it if fixes are needed, or replace the adapter.

---

## 9. Channels, rollout and pulling releases

### 9.1 Channels

| Channel | Gets | Who |
|---|---|---|
| **stable** | final releases, with gradual rollout | everyone, default |
| **beta** | the newest of final or pre-release per component; no rollout | opt-in in Settings |

A beta user is never offered something older than stable. Going back from beta
to stable never downgrades; it waits until stable passes the installed
version. A nightly channel is not planned.

### 9.2 Gradual rollout

Each stable entry carries `rollout.percent`. The app decides locally whether
it is in: `hash(install_id, component, version) mod 100 < percent`, where
`install_id` is the random ID each install already has
(`desktop/daemon/src/identity.rs`). Nothing is sent anywhere: the feed is
the same file for everyone, and the server learns nothing about who updated.
The same install always lands in the same bucket for a given version.

Typical sequence: 10 % → (a few days, no reports) → 50 % → 100 %.

### 9.3 Pausing and pulling

- **Pause:** set `percent` to 0 and republish the feed.
- **Pull (yank):** list it in `updates/policy.json` (`"yanked": ["app 1.2.0"]`)
  and run `update-feed.yml` by hand. It stops being offered at the next check.
  The same file holds rollouts (`"rollout": {"app 1.3.0": 10}`) and
  `min_app` (`"min_app": {"firmware 2.0.0": "1.5.0"}`).
- Neither undoes installs that already happened. The answer to a bad release
  is a fixed release (fix forward); nobody is downgraded automatically.

---

## 10. Distribution channels

### 10.1 Linux

| Distro | Install | Updates |
|---|---|---|
| Ubuntu, Debian, Mint, Pop!_OS | add the OPad apt repo, `apt install opad` | app button, or `apt upgrade` |
| Fedora, Nobara, RHEL/Rocky | add the OPad dnf repo, `dnf install opad` | app button, or `dnf upgrade` |
| Arch | AUR `opad-bin` (release binaries) and `opad-git` (from source), with `provides`/`conflicts` | `yay`/`paru`; the app notifies |
| anything else | AppImage | app button |
| any | `.deb`/`.rpm` from the release page | app button |

- One `.deb` and one `.rpm` cover all of these, being built against glibc 2.31.
- The repos are static files built by `update-feed.yml` (apt: `dists/` +
  `pool/` + signed `InRelease`; dnf: `repodata/` from `createrepo_c`), keeping
  the last three versions per channel (Pages: 1 GB per site, 100 MB per file).
- They need the GPG key, so they go live with final signing. Until then Linux
  users take packages from the release page and update with the button.
- AUR: `release-app.yml` produces the updated PKGBUILD and `.SRCINFO`; pushing
  them to the AUR is manual (your AUR SSH key).
- **No Flatpak:** tosu must read osu!'s memory and a Flatpak cannot see other
  processes; the workaround (`flatpak-spawn --host`) undoes the sandbox. It
  also cannot install the pad's udev rule. The AppImage covers "any distro".

### 10.2 Windows

The installer from the release page once, then the app button for every
update. No winget, Scoop, Store or MSIX: MSIX needs mandatory signing and
fits a background daemon with serial-port access badly, and the Inno
installer already works per-user with no UAC.

---

## 11. Local builds on Linux

`make release-local` builds every installer **unsigned** into `dist-local/`,
in containers (Docker or Podman):

| Output | How |
|---|---|
| `.deb`, `.rpm`, AppImage, tarball | the same container and steps as `release-app.yml`'s Linux job |
| Windows binaries | `cargo-xwin` (downloads the Microsoft CRT/SDK on first use, which means accepting Microsoft's licence) |
| `opad-setup.exe` | Inno Setup under Wine |
| `tosu.exe` | first try: cross-built with zig (its native addon is the hard part); fallback: from the latest `tosu-v…` release |
| firmware | `espressif/idf:v5.5.2` |

`make feed-local` builds a feed over `dist-local/`, signs it with the test key
and serves it locally. An installed OPad pointed at it with
`OPAD_MANIFEST_URL` (already supported, `client.rs`) exercises the whole
update flow, buttons included, without publishing anything.

---

## 12. Testing

### 12.1 Automated (CI and `make feed-local`)

| Test | Pass when |
|---|---|
| Feed with a bad signature | rejected, nothing downloaded |
| Feed with a lower `serial` | rejected |
| Expired feed | rejected, GUI says "could not check" |
| Beta feed served as stable | rejected |
| Artifact with the wrong hash or too big | rejected before staging |
| Kill the daemon in every state of §6.2 | resumes or restarts cleanly, running version untouched |
| Install / upgrade / remove on clean distros | `dpkg -V` / `rpm -V` clean, origin marker right |
| Rollout bucket | stable per install and version, roughly uniform |

### 12.2 On a real pad (`docs/testing-checklist.md`)

| Test | Pass when |
|---|---|
| Normal firmware update | new version reported, counters intact |
| Unplug during the write | old firmware boots after replug |
| Image that never configures USB | rollback within about 20 s |
| Update from the oldest supported firmware | works in one step, or the feed asks for the app first |
| Pad stuck in download mode | `opadctl recover` + replug brings it back |
| Firmware update with no tosu running | works |

**Possible later:** a self-hosted runner with a test pad and a controllable USB
hub, running these on every firmware release. Not in the phases below.

---

## 13. Licences in releases

OPad is GPL-3.0-or-later (software) and CERN-OHL-S-2.0 (hardware), see
`REUSE.toml`.

- **App:** GitHub's source archive of the tag, plus `THIRD_PARTY_NOTICES.html`
  (`make notices`).
- **Firmware:** a source bundle including the ESP-IDF managed components at
  the versions in `firmware/dependencies.lock`, since they are compiled into
  the image.
- **tosu:** LGPL source archive and notices (`make tosu-source`).

---

## 14. Plans for when things go badly

| Situation | Plan |
|---|---|
| **The Pages site or GitHub account is compromised** | Clients only accept a feed signed with a trusted key, fresher than the last one they saw. Without the feed key, an attacker can serve nothing that installs. Approval-gated signing plus 2FA protect the key. |
| **The feed key leaks** | Ship a rotation release (§5.4) to a new key from the backup, as fast as possible; until then the leaked key can sign. This is the scenario the passphrase, the approval gate and the backup exist for. |
| **Every key is lost** | Cannot be fixed remotely; users reinstall by hand. Hence the offline backup. |
| **The updater has a bug** | Windows: the full installer from the release page repairs it. Linux: the package manager reinstalls a fixed version. Pad: `opadctl recover` works without the updater. |
| **Hosting has to move** (Pages, GitHub) | The feed address is compiled in; a custom domain (§16, question 1) lets it move without an app update. |
| **tosu is abandoned or broken** | §8. |

---

## 15. What the user sees

The Settings page gets one **Updates** card:

```
 Updates                                         Channel: [ Stable ▾ ]
 ───────────────────────────────────────────────────────────────────
 OPad app        1.2.0                           Up to date
 Pad firmware    1.1.0 → 1.2.0 available         [ Update firmware ]
                 Verified for your board. The pad stops working as a
                 keyboard for about 30 s; don't unplug it. If anything
                 fails it returns to the current version.
 tosu            4.26.2                          Up to date   [ Update ]
 ───────────────────────────────────────────────────────────────────
 Last checked 2 hours ago                        [ Check now ]
 [x] Update the app automatically  (Windows / AppImage)
 [x] Update tosu automatically
```

Plus each component's release notes, the result of the last update and, after
an app update, a notice to restart OPad (a *Restart* button is not built yet). *Check now* runs immediately when IDLE, at most once a
minute.

---

## 16. Decisions this document closes

1. **One pipeline** builds, tests, signs and publishes everything, from tags,
   on GitHub Actions. Local builds are for testing.
2. **Three independent tracks** (app, firmware, tosu), each with its own
   version, tag and changelog.
3. **One signed feed per channel**, static, with serial and expiry; files
   pinned per tag.
4. **The in-app button is the update path everywhere**, applied the way each
   install origin requires; package managers are never bypassed.
5. **Firmware: A/B slots, host-verified, rollback without user action,
   recovery over USB only.** Never BOOT/RESET, never Secure Boot.
6. **tosu is pinned, built by us, isolated behind a process boundary**, and
   never pulled from upstream at runtime.
7. **Small, stable, negotiated contracts** (protocol, IPC, feed schema,
   install-origin) instead of coupling app and firmware versions.

---

## 17. Open questions

1. **Feed address:** `opad-dev.github.io/OPad/updates/…`, or your own domain
   pointed at Pages (`updates.<domain>`)? A domain lets hosting move later
   without an app update; the address is compiled into every install.
2. ~~First versions~~ **Decided in phase 1:** app and firmware both start
   from `1.0.0-rc` (`.release-please-manifest.json`), so the first release PRs
   propose `1.0.0-rc.1`. The firmware moved from `1.0.0` to `1.0.0-rc` for
   this: a pad already flashed with a `1.0.0` build will not be offered
   `1.0.0-rc.1` (it is older) and has to be flashed by hand once.
3. **Automatic install:** keep it opt-in for Windows and AppImage, or always
   ask?
4. **Rust crate sources:** also attach a `cargo vendor` archive to app
   releases? Not required while crates.io keeps the crates; cheap to add.

---

## 18. Phases

Each phase is finished, tested and merged before the next. Phases 1–5 sign
nothing (§5.4); phase 6 is you following §19.

| Phase | Work | Done when | Needs keys/accounts |
|---|---|---|---|
| 1 | CI cleanup; release-please for app and firmware | release PRs appear with correct changelogs | no |
| 2 | Split release workflows (app, firmware, tosu), per-tag files, pre-release and not-Latest handling | a test pre-release of each track builds and publishes | no |
| 3 | Feed schema 2 + Pages + `update-feed.yml`; test key; serial/expiry/channel checks; `board`/`min_app`; rollout and yank; persisted state machine; Settings card, *Check now*, tosu *Update* | §12.1 passes against a published beta feed | no |
| 4 | `make release-local`, `make feed-local` | all installers build on Linux; local feed updates an installed OPad | no |
| 5 | apt/dnf repo generation, PackageKit path, AUR `opad-bin` | clean-distro repo install and upgrade tests pass against a locally served repo | no (going live needs §19) |
| 6 | You follow §19; then the rotation release and the first stable release | first stable release | yes |

### 18.1 Phase 1 (done 2026-10-12)

What it changed:

- `.github/workflows/*.yml`: actions on their current major versions
  (`checkout@v7`, `upload-artifact@v7`, `download-artifact@v8`,
  `setup-node@v7`, `setup-python@v7`, `pnpm/action-setup@v6`,
  `mlugg/setup-zig@v2`, `softprops/action-gh-release@v3`), checked with
  `actionlint`.
- `ci.yml`: a `versions` job, and the firmware build caches
  `managed_components` and ccache.
- `scripts/release/check_versions.py` (also in `make check`): both versions
  are SemVer and agree with `.release-please-manifest.json` and `Cargo.lock`,
  and every workspace crate is in release-please's `Cargo.lock` list. With
  `--tag` it checks a release tag against the tree (used by phase 2).
- release-please: `release-please-config.json`,
  `.release-please-manifest.json`, `.github/workflows/release-please.yml`.
  The app is the repository root minus `firmware/` and `hardware/`; the
  firmware is `firmware/`. Its Rust mode cannot handle a workspace whose root
  `Cargo.toml` has no `[package]`, so the version lines carry an
  `x-release-please-version` comment and `Cargo.lock` is bumped through an
  explicit list of the workspace crates. Changelogs start after `c0babc3`
  (`bootstrap-sha`).
- The RPM version is no longer written by hand in `desktop/gui/Cargo.toml`:
  `make rpm` and `build_packages.sh` pass it, turning `-rc.1` into `~rc.1`.
  (cargo-deb already does that conversion for the `.deb`.)

What you need to do once, on GitHub:

- **Settings → Actions → General → Workflow permissions:** tick *Allow GitHub
  Actions to create and approve pull requests*. Without it release-please
  cannot open its PRs.
- **Do not merge a release PR before phase 2.** Merging tags and creates an
  empty draft release; nothing builds it yet.

Known limits:

- A PR opened with the default `GITHUB_TOKEN` does not start other workflows,
  so CI does not run on release-please's PRs. The release workflows of phase 2
  run `check_versions.py --tag` before building, which catches what CI would.
  If CI on those PRs is wanted later, release-please needs a GitHub App token.
- The `Cargo.lock` bump could not be run locally (it is release-please's own
  code); the first release PR must show all twelve crates bumped.

### 18.2 Phases 2–5 (written 2026-10-12, on Windows)

| Phase | What exists now | Checked on Windows |
|---|---|---|
| 2 | `release-app.yml`, `release-firmware.yml`, `release-tosu.yml`, `scripts/release/publish_release.sh`; `release-please.yml` calls the first two | `actionlint` |
| 3 | feed schema 2, `opad-feed`, `update-feed.yml`, test key, `TRUSTED_KEYS`, rollout, channel / *Check now* / tosu *Update now*, last result, GUI card, installer relaunch | clippy, all tests, `opad-feed local` → minisign → `opad-feed verify`, ISCC compile |
| 4 | `make release-local` (`scripts/release/release_local.sh`, `local/`), `make feed-local` | `bash -n` only |
| 5 | `build_repos.sh`, `test_repos.sh`, `sign-rpm`, repository upgrades in the app, `aur_bin.sh` + `PKGBUILD-bin.in`, attestations | clippy, tests, `aur_bin.sh` output |

**Once on GitHub, before the first release:**

- **Settings → Pages → Source: GitHub Actions** (the feed is deployed there).
- **Settings → Actions → General → Workflow permissions:** *Read and write*,
  and *Allow GitHub Actions to create and approve pull requests* (§18.1).
- Run **Release tosu** by hand once (revision 1), so the feed has a tosu
  build and `make release-local` has a `tosu.exe`.
- Then merge the first release PRs; each release calls **Update feed**.
  Check `https://opad-dev.github.io/OPad/updates/beta.json` appears.

### 18.3 Pending for a session on Linux

Written but never run, because they need Linux. In this order:

1. **`make release-local`** on the Arch machine (Docker running):
   - the Linux container build (`local/Dockerfile.linux`, `local/build_linux.sh`),
     including `make tosu`'s nested container through the docker socket;
   - `cargo xwin` for the Windows programs; the GUI crate builds LVGL C code
     for the Designer preview, which is the likeliest thing to fail;
   - Inno Setup in `amake/innosetup` with `/DSourceDir` and `/DTosuDir`;
   - try cross-building `tosu.exe` with zig; until then it comes from the
     latest `tosu-v*` release (`TOSU_EXE` overrides).
2. **`make feed-local`**, then an end-to-end update: install an older build,
   serve a newer one, `OPAD_MANIFEST_URL=http://127.0.0.1:8000/updates/beta.json`,
   beta channel, *Check now*, *Install now* — for the AppImage, the tarball,
   the `.deb` and the `.rpm` (in a VM or container with a desktop session for
   `pkexec`). Also tosu *Update now*, and a firmware update on a real pad.
   (The feed only offers something newer: build the "new" one with a higher
   version in `desktop/Cargo.toml`, without committing it.)
3. **`scripts/release/test_repos.sh`** over `dist-local/` (needs
   `apt-ftparchive`, `createrepo_c`, `rpmsign`, Docker).
4. **`opad-bin`**: `aur_bin.sh` over a real release `.deb`, then
   `makepkg -si` and `makepkg --printsrcinfo | diff - .SRCINFO` on Arch.
5. **The repository upgrade path in the app** (PackageKit and the `pkexec`
   fallback) on Ubuntu and Fedora with the test repository added.
6. Whatever fails in 1–5 is fixed and these lines removed.

---

## 19. Signing guide (for when you are ready)

Everything here is done by you, once. Nothing in phases 1–5 needs it. Each
step says what to create, where to put it, and how to check it.

### 19.0 Before you start

- **Two-factor authentication on your GitHub account** (Settings → Password
  and authentication). With the keys stored as GitHub secrets, your account
  is what protects them.
- **A password manager available on both Arch and Windows** (Bitwarden,
  1Password or KeePassXC). Every key file and passphrase below goes in it.
- **An offline copy** (a USB stick kept somewhere else) for the encrypted key
  files. Passphrases stay in the password manager, never next to the files.
- `gh` (GitHub CLI) logged in: Arch `sudo pacman -S github-cli`, Windows
  `winget install GitHub.cli`, then `gh auth login`.

### 19.1 The `release` environment

GitHub → repository **Settings → Environments → New environment** →
`release`.

- **Required reviewers:** yourself. Every signing job then waits for your
  click, on any device.
- **Deployment branches and tags:** only `main` and the tags `v*`,
  `firmware-v*`, `tosu-v*`.

All secrets below go in **this environment**, not in the repository secrets,
so only the approved jobs can read them.

### 19.2 Feed key (minisign)

Signs `stable.json` and `beta.json`; the app trusts its public half.

1. Install minisign: Arch `sudo pacman -S minisign`, Windows
   `winget install jedisct1.minisign`.
2. Create it, with a long passphrase:
   ```
   minisign -G -p opad-feed.pub -s opad-feed.key
   ```
3. Put it on GitHub:
   ```
   gh secret set MINISIGN_SECRET_KEY --env release < opad-feed.key
   gh secret set MINISIGN_PASSWORD --env release
   ```
   (the second command asks for the passphrase).
4. Save `opad-feed.key`, `opad-feed.pub` and the passphrase in the password
   manager; copy `opad-feed.key` to the offline USB. Then delete the local
   copies.
5. Send the **public** key (the second line of `opad-feed.pub`) for the
   rotation release (§5.4). It goes into `opad-update/src/verify.rs`; it is
   not secret.

### 19.3 Repository key (GPG, for apt and dnf)

1. Create it (RSA 4096 works with every apt and rpm version we support):
   ```
   gpg --quick-generate-key "OPad packages <info@gferreiro.com>" rsa4096 sign 3y
   gpg --list-secret-keys --keyid-format long      # note the key ID
   gpg --armor --export-secret-keys KEYID > opad-repo.key.asc
   gpg --armor --export KEYID > opad-archive-keyring.asc
   ```
2. Put it on GitHub:
   ```
   gh secret set GPG_PRIVATE_KEY --env release < opad-repo.key.asc
   gh secret set GPG_PASSPHRASE --env release
   ```
   It signs the repositories (`update-feed.yml`) and each `.rpm` itself
   (`release-app.yml`'s `sign-rpm` job), which dnf's `gpgcheck=1` needs.
3. Send `opad-archive-keyring.asc` (public) to be committed; the install
   instructions and the repo files point at it.
4. Back up as in 19.2. Put a reminder in your calendar a month before the
   3-year expiry: extending it is `gpg --quick-set-expire KEYID 3y`, then
   repeat step 2 and republish the public key.

### 19.4 Windows code signing (SignPath Foundation)

Needs the repository to be **public**.

1. Apply at <https://signpath.org> (the Foundation's open-source programme).
   They check the licence (OSI-approved: GPL-3.0-or-later), that the project
   is active, and that releases are built by CI from public source.
2. Once accepted: install the SignPath GitHub App on the repository, and in
   SignPath create the project `opad` with a signing policy `release-signing`
   and an artifact configuration for `opad-setup-*.exe` and the `.exe`
   files inside it.
3. In SignPath, give the signing policy **manual approval**: the token is a
   repository secret (the Windows build job reads it), so SignPath's approval
   is what stands between a workflow run and a signature.
4. Put on GitHub:
   ```
   gh secret set SIGNPATH_API_TOKEN
   gh variable set SIGNPATH_ORGANIZATION_ID --body "<your organisation id>"
   ```
5. The signing steps in `release-app.yml` are already there, switched off.
   They expect two artifact configurations in SignPath: `programs` (a zip of
   `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe`) and `installer`
   (`opad-setup-*.exe`).

### 19.5 Turn signing on

```
gh variable set SIGNING_ENABLED --body true
```

Every signing step in the workflows runs only when this repository variable
is `true`; until then they are skipped. Then:

1. The rotation release (§5.4): an app release that trusts the test key and
   the real feed key.
2. The first stable release: in `release-please-config.json` set
   `"prerelease": false` and `"release-as": "1.0.0"` for the app (and the
   firmware), merge the release PR, approve the signing jobs. Remove
   `release-as` afterwards.
3. **Settings → General → Releases → Enable release immutability**, so
   published files can never be replaced.
4. Build provenance (`actions/attest`) runs from now on too. It needs the
   repository to be public (on GitHub Free/Pro/Team).
5. Add the repository instructions (§10.1, the comment at the top of
   `scripts/release/build_repos.sh`) to the README: the repositories exist
   only from this point.

### 19.6 Check that it worked

```
# The feed verifies with the real key
curl -fsSLO https://opad-dev.github.io/OPad/updates/stable.json
curl -fsSLO https://opad-dev.github.io/OPad/updates/stable.json.minisig
minisign -Vm stable.json -P '<public key>'

# A release file was built by the workflow from this repository
gh attestation verify opad-setup-1.0.0.exe -R OPad-dev/OPad

# Windows: right click opad-setup-1.0.0.exe → Properties → Digital Signatures
```

And on a clean Ubuntu and Fedora container, add the repository and install
OPad (the README's instructions); `apt` and `dnf` must not warn about
signatures.

### 19.7 Where everything lives

| What | Secret / file | Where |
|---|---|---|
| Feed key, private | `MINISIGN_SECRET_KEY`, `MINISIGN_PASSWORD` | `release` environment + password manager + USB |
| Feed key, public | in the code | `desktop/crates/opad-update/src/verify.rs` |
| Repo key, private | `GPG_PRIVATE_KEY`, `GPG_PASSPHRASE` | `release` environment + password manager + USB |
| Repo key, public | `opad-archive-keyring.asc` | repository and Pages |
| SignPath | `SIGNPATH_API_TOKEN`, `SIGNPATH_ORGANIZATION_ID` | repository secret / repository variable |
| The switch | `SIGNING_ENABLED` | repository variable |

### 19.8 If a key leaks or is lost

- **Feed key leaked:** create a new one (19.2), ship a release that trusts
  only the new key as fast as possible, signed with the old one (§5.4).
- **Feed key lost, backups too:** installs cannot be moved to a new key
  remotely; users reinstall from the release page (§14).
- **GPG key leaked:** create a new one, publish the new public key, and tell
  users to re-run the repository setup command.
- **SignPath:** revoke the token in SignPath and set a new one.
