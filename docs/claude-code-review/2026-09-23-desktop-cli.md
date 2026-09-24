# Code review: `desktop/cli`

- Date: 2026-09-23
- Command: `/code-review high desktop/cli`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched `desktop/cli`, so the review covers the whole crate (`Cargo.toml` + `src/main.rs`), traced into `opad-ipc`, `opad-device`, `opad-model`, the daemon handlers, and the scripts that consume `opadctl` output.
- Verification: findings were not re-verified after the review; treat each as *plausible* until checked against the code.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `monitor --follow` advances to `latest_seq`, skipping bursts larger than `limit` — `fixed`

**File:** `desktop/cli/src/main.rs:478`
**Category:** correctness

In `monitor --follow`, `since_seq` is advanced to the daemon's global `latest_seq`, but the daemon returns only the first `limit` entries after `since`, so any burst larger than `limit` in one 500 ms poll silently skips the remainder.

**Failure scenario:** Pad connects and the daemon emits 80 log lines within 500 ms while `opadctl monitor --follow --limit 50` is running: `log_hub.get_entries(Some(s), 50)` returns entries s+1..s+50 and `latest_seq = s+80`; the CLI sets `since_seq = Some(s+80)`, so entries s+51..s+80 are never printed and no gap is reported.

**Suggested fix:** Advance to `entries.last().map(|e| e.seq)` instead of `latest_seq`.

## 2. `opadctl bootloader` sends `FinishFlash` and waits 15 s for a reconnect that cannot happen — `fixed`

**File:** `desktop/cli/src/main.rs:554`
**Category:** correctness

`opadctl bootloader` sends `FinishFlash` after deliberately leaving the pad in the ROM bootloader, and the daemon handler blocks up to 15 s waiting for a `DeviceEvent::Connected` that cannot arrive.

**Failure scenario:** With the daemon running, `opadctl bootloader` prints "Resuming daemon device communication and verifying new firmware..." then hangs 15 s (daemon `ipc_handlers.rs:1040` timeout) before printing success. Same root cause makes a failed `opadctl flash` (pad left in bootloader) wait 15 s before `result?` surfaces the real flash error.

**Suggested fix:** The daemon needs a plain "resume, do not verify" request, or the CLI should only call `FinishFlash` when the pad is expected back as the app.

## 3. CLI death between `PrepareFlash` and `FinishFlash` leaves the daemon paused forever — `fixed`

**File:** `desktop/cli/src/main.rs:509`
**Category:** correctness

If the CLI dies between `PrepareFlash` and `FinishFlash` (Ctrl-C, kill, panic in espflash spawn), the daemon's device manager stays paused indefinitely — the CLI installs no signal handler and the daemon has no resume-on-client-disconnect.

**Failure scenario:** User runs `opadctl flash fw.bin`, espflash stalls, user presses Ctrl-C. `finish_flash` never runs; daemon `is_paused` remains true (only `ipc_handlers.rs:1023/1039` and `firmware_update.rs` ever call `resume()`), so the pad never reconnects, counters never sync, and `opadctl status` shows Disconnected until the daemon is restarted.

**Suggested fix:** Root fix is daemon-side: resume when the connection that sent `PrepareFlash` drops; a `tokio::signal::ctrl_c` guard in the CLI is the local mitigation.

## 4. Incompatibility message prints the IPC version as the device-protocol version — `fixed`

**File:** `desktop/cli/src/main.rs:227`
**Category:** correctness

The incompatibility message prints `IPC_PROTOCOL_VERSION` (the CLI↔daemon IPC version) as the "daemon protocol" the device's serial protocol was compared against, but the daemon actually compares `info.protocol_version` against a hardcoded `1` in `runtime.rs:340`.

**Failure scenario:** Both constants happen to be 1 today, so the text is coincidentally right. Bump IPC to v2 (or the serial protocol to v2 with IPC staying v1) and `opadctl status` reports wrong numbers pointing the user at the wrong component to update.

**Suggested fix:** The expected device-protocol version should come from the daemon (in `IncompatibleDevice`) or a shared `opad-protocol` constant, not the IPC constant.

## 5. Unrecognised `--level` / `--source` silently disables the filter — `fixed`

**File:** `desktop/cli/src/main.rs:435`
**Category:** correctness

An unrecognised `--level` or `--source` value is mapped to `None` and silently disables the filter instead of erroring.

**Failure scenario:** `opadctl monitor --level err` or `--source firmware` prints the full unfiltered log with no message, so the user believes there were no matching entries to exclude or, worse, that all shown lines are at the requested level.

**Suggested fix:** Derive `clap::ValueEnum` on `LogLevel`/`LogSource` (or a `FromStr` in `opad_model::log`) so clap rejects bad values; removes the two hand-written match tables.

## 6. Level/source filters applied after the daemon truncated to `limit` — `fixed`

**File:** `desktop/cli/src/main.rs:465`
**Category:** correctness

Level/source filters are applied client-side after the daemon has already truncated to `limit`, so `--limit N --level X` returns far fewer than N lines and the header count is wrong.

**Failure scenario:** Daemon holds 5000 entries of which 12 are ERROR, all older than the last 50. `opadctl monitor --level error` fetches the last 50 (all INFO), prints `=== OPad Monitor (Last 50 entries) ===` and then zero lines, so the user concludes there are no errors.

**Suggested fix:** Pass the filter to the daemon in `GetLogEntries`, or keep fetching until `limit` matching entries are collected.

## 7. Post-flash verification failure has different exit codes with/without daemon — `fixed`

**File:** `desktop/cli/src/main.rs:527`
**Category:** correctness

Post-flash verification failure is exit 0 with a warning when the daemon is present but exit 1 via `bail!` when it is absent, so the same outcome ("pad did not come back") yields different exit statuses.

**Failure scenario:** A CI/recovery script runs `opadctl flash fw.bin && echo ok`. With the daemon running and the pad failing to re-enumerate, the daemon returns `Error("Device did not reconnect within 15 seconds")`, the CLI prints a warning and exits 0, and the script reports success for a bricked pad; the identical situation without the daemon exits 1.

**Suggested fix:** Return an error in both branches (or make both a warning).

## 8. `opadctl setup` prints a repo-relative udev path and ignores the installed rule — `open`

**File:** `desktop/cli/src/main.rs:839`
**Category:** correctness

`opadctl setup` tells the user to `sudo cp packaging/linux/udev/70-opad.rules ...`, a repo-relative path that only exists in a source checkout, and never checks whether `/etc/udev/rules.d/70-opad.rules` is already installed by the package.

**Failure scenario:** A .deb/.rpm/tarball user runs `opadctl setup` from their home directory: the rule is already installed by the package, yet the tool prints "Recommended udev rule" plus a `cp` command that fails with "No such file or directory".

**Suggested fix:** Since the rule text is already embedded via `include_str!`, compare it with the installed file and print a `sudo tee` of the embedded content (or "already installed") instead.

**Status note (A8, 2026-09-24):** Verified: `run_setup` still prints `sudo cp packaging/linux/udev/70-opad.rules …` and never reads `/etc/udev/rules.d/70-opad.rules`. Not fixed in the Antigravity session. That session was limited to behaviour-preserving refactors and efficiency/packaging fixes, and this fix changes what `opadctl setup` prints and checks. It needs an explicit go-ahead, then a run with and without the rule installed.

## 9. Interactive y/N confirmation copy-pasted between `Import` and `FirmwareUpdate` — `fixed`

**File:** `desktop/cli/src/main.rs:602`
**Category:** simplification

The interactive y/N confirmation block (`IsTerminal` check, prompt, flush, `read_line`, trim/lowercase, `y|yes` compare) is copy-pasted verbatim between `Import` (lines 366-380) and `FirmwareUpdate` (lines 602-616).

**Suggested fix:** Extract `fn confirm(prompt: &str, non_tty_msg: &str) -> Result<bool>` and call it from both.

## 10. `if latest_seq > 0` guard reprints entry 0 every 500 ms — `invalid`

**Status note:** `LogHub` numbers entries from 1 (`next_seq` starts at 1), so `latest_seq == 0` only when nothing has been logged and the reply is empty; with one entry `latest_seq` is 1 and the cursor advanced. Nothing was reprinted. The guard is gone anyway with finding 1's fix (S11), which advances from the last entry received.

**File:** `desktop/cli/src/main.rs:477`
**Category:** correctness

The `if latest_seq > 0` guard keeps `since_seq` at `None` while the daemon has emitted exactly one entry (seq 0), so `monitor --follow` reprints that entry every 500 ms until a second one arrives.

**Failure scenario:** Freshly started daemon that logged a single line: `get_entries(None, limit)` returns `[entry 0]` with `latest_seq = 0`; the guard leaves `since_seq = None`, so each poll re-fetches and re-prints entry 0.

**Suggested fix:** Advance from the last printed entry's seq (which also fixes finding 1).
