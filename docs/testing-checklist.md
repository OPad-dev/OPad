# osu!pad Hardware & Release Verification Checklist

This checklist documents the manual hardware release verification procedure
(§34, §35) required prior to tagged releases. Every row is run by a person with
the pad in front of them; nothing here can be automated, and nothing here may be
marked from the other platform's result.

**Target Hardware:** Waveshare ESP32-S3-Touch-LCD-2.0  
**Tester:** GFerreiroS  

| Run | Date | Host | Firmware | Status |
|---|---|---|---|---|
| Linux | 2026-09-13 | Linux x86_64, kernel 6.x | v1.0.0 | Complete — but see the note below |
| Linux re-run (U-3a) | 2026-09-17 | Linux x86_64, kernel 7.2 | 1.0.0 on the two-slot table | Partial — §9 |
| Windows (§W4-2) | — | Windows 10/11 x86_64 | — | **Not run** |

> **The 2026-09-13 Linux column is stale for the counter rows.** It predates
> both the NVS config blob v2 → v3 change (§W3-2) and the two-slot partition
> table (§U-3a), so **HW-01, HW-02, FAIL-03 and STR-02 are no longer evidence**
> for the shipping firmware and are re-opened in §9. The rest of the Linux
> column is unaffected: nothing in those rows touches NVS layout or partitioning.

> **The Windows column below is empty on purpose.** It is filled in by running
> the tests on Windows hardware, not by reasoning from the Linux column.
> **HW-05** (HID-first on display wake) and **STR-02** (zero storage writes) are
> the invariant-critical pair: §W4-2 requires both to be re-verified on Windows
> rather than assumed.

Legend: **PASS** / **FAIL** / **NOT RUN** / **n/r** = not required on this
platform by §W4-2.

---

## 1. Physical Key Performance & Latency Invariants

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| HW-01 | Key 1 Press & Release | Tap Key 1 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **RE-OPENED** (§9) | **NOT RUN** |
| HW-02 | Key 2 Press & Release | Tap Key 2 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **RE-OPENED** (§9) | **NOT RUN** |
| HW-03 | Simultaneous Key Press | Press Key 1 + Key 2 concurrently within 1 ms window. | Both keys reported down and up without key ghosting or lockups. | **PASS** | **NOT RUN** |
| HW-04 | Rapid Alternating Stream | Stream alternating K1/K2 at > 20 presses/sec for 60 seconds. | 0 missed presses, 0 chatter/double-taps, p99.9 latency delta < 0.1 ms over baseline. | **PASS** | **NOT RUN** |
| HW-05 | Display Asleep -> Wake on Press | Allow display to sleep (10 min idle), then press Key 1. | **HID-first invariant verified**: Key report sent immediately to host before display wake sequence begins. No input delay. | **PASS** | **NOT RUN** |

---

## 2. Communication & Protocol Robustness

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| COM-01 | CDC Host Absent | Boot pad with host daemon stopped (pure USB HID host). | Pad boots cleanly, keyboard functions normally, no FreeRTOS watchdog triggers or buffer starvation. | **PASS** | **NOT RUN** |
| COM-02 | Malformed Protocol Frame | Inject random garbage bytes and invalid frame headers over `/dev/ttyACM0` using test script. | Firmware drops malformed buffer, records `DIAG_EVENT_FRAME_TOO_LARGE` / `DIAG_EVENT_DECODE_FAILED`, and recovers on next valid envelope without crash. | **PASS** | **NOT RUN** |
| COM-03 | Rapid USB Reconnect | Unplug and replug USB cable 20 times rapidly (1s interval). | Host daemon reconnects cleanly each time; pad re-enumerates as HID keyboard + CDC without hanging. | **PASS** | n/r |
| COM-04 | Daemon Kill During Play | Terminate `osupad-daemon` (`kill -9`) in the middle of active gameplay. | Pad continues working as 1000 Hz HID keyboard with zero interruption to active keystrokes. | **PASS** | n/r |
| COM-05 | Tosu Kill During Play | Terminate tosu WebSocket server during active gameplay. | Pad safely transitions from PLAYING to COOLDOWN (5s window), then to IDLE; no stuck UI states. | **PASS** | n/r |

---

## 3. Storage & Failure Isolation (§P0-1, §P2-12)

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| FAIL-01 | LCD Fail Fallback Build | Compile with `CONFIG_OSUPAD_TEST_FAIL_LCD=y` and flash to pad. | Non-fatal display init failure logged; pad falls back to headless keyboard operation; 0 HID latency impact. | **PASS** | n/r |
| FAIL-02 | NVS Fail Fallback Build | Compile with `CONFIG_OSUPAD_TEST_FAIL_NVS=y` and flash to pad. | Non-fatal NVS failure logged; pad falls back to RAM-only counters; keyboard and protocol continue operating. | **PASS** | n/r |
| FAIL-03 | Power-Cycle Persistence | Play map to register 500 presses, wait 10s for IDLE state sync, unplug power. Reconnect power. | Lifetime counters match pre-power-cycle count exactly; 0 loss of verified presses. | **RE-OPENED** (§9) | n/r |
| FAIL-04 | SQLite Degraded Mode | Revoke write permissions on SQLite database (`chmod 400 osupad.db`), run daemon. | Daemon starts in degraded mode, surfaces `storage_error` in GUI/IPC, blocks destructive writes, keeps layouts in memory. | **PASS** | n/r |

---

## 4. Endurance & Stress Testing

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| STR-01 | Extended Play Session | Execute continuous gameplay session for ≥ 2 hours with live tosu streaming and display active. | 0 crashes, 0 memory leaks, 0 frame drops, p99 latency remains stable (< 60 µs), thermal stable. | **PASS** | **NOT RUN** |
| STR-02 | Zero Storage Writes Invariant | Monitor disk I/O while playing a map and during the 5s cooldown window. | Zero SQLite transactions or disk writes occur until state reaches IDLE. Verified by `test_writes_blocked_guard`. | **RE-OPENED** (§9) | **NOT RUN** |

### Windows equivalents for the Linux tooling above

The procedures are written with the Linux tools. On Windows, substitute:

| Linux | Windows |
|---|---|
| `evtest` (HW-01…HW-05) | Microsoft's **Keyboard Tester** or any raw-input viewer that shows key-down and key-up separately. A text editor is **not** sufficient — it hides auto-repeat and stuck keys. |
| `/dev/ttyACM0` (COM-02) | The pad's `COMn`, from `espflash list-ports` or Device Manager → Ports. |
| `iotop` / `strace` on the SQLite file (STR-02) | Sysinternals **Process Monitor**, filtered to `osupad-daemon.exe` and path `osupad.db`. Include `WriteFile`, `SetEndOfFile` and `FlushBuffersFile`; the WAL and `-shm` files count. |
| `systemctl --user stop osupad-daemon` (COM-01) | End `osupad-daemon.exe` in Task Manager, or stop it from the tray. |
| `kill -9` (COM-04) | `taskkill /F /IM osupad-daemon.exe`. |

---

## 5. Windows platform integration (§W4-2)

These have no Linux counterpart and are new work, so every one starts NOT RUN.

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| WIN-01 | No driver hunt | Plug the pad into a Windows 10/11 machine that has never seen the installer. | Enumerates as an HID keyboard and a `usbser.sys` COM port with no prompt, no `.inf`, no Zadig. Typing works immediately. **This is the §0 invariant on Windows.** | **PASS** (2026-09-17) — Win 11 Pro 25H2 VM that has never seen the installer. `HID Keyboard Device` on inbox `hidclass.sys` and `USB Serial Device (COM3)` on inbox `usbser.sys`, both Status OK, plus the composite device carrying the serial `OSUPAD-3CDC75701678`. No prompt, no `.inf`, no Zadig. |
| WIN-02 | Second daemon refuses to start | Start `osupad-daemon.exe` twice. | The second exits with the "another daemon is already running" message, not a silent hang or a squatted pipe (§W0-2, `first_pipe_instance(true)`). | **PASS** (2026-09-17) — Second instance exits 1 with "Another osupad-daemon instance is already running at `\\.\pipe\osupad-ipc-S-1-5-21-…-1000`"; one process remains. The pipe name carries the user SID as §W0-2 requires. |
| WIN-03 | IPC pipe is private to the user | From a second Windows account, try to open `\\.\pipe\osupad-ipc-{sid}` of the first. | Access denied. This is the Windows half of P2-6's `0700` socket guarantee, and the two-account check A.7 deferred from W0-2. | **PASS** (2026-09-17) — From `tester2`: `Access to the path is denied.` From `osupad` (the owner): opens. `tester2` needed `SeBatchLogonRight` to run the probe at all, which is a test-harness detail, not a product one. |
| WIN-04 | Autostart values | Install, log out, log back in. | `HKCU\...\Run` holds exactly `osupad-daemon` → `"<dir>\osupad-daemon.exe"` and `osupad-gui` → `"<dir>\osupad-gui.exe" --tray` (§W1-1), and both are running. | **BLOCKED** — needs the installer, which bundles `osupad-gui`, which does not compile on Windows (§10.2). |
| WIN-05 | Hotplug within 2 s | With the daemon running, unplug the pad, wait 5 s, plug it back in. | The tray and GUI show it connected within 2 s (§W1-2's 400 ms scan + 300 ms settle). | **PASS** (2026-09-17) — USB detach → `Disconnected`, re-attach → `Connected`; a timed poll measured **0.75 s**, inside the 2 s budget. |
| WIN-06 | Flash with the GUI running | `osupadctl flash firmware\build\osupad-firmware.bin` without stopping the daemon first. | Succeeds. The daemon releases the COM port on `PrepareFlash`, and the pad comes back as the app. **This is §W1-3's acceptance criterion.** | **FAIL — VM only, see §11.3** (2026-09-17). The daemon released the port correctly and the flash began; it then died because **libvirt did not hot-attach the pad when its PID flipped** to the bootloader. Not a product defect and **not a pass either** — it needs bare metal or PCI passthrough of the whole USB controller. |
| WIN-07 | Recovery flash from a release | Follow `docs/recovery.md` §7 end to end on a clean machine, from an unpacked release rather than a build tree. | The pad comes back unclaimed, `osupadctl status` shows `Running Slot: ota_0`, and nothing in the doc turned out to be wrong. **This is §W3-4's acceptance criterion.** | **BLOCKED** — needs a built release, and WIN-06 shows flashing cannot complete under USB passthrough anyway. |
| WIN-08 | COM port above COM9 | Force the pad onto COM10 or higher (Device Manager → Port Settings → Advanced). | `osupadctl status`, `flash` and `bootloader` all still find and open it. | **PASS** (2026-09-17) — Forced to **COM15** via the device's `PortName`. Daemon logged "Opening osu!pad serial port at COM15" / "Connected to osu!pad on COM15" and `osupadctl status` returned the full pad state. `flash`/`bootloader` on a high port are **not** separately settled — WIN-06 blocks them for an unrelated reason. |

---

## 6. Install and uninstall leave nothing behind (§W2-3)

Run on a clean VM with a filesystem and registry snapshot taken before the
installer runs. Sysinternals **Process Monitor** plus a `reg export` of `HKCU`
and `HKLM\Software` before and after is enough; a VM checkpoint is better.

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| PKG-01 | Install diff | Snapshot, install, snapshot. | Every new path is under the install directory, `%LOCALAPPDATA%\osupad`, or the two `Run` values from WIN-04. Nothing is written outside them. | **NOT RUN** |
| PKG-02 | Uninstall diff | Uninstall, snapshot, compare against the pre-install snapshot. | The install directory is gone. The `Run` values are gone. No stray `HKCU\Software\osupad`, no Start Menu entry, no scheduled task, no service. | **NOT RUN** |
| PKG-03 | User data survives an uninstall, and is removable | Uninstall with the "keep my settings" default, then with the box ticked. | Default: `osupad.db` survives, so reinstalling keeps the lifetime counters. Ticked: it is removed too, and the uninstaller said so first. | **NOT RUN** |
| PKG-04 | The install-origin marker | After install, read `install-origin` from the install directory. | It contains exactly `windows`. §U-2a: an absent or unknown value makes every updater notify-only, which would silently disable app updates. | **NOT RUN** |
| PKG-05 | Bundled tosu is complete | After install, list the bundled tosu directory. | `tosu.exe`, `NOTICE` and `VERSION` are all present, and `VERSION` is one line holding the version number (§T-3, and U-1 cannot tell what is installed without it). | **NOT RUN** |
| PKG-06 | Uninstall while running | Uninstall with the GUI open and the daemon running. | Inno's `CloseApplications` stops them; no "file in use" prompt, no reboot required, nothing left behind. | **NOT RUN** |

---

## 7. Update verification (§W4-2b)

These are the paths that can break a working install, so each is run
deliberately rather than assumed from a successful update.

Prerequisite, **settled 2026-09-17**: §U-0.3's signing keypair exists and
`osupad_update::verify::MANIFEST_PUBLIC_KEY` holds its public half. The secret
key lives at `~/.config/osupad/osupad-manifest.key`, outside the repository.

Build the manifest and its detached signature with the signing tool, which
re-verifies its own output against the key compiled into the build:

```bash
cargo run -p osupad-update --bin osupad-manifest -- \
    --dist dist --base-url https://example.invalid/download \
    --firmware-version 1.0.0
```

Serve `dist/` over HTTP and point the daemon at it with
`OSUPAD_MANIFEST_URL=http://…/osupad-manifest.json`. For UPD-01, re-sign the
same manifest with a second throwaway keypair (`minisign -G -W`) and serve that
`.minisig` instead.

> **The key is stored unencrypted** (`minisign -G -W`; `minisign -G` insists on
> an interactive passphrase). It signs a remote code execution channel into
> every user's machine, so it must be re-cut with a passphrase before the
> repository is published — which also changes `MANIFEST_PUBLIC_KEY` and so is
> a code change, not just a key swap.

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| UPD-01 | A tampered manifest is rejected | Serve a manifest with one byte changed, or signed with a different key. | Rejected as a bad signature. Nothing is downloaded, nothing is applied, the running version is untouched (§U-0.3). | **NOT RUN** |
| UPD-02 | A tampered artifact is rejected | Keep the signature valid but serve an artifact whose bytes do not match its recorded SHA-256. | `HashMismatch`. The staged file is deleted, the previous version survives. | **NOT RUN** |
| UPD-03 | Interrupted download — tosu | Kill the daemon mid-transfer. Repeat for the app, and for the firmware. | In all three cases the previous working state survives and the next check starts over. A half-written binary is never installed (§U-0.5). | **NOT RUN** |
| UPD-04 | Nothing updates mid-map | Start a map with an update pending. | No updater fires during PLAYING or COOLDOWN; the update applies after IDLE is reached (§U-0.1). Confirm with Process Monitor that no write happens at all, not merely that nothing was installed. | **NOT RUN** |
| UPD-05 | A cancelled apply changes nothing | Cancel the polkit prompt (Linux) or the UAC prompt (Windows) during an app update. | The running version is unchanged and the update stays pending. | **NOT RUN** |
| UPD-06 | Firmware update preserves the counters | Note the lifetime counts, run `osupadctl firmware-update`, check them afterwards. | Identical. §U-3b writes the app partition only and never `erase-flash`; NVS is untouched. | **PASS** (2026-09-17, §9.3) — 1.0.0 → 1.0.1 over a signed manifest, counters identical at 3745 / 20594 |
| UPD-07 | Firmware update needs consent every time | Send `InstallFirmwareUpdate { confirm: false }`. Then run it twice in a row with consent. | The first is rejected outright. The second time still asks — consent is never remembered (§U-3b). | **PARTIAL** (2026-09-17, §9.3) — the no-consent half passes: `firmware-update` without `--yes` printed the offer and refused. "Asks again the second time" is **not** run. |
| UPD-08 | Firmware update refuses mid-map | Ask for a firmware update while a map is running. | Rejected, naming the map as the reason. Nothing is downloaded and the serial port is never released. | **NOT RUN** |
| UPD-09 | The pad is still a keyboard afterwards | After a firmware update, plug the pad into a machine with no osu!pad software at all. | It enumerates and types. **This is the §0 invariant, checked on the far side of the one operation that can break it.** | **NOT RUN** |
| UPD-10 | A wrong-chip image is refused | Record an ESP32-C3 image in the manifest, correctly signed and hashed. | Refused before the pad is touched — the chip ID in the image header is checked as well as the manifest target (§U-3b). | **NOT RUN** |
| UPD-11 | An interrupted flash is recoverable | Unplug the pad mid-write. | The pad does not run as a keyboard, and `docs/recovery.md` §7.6 brings it back. Confirms the honest failure mode is honest, and is the argument for U-3c. | **PARTIAL** (2026-09-17, §11.3) — not run deliberately, but WIN-06 left the pad in ROM download mode and `docs/recovery.md` §7.6 brought it back in one `osupadctl flash`. The mid-write unplug itself is untested. |

---

## 9. Linux hardware run, 2026-09-17 (U-3a, W3-2, U-3b)

Run on the pad with `OSUPAD-3CDC75701678`, counters **3745 / 20594** throughout.
Firmware built with ESP-IDF v5.5.2. Every row below was actually executed;
nothing here is inferred.

### 9.1 U-3a — the two-slot partition table on real hardware

| ID | Test | Result | Evidence |
|---|---|---|---|
| U3A-01 | Pre-flash export | **PASS** | `osupadctl export ~/osupad-backup-preU3a.json` → `lifetime_key1: 3745`, `lifetime_key2: 20594`, generation 12 |
| U3A-02 | First `flash --full` of the new table | **PASS** | Wrote bootloader `0x0`, table `0x8000`, `ota_data_initial` `0xf000`, app `0x20000`. No `erase-flash`. `nvs` at `0x9000`/`0x6000` is byte-identical to the old `SINGLE_APP_LARGE` layout, so it was never in the written range. |
| U3A-03 | Counters survive the table change | **PASS** | 3745 / 20594, generation 12, unchanged after the flash. **This is the whole point of the test and it held.** |
| U3A-04 | Running slot is reported | **PASS** | `osupadctl status` → `Running Slot: ota_0` |

### 9.2 W3-2 — the five ownership checks

The firmware logs claims with `ESP_LOGI`, and the pad forwards only
`diag_record` events, so **no claim, refusal or re-claim is visible to the
host at all**; `DeviceStatus.nvs_writes` (field 17) exists in the proto but
nothing host-side reads it either. These were therefore run by reading
`HelloAck.owner_id` back off the pad, which is the stored state itself and so
is stronger evidence than a log line would have been. Recorded as an
observability gap, not a functional one.

| ID | Test | Result | Evidence |
|---|---|---|---|
| W32-01 | Silent claim survives a replug | **PASS** (completed 2026-09-17, §11.2) | The *mechanism* is proven: the pad reported owner `f4fc8f69…` — this install's id — on a fresh daemon start, so the silent claim really was written to NVS, and no prompt appeared. **Completed via USB passthrough:** the pad was handed to the Windows VM and taken back, which unbinds it from the host, re-enumerates it and gives the daemon a fresh `/dev/ttyACM0`; it was also reflashed and rebooted in between. On reconnect there was **no prompt**, sync completed immediately and the counters were unchanged — the `aaf6ca9` regression, on a real re-enumeration. It still does **not** power-cycle the pad, so FAIL-03 remains open. |
| W32-02 | A re-claim writes nothing | **PASS** | On every reconnect after the first, the pad reported the owner it already had and the host sent no `ClaimOwnership` (host side, `Ownership::Ours`). The firmware's own `OWNER_CLAIM_ALREADY_OWNED` no-op is covered by `test_reclaim_by_the_same_host_writes_nothing`. |
| W32-03 | Takeover prompt with a wiped `app_state` | **PASS** | Deleting `install.id` and restarting produced `pending_takeover { device_id, device_key1: 3745, device_key2: 20594, pc_key1: 3745, pc_key2: 20594 }` over IPC, and counter sync stayed blocked until it was answered. Resolving it wrote the new owner — a later fresh start saw the pad as its own and resumed syncing, which is the `aaf6ca9` regression, re-confirmed. |
| W32-04 | A claim during PLAYING is refused | **PASS** | Drove the pad into PLAYING with `HostStatus { playing: true }`, sent a claim for a different owner: `owner_id` unchanged. **Control:** the identical claim applied once the pad returned to IDLE, so the refusal was the P1-3 state guard and not a broken claim path. |
| W32-05 | An all-zero claim is refused | **PASS** | Sent `ClaimOwnership { owner_id: [0; 16] }`: `owner_id` unchanged. There is no wire path to unpairing (§W3-4). |

The pad was restored to this install's owner id at the end of the run and
verified.

### 9.3 U-3b — host-driven firmware update

| ID | Test | Result | Evidence |
|---|---|---|---|
| UPD-06 | Firmware update preserves the counters | **PASS** | Built a 1.0.1 image, signed a manifest with the §U-0.3 key, served it, `osupadctl firmware-update --yes` → `Firmware updated from 1.0.0 to 1.0.1`, `Running Slot: ota_0`, counters still **3745 / 20594**. The pad was then flashed back to the committed 1.0.0. |
| UPD-07 | Consent is required | **PASS (first half)** | `osupadctl firmware-update` without `--yes` printed the full offer and refused: "A firmware update needs confirmation." The "asks again the second time" half is not yet run. |

**How to reproduce the update rows.** The updater requires HTTPS and pins
bundled Mozilla roots, so a plain local server is refused — correctly. This run
used a throwaway local CA, an HTTPS server on `127.0.0.1:8443`, and a daemon
built with `rustls-tls-native-roots` plus `SSL_CERT_FILE` pointing at the CA.
**That feature change was reverted and never committed**: only the TLS root
store differed, so the path under test — signature, hash, chip-id check, port
hand-off, flash, version read-back — was the shipping one. The alternative is a
real HTTPS release host. Nothing was added to production to make this testable.

### 9.4 Still open on Linux

| ID | Why it is not done |
|---|---|
| ~~W32-01 (replug half)~~ | **Done 2026-09-17** via USB passthrough — see §9.2 and §11.2. |
| HW-01, HW-02 | Need 20 deliberate physical taps each, watched in `evtest`. |
| FAIL-03 | Needs ~500 presses, then the power physically removed. |
| STR-02 | Needs a real map played while disk I/O is watched. |
| ~~Automatic backup~~ | **Done — see §9.5.** |

### 9.5 Automatic counter backup, end to end on a running daemon

`docs/recovery.md` §5.1. Driven with a stub tosu v2 server (state 2 for 25 s,
then state 5) on a second port, so a real play session ran through the real
daemon with the real pad attached. Nothing in the daemon knew it was a test.

| Observation | Result |
|---|---|
| During PLAYING (t = 5, 10, 15 s) | No backup, `<data>/backups/` empty — **P1-3 held** |
| During COOLDOWN (t = 20 s) | No backup |
| IDLE reached | t = 25 s; post-play sync settled 06:54:25Z |
| Backup written | 06:54:45Z — **exactly 20 s after IDLE**, seen between the t = 40 s and t = 45 s polls |
| File | `osupad-backup-20260917T065445Z.json`, a valid `format_version: 1` document with `3745 / 20594` and generation 15 |
| `osupadctl status` | `Last Backup: 2026-09-17T06:54:45Z` |
| Daemon log | `Automatic counter backup written: …/osupad-backup-20260917T065445Z.json (3745 / 20594)` |

Rotation to ten and cancellation by a new map inside the window are covered by
unit tests only — reproducing them live needs eleven sessions and a second
map, and the state machine exercised above is the same code.

---

## 10. W4-1 CI, run by hand on Windows (2026-09-17)

No CI job in this repository has ever executed — there is no remote (A.2).
These are the `.github/workflows/ci.yml` Windows job commands, run by hand in a
Windows 11 Pro 25H2 (build 26200) VM with Rust 1.98.1 MSVC and VS 2022 Build
Tools 14.44.

| Job / step | Result |
|---|---|
| `rust-windows` / `cargo fmt --check` | **PASS** |
| `rust-windows` / `cargo clippy --workspace --exclude osupad-gui --all-targets -- -D warnings` | **PASS** |
| `rust-windows` / `cargo build --workspace --exclude osupad-gui` | **PASS** |
| `rust-windows` / `cargo test --workspace --exclude osupad-gui` | **PASS** |
| `rust-windows-gui` / `cargo clippy -p osupad-gui --all-targets -- -D warnings` | **FAIL** — four errors, §10.2 |
| `rust-windows-gui` / `cargo build -p osupad-gui` | **FAIL** — same four |

**The eleven non-GUI crates are green on Windows**: they compile, lint clean
under `-D warnings`, and their whole test suite passes on a real windows-msvc
host. That is the first actual evidence for §W4-2 that the port works.

### 10.1 Three defects had to be fixed before any of it could run

1. **`protoc` was installed by no job at all** (`1c0fb3f`). `osupad-protocol`'s
   build script drives prost-build, which needs `protoc` on every platform and
   has not vendored it since 0.11. Neither the Ubuntu apt list nor the two
   Windows jobs provided it, so the first crate in the graph failed everywhere.
   **The Linux job was equally broken and nobody could have known.**
2. **The LVGL include macro could not survive MSVC** (`67f00fc`). MSVC strips
   the quotes from `-DX="..."`, so `#include LV_CONF_KCONFIG_EXTERNAL_INCLUDE`
   expanded bare and every LVGL file died with `error C2006`.
3. **`\\?\` verbatim paths broke the source list** (`cd33dc4`).
   `canonicalize()` returns the extended-length form on Windows; `cc` shortens
   long command lines by relative-ising source paths, and doing that to a
   verbatim path yielded `'\\lv_group.c'`.

### 10.2 `osupad-gui` — four Windows errors, for column B

These are in **B's files** and are left for B, per A.3. Exact locations:

| # | Where | Error |
|---|---|---|
| 1 | `gui/src/single_instance.rs:111:52` | `E0425`: `CreateMutexW` not found in `windows_sys::Win32::System::Threading` — the W0-3 mutex; looks like a missing `windows-sys` feature or a moved module path |
| 2 | `gui/src/single_instance.rs:86:26` | `E0308`: `if self.0 != 0` — expected `*mut c_void`, found `usize`. The handle is a pointer; compare against `std::ptr::null_mut()` |
| 3 | `gui/src/tray.rs:341:46` | `unused import: TrayIcon` — fatal under `-D warnings` |
| 4 | `gui/src/main.rs:358:17` | `E0560`: `iced::window::settings::PlatformSpecific` has no field `application_id` — that field is Linux-only and needs a `cfg` |

### 10.3 Still blocking CI regardless of the above

`osupad-ui-preview` needs `firmware/sdkconfig` and
`firmware/managed_components/lvgl__lvgl`, and **both are gitignored**, so the
crate cannot build from a clean checkout on any runner. `osupad-gui` depends on
it, so `--exclude` cannot hide it. This VM run only got past it because those
204 MB of build inputs were copied in by hand. Giving CI an ESP-IDF step,
vendoring LVGL at a pinned version, or letting the crate degrade without the
firmware tree are all defensible — **owner decision, not settled here.**

---

## 11. Windows run, 2026-09-17 — environment and caveats

### 11.1 The machine

Windows 11 Pro 25H2 (build 26200.8037) in a libvirt/QEMU VM: q35, UEFI with
Secure Boot, emulated TPM 2.0 (swtpm), 4 vCPU, 6 GB, SATA disk, e1000e NIC.
Installed fully unattended. Rust 1.98.1 `x86_64-pc-windows-msvc`, VS 2022 Build
Tools 14.44, protoc 36.0, Inno Setup 6.7.3. Two local accounts, `osupad`
(admin) and `tester2` (standard), created at install time for WIN-03.

**The installer was never run on this machine**, which is what makes WIN-01 a
real test of the §0 invariant rather than a formality.

### 11.2 The pad reached the VM by USB passthrough

Two `<hostdev>` entries with `startupPolicy='optional'`, one for `303a:4001`
(application) and one for `303a:1001` (ROM bootloader), attached live.

Because attach and detach are software operations, WIN-05's hotplug was done
without anyone touching the cable — and on the Linux side, handing the pad to
the VM and taking it back is a genuine USB re-enumeration, which is how
**W32-01's replug half finally got tested** (§9.2 is updated).

### 11.3 WIN-06 failed for an environmental reason, and that matters

`osupadctl flash` got as far as "Asking osupad-daemon to release the serial
port…", the pad rebooted into the ROM bootloader, and the flash then died with
`COM3 could not be opened`. The cause is the one §W4-2's own instructions
warned about: **`startupPolicy='optional'` governs VM start, not hot-attach.**
When the pad re-enumerated as `303a:1001`, libvirt left that entry
`missing='yes'` and handed the device back to the *host* instead of the guest.

- This is **not** evidence against §W1-3. The daemon's half — releasing the
  port on `PrepareFlash` — worked.
- It is **not a pass** either. Flashing from Windows is unverified.
- The pad was left in ROM download mode, exactly the recoverable state
  `docs/recovery.md` §7.6 describes, and was recovered from Linux in one
  `osupadctl flash`. The honest failure mode really is honest.
- **To settle WIN-06:** bare metal, or PCI-pass the whole USB controller. The
  latter was **not** attempted here: the pad shares bus 001 with this machine's
  webcam, Bluetooth radio and another HID device, so passing the controller
  through would strip them from the host mid-session.

### 11.4 Verified along the way, outside the numbered rows

- **§W0-4 paths on Windows.** The daemon resolved
  `C:\Users\osupad\AppData\Roaming\osupad\osupad.db` and
  `…\osupad\backups` with no configuration.
- **§W3-3 ownership across two machines, on real hardware.** The pad is owned
  by the Linux install. The Windows daemon saw a foreign pad, **paused counter
  sync** (`Last Sync: Never`) and printed the takeover warning with both
  counter pairs. Nothing was written and the counters were untouched — the pad
  kept working as a keyboard throughout, and it came back to Linux still owned
  by Linux.
- **The new `Ownership:` line in `osupadctl status`** is what made that legible
  on a machine with no GUI. Without it the pad looks connected and simply never
  syncs.

One cosmetic thing to be aware of: for a foreign pad, `osupadctl status` prints
the **host's** `Key Pins` (the local default, `GPIO14`/`GPIO9`), not the pad's
(`GPIO2`/`GPIO13`), because config is not synced from a pad this install does
not own. Correct behaviour, mildly misleading presentation.

---

## 8. Verification Sign-Off

- **Linux v1.0.0, 2026-09-13:** sections 1–4 verified and passing, **except**
  HW-01, HW-02, FAIL-03 and STR-02, which predate the NVS v2→v3 change and the
  two-slot partition table and are re-opened in §9.4.
- **Linux, 2026-09-17:** U-3a, four of the five W3-2 checks, and U-3b's happy
  path all pass on the pad — see §9. The rest need a person at the hardware.
- **Windows, 2026-09-17:** the port is real. WIN-01, WIN-02, WIN-03, WIN-05 and
  WIN-08 **pass** on Windows 11 Pro 25H2; the `rust-windows` CI job passes in
  full (§10). WIN-06 **fails for an environmental reason** (§11.3) and WIN-04 /
  WIN-07 are **blocked** on `osupad-gui`, which does not compile on Windows
  (§10.2, four errors handed to column B). Sections 1–4's Windows column and
  section 6 are still untouched — they need physical key presses, a real map,
  and an installer.
- **Updates (section 7):** unblocked — the §U-0.3 signing key exists as of
  2026-09-17 (A.8 settled). No row in section 7 has been run yet.
- **Latency:** see `docs/latency-testing.md`. The Linux table is still empty
  (P3-1), so there is no baseline for Windows to be compared against yet.

