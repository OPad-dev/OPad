---
title: "Launch Checklist"
description: "Pre-sale verification covering repository, pipelines, hardware, desktop app, compliance, and manufacturing."
draft: true
---

Everything that has to be true before OPad is sold. It covers the repository,
the GitHub pipeline, the pad, the desktop app on every supported platform, the
updaters, manufacturing, and the legal and support side of selling hardware.

**Freshness rule.** A result older than **7 days** is not evidence. On
2026-11-20 the only recent pad results are HW-06 to HW-15 and BAK-01 in
`docs/testing-checklist.md` (Linux, V3 bench, no case). Every other PASS in that
file dates from 2026-09-13 to 2026-10-10, was run on V1 hardware and with the
old `opad-*` names, and is re-opened below. Re-run the whole of §3 to §7 on the
release candidate itself, and again on any later candidate that touches
firmware, USB, input or packaging.

**How to run it.** Do it on one tagged release candidate (`v1.0.0-rc.N` and
`firmware-v1.0.0-rc.N`), installed from the published GitHub release, never
from a build tree. Write the date, host, build and result next to each item.
"Probably fine" is a FAIL.

Legend: **P0** blocks the launch, **P1** must be done before the first unit
ships, **P2** should be done before launch but can follow within weeks.

---

## Code work list (what is left in the repository, in order)

The owner's own list (accounts, DNS, hardware, legal) is kept outside the
repository. Items marked *waits* need something from the owner first.

**Done (2026-11-20/21):** CI green; release-please opens PRs and bumps
`Cargo.lock`; Pages and the feed deploy; nothing named osupad but what pads
store, and that migrates (`OPAD-<MAC>`, `opad*` NVS namespaces); checked
frames (0.12); feed at `updates.opad.dev`, overridable by forks; SECURITY.md,
issue templates; V3 hardware and case docs; printing guide.

1. [x] **Pages custom domain** `updates.opad.dev`, HTTPS enforced (2026-11-21;
   the Pages URL redirects to it).
2. [ ] **tosu, watched:** a weekly job that builds a new upstream tosu, checks it
   starts and that its v2 JSON still has every field `opad-tosu` reads, then
   opens the version bump PR and publishes it to beta. Then an acceptance run
   on the Windows mini PC: osu! stable and lazer play a recorded replay of the
   test map (`scripts/ppm_test_map.py`), and the run checks tosu and the daemon
   go menu → playing → results with the right data; passing promotes it to
   stable. *Waits for the acceptance half: the mini PC on Windows 11, osu!, a
   replay.*
3. [ ] **`make hil`:** the pad suite (board test, flash, rollback, recover,
   latency, old/new app × firmware, soak) against one pad picked by serial,
   with a report; a daemon option to ignore that serial so the owner's daemon
   leaves the test pad alone.
4. [ ] **`opadctl factory`:** per unit, flash the published release (all four
   images), board test (`--magnet` on HE), HE calibration, taps per key, a
   touch check, factory state (no owner, zero counters), and a report saved
   under the serial.
5. [ ] **Installers tested in CI:** silent install, run, uninstall of the
   Windows installer on the Windows runner (PKG-01 … PKG-06); daemon and
   `opadctl` smoke tests on the packages; the AppImage.
   Done except the AppImage: `lab-install.yml` runs after every app release, in
   the lab VMs (Ubuntu 22.04/24.04/26.04, Fedora 43, Arch, Windows 10/11):
   upgrade from the previous release, file hashes against `FILES.sha256`,
   start at login, `opadctl status`, the GUI, data kept, clean uninstall. The
   feed only deploys when it passes (first full pass: v1.0.0-rc.5).
6. [ ] **`make release-local` and `make feed-local` on Linux** (release spec
   §18.3), and an end-to-end update from a local feed for each package type.
7. [ ] **First release candidate.** Done 2026-11-21: `tosu-v4.26.2-r1`,
   `firmware-v1.0.0-rc.1` and app `v1.0.0-rc.3` are pre-releases, and
   `https://updates.opad.dev/beta.json` lists all three (`opad-feed verify`
   OK). The app's rc.1 and rc.2 builds failed and stay unpublished drafts
   (tosu updating itself, notices fetch, the package test's quoting). Left:
   install each artifact and update to it from the feed (needs an older
   build installed, so it waits for the next rc).
8. [ ] **The not-run rows of §16 in `docs/testing-checklist.md`:** layouts and
   HE calibration across the upgrade (ID-02), a rollback (ID-03), Windows
   (ID-04, CRC-01/02), the 8-hour soak (CRC-03).
9. [ ] **USB PID** (0.9): the firmware enumerates as `303a:8415`, granted by
   Espressif; app, udev and flasher follow. Tested on Linux 2026-11-25 (owner's
   HE pad, Arch): `opadctl flash` from `4001` firmware, `opadctl recover` with a
   replug, the installed rc.6 daemon finding the pad, and the new udev rule
   giving the user an ACL on the tty and the keyboard. Left: Windows, and drop
   `4001` from `70-opad.rules` once no rc pad still runs older firmware.
10. [ ] **Signing** (0.8, release spec §19): build-time trusted key override
    for forks, the `release` environment, the rotation release, SignPath wired
    and tested, then `1.0.0`. *Waits: the owner's keys and SignPath.*
11. [ ] **Docs a buyer follows:** README install per OS with the real download
    URLs, quick start, update, recovery and troubleshooting, each checked
    against a release; Discord link in the app, README and issue templates
    (*waits: the invite*).
12. [ ] **Compliance paperwork drafts** (kept outside the repository): technical
    file, risk analysis, Declaration of Conformity, safety and GPSR texts in
    English and Spanish, labels. *Waits: Waveshare's answer.*
13. [ ] **Repository hygiene:** Dependabot for Actions and Cargo, branch
    protection on `main`, `cargo deny check` in `make check`, `chrono` → `jiff`
    (0.16).

## 0. Found broken during this review (fix first)

Each of these was checked on 2026-11-20 against GitHub or the code.

| # | P | Problem | Evidence | Fix |
|---|---|---|---|---|
| 0.1 | P0 | **CI is red on `main`, and has been since at least 2026-10-10.** Every CI run listed by `gh run list` failed. | Run 44802343568 (`cd4616d`): *Licences* and *Protobuf Drift Check* fail; the Rust, Windows and firmware jobs pass. | See 0.2 and 0.3. Then require CI green on `main` (branch protection, or at least by habit before every merge). **Done** 2026-11-21 (`827ceb0`): every CI job green on `main`. |
| 0.2 | P0 | cargo-deny fails on four rustls advisories. | `rustls 0.23.45` in `desktop/Cargo.lock`: RUSTSEC-2026-0346, -0347, -0348, -0349. | `cd desktop && cargo update -p rustls`, then `cargo deny check` locally. Add `cargo deny check` to `make check` so this is caught before a push. **Done** (`827ceb0`): rustls, rustls-webpki, webpki-roots and tokio updated, rusqlite 0.32 → 0.41 for the bundled SQLite (RUSTSEC-2026-0459); `chrono`'s "unmaintained" notice ignored with its reason (0.16). |
| 0.3 | P0 | Protobuf drift check fails on a version string, not on a real change. | `.github/workflows/ci.yml:178` installs an unpinned `nanopb` (CI got 0.4.9.3); the committed files say `Generated by nanopb-0.4.9.1`. | Pin `nanopb==0.4.9.1` (and `protobuf`, `grpcio-tools`) in `ci.yml`, matching the version the handoff says to use locally. **Done** (`827ceb0`): nanopb pinned to 0.4.9.1; regenerating gives no diff. |
| 0.4 | P0 | **Release Please cannot open its release PRs**, so no release can start. | Run 44802343510: "GitHub Actions is not permitted to create or approve pull requests". `actions/permissions/workflow` → `can_approve_pull_request_reviews: false`. | Settings → Actions → General → Workflow permissions: *Read and write* and *Allow GitHub Actions to create and approve pull requests* (`docs/specs/release-and-updates.md` §18.1). **Done** 2026-11-21: allowed at the organisation and the repository; release PRs #9 (firmware) and #10 (app) open. They also showed that release-please never bumped `Cargo.lock`; the `lockfile` job in `release-please.yml` does it now (`d2689f8`). |
| 0.5 | P0 | **The update feed has never been deployed.** The weekly *Update feed* job fails at *Deploy to GitHub Pages* every week. | Run 43900948560: `Failed to create deployment (status: 404) … Ensure GitHub Pages has been enabled`. `gh api repos/OPad-dev/OPad/pages` → 404. The app reads `https://opad-dev.github.io/OPad/updates/` (`desktop/crates/opad-update/src/client.rs:21`), so every install would see a 404. | Settings → Pages → Source: *GitHub Actions*. Re-run *Update feed* and check `…/updates/beta.json` loads. **Pages on** 2026-11-21 and the feed deploys. The app now reads `https://updates.opad.dev/` (`DEFAULT_FEED_BASE`; forks set `OPAD_FEED_BASE`). **Open, owner:** register `opad.dev`, add the DNS record `updates CNAME opad-dev.github.io`, then the custom domain is set on Pages. |
| 0.6 | P0 | **Nothing has ever been released.** `gh release list` is empty. `release-app.yml`, `release-firmware.yml` and `release-tosu.yml` have never run; `make release-local` and `make feed-local` were written on Windows and never run (§18.3 of the release spec). The last local packages in `dist/` still carry the old names (September). | `gh release list`, `ls dist/`. | §2 below, in order. **Open.** Decided: pre-releases `1.0.0-rc.N` on the beta channel to test the updater, then `1.0.0` for app and firmware on launch day. |
| 0.7 | P0 | **The firmware sends users to a domain that does not exist.** The HE lock screen shows `https://opad.dev/download`. | `firmware/main/ui/he_cal_ui.h:22`; `opad.dev` has no DNS record (2026-11-20). | Register the domain and make `/download` work (redirect to the GitHub release or the website) before the first HE unit ships, or change the URL. **Open**, owner: registering `opad.dev`; `/download` must follow the latest release by itself. |
| 0.8 | P0 | **The app trusts only the committed test key.** Both halves of it are public. | `TRUSTED_KEYS = &[TEST_KEY]` in `desktop/crates/opad-update/src/verify.rs:36`; `gh variable list` and `gh secret list` are empty, so `SIGNING_ENABLED` is off and every release is a pre-release on the beta feed. | Release spec §19 end to end: `release` environment, minisign feed key, GPG repo key, SignPath, `SIGNING_ENABLED=true`, the rotation release that trusts both keys, then the first stable release. **Open.** Decided: SignPath Foundation for Windows; no apt/dnf repositories, so no GPG key; forks must build and update without the official keys. |
| 0.9 | P1 | The USB product ID is not OPad's own. | `firmware/main/usb/usb_descriptors.c:37-38` uses Espressif's VID `0x303A` with PID `0x4001`, the PID esp_tinyusb assigns by default to a CDC device. `packaging/linux/udev/70-opad.rules` gives any `303a:4001` device `uaccess`, and the daemon will try to talk to it. | Ask Espressif for a PID (free for products on their chips, via the `espressif/usb-pids` repository) and move to it **with** compatibility: the app, udev rules and flasher accept both PIDs for as long as old firmware exists. Ask before changing it: it is a USB-visible change. **Done in code** 2026-11-25: Espressif granted `303a:8415` (espressif/usb-pids#491). The owner chose to switch during the release candidates without the two-step rollout: the firmware sends `0x8415`; the app finds the pad by its strings, so any PID works; `70-opad.rules` keeps `4001` so rc pads on older firmware can still be updated on Linux. Tested on Linux on the owner's HE pad (flash, recover, old daemon, udev ACL); Windows not yet. |
| 0.10 | P1 | Hardware docs contradict the bench. | `hardware/pcb/V3/README.md` says every V3 board is "Not ordered, not built" and "No firmware supports them yet"; `docs/history/2026-11-20-v3-modules-handoff.md` has V3 boards on the bench and their firmware merged. `hardware/3d/README.md` lists only V1 and V1.1 cases. | Update both READMEs to what was built and what was not. **Done** (`911c2c1`). |
| 0.11 | P1 | User-facing docs still use the old names. | 59 mentions of the old binary names in `README.md` and `docs/*.md` (32 in `docs/testing-checklist.md`, whose §7 recipe also runs a manifest binary that no longer exists). | Rename, and move the old reviews (`docs/software-*review*`, `docs/todo-2026-09-21-evening.md`, `docs/full-repo-…guide.md`) to `docs/history/` (AGENTS.md §7). **Done** (`fd76efd`, `911c2c1`): nothing is named osupad except what pads store (0.15); old reviews in `docs/history/`. |
| 0.12 | P1 | Open firmware bug: the pad dropped two host messages (231 and 175 bytes) as an HE map ended. | Handoff "Open" item 1; `5715b9f` only adds logging. | Reproduce with `opadctl logs` during the soak in §3.9, then fix the host or `protocol/opad.options`. Not shippable while unexplained. **Fixed** 2026-11-21: checked frames with a CRC, negotiated (see the note below and `docs/reference/serial-protocol.md`). |
| 0.13 | P1 | The V3 case has never been printed. | `hardware/3d/custom_case/V3/README.md:9`: "Not printed or test-fitted yet". HW-12 and HW-14 note "no case yet". | §4. **Done**: printed and fitted (owner); the latest small changes are not printed, accepted. 2026-11-26: the case to ship is now V4 (`hardware/3d/custom_case/V4`), V3 inside a new faceted outside; its fit check passes, it is **not printed yet** (§4 Fit and assembly). |
| 0.14 | P2 | Repository hygiene for a sold product. | No `SECURITY.md`, `CONTRIBUTING.md`, issue templates or `CHANGELOG.md` (release-please creates the last on its first PR). | Add `SECURITY.md` (where to report, response time) and issue templates (bug, pad info from `opadctl status`). **Done** (`911c2c1`): `SECURITY.md` with GitHub private reporting (enabled), issue templates sending help to Discord. |

| 0.15 | P1 | What pads and PCs store still says osupad. | NVS namespaces `osupad`, `osupad_cfg`, `osupad_cal`, `osupad_cal3`, `osupad_ui` (`counters.c`, `device_config.c`, `he_cal.c`, `ui_store.c`); USB serial and device ID `OSUPAD-<MAC>` (`usb_descriptors.c`, `protocol.c`, `DEVICE_ID_PREFIX` in `opad-device`), which the app keys counters and ownership on. | **Done** 2026-11-21: namespaces copied to `opad*` at boot (old ones kept for rollbacks), serial and ID `OPAD-<MAC>`, the app maps `OSUPAD-` to the same pad. Tested on the owner's MX V3 pad (`docs/testing-checklist.md` §16). The PID is 0.9. |
| 0.16 | P2 | `chrono` is unmaintained (RUSTSEC-2026-0543). | Used in 21 files for timestamps. | Move to `jiff`; ignored in `desktop/deny.toml` until then. |

### 0.12: what is known

The pad dropped two host messages (231 and 175 bytes) as an HE map ended,
reported only as "Protobuf frame decode failed". Frames carry no CRC
(`firmware/main/protocol/frame_parser.h`). If one host write takes longer than
1 s, `write_frame` gives up mid-frame (`WRITE_DEADLINE`,
`desktop/crates/opad-device/src/lib.rs`); the pad keeps the partial frame and,
if the next one arrives within 500 ms (`RX_STALE_US`), glues it on, so two
messages fail. The end of a map, when the pad writes flash and starts the
result animation, is the likeliest moment for a stalled write. Unproven:
`5715b9f` makes the next drop log nanopb's reason, the payload and the byte
where decoding stopped. Neither keys nor latency are affected; the cost is a
display update or a sync that is resent. Fixed with checked frames (owner's go,
2026-11-21): a cut frame is dropped on its CRC and the frame behind it is
found and delivered.

### Decisions (owner, 2026-11-21)

- **Launch:** Kickstarter first, then eBay and Etsy, worldwide. Tiers differ only
  in the modules: MX, HE, or both. Every pad has the carrier, case, USB board,
  USB-C cable and both FFCs.
- **First batch:** 5 units, assembled by the owner: one his, four for promotion.
- **Versions:** `1.0.0-rc.N` pre-releases on beta, then `1.0.0` (app and firmware
  the same day). Tester pads are flashed from the published release, so the
  updater offers them every later rc with nothing done by hand.
- **Platforms:** Windows 10 and 11, Ubuntu 22.04 / 24.04 / 26.04 (22.04 dropped
  if it gets hard), Debian 12 / 13, Fedora, Arch (AUR `opad-bin`, the official
  Arch package), AppImage. Windows 11 and Arch on bare metal, the rest in VMs.
- **Updates:** no apt/dnf repositories. The app's updater installs the app, the
  firmware and the bundled tosu.
- **Signing:** SignPath Foundation for Windows. The owner sells as a private
  person for now.
- **Support:** Discord first; GitHub issues for bugs that need a code change.
- **Easter-egg GIF:** stays, with a notice that it is removed if its rights
  holder asks.
- **Compliance (2026-11-21):** compliance testing is funded by the Kickstarter
  and done before rewards ship. Sales are worldwide, so the radio on the
  Waveshare board matters in every market (CE/RED, UKCA, FCC, ISED, TELEC,
  RCM, KC). Waveshare is asked for its certificates first.
- **Case:** PETG printed in-house for testers and early batches; MJF PA12
  quoted in batches for volumes (`hardware/3d/custom_case/V4/README.md`).
- **Test machines:** the owner's mini PC becomes a bare-metal Windows 11 test
  machine (flashing and WIN rows cannot run in a VM).
- **Domain:** `opad.dev` is registered at Cloudflare.

---

## 1. Repository and CI (P0)

- [ ] `make check` passes on a clean clone (passed on `cd4616d`, 2026-11-20).
- [ ] `cargo deny check` and `python3 scripts/check_reuse.py` pass locally.
- [x] Every CI job green on `main` again (2026-11-21). Keep it so on the release commit: versions, Rust Linux, Rust Windows, Windows GUI, firmware host tests, protobuf drift, firmware build, licences.
- [ ] The firmware CI artifact is byte-for-byte what `release-firmware.yml` would publish (same IDF image, same `sdkconfig.defaults`, no local `sdkconfig` drift).
- [ ] No debug option is on in the release image: every `CONFIG_OPAD_TEST_*`, `BENCH_*`, `HALL_BENCH`, `HE_TRACE`, `HE_DEBUG`, `UI_PERF_TEST`, `DEBUG_I2C_SCAN` is `n` in `firmware/build/config/sdkconfig.h` of the release build (`firmware/main/Kconfig.projbuild`).
- [ ] Dependabot or Renovate is on for GitHub Actions and Cargo, so the next rustls advisory arrives as a PR rather than a red CI.
- [ ] Branch protection on `main`: CI required, linear history, no force-push.

## 2. Release pipeline, end to end (P0)

In this order (release spec §18.2–§18.3, §19):

- [ ] 0.4 and 0.5 done (PR permission, Pages).
- [ ] *Release tosu* run by hand, revision 1: `tosu-v4.26.2-r1` exists with Linux and Windows builds, notices and source.
- [ ] `make release-local` on Linux builds every installer (Linux packages in the container, Windows programs with `cargo xwin`, Inno Setup in `amake/innosetup`).
- [ ] `make feed-local` serves a feed, and an installed older build updates from it (AppImage, tarball, `.deb`, `.rpm`, Windows installer).
- [ ] First release PRs open; each shows all twelve crates bumped in `Cargo.lock` (§18.1 "Known limits").
- [ ] Merge them: `v1.0.0-rc.1` and `firmware-v1.0.0-rc.1` build, `test_packages.sh` passes on ubuntu 22.04/24.04/26.04, debian 12/13, rockylinux 9, fedora 43, and both publish as pre-releases, not Latest for the firmware.
- [ ] The release workflow's "image reports the tagged version" step passes, and `opadctl status` on a flashed pad shows that version.
- [x] *Update feed* deploys; `beta.json` lists the app, firmware and tosu; `opad-feed verify` passes on it (v1.0.0-rc.5, 2026-11-22).
- [ ] Signing (§19): `release` environment with you as reviewer; minisign key; SignPath accepted and configured; `SIGNING_ENABLED=true`. (No GPG key: no apt/dnf repositories.)
- [ ] Rotation release trusting the test key **and** the real key is out, and an install of it updates from a real-key feed.
- [ ] First stable release: `prerelease: false`, `release-as: 1.0.0`; `stable.json` lists it; release immutability on; `gh attestation verify` passes on a downloaded file; the Windows installer shows a valid signature.
- [ ] AUR `opad-bin` from the release run's `release-extras-<tag>` artifact (`aur_bin.sh`); `makepkg -si` on Arch; `.SRCINFO` matches.
- [ ] Download page (0.7) links the current stable files and their SHA-256.

## 3. Pad: firmware and V3 hardware

The printable version of this section is `docs/pad-test-sheet.md`.

Run every row on **each SKU you sell** (MX V3 and HE V3), assembled in the V3
case, on the release firmware, flashed with `opadctl flash` from the release.
IDs in brackets are the rows in `docs/testing-checklist.md`; re-run them, do
not copy old results.

### 3.1 Keyboard with no software (rule 2)

- [ ] Fresh Linux machine, no OPad software: types Z/X/C, `evtest` shows clean down/up, no repeat glitch.
- [ ] Fresh Windows 10 **and** Windows 11 machine (bare metal), never seen OPad: HID keyboard and COM port on inbox drivers, no prompt (WIN-01).
- [ ] BIOS/UEFI setup and a boot menu accept its keys (the keyboard interface is boot protocol, `usb_descriptors.c`).
- [ ] Through a USB hub, a USB-C to A adapter, the front panel ports, and a laptop on battery.
- [ ] Host suspend/resume and screen lock: keys work after wake; no stuck key.
- [ ] Replug 20 times (COM-03); cold boot of the PC with the pad plugged in.
- [ ] Waveshare's own USB-C and the V3 USB board's port both work for typing, flashing and recovery; a cable in each at once does no harm (V3 Schottky).

### 3.2 Keys and latency (rule 1)

- [ ] Each key 20 clean taps, 1:1 in `evtest`/Keyboard Tester, counters +20 (HW-01, HW-02, HW-08, HW-14).
- [ ] Simultaneous keys, all three pairs; no ghosting (HW-03).
- [ ] Alternating stream above 20 presses/s for 60 s, 0 missed, 0 chatter (HW-04).
- [ ] `opadctl latency` over a full map with the display on: p50/p99.9/max recorded per SKU, no worse than the 2026-11-20 numbers (MX V3: p99.9 50 µs; HE V3: p99.9 40 µs), 0 deferred reports.
- [ ] Same with the daemon stopped, with tosu stopped, and while the GUI Designer pushes a layout between maps.
- [ ] Display asleep → first key press is sent before the wake (HW-05).
- [ ] Hand-held edges: hold a key for 30 s (OS auto-repeat only), half-press and wiggle (MX chatter; HE rapid trigger stays clean).
- [ ] HE: no phantom presses in a 1-hour idle with keys untouched and the screen animating (the V1.1 phantom spikes are unresolved, `docs/history/2026-10-22-he-phantom-presses.md`; V3 is a new design and has to show it is free of them).

### 3.3 Touchscreen (rule 4), all rows of §14 SWP-01 … SWP-12

- [ ] Every SWP row, on Linux and on Windows. None has ever been run (all `n/r`).
- [ ] Quick Retry: press, hold and release are clean; a resting finger mid-map retries within ~60 ms (SWP-07).
- [ ] No ` outside a map; no swipe ever sends ` (SWP-02, SWP-06).
- [ ] No host / no tosu: every touch is Quick Retry (SWP-08).
- [ ] No latency cost from swiping (SWP-09).
- [ ] Wet or sweaty finger, two fingers, palm: no stuck Quick Retry.

### 3.4 Hall Effect calibration (rule 4's exception, D7), all of §15 HE-CAL-01 … 12

- [ ] Every HE-CAL row on an HE V3 pad, including key 3. None has ever been run.
- [ ] HW-10's open half: with key 3 uncalibrated the pad is locked.
- [ ] HW-07's magnet test.
- [ ] The lock screen's URL works (0.7).
- [ ] Calibration survives a power cut, an OTA update and an OTA rollback (`cal3_*` records, handoff "Storage must survive an OTA rollback").

### 3.5 Board test and modules

- [ ] HW-06, HW-07, HW-13 on production boards (not only the bench pair).
- [ ] Swapping MX V3 ↔ HE V3 with USB out: the pad comes up as the new module every time.
- [ ] Module cable unplugged at boot, and pulled while running: no crash, no stuck key; it is reported.
- [ ] HW-15's open half on a hand-wired pad, if hand-wired pads are supported at all; otherwise say so in the docs.

### 3.6 Display and UI

- [ ] Every screen: idle, playing, results with each grade animation, calibration, lock, easter egg.
- [ ] Brightness and sleep settings apply and persist.
- [ ] LCD failure build still types (FAIL-01).
- [ ] 2-hour session with tosu streaming: no freeze, no memory growth (STR-01, with the heap watermark logged at start and end).

### 3.7 Storage and counters

- [ ] Power-cycle after 500 presses: counters exact (FAIL-03).
- [ ] NVS failure build still types (FAIL-02).
- [ ] Zero host writes during PLAYING and COOLDOWN, on Linux and Windows (STR-02).
- [ ] Backup export/import, with and without key 3 fields (BAK-01).
- [ ] Ownership and takeover between two PCs (W32-01 … W32-05).

### 3.8 Firmware update and recovery (rule 5)

- [ ] `opadctl flash` from the release goes to the other slot (FAIL-05).
- [ ] A broken image rolls back by itself (FAIL-06).
- [ ] `opadctl recover` + replug, no button (FAIL-07), on Linux and on Windows bare metal.
- [ ] In-app firmware update from the real feed: counters and calibration unchanged (UPD-06), consent every time (UPD-07), refused mid-map (UPD-08).
- [ ] After an update, the pad types on a machine with no OPad software (UPD-09, never run).
- [ ] Unplug mid-write, then recover (UPD-11, never run deliberately).
- [ ] A pad flashed with an older `1.0.0` build (above `1.0.0-rc`) is handled: either it is offered the release or the docs say how to flash it once (release spec §17.2).

### 3.9 Soak and abuse

- [ ] 8 hours of maps (autoplay plus real play) with `opadctl logs` open: no dropped host message (0.12), no reset, no watchdog, no deferred report.
- [ ] 48 hours plugged in idle, display sleeping: still responds instantly.
- [ ] Garbage on the CDC port (COM-02); daemon `kill -9` mid-map (COM-04); tosu killed mid-map (COM-05).
- [ ] Brown-out: a weak port or a long thin cable; the pad either works or resets cleanly, never sends a stuck key.
- [ ] ESD: touch the case seams, the USB shell and the FFC area with a charged finger (and, for CE, the formal test in §6).

## 4. Mechanical (P0 for each SKU)

"It fits" is checked. These ask whether a finished pad survives what a buyer
and the post will do to it. Each needs one assembled pad, ideally one you can
afford to break, and a note of the result. Nothing here has been done yet
(owner, 2026-11-21).

**Fit and assembly**
- [ ] Print the case as production will (same printer, filament, settings); fit every board. Re-check the clearances under 0.5 mm listed in `hardware/3d/custom_case/V4/README.md`: nothing rubs or is forced.
- [ ] Both FFCs fold as modelled, without a sharp crease or pulling at a connector; the screen sits flush.
- [ ] Assemble one from a written step list, timing it; then someone else assembles one from the list alone. Every step they get wrong is a fix to the list or the case.
- [ ] Swap the module (MX ↔ HE) 20 times as a buyer would: the FFC and its latch still hold, nothing cracks.

**USB-C port** (the part users stress most)
- [ ] Plug and unplug a cable 200 times, at the angles people really pull from. Afterwards the USB board has not moved, the Waveshare's header sockets have not loosened (they must take no load), and the pad still enumerates on the first try.
- [ ] Pull the cable sideways with a firm hand while plugged in: the case and the board's stops take it, not the solder joints.

**Typing loads**
- [ ] Mash each key hard for a minute (a stream, then full-force bottom-outs): the case does not flex, creak or move the module, and no key sticks.
- [ ] Keys have no wobble and no rub against the plate, with the switches and keycaps you will ship.
- [ ] Rubber feet: the pad does not slide on a desk, wood and glass, during a hard stream.

**Transport and handling**
- [ ] Drop it from desk height (75 cm) onto a hard floor, on each face, outside the box: it still works and nothing came loose inside.
- [ ] Drop the packed retail box from 1 m, five times: the pad inside is undamaged. This is what the post does.
- [ ] Leave it for a day in a hot car or by a radiator (about 50 °C), and in the cold: the print does not warp and the screen and keys still work. PLA softens near 55 °C; if it warps, print in PETG or ASA.
- [ ] Touch the case seams, the USB shell and the screen edge with a charged finger (walk on carpet first): no reset, no stuck key (the formal test is §6).

## 5. Desktop app

### 5.1 Platforms

Run §5.2 on each: Windows 10 22H2 (VM), Windows 11 (bare metal, standard user,
SmartScreen on), Ubuntu 22.04 / 24.04 / 26.04, Debian 12 / 13, Fedora (current),
all in VMs, Arch (bare metal, from the AUR), and the AppImage on one distro
without the package. GNOME and KDE, Wayland and X11. What can run without a
person (install, uninstall, daemon start, `opadctl`, updates against a local
feed) runs in CI; the pad-specific rows run on the hardware runner.

### 5.2 Per platform

- [ ] Install from the release; the installer or package manager raises no warning other than expected ones (Windows: none once signed).
- [ ] PKG-01 … PKG-06 (install diff, uninstall diff, uninstall deletes the user data, install-origin, bundled tosu, uninstall while running). The lab VMs run install, start at login and uninstall for every release (`lab-install.yml`).
- [ ] Autostart after login; tray icon appears and its menu works (WIN-04; GNOME needs the AppIndicator extension: say so in the docs).
- [ ] Hotplug within 2 s (WIN-05); flash with the GUI running (WIN-06); COM port above COM9 (WIN-08); second daemon refused (WIN-02); IPC private to the user (WIN-03).
- [ ] tosu finds osu! stable (Windows) and lazer (Linux and Windows); stats, PPM/KPM (PPM-01 … PPM-06), results screen.
- [ ] Every GUI page opens and does what it says: Dashboard, Stats, Keys (bindings, key 3, game keys, key feel, calibration), Screen (display, swipe actions, result animations), Designer (preview matches the pad), Pad (counters, backup), Settings (updates, osu! connection), Troubleshooting (logs, key test, board test, magnet test, report). Rows APP-01 to CLI-04 in `docs/testing-checklist.md` §17.
- [ ] High-DPI and a second monitor; window restore after a restart.
- [ ] SQLite read-only: degraded mode (FAIL-04).
- [ ] App update from the feed: *Check now*, *Install now*, cancelled UAC/polkit changes nothing (UPD-05).
- [ ] An update keeps the counters; an uninstall deletes the PC's data (the pad keeps its own counters).

### 5.3 Old and new, both ways (rule 3)

- [ ] New app with the previous released firmware, and previous app with the new firmware: keys, display, config, layouts, stats, calibration and updates work, or fall back as designed (SWP-12, PPM-06).
- [ ] App with no pad attached for a day: no CPU or memory growth.

## 6. Legal and compliance for selling hardware (P0 unless noted)

These are not code, but they decide whether you can sell. Check each with a
compliance lab or advisor; this list is what to ask about, not legal advice.

- [ ] **CE marking (EU):** EMC Directive 2014/30/EU and RoHS 2011/65/EU. The ESP32-S3 has a radio, so ask whether the Radio Equipment Directive 2014/53/EU applies even with Wi-Fi/BT unused, and whether Waveshare's module certification can be relied on. Pre-compliance EMC scan (emissions, ESD to IEC 61000-4-2), technical file, Declaration of Conformity.
- [ ] **RoHS/REACH declarations** from each supplier: Waveshare, JLCPCB assembly, switches, FFCs, filament.
- [ ] **GPSR (EU 2023/988):** your name, postal and electronic address on the product or packaging; type/batch/serial number on each unit; safety information and instructions in the language of each country you sell to (Spanish for Spain); a process to record and report incidents.
- [ ] **WEEE:** register as a producer in Spain (RII-AEE) and every other EU country you ship to, or use a marketplace's scheme; crossed-out bin symbol on the product.
- [ ] **Packaging:** packaging producer registration (Spain's EPR for packaging) if applicable to your volume.
- [ ] **UK (UKCA) and US (FCC Part 15 B)** only if you sell there.
- [ ] **USB:** do not use the USB-IF logo; resolve 0.9.
- [ ] **Open-source obligations:** the firmware is GPL-3.0-or-later and the boards CERN-OHL-S-2.0. Each unit ships with a written notice saying where the source and hardware files are (the firmware release already attaches its source bundle). Bundled tosu, espflash and LVGL notices are in the packages (`THIRD_PARTY_NOTICES.html`) and the firmware release (`FIRMWARE_THIRD_PARTY_NOTICES.md`).
- [ ] **The easter-egg GIF** is `LicenseRef-Unknown-ThirdParty`. In a sold product it needs a licence from its author, or it comes out of the release image.
- [ ] **Trademark:** "osu!" is ppy's. The disclaimer in `README.md:296` also goes on the store page and the box; product name, logo and photos must not suggest endorsement. Read ppy's brand guidelines before using "osu!" in marketing.
- [ ] **Consumer law (Spain/EU):** 3-year legal guarantee, 14-day withdrawal for online sales, terms of sale, privacy policy (the app contacts GitHub for updates: say so).

## 7. Manufacturing and per-unit QA (P1)

- [ ] Bill of materials frozen with a second source for every part that can go out of stock (the ferrite already did, V3 README).
- [ ] Incoming check of each Waveshare board (screen, touch, flash size, PSRAM).
- [ ] One documented, scripted procedure per unit: `opadctl recover` / full flash of the release image, `opadctl pad test` (and `--magnet` for HE), calibration for HE, 20 taps per key, a touch test, `opadctl status` saved with the serial number.
- [ ] Every unit leaves owned by no PC and with zero counters (or a documented factory state).
- [ ] Serial number / batch label on the unit and box (GPSR).
- [ ] 30-minute burn-in per unit (display animating, keys untouched, no phantom press).
- [ ] Box contents: pad, the right USB-C cable, quick-start sheet (download URL, recovery, safety, guarantee, open-source notice).

## 8. Support and operations (P1)

- [ ] Public docs a buyer can follow: quick start, install per OS, update, recovery (`docs/recovery.md`), troubleshooting (`docs/troubleshooting.md`), FAQ. Each re-checked against the release (0.11).
- [ ] Support contact and `SECURITY.md`; an RMA process and spare parts (modules, cables, case parts).
- [ ] A way to pull a bad release (`updates/policy.json` yank and rollout, release spec §9) rehearsed once on beta.
- [ ] Feed expiry: the weekly *Update feed* job succeeds (a feed expires 30 days after it is built), with a notification when it fails.
- [ ] Known issues list published with the release.

## 9. Sign-off

| Area | Date | Build | Who | Result |
|---|---|---|---|---|
| §0 fixed | | | | |
| §1 CI | | | | |
| §2 Release pipeline | | | | |
| §3 Pad, MX V3 | | | | |
| §3 Pad, HE V3 | | | | |
| §4 Mechanical | | | | |
| §5 Windows 10 / 11 | | | | |
| §5 Linux distros | | | | |
| §6 Compliance | | | | |
| §7 Manufacturing | | | | |
| §8 Support | | | | |
