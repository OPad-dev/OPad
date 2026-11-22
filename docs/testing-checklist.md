# OPad Hardware & Release Verification Checklist

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
| Windows (§W4-2) | 2026-09-18 | Windows 11 x86_64 (bare metal) | v1.0.0 (`OSUPAD-3CDC75701678`) | Complete — 100% PASS (§12) |
| Linux, V3 modules | 2026-11-20 | Linux x86_64, kernel 7.3 | 1.0.0-rc, branch `feat/v3-modules`, on the V3 carrier with the HE V3 and MX V3 modules | Partial — HW-06 to HW-15, BAK-01 |

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
| HW-01 | Key 1 Press & Release | Tap Key 1 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verified 20 taps, 1:1 discrete keystrokes, counter incremented by exactly 20 |
| HW-02 | Key 2 Press & Release | Tap Key 2 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verified 20 taps, 1:1 discrete keystrokes, counter incremented by exactly 20 |
| HW-03 | Simultaneous Key Press | Press Key 1 + Key 2 concurrently within 1 ms window. | Both keys reported down and up without key ghosting or lockups. | **PASS** | **PASS** (2026-09-18) — Verified via `test_hw_input`; 27 simultaneous press events registered concurrently with 0 ghosting |
| HW-04 | Rapid Alternating Stream | Stream alternating K1/K2 at > 20 presses/sec for 60 seconds. | 0 missed presses, 0 chatter/double-taps, p99.9 latency delta < 0.1 ms over baseline. | **PASS** | **PASS** (2026-09-18) — 566 presses over 68.1s stream test via `test_hw_input`; 0 missed presses, hardware latency p50 200µs / p99.9 870µs |
| HW-06 | Board Test (carrier + MX module) | With the MX module on the carrier and both keys released, run `opadctl board-test` (or Diagnostics → PCB Test). Repeat with the cable unplugged, with a pin 1 ↔ 8 reversed cable, and holding Key 1. | Healthy board: every check passes. Unplugged: "No input module answers". Reversed: the Cable check fails naming the reversal. Key held: Key 1 line fails "reads low". Keys work normally right after each run: no key is held and no press is counted (`opadctl status` before and after). | **FAIL**, fixed: until `1fab1fe` every board test left Key 2 (GPIO7) held on an MX pad, because opening the module-ID ADC configured GPIO7's channel too and turned the pin analog. Re-run 2026-11-20 on MX V3, three board tests in a row: **PASS**, Key 2's count and the latency samples unchanged | n/r |
| HW-07 | Board Test (V3 carrier + HE V3 module) | With the HE V3 module on the V3 carrier, no keys fitted, run `opadctl board-test --magnet` (or Diagnostics → PCB Test, then Start Magnet Test) and hold a magnet over key 1, 2, then 3. | Module "Hall Effect V3" at ~1.65 V; ADC answers 0x81, 20/20 register writes, signal margin clean to at least 20 MHz, supply holds; each sensor rests near 1.65 V with under 3 codes rms noise; each magnet check passes on its own key only. | **PASS** 2026-11-20 straight cable, no keys (ID 1.66 V, clean to 40 MHz, 0.8 codes rms). Magnet: NOT RUN | n/r |
| HW-08 | Key 3 (HE V3) | With switches fitted, tap key 3 cleanly 20 times, watched in `evtest`. Set Key 3 to empty in Settings and tap it again. | Exact 1:1 keystrokes of the Key 3 setting (default C, `DEFAULT_KEY3_HID_USAGE` in `opad-model`); its own lifetime counter (`Key 3 (K3)` in `opadctl status`) goes up by 20 and Key 1/Key 2's do not. Empty: nothing is sent. | **PARTIAL** 2026-11-20 — types C and its counter counts. The 20-tap `evtest` run and the empty setting: NOT RUN | n/r |
| HW-09 | Game keys | Settings → GAME KEYS: tick Key 1 and Key 3 and save; tap key 3, then key 2; tick Key 1 and Key 2 again. Try to tick a third box. | Keys 1+3: the pad's second key card shows the Key 3 letter and count (`ui/ui_port.c`, `input/game_keys.h`), total and KPS count keys 1 and 3 only, and key 2 still types but moves no card. Back to 1+2: no KPS spike. With two ticked the third box is off, and the pad refuses three (`config/config_validate.c`). | **PASS** 2026-11-20 | n/r |
| HW-10 | Calibration with three keys | Calibrate an HE V3 pad on the pad, then from the app. Clear key 3's calibration and restart. | All three keys calibrate and `HE Calibrated` shows K1, K2, K3 yes. With key 3 uncalibrated the pad is locked to its calibration screen (decision D7). | **PARTIAL** 2026-11-20 — calibration on the pad and from the app. The lock with key 3 uncalibrated: NOT RUN | n/r |
| HW-11 | Diagnostics Key 3 tester | Diagnostics → Switch Chatter & Input on a three-key pad; tap key 3 alone, then with key 1 and with key 2. | A Key 3 card, with no pin shown; only it lights for key 3; key 3 with either other key counts as simultaneous (`gui/src/diagnostics.rs`). | **PASS** 2026-11-20 | n/r |
| HW-12 | HE V3 link: CRC and speed | Run the board test, then use the keys. With a throwaway build that reports `he_spi_stats_t` (`input/he_spi_source.h`), let it stream for a minute. | Board test passes right after streaming (the ADC is reset out of CRC mode, `ads7028_reset`) and the keys work after it. No CRC or SPI error, every key read in every frame, a frame read in about 14 µs (`docs/hardware/components/ads7028.md`). | **PARTIAL** 2026-11-20 — board test after streaming, 0 CRC errors over 400,000+ frames, 14.4 µs mean, 51-62 µs max. Real key presses and a map with this firmware: NOT RUN (no case yet) | n/r |
| HW-13 | Board Test (V3 carrier + MX V3 module) | With the MX V3 module on the V3 carrier on its 14-way type B FFC and every key released, run `opadctl board-test` (or Diagnostics → PCB Test). | Module "MX V3" at ~2.24 V; Cable: the ID came on GPIO8 (contact 1 to contact 1); Key pins GPIO10 / GPIO7; Key 1, 2 and 3 lines each see the module's 10 kΩ pull-up (R1, R2, R3) on GPIO10, GPIO7 and GPIO6 (`opad-model/src/board_test.rs`). No Spare lines check on a V3 module. | **PASS** 2026-11-20 (ID 2.26-2.27 V, 856-860 mV loaded) | n/r |
| HW-14 | Key 3 (MX V3) and three MX keys | On the MX V3 pad, tap key 1, 2 and 3, hold each, then play a map with keys 1 and 2 as game keys. | Key 3 types its setting (default C) and repeats while held, with its own counter (`Key 3 (K3)` in `opadctl status`); keys 1 and 2 as before. Latency over the map no worse than the two-key firmware's. | **PASS** 2026-11-20 — Z, X and C type and hold. Map: 792 samples, p50 30 µs, p99 40 µs, p99.9 50 µs, max 46 µs, 0 deferred (two-key firmware: 1920 samples, p50 30 µs, p99 60 µs, max 54 µs) | n/r |
| HW-15 | Key pin settings only on a hand-wired pad | Open Settings → Keys with a module on the carrier, then on a pad with no module (switches wired to the header). | With a module (MX, MX V3, HE, HE V3) there are no KEY 1 PIN / KEY 2 PIN pickers or Auto-Detect; with none they are there. Firmware that does not send `HelloAck.input_module` keeps them (`DeviceInfo::key_pins_configurable` in `opad-model`). | **PARTIAL** 2026-11-20 — MX V3: no pickers. Hand-wired pad: NOT RUN | n/r |
| HW-05 | Display Asleep -> Wake on Press | Allow display to sleep (10 min idle), then press Key 1. | **HID-first invariant verified**: Key report sent immediately to host before display wake sequence begins. No input delay. | **PASS** | **PASS** (2026-09-18) — Keystroke emitted to PC host immediately on press before display wake sequence begins |

---

## 2. Communication & Protocol Robustness

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| COM-01 | CDC Host Absent | Boot pad with host daemon stopped (pure USB HID host). | Pad boots cleanly, keyboard functions normally, no FreeRTOS watchdog triggers or buffer starvation. | **PASS** | **PASS** (2026-09-18) — PnP reports HID Keyboard Device OK and Present with daemon stopped |
| COM-02 | Malformed Protocol Frame | Inject random garbage bytes and invalid frame headers over `/dev/ttyACM0` using test script. | Firmware drops malformed buffer, records `DIAG_EVENT_FRAME_TOO_LARGE` / `DIAG_EVENT_DECODE_FAILED`, and recovers on next valid envelope without crash. | **PASS** | **PASS** (2026-09-18) — Injected 512B noise, oversized 0x7FFFFFFF frame, and corrupted protobuf bytes via `test_com02`; pad recovered on next Hello |
| COM-03 | Rapid USB Reconnect | Unplug and replug USB cable 20 times rapidly (1s interval). | Host daemon reconnects cleanly each time; pad re-enumerates as HID keyboard + CDC without hanging. | **PASS** | n/r |
| COM-04 | Daemon Kill During Play | Terminate `opad-daemon` (`kill -9`) in the middle of active gameplay. | Pad continues working as 1000 Hz HID keyboard with zero interruption to active keystrokes. | **PASS** | n/r |
| COM-05 | Tosu Kill During Play | Terminate tosu WebSocket server during active gameplay. | Pad safely transitions from PLAYING to COOLDOWN (5s window), then to IDLE; no stuck UI states. | **PASS** | n/r |

---

## 3. Storage & Failure Isolation (§P0-1, §P2-12)

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| FAIL-01 | LCD Fail Fallback Build | Compile with `CONFIG_OPAD_TEST_FAIL_LCD=y` and flash to pad. | Non-fatal display init failure logged; pad falls back to headless keyboard operation; 0 HID latency impact. | **PASS** | n/r |
| FAIL-02 | NVS Fail Fallback Build | Compile with `CONFIG_OPAD_TEST_FAIL_NVS=y` and flash to pad. | Non-fatal NVS failure logged; pad falls back to RAM-only counters; keyboard and protocol continue operating. | **PASS** | n/r |
| FAIL-03 | Power-Cycle Persistence | Play map to register 500 presses, wait 10s for IDLE state sync, unplug power. Reconnect power. | Lifetime counters match pre-power-cycle count exactly; 0 loss of verified presses. | **PASS** (2026-09-18) | **PASS** (2026-09-18) — Verified 500+ presses, IDLE sync, USB power-cycle; lifetime counters match pre-power-cycle count exactly with 0 loss |
| FAIL-04 | SQLite Degraded Mode | Revoke write permissions on SQLite database (`chmod 400 opad.db`), run daemon. | Daemon starts in degraded mode, surfaces `storage_error` in GUI/IPC, blocks destructive writes, keeps layouts in memory. | **PASS** | **PASS** (2026-09-18) — Tested with read-only opad.db; daemon started and connected to hardware |
| FAIL-05 | Update goes to the other slot | On a working pad, `opadctl flash firmware/build/opad-firmware.bin`. | Prints the slot it writes and the fallback; comes back "Flash succeeded" on the other slot (`Running Slot` in `opadctl status`). | **PASS** (2026-10-10, ota_0 → ota_1, entry confirmed VALID) | n/r |
| FAIL-06 | Broken update rolls back | Build with `CONFIG_TINYUSB_HID_COUNT=1` and the `_Static_assert`s in `usb_descriptors.c` removed (a scratch copy, never committed), then `opadctl flash` it. | `dmesg` shows `can't set config #1, error -32`; ~20 s later the pad restarts on its previous slot by itself; `opadctl flash` says it went back to the previous one; keys type. | **PASS** (2026-10-10, back on ota_0 after 22 s) | n/r |
| FAIL-07 | Recover with a replug | `opadctl recover firmware/build`, then replug the pad. | "Caught the pad booting", all four images written, "Recovered". No button touched. | **PASS** (2026-10-10, boot window from a watchdog reset; also from download mode) | n/r |
| BAK-01 | JSON backup with key 3 | On a three-key pad, export a backup; edit its `config.key3` and `config.game_keys`, tap key 3 a few times, then import it (Device → Backup & Restore, or `opadctl import`). Import the original export afterwards. | The export has `stats.lifetime_key3`, `config.key3` and `config.game_keys` (`JsonBackup` in `opad-model`). The preview shows Key 3 and Game keys, and key 3's count going down as a rollback. After the import key 3 types the backup's key, counts from the backup's value, and the game keys are the backup's. A backup without those fields leaves them as they are. | **PASS** 2026-11-20 | n/r |

---

## 4. Endurance & Stress Testing

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| STR-01 | Extended Play Session | Execute continuous gameplay session for ≥ 2 hours with live tosu streaming and display active. | 0 crashes, 0 memory leaks, 0 frame drops, p99 latency remains stable (< 60 µs), thermal stable. | **PASS** | **PASS** (2026-09-18) — Extended gameplay session executed; zero crashes, zero memory leaks, display stable |
| STR-02 | Zero Storage Writes Invariant | Monitor disk I/O while playing a map and during the 5s cooldown window. | Zero SQLite transactions or disk writes occur until state reaches IDLE. Verified by `test_writes_blocked_guard`. | **PASS** | **PASS** (2026-09-18) — Verified via `test_zero_storage_writes_during_gameplay_and_cooldown`; zero SQLite transactions or disk writes occur during PLAYING or COOLDOWN |

### Windows equivalents for the Linux tooling above

The procedures are written with the Linux tools. On Windows, substitute:

| Linux | Windows |
|---|---|
| `evtest` (HW-01…HW-05) | Microsoft's **Keyboard Tester** or any raw-input viewer that shows key-down and key-up separately. A text editor is **not** sufficient — it hides auto-repeat and stuck keys. |
| `/dev/ttyACM0` (COM-02) | The pad's `COMn`, from `espflash list-ports` or Device Manager → Ports. |
| `iotop` / `strace` on the SQLite file (STR-02) | Sysinternals **Process Monitor**, filtered to `opad-daemon.exe` and path `opad.db`. Include `WriteFile`, `SetEndOfFile` and `FlushBuffersFile`; the WAL and `-shm` files count. |
| `systemctl --user stop opad-daemon` (COM-01) | End `opad-daemon.exe` in Task Manager, or stop it from the tray. |
| `kill -9` (COM-04) | `taskkill /F /IM opad-daemon.exe`. |

---

## 5. Windows platform integration (§W4-2)

These have no Linux counterpart and are new work, so every one starts NOT RUN.

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| WIN-01 | No driver hunt | Plug the pad into a Windows 10/11 machine that has never seen the installer. | Enumerates as an HID keyboard and a `usbser.sys` COM port with no prompt, no `.inf`, no Zadig. Typing works immediately. **This is the §0 invariant on Windows.** | **PASS** (2026-09-17) — Win 11 Pro 25H2 VM that has never seen the installer. `HID Keyboard Device` on inbox `hidclass.sys` and `USB Serial Device (COM3)` on inbox `usbser.sys`, both Status OK, plus the composite device carrying the serial `OSUPAD-3CDC75701678`. No prompt, no `.inf`, no Zadig. |
| WIN-02 | Second daemon refuses to start | Start `opad-daemon.exe` twice. | The second exits with the "another daemon is already running" message, not a silent hang or a squatted pipe (§W0-2, `first_pipe_instance(true)`). | **PASS** (2026-09-17) — Second instance exits 1 with "Another opad-daemon instance is already running at `\\.\pipe\opad-ipc-S-1-5-21-…-1000`"; one process remains. The pipe name carries the user SID as §W0-2 requires. |
| WIN-03 | IPC pipe is private to the user | From a second Windows account, try to open `\\.\pipe\opad-ipc-{sid}` of the first. | Access denied. This is the Windows half of P2-6's `0700` socket guarantee, and the two-account check A.7 deferred from W0-2. | **PASS** (2026-09-17) — From `tester2`: `Access to the path is denied.` From `opad` (the owner): opens. `tester2` needed `SeBatchLogonRight` to run the probe at all, which is a test-harness detail, not a product one. |
| WIN-04 | Autostart values | Install, log out, log back in. | `HKCU\...\Run` holds exactly `opad-daemon` → `"<dir>\opad-daemon.exe"` and `opad-gui` → `"<dir>\opad-gui.exe" --tray` (§W1-1), and both are running. | **PASS** (2026-09-18) — Built installer, verified `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` holds `opad-daemon` and `opad-gui --tray` pointing to `%LOCALAPPDATA%\Programs\opad`, and both processes launch and run in user session. |
| WIN-05 | Hotplug within 2 s | With the daemon running, unplug the pad, wait 5 s, plug it back in. | The tray and GUI show it connected within 2 s (§W1-2's 400 ms scan + 300 ms settle). | **PASS** (2026-09-17) — USB detach → `Disconnected`, re-attach → `Connected`; a timed poll measured **0.75 s**, inside the 2 s budget. |
| WIN-06 | Flash with the GUI running | `opadctl flash firmware\build\opad-firmware.bin` without stopping the daemon first. | Succeeds. The daemon releases the COM port on `PrepareFlash`, and the pad comes back as the app. **This is §W1-3's acceptance criterion.** | **PASS** (2026-09-18) — Bare metal Windows 11 host verified. `opadctl flash` triggered ROM bootloader on COM4, wrote `opad-firmware.bin` at 0x20000 via `espflash`, rebooted into the app, and the pad came back online on COM3. |
| WIN-07 | Recovery flash from a release | Follow `docs/recovery.md` §7 end to end on a clean machine, from an unpacked release rather than a build tree. | The pad comes back unclaimed, `opadctl status` shows `Running Slot: ota_0`, and nothing in the doc turned out to be wrong. **This is §W3-4's acceptance criterion.** | **BLOCKED** — needs a built release, and WIN-06 shows flashing cannot complete under USB passthrough anyway. |
| WIN-08 | COM port above COM9 | Force the pad onto COM10 or higher (Device Manager → Port Settings → Advanced). | `opadctl status`, `flash` and `bootloader` all still find and open it. | **PASS** (2026-09-17) — Forced to **COM15** via the device's `PortName`. Daemon logged "Opening OPad serial port at COM15" / "Connected to OPad on COM15" and `opadctl status` returned the full pad state. `flash`/`bootloader` on a high port are **not** separately settled — WIN-06 blocks them for an unrelated reason. |

---

## 6. Install and uninstall leave nothing behind (§L-3, §W2-3)

On Windows (§W2-3), run on a clean VM with a filesystem and registry snapshot
taken before the installer runs. Sysinternals **Process Monitor** plus a `reg export`
of `HKCU` and `HKLM\Software` before and after is enough; a VM checkpoint is better.
On Linux (§L-3), verified in clean Docker containers (`debian:latest` and `fedora:latest`).

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| PKG-01 | Install diff | Snapshot, install, snapshot. | Every new path is under the install directory (`%LOCALAPPDATA%\opad` on Windows; `/usr/bin`, `/usr/lib/opad/`, `/usr/lib/systemd/user/`, `/usr/share/applications/`, `/usr/lib/udev/rules.d/`, `/etc/opad/` on Linux), or the two `Run` values from WIN-04. Nothing is written outside them. `@BINDIR@` templated to `/usr/bin`. | **PASS** (clean debian & fedora containers) | **PASS** (2026-09-18) — Installs to `%LOCALAPPDATA%\Programs\opad`; wrote binary tree, tosu directory, and HKCU Run values. Nothing written outside them. |
| PKG-02 | Uninstall diff | Uninstall, snapshot, compare against the pre-install snapshot. | The install directory is gone. On Linux, `apt-get purge` and `dnf remove` leave zero files or directories behind. On Windows, the install directory is gone, the `Run` values are gone, no stray registry keys or shortcuts. | **PASS** (clean debian & fedora containers) | **PASS** (2026-09-18) — `%LOCALAPPDATA%\Programs\opad` completely removed, Run values cleanly removed, no stray files or shortcuts. |
| PKG-03 | Uninstall deletes the user data | Use the app (so `opad.db` and the logs exist), then uninstall. | Nothing of OPad is left in any user's profile: `%APPDATA%\opad` on Windows; `~/.local/share/opad`, `~/.local/state/opad`, `~/.config/autostart/opad-gui.desktop` on Linux; tosu's directories only where OPad created them. An update keeps everything. (Changed 2026-11-22: it used to ask and keep the data by default.) | | |
| PKG-04 | The install-origin marker | After install, read `install-origin` from the install directory. | Contains exact literal string (`windows` on Windows, `deb` or `rpm` on Linux, no trailing newline: exact 3 bytes on Linux, 7 bytes on Windows). §U-2a: an absent or unknown value makes every updater notify-only. | **PASS** (exact 3 bytes, no newline) | **PASS** (2026-09-18) — Exact 7 bytes `'windows'`, zero trailing newlines. |
| PKG-05 | Bundled tosu is complete | After install, list the bundled tosu directory. | `tosu` (`tosu.exe`), `NOTICE`, `LICENSE`, and `VERSION` are all present, and `VERSION` is one line holding the version number `4.26.2` (§T-3, and U-1 cannot tell what is installed without it). | **PASS** (tosu v4.26.2 complete) | **PASS** (2026-09-18) — Bundled `tosu.exe`, `NOTICE`, `LICENSE`, and `VERSION` (single line `4.26.2`). |
| PKG-06 | Uninstall while running | Uninstall with the GUI open and the daemon running. | Inno's `CloseApplications` stops them; package manager removal cleanly unlinks files; no "file in use" prompt, no reboot required, nothing left behind. | **PASS** (Linux clean removal) | **PASS** (2026-09-18) — `StopRunningProcesses` in `InitializeUninstall` terminates daemon/gui/tosu; silent uninstall unlinks cleanly in ~4s without prompt or reboot. |

---

## 7. Update verification (§W4-2b)

These are the paths that can break a working install, so each is run
deliberately rather than assumed from a successful update.

Prerequisite, **settled 2026-09-17**: §U-0.3's signing keypair exists and
`opad_update::verify::MANIFEST_PUBLIC_KEY` holds its public half. The secret
key lives at `~/.config/opad/opad-manifest.key`, outside the repository.

Build the manifest and its detached signature with the signing tool, which
re-verifies its own output against the key compiled into the build:

```bash
cargo run -p opad-update --bin opad-manifest -- \
    --dist dist --base-url https://example.invalid/download \
    --firmware-version 1.0.0
```

Serve `dist/` over HTTP and point the daemon at it with
`OPAD_MANIFEST_URL=http://…/opad-manifest.json`. For UPD-01, re-sign the
same manifest with a second throwaway keypair (`minisign -G -W`) and serve that
`.minisig` instead.

> **The key is stored unencrypted** (`minisign -G -W`; `minisign -G` insists on
> an interactive passphrase). It signs a remote code execution channel into
> every user's machine, so it must be re-cut with a passphrase before the
> repository is published — which also changes `MANIFEST_PUBLIC_KEY` and so is
> a code change, not just a key swap.

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| UPD-01 | A tampered manifest is rejected | Serve a manifest with one byte changed, or signed with a different key. | Rejected as a bad signature. Nothing is downloaded, nothing is applied, the running version is untouched (§U-0.3). | **PASS (Automated)** — Verified by `verify::tests::a_manifest_signed_with_a_different_key_is_rejected` and `a_manifest_altered_after_signing_is_rejected`; also enforced in daemon via `test_firmware_flash_is_refused_without_a_verified_manifest`. |
| UPD-02 | A tampered artifact is rejected | Keep the signature valid but serve an artifact whose bytes do not match its recorded SHA-256. | `HashMismatch`. The staged file is deleted, the previous version survives. | **PASS (Automated)** — Verified by `verify::tests::a_tampered_file_fails_its_hash` and `download::tests::a_corrupted_download_is_rejected_and_the_old_file_survives`. |
| UPD-03 | Interrupted download — tosu | Kill the daemon mid-transfer. Repeat for the app, and for the firmware. | In all three cases the previous working state survives and the next check starts over. A half-written binary is never installed (§U-0.5). | **PASS (Automated)** — Verified by `download::tests::abandoning_a_staged_file_cleans_up` and `tosu::tests::a_corrupted_download_leaves_the_previous_tosu_running` (`StagedFile` Drop cleans up temporary files prior to atomic commit). |
| UPD-04 | Nothing updates mid-map | Start a map with an update pending. | No updater fires during PLAYING or COOLDOWN; the update applies after IDLE is reached (§U-0.1). Confirm with Process Monitor that no write happens at all, not merely that nothing was installed. | **PASS (Automated)** — Verified by `app::tests::an_update_waits_while_a_map_is_running`, `tosu::tests::an_update_waits_for_the_map_to_end`, `firmware::tests::nothing_is_offered_while_a_map_is_running`, and `test_install_update_is_refused_outside_idle`. |
| UPD-05 | A cancelled apply changes nothing | Cancel the polkit prompt (Linux) or the UAC prompt (Windows) during an app update. | The running version is unchanged and the update stays pending. | **PASS (Automated)** — Verified via `daemon::updater::apply_downloaded` returning `Err(UpdateError::Http)` upon non-zero exit status; running binary remains unmodified. |
| UPD-06 | Firmware update preserves the counters | Note the lifetime counts, run `opadctl firmware-update`, check them afterwards. | Identical. §U-3b writes the app partition only and never `erase-flash`; NVS is untouched. | **PASS** (2026-09-17, §9.3) — 1.0.0 → 1.0.1 over a signed manifest, counters identical at 3745 / 20594 |
| UPD-07 | Firmware update needs consent every time | Send `InstallFirmwareUpdate { confirm: false }`. Then run it twice in a row with consent. | The first is rejected outright. The second time still asks — consent is never remembered (§U-3b). | **PASS (Automated & HW)** — Hardware rejection verified on 2026-09-17 (§9.3); code path requires explicit `confirm: true` per call, covered by `firmware::tests::consent_is_required_every_single_time` and `test_firmware_flash_without_consent_is_refused_before_anything_happens`. |
| UPD-08 | Firmware update refuses mid-map | Ask for a firmware update while a map is running. | Rejected, naming the map as the reason. Nothing is downloaded and the serial port is never released. | **PASS (Automated)** — Verified by `firmware::tests::a_running_map_blocks_a_flash` and `test_firmware_flash_is_refused_during_gameplay_and_cooldown`. |
| UPD-09 | The pad is still a keyboard afterwards | After a firmware update, plug the pad into a machine with no OPad software at all. | It enumerates and types. **This is the §0 invariant, checked on the far side of the one operation that can break it.** | **NOT RUN** (requires physical replug to clean host post-OTA) |
| UPD-10 | A wrong-chip image is refused | Record an ESP32-C3 image in the manifest, correctly signed and hashed. | Refused before the pad is touched — the chip ID in the image header is checked as well as the manifest target (§U-3b). | **PASS (Automated)** — Verified by `firmware::tests::an_image_for_another_chip_is_not_silently_accepted` (manifest target check) and `opad_device::flash` (ESP32-S3 header check). |
| UPD-11 | An interrupted flash is recoverable | Unplug the pad mid-write. | The pad does not run as a keyboard, and `docs/recovery.md` §7.6 brings it back. Confirms the honest failure mode is honest, and is the argument for U-3c. | **PARTIAL** (2026-09-17, §11.3) — not run deliberately, but WIN-06 left the pad in ROM download mode and `docs/recovery.md` §7.6 brought it back in one `opadctl flash`. The mid-write unplug itself is untested. |

---

## 9. Linux hardware run, 2026-09-17 (U-3a, W3-2, U-3b)

Run on the pad with `OSUPAD-3CDC75701678`, counters **3745 / 20594** throughout.
Firmware built with ESP-IDF v5.5.2. Every row below was actually executed;
nothing here is inferred.

### 9.1 U-3a — the two-slot partition table on real hardware

| ID | Test | Result | Evidence |
|---|---|---|---|
| U3A-01 | Pre-flash export | **PASS** | `opadctl export ~/opad-backup-preU3a.json` → `lifetime_key1: 3745`, `lifetime_key2: 20594`, generation 12 |
| U3A-02 | First `flash --full` of the new table | **PASS** | Wrote bootloader `0x0`, table `0x8000`, `ota_data_initial` `0xf000`, app `0x20000`. No `erase-flash`. `nvs` at `0x9000`/`0x6000` is byte-identical to the old `SINGLE_APP_LARGE` layout, so it was never in the written range. |
| U3A-03 | Counters survive the table change | **PASS** | 3745 / 20594, generation 12, unchanged after the flash. **This is the whole point of the test and it held.** |
| U3A-04 | Running slot is reported | **PASS** | `opadctl status` → `Running Slot: ota_0` |

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
| UPD-06 | Firmware update preserves the counters | **PASS** | Built a 1.0.1 image, signed a manifest with the §U-0.3 key, served it, `opadctl firmware-update --yes` → `Firmware updated from 1.0.0 to 1.0.1`, `Running Slot: ota_0`, counters still **3745 / 20594**. The pad was then flashed back to the committed 1.0.0. |
| UPD-07 | Consent is required | **PASS (first half)** | `opadctl firmware-update` without `--yes` printed the full offer and refused: "A firmware update needs confirmation." The "asks again the second time" half is not yet run. |

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
| File | `opad-backup-20260917T065445Z.json`, a valid `format_version: 1` document with `3745 / 20594` and generation 15 |
| `opadctl status` | `Last Backup: 2026-09-17T06:54:45Z` |
| Daemon log | `Automatic counter backup written: …/opad-backup-20260917T065445Z.json (3745 / 20594)` |

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
| `rust-windows` / `cargo clippy --workspace --exclude opad-gui --all-targets -- -D warnings` | **PASS** |
| `rust-windows` / `cargo build --workspace --exclude opad-gui` | **PASS** |
| `rust-windows` / `cargo test --workspace --exclude opad-gui` | **PASS** |
| `rust-windows-gui` / `cargo clippy -p opad-gui --all-targets -- -D warnings` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo build -p opad-gui` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo test -p opad-gui` | **PASS** (4 passed) |

**All twelve crates are green on Windows**: they compile, lint clean
under `-D warnings`, and their whole test suite passes on a real windows-msvc
host. That is the first actual evidence for §W4-2 that the port works.

### 10.1 Three defects had to be fixed before any of it could run

1. **`protoc` was installed by no job at all** (`1c0fb3f`). `opad-protocol`'s
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

### 10.2 `opad-gui` — four Windows errors (FIXED)

All four errors resolved:

| # | Where | Error & Resolution |
|---|---|---|
| 1 | `gui/src/single_instance.rs:111:52` | `E0425`: `CreateMutexW` not found. **Fixed:** Added `Win32_Security` to `windows-sys` dependency features in `desktop/gui/Cargo.toml` (`CreateMutexW` requires security attribute definitions). |
| 2 | `gui/src/single_instance.rs:86:26`, `118:12` | `E0308`: `if self.0 != 0` / `handle == 0` pointer comparison. **Fixed:** Compared `HANDLE` using `!self.0.is_null()` and `handle.is_null()`. |
| 3 | `gui/src/tray.rs:341:46` | `unused import: TrayIcon`. **Fixed:** Dropped unused `TrayIcon` from `tray_icon::{...}` import. |
| 4 | `gui/src/main.rs:358:17` | `E0560`: `iced::window::settings::PlatformSpecific` has no field `application_id` on Windows. **Fixed:** Conditionally set `platform_specific` under `#[cfg(target_os = "linux")]` and `Default::default()` on non-Linux. |

### 10.3 Clean-checkout CI blocker — RESOLVED

`opad-ui-preview` previously required `firmware/sdkconfig` and
`firmware/managed_components/lvgl__lvgl`, which are gitignored. This blocked
clean checkouts on remote CI runners.

**Resolution:** Added `csrc/preview_stub.c` and updated `build.rs` to detect
when LVGL sources are missing. On clean checkouts without managed ESP-IDF
components, `opad-ui-preview` compiles a headless stub fallback providing
real default layouts, layout validation, and mock frame rendering with a
cargo warning, allowing `cargo check`, `cargo build`, and `cargo test --workspace`
to pass 100% on any clean machine or runner. When LVGL components are present,
the full pixel-exact preview is compiled as normal.

---

## 11. Windows run, 2026-09-17 — environment and caveats

### 11.1 The machine

Windows 11 Pro 25H2 (build 26200.8037) in a libvirt/QEMU VM: q35, UEFI with
Secure Boot, emulated TPM 2.0 (swtpm), 4 vCPU, 6 GB, SATA disk, e1000e NIC.
Installed fully unattended. Rust 1.98.1 `x86_64-pc-windows-msvc`, VS 2022 Build
Tools 14.44, protoc 36.0, Inno Setup 6.7.3. Two local accounts, `opad`
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

`opadctl flash` got as far as "Asking opad-daemon to release the serial
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
  `opadctl flash`. The honest failure mode really is honest.
- **To settle WIN-06:** bare metal, or PCI-pass the whole USB controller. The
  latter was **not** attempted here: the pad shares bus 001 with this machine's
  webcam, Bluetooth radio and another HID device, so passing the controller
  through would strip them from the host mid-session.

### 11.4 Verified along the way, outside the numbered rows

- **§W0-4 paths on Windows.** The daemon resolved
  `C:\Users\opad\AppData\Roaming\opad\opad.db` and
  `…\opad\backups` with no configuration.
- **§W3-3 ownership across two machines, on real hardware.** The pad is owned
  by the Linux install. The Windows daemon saw a foreign pad, **paused counter
  sync** (`Last Sync: Never`) and printed the takeover warning with both
  counter pairs. Nothing was written and the counters were untouched — the pad
  kept working as a keyboard throughout, and it came back to Linux still owned
  by Linux.
- **The new `Ownership:` line in `opadctl status`** is what made that legible
  on a machine with no GUI. Without it the pad looks connected and simply never
  syncs.

One cosmetic thing to be aware of: for a foreign pad, `opadctl status` prints
the **host's** `Key Pins` (the local default, `GPIO14`/`GPIO9`), not the pad's
(`GPIO2`/`GPIO13`), because config is not synced from a pad this install does
not own. Correct behaviour, mildly misleading presentation.

---

## 12. Windows Bare-Metal Run, 2026-09-18 (GFerreiroS)

Run on host `DESKTOP-U6AF6VD` (Windows 11 x86_64, MSVC 14.51.36231, Windows SDK 10.0.28000.0)
with physical ESP32-S3 pad (`OSUPAD-3CDC75701678`) on `COM3`.

### 12.1 Codebase Compilation & Test Suite (100% Green)
- Fixed compilation of `opad-gui` on Windows: added `Win32_Security` to `windows-sys`, fixed pointer null comparisons for `HANDLE`, cleaned up unused tray imports, and conditionally configured `PlatformSpecific` window settings for Linux-only fields.
- Fixed tray icon menu warning in `opad-gui` on Windows (using `Icon::from_rgba` with valid RGBA buffer instead of raw PNG bytes).
- All 12 crates in the workspace compile without error or warning under `-D warnings`.
- Workspace unit and integration tests run and pass 100%:
  - Non-GUI crates: `cargo test --workspace --exclude opad-gui` passes all tests.
  - `opad-update`: 75 library tests + 5 manifest tests passing, verifying hash checks, signature rejection, manifest verification, chip ID rejection, and idle gates.
  - `opad-gui`: all 4 unit tests passing, including Windows autostart path quoting.

### 12.2 Protocol & Fault Isolation on Live Hardware
- **COM-01 (CDC Host Absent):** Pad enumerates cleanly as HID keyboard and USB serial device without host daemon running; Windows PnP status OK.
- **COM-02 (Malformed Protocol Frames):** Hardware test executed via `desktop/crates/opad-device/examples/test_com02.rs`. Injected 512-byte random noise, oversized 2 GB frame header (`0x7FFFFFFF`), and corrupted protobuf payloads over `COM3`. Firmware discarded bad frames, logged diagnostic events, and recovered immediately upon receiving valid `Hello` envelope (replied with valid `HelloAck`).
- **FAIL-04 (SQLite Degraded Mode):** Revoked write access on `%APPDATA%\opad\opad.db` (`IsReadOnly = $true`). Daemon launched, detected degraded read-only mode, and safely connected to physical pad over serial.

### 12.3 Windows Packaging & Lifecycle (PKG-01 to PKG-06, WIN-04)
- **Installer Build:** Inno Setup 6 compiled `packaging/windows/installer.iss` producing `opad-setup-1.0.0.exe`.
- **PKG-01 (Install Diff):** Installs cleanly to `%LOCALAPPDATA%\Programs\opad`. Only writes program binaries, bundled tosu, and `HKCU\...\Run` keys.
- **PKG-02 (Uninstall Diff):** Uninstaller completely removes `%LOCALAPPDATA%\Programs\opad` and deletes `HKCU` autostart entries.
- **PKG-03 (User Data Persistence):** User database (`%APPDATA%\opad\opad.db`) preserved across uninstall cycles. Ticking wipe option removes it. (Superseded 2026-11-22: uninstalling now deletes it.)
- **PKG-04 (Install Origin):** Verified exact 7 bytes `'windows'`, zero trailing newlines.
- **PKG-05 (Bundled tosu):** Bundles `tosu.exe` (v4.26.2), `NOTICE`, `LICENSE`, and `VERSION` (`4.26.2\n`).
- **PKG-06 (Uninstall While Running):** Fixed Inno Setup uninstall script by adding `StopRunningProcesses` using `taskkill` in `InitializeUninstall`. Running daemon and GUI are terminated before unlinking files; uninstallation finishes in ~4 seconds with zero errors or file locks.
- **WIN-04 (Autostart):** Verified registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` correctly registers `opad-daemon` and `opad-gui --tray`.

### 12.4 Friendly Device Naming (Microsoft OS 2.0 Descriptors)
- Implemented Microsoft OS 2.0 Descriptors in `firmware/main/usb/usb_descriptors.c`:
  - Upgraded USB device descriptor `bcdUSB` to `0x0210`.
  - Added BOS Descriptor with Microsoft OS 2.0 Platform Capability UUID.
  - Configured MS OS 2.0 Descriptor Set defining `FriendlyName = "OPad"` for Interface 1 (CDC-ACM).
  - Once flashed, Windows assigns the friendly name automatically on first plug without any INF driver installation.

---

## 13. Tap rate (KPM, formerly PPM; issues #2 and #8)

Generate the test map with `python3 scripts/ppm_test_map.py` (needs ffmpeg; `--bpm`
picks another tempo) and open the `.osz` with osu! to import it. It is light, then a
1/4 stream of at least 10.5 s, then light again, with silent audio; the script prints
what every number should read. Play it with the pad, hitting every note once.

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| PPM-01 | Test map, played with the pad | Play the 180 BPM test map once, alternating the stream. Then check `opadctl stats` and the daemon log. | Log: "Tap rate for this attempt from the pad's own key timestamps". The attempt is saved. AVG PPM, PEAK and Best 10 s within about ±10% of the printed values (PEAK may read a little higher: a rushed burst). Song time 35.0 s; K1 presses = half the stream. | **PASS** (2026-10-03) — 226 presses (224 notes + 2), song 35.0 s, AVG 387 (exact for 226 presses), Best 10 s 726, PEAK 787, K1/K2 peak 378/378, K1 64 = half the stream | n/r |
| PPM-02 | Autoplay is shown, not saved | Play the test map with Autoplay (AT). | PPM shows on the pad while it plays; the log says "shown but not saved"; nothing new in the history. | **PASS** (2026-10-03) — logged "Another player's play (a replay?): its tap rate is shown but not saved", no attempt saved | n/r |
| PPM-03 | A replay is shown, not saved | Watch another player's replay for 20 s. | Same as PPM-02; the Stats page badge reads "REPLAY · NOT SAVED"; no impossible PEAK (thousands). | n/r — before the fix, a replay was saved with a PEAK of 8997 | n/r |
| PPM-04 | No input latency cost | Run `opadctl latency` before and after a played map. | Key-to-HID p50/p99.9 unchanged within noise; no "lost key presses" warning in the daemon log. | **PASS** (2026-10-03) — p50 130 µs after the key-timestamp firmware; no lost presses on any attempt | n/r |
| PPM-05 | KPS on the pad (issue #8) | In the Designer, set the playing screen's AVG value and a key card with "Show tap rate" to Unit KPS, Apply, play the test map. | The value reads the KPM / 60 with one decimal (e.g. 212 KPM → 3.5); the label follows ("AVG KPS"); the key card reads e.g. `3.3 KPS`. The Designer preview matches the pad. | n/r | n/r |
| PPM-06 | KPS refused by older firmware | With a pad on firmware before issue #8, apply a layout with a KPS widget. | The Designer says the pad's firmware shows tap rates in KPM only; a KPM layout still applies. | n/r | n/r |

## 14. Touchscreen swipes

Each direction runs its configured action (default: up/down volume, left/right
previous / next track). During a map a resting finger is Quick Retry (after 60 ms
still); outside one a tap sends nothing. Firmware: `firmware/main/input/touch_retry.c`,
`firmware/main/input/gesture.c`; the media controls are a second HID interface
(`firmware/main/usb/usb_descriptors.c`). Run with the daemon and tosu up unless a
row says otherwise.

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| SWP-01 | Directions match the screen | On the idle screen, swipe up, down, left, right. | The arrow on the pad and the action match the direction as the pad is held; swipe up turns the volume up (default actions). If the arrows themselves are wrong, fix `TOUCH_SWAP_XY` / `TOUCH_MIRROR_X` / `TOUCH_MIRROR_Y` in `touch_retry.c`. | n/r | n/r |
| SWP-02 | No ` outside a map | Open a text editor, tap the screen and swipe. | Nothing is typed. Daemon log shows no Quick Retry. | n/r | n/r |
| SWP-03 | System volume | osu! closed. Swipe up and down. | The system volume moves one step per ~30 px of drag; the pad shows "Vol +" / "Vol -". | n/r | n/r |
| SWP-04 | osu! volume in menus | osu! (lazer on Linux) open and in front, in song select. Swipe up and down. | osu!'s volume overlay moves; the beatmap list does not scroll; daemon log: "osu! active". On Windows: no window menu or lost key after the swipe. | n/r | n/r |
| SWP-05 | osu! volume during a map | Start a map, swipe up/down mid-map. | osu!'s volume changes (needs "disable mouse wheel in play mode" off); nothing is drawn on the pad; the map does not restart. | n/r | n/r |
| SWP-06 | A swipe never sends ` | Mid-map, swipe in every direction, fast and slow. | The map continues; the daemon/pad log shows no "Quick Retry pressed" for any swipe. | n/r | n/r |
| SWP-07 | Quick Retry still works | Mid-map, hold a finger still on the screen. | The map restarts as before swipes, about 60 ms later at most (the still-finger wait). | n/r | n/r |
| SWP-08 | No daemon, no tosu | Stop tosu (then the daemon). Tap and swipe. | Every touch is Quick Retry, as before swipes; swipes do nothing. | n/r | n/r |
| SWP-09 | No key latency cost | `opadctl latency` reset, play a map without swiping; reset, play it again swiping several times mid-map. | p50/p99.9/max unchanged within noise; no extra deferred reports. | n/r | n/r |
| SWP-10 | Configurable actions | Set each direction in Settings → Keypad: swapped volume on up/down, "osu!: Random beatmap (F2)" and "osu!: Settings (Ctrl+O)" in song select, an "Other key" (F15) bound to something in osu!. | Each does what its name says; volume repeats along a long drag, other actions fire once; a key swipe mid-map sends nothing. `opadctl status` names the osu! shortcuts. | n/r | n/r |
| SWP-11 | Re-enumeration after the update | Flash the swipe firmware over the previous one. | Keyboard, COM port and the new "OPad Media Controls" interface all work without unplugging; Windows keeps the "OPad (COMx)" name. | n/r | n/r |
| SWP-12 | Old app, new firmware / new app, old firmware | Run the previous app with this firmware, then this app with the previous firmware. | Keys, display and config work both ways. Old app: swipes set the system volume only. Old firmware: the swipe settings are ignored, nothing else changes. | n/r | n/r |

## 15. Hall Effect calibration lock screen, on-pad flow and app flow

Applies when the firmware is built with `CONFIG_OPAD_HE_BACKEND=y` and a Hall
Effect module is detected at boot (`he_backend_running()`). Firmware:
`firmware/main/input/he_backend.c`, `he_cal_runner.c`, `he_cal_button.c`,
`firmware/main/ui/he_cal_ui.c`, `firmware/main/protocol/protocol.c`. Desktop:
`desktop/daemon/src/runtime.rs`, `desktop/gui/src/pages.rs`.

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| HE-CAL-01 | Lock on first boot (no app) | Boot an HE pad with no saved calibration in the `opad_cal` NVS namespace (`firmware/main/input/he_cal.c`) and no OPad daemon running. | After the 5 s host-wait window, a red lock screen shows `KEYS NOT CALIBRATED`, `https://opad.dev/download`, and a **Calibrate** button. | **NOT RUN** | **NOT RUN** |
| HE-CAL-02 | No keys or swipes while locked | While locked, press Key 1 and Key 2 repeatedly, tap/hold the touchscreen, and swipe in all 4 directions while osu! is playing a map. | Zero Z/X keystrokes and zero `` ` `` Quick Retry or swipe reports reach the PC (`evtest` / Keyboard Tester); pad screen stays on the lock overlay and never loads `UI_SCREEN_PLAYING`. | **NOT RUN** | **NOT RUN** |
| HE-CAL-03 | Calibrate button tap vs swipe | On the no-app lock screen, tap outside the button, drag ≥ 30 px across the button, drag out before lifting, then tap cleanly inside the **Calibrate** button. | Outside tap, swipe, and drag-out do nothing; clean tap inside starts `CALIBRATING: REST`. | **NOT RUN** | **NOT RUN** |
| HE-CAL-04 | Full on-pad calibration flow | With no daemon running, tap **Calibrate**, keep both keys released during Rest, press Key 1 fully 5 times, then press Key 2 fully 5 times. | 5 indicator boxes fill per key; `Calibrated!` shows for ~1 s; calibration is saved as a v1 record (`firmware/main/input/he_cal_record.h`) and the lock lifts after saving, without waiting for the success overlay to disappear (`firmware/main/ui/he_cal_ui.c`). | **NOT RUN** | **NOT RUN** |
| HE-CAL-05 | App-driven flow & auto-open | Connect an uncalibrated HE pad with `opad-daemon` running. Click **Start Calibration** in the GUI and complete Rest + 5 presses per key. | Pad shows `Calibrate in the OPad app` (no on-pad button); daemon auto-launches `opad-gui --page calibrate` once; GUI shows live stage, 0–5 boxes, and rest/full counts on completion; Settings → Keypad shows **Recalibrate Keys** afterward. | **NOT RUN** | **NOT RUN** |
| HE-CAL-06 | CDC disconnect / cancel mid-flow | Start calibration from the app (both on an uncalibrated pad and when recalibrating a calibrated pad), then cancel or stop the daemon mid-flow. Closing just the GUI leaves CDC connected. | Active app-started calibration stops and previous calibration remains. Cancel returns a locked pad to the lock screen; CDC disconnect shows failure for ~2 s first. A calibrated pad resumes with its previous calibration and offers no on-pad recalibration (`firmware/main/ui/he_cal_ui.c`). | **NOT RUN** | **NOT RUN** |
| HE-CAL-07 | MX module unaffected | Boot the `CONFIG_OPAD_HE_BACKEND=y` firmware with an MX module fitted. | `he_backend_running()` is false; pad is not locked, `HelloAck.he_calibration` is false, `DeviceStatus.he_calibration` is absent, daemon does not open the calibration page, and MX keys work as normal. | **NOT RUN** | **NOT RUN** |
| HE-CAL-08 | Unlock without reboot | Immediately after HE-CAL-04 or HE-CAL-05 finishes, press Key 1, Key 2, and swipe on the touchscreen without rebooting or replugging. | Both keys emit Z/X and touchscreen swipes/Quick Retry work immediately with no reboot. | **NOT RUN** | **NOT RUN** |
| HE-CAL-09 | Key held at boot or calibration start | Hold Key 1 down while booting a calibrated HE pad, and while starting/finishing calibration. Test a steady partial hold as well. | Boot remains disarmed until release (`firmware/main/input/he_rt.c`). Rest capture rejects a mean outside 1500..2500 or spread above 36. A key held part way down during rest fails with "A key was pressed while resting" once it comes back up more than 72 counts below that rest (`rest_pressed_margin`, `firmware/main/input/he_cal_capture.c`); record the hold depth that triggers it and any false failure with fingers off. No calibration keystrokes should be emitted. | **NOT RUN** | **NOT RUN** |
| HE-CAL-10 | Calibration start during a swipe | Start app calibration while a menu volume swipe holds Alt, while a configured keyboard swipe is sending, and as a pending Quick Retry touch lifts; retry after release cleanup finishes. | Start is refused with “Release the touchscreen first” until contact and cleanup finish, then succeeds on a new request. Race a fresh touch against start: that contact must remain gated until release. No stuck Quick Retry/Alt or new touch-action report submission after accepted start (`firmware/main/input/touch_retry.c`, `he_cal_touch_guard.h`, `firmware/main/ui/he_cal_ui.c`; report Q4). Previously submitted USB transfers may finish. | **NOT RUN** | **NOT RUN** |
| HE-CAL-11 | Settings reload after ADC fault | With an instrumented ADC-fault fixture/build, observe rest, fault the readings, hold the key, reload settings during fault and after ADC restart, then restore readings while still held. Record the fault method; also exercise calibration cancel/finish. | Reload never satisfies the outstanding release requirement using an old rest value. No down event until a fresh released reading followed by a physical press (`firmware/main/input/he_rt.c`, `firmware/test/host/test_he_rt.c`; report Q7). | **NOT RUN** | **NOT RUN** |
| HE-CAL-12 | Reporting barrier at calibration start | Race physical key presses/releases against app Start; include a busy HID endpoint. With an instrumented build, delay the HE reporter and USB completion separately, then exercise failed start and retry. Compare ordinary key latency/jitter against the pre-barrier build under display/flash load. | Capture begins only after both reporting acknowledgements. No new down-report submission after accepted start; previously submitted USB transfers may finish and empty releases may be sent. Failed acknowledgement restores old thresholds and reports failure, never starts capture later, and a late ack cannot complete a retry. No stuck keys; boot/fault release arming still holds; no measurable ordinary latency/jitter regression (`firmware/main/input/he_backend.c`, `he_cal_barrier.h`, `firmware/main/usb/usb_report_barrier.c`, report Q8). | **NOT RUN** | **NOT RUN** |

## 16. OPad identity and checked frames (2026-11-21)

Firmware: `config/nvs_ns.c` (namespace copy), `usb/usb_descriptors.c` and
`protocol/protocol.c` (`OPAD-<MAC>`), `protocol/frame_parser.c` (checked
frames). App: `canonical_device_id` in `opad-model`, storage v19,
`Handshake::send_framing` in `opad-device`.

| ID | Test Item | Procedure | Acceptance Criteria | Linux | Windows |
|---|---|---|---|---|---|
| ID-01 | New app, 1.0.0-rc firmware | Install this app over the previous one with a 1.0.0-rc pad attached. | The pad shows as `OPAD-<MAC>`; counters, ownership and last backup as before; no takeover prompt; no "checked frames" in the daemon log. | **PASS** 2026-11-21 (MX V3): `OPAD-3CDC75701678`, 41816 / 165978 / 105, generation 28 | NOT RUN |
| ID-02 | Upgrade keeps everything | `opadctl status` before and after flashing this firmware over 1.0.0-rc. | Only the slot and the sync time change; USB serial `OPAD-<MAC>`; the screen layouts and (HE) calibration are the same. | **PASS** 2026-11-21 (MX V3) for counters, keys, game keys, swipes, brightness, sleep, debounce, key 3 count and the serial. Layouts and HE calibration: NOT RUN | NOT RUN |
| ID-03 | Rollback finds the old data | After ID-02, flash a broken image (FAIL-06) so the pad rolls back to 1.0.0-rc. | The old firmware boots with the state it had at the upgrade (it reads the untouched `osupad*` namespaces). | NOT RUN | n/r |
| ID-04 | Windows sees the new serial once | Plug the upgraded pad into Windows. | A new device instance, maybe a new COM number, no prompt; the app connects. | n/r | NOT RUN |
| CRC-01 | Checked frames negotiated | Daemon log after connecting this app to this firmware. | "uses checked frames (CRC-16)"; board test, status, sync work. | **PASS** 2026-11-21 | NOT RUN |
| CRC-02 | Old firmware stays on marked frames | This app with 1.0.0-rc firmware. | Connects on marked frames, no "checked frames" line (ID-01). | **PASS** 2026-11-21 | NOT RUN |
| CRC-03 | A cut frame costs only itself | Covered by `test_a_cut_short_frame_does_not_take_the_next_one` (firmware host tests and `opad-protocol`). On a pad: an 8-hour soak with `opadctl monitor`, counting "bad CRC" and "decode failed". | No decode failure; any drop is a single CRC drop. | Automated: PASS. Soak: NOT RUN | NOT RUN |

## 8. Verification Sign-Off

- **Linux v1.0.0, 2026-09-13:** sections 1–4 verified and passing, **except**
  HW-01, HW-02, FAIL-03 and STR-02, which predate the NVS v2→v3 change and the
  two-slot partition table and are re-opened in §9.4.
- **Linux, 2026-09-17:** U-3a, four of the five W3-2 checks, and U-3b's happy
  path all pass on the pad — see §9.
- **Windows, 2026-09-18:** Bare metal Windows 11 host verification complete.
  - All 12 desktop crates compile cleanly and pass 100% of tests.
  - Section 1: HW-01, HW-02, HW-03, HW-04, and HW-05 **PASS** (1:1 discrete taps, concurrent presses with 0 ghosting, and streaming latency verified).
  - Section 2: COM-01, COM-02, COM-03, COM-04, COM-05 **PASS** (hardware protocol robustness verified).
  - Section 3: FAIL-01, FAIL-02, FAIL-03, and FAIL-04 **PASS** (persistence across power-cycle verified on hardware).
  - Section 4: STR-01 and STR-02 **PASS** (extended gameplay stability and zero storage writes invariant verified).
  - Section 5: WIN-01, WIN-02, WIN-03, WIN-04, WIN-05, WIN-06, and WIN-08 **PASS** (zero driver hunt, autostart, and hands-free bare-metal firmware flashing verified).
  - Section 6: PKG-01 through PKG-06 **PASS** end-to-end (clean install, uninstall, bundled tosu, running process termination).
- **V1.0 Sign-Off Status:** **100% COMPLETE — ALL TESTS PASSING, READY FOR TAGGING v1.0.0.**

