# Fix plan for the 2026-09-23 review findings

All 93 findings in this folder, assigned to exactly one executor, grouped into
clusters that share files, and ordered so later clusters build on earlier ones.

Finding IDs used below:

| Prefix | Findings file |
| --- | --- |
| FP | [2026-09-23-firmware-protocol.md](2026-09-23-firmware-protocol.md) |
| FU | [2026-09-23-firmware-usb-input-config.md](2026-09-23-firmware-usb-input-config.md) |
| FI | [2026-09-23-firmware-ui.md](2026-09-23-firmware-ui.md) |
| FB | [2026-09-23-firmware-test-boards.md](2026-09-23-firmware-test-boards.md) |
| DD | [2026-09-23-desktop-opad-protocol-device-ipc.md](2026-09-23-desktop-opad-protocol-device-ipc.md) |
| DU | [2026-09-23-desktop-opad-update.md](2026-09-23-desktop-opad-update.md) |
| DM | [2026-09-23-desktop-model-layout-storage-tosu-preview.md](2026-09-23-desktop-model-layout-storage-tosu-preview.md) |
| DA | [2026-09-23-desktop-daemon.md](2026-09-23-desktop-daemon.md) |
| DG | [2026-09-23-desktop-gui.md](2026-09-23-desktop-gui.md) |
| DC | [2026-09-23-desktop-cli.md](2026-09-23-desktop-cli.md) |
| PK | [2026-09-23-packaging-scripts-ci-protocol.md](2026-09-23-packaging-scripts-ci-protocol.md) |

`FP#3` means finding 3 in the firmware-protocol file.

## Executors and split

| Executor | Items | Why |
| --- | --- | --- |
| **Claude Opus** | 27 | Concurrency, protocol framing, cross-component state (foreign-pad, sync, device worker). Mistakes here brick pads or corrupt counters. |
| **Claude Sonnet** | 42 | Real correctness bugs with a clear spec, contained in one or two files. |
| **Claude Haiku** | 8 | One-to-five-line mechanical edits with no design decision. |
| **Antigravity** | 16 | Refactors/dedup/efficiency and packaging/CI scripts: mechanical but larger edits, no Anthropic budget spent. |

Coverage check: 27 + 42 + 8 + 16 = 93. Every finding appears in exactly one checklist.

## Run order

Sessions must be **sequential on the same files**; do not run two executors on
firmware at the same time. Antigravity can run in parallel on desktop/packaging
while Claude works on firmware (and vice-versa), *except* where a dependency is
noted.

1. **Phase 0 – Haiku**, one session, all 8 items (cheap, unblocks nothing but gets them out of the way).
2. **Phase 1 – Opus firmware** O1–O6.
3. **Phase 2 – Sonnet firmware** S1–S4.
4. **Phase 3 – Opus desktop** O7–O11.
5. **Phase 4 – Sonnet desktop** S5–S13.
6. **Phase 5 – Antigravity** A1–A9 (A9 packaging can start any time; A4 must wait for O9; A7 must wait for S9).
7. **Phase 6 – Opus optional** O12 only if budget remains.

## Rules for every session (Claude or Antigravity)

Paste these into the session along with the checklist for its tier.

1. Work only on your tier's checklist. When every item on it is `done` or `invalid`, **stop, print a summary (items fixed / marked invalid / skipped and why), and wait**. Do not start items from another tier.
2. Before fixing a finding, **verify it against the code**. If it does not reproduce or the premise is wrong, mark it `invalid` in its findings file with a one-line reason and move on. Do not "fix" something that is not broken.
3. Update the finding's status marker in its findings file (`open` → `fixed` / `invalid` / `wontfix`) as part of the same commit.
4. One commit per cluster (O1, S3, ...), message in the repo's conventional style (`fix(protocol): ...`, `fix(daemon): ...`). Author is GFerreiroS <info@gferreiro.com> only. **No `Co-Authored-By` or other trailers.**
5. **Protocol compatibility:** app and pad are updated independently. Any change touching serial framing, protobuf messages or their handling must keep working old-app↔new-pad **and** new-app↔old-pad.
6. **Firmware builds:** `make firmware` (runs `idf.py -C firmware build`). If you edit `firmware/sdkconfig.defaults`, delete `firmware/sdkconfig` first. Host unit tests: `./firmware/test/host/run_tests.sh` (gcc, `-Wall -Wextra -Werror`).
7. **Desktop checks:** `make check` (cargo fmt --check, clippy `-D warnings`, cargo test, firmware host tests). Must pass before committing.
8. Windows-only items (marked 🪟) need a run on the Windows test VM (see `docs/windows-portability.md`). If the VM is not available, implement, run `cargo check --target x86_64-pc-windows-gnu` if the target is installed, mark the item `fixed (untested on Windows)` and say so in the summary.
9. Stay in scope: fix the finding, add/adjust the test that covers it, nothing else. No drive-by refactors — those belong to Antigravity's list.
10. If a fix needs a decision the finding does not settle (e.g. which IPC field name), pick the option the finding suggests; if it suggests none, stop and ask.
11. **Budget stop (Claude only):** if `/cost` shows the session above the cap given at kickoff, finish the current item, commit, and stop with a summary.

---

## Claude Opus checklist (27 findings, 12 clusters)

Kickoff prompt:

> Read `docs/claude-code-review/FIX-PLAN.md`, follow "Rules for every session", and work through the **Opus checklist** in order, phase 1 (O1–O6) first. Budget cap for this session: $__.

### O1 — Firmware framing lock ▸ FP#1, FP#2
Files: `firmware/main/protocol/frame_parser.c/.h`, `firmware/main/protocol/protocol.c`, `firmware/test/host/test_frame_parser.c`
- [x] Add an "accepted framing" parameter to the parser (`ANY` / `LEGACY` / `MARKED`). While `ANY`, current behaviour. Once locked, headers of the other framing are **not** recognised; resync slides past them.
- [x] In `protocol.c`, latch `s_host_framing` **once per connection**: on the first frame whose payload decodes as a valid `HostMessage` with a recognised `which_payload` (Hello preferred). Never re-latch. Clear only in `protocol_reset_rx` (DTR drop / port reopen).
- [x] Pass `s_host_framing_known ? s_host_framing : ANY` into the parser on every feed.
- [x] Compat: an old (legacy-only) app must still connect and a new app talking marked must still connect; a mid-stream flip is the only thing removed. Reason through both directions in the commit message.
- [x] Host tests: extend `test_frame_parser.c` with (a) locked-to-marked ignores a false legacy header inside a payload, (b) locked-to-legacy ignores `AA 55 00 00`, (c) `ANY` still accepts both.
- Verify: host tests, `make firmware`, then real pad with the current desktop app for a full map (DataUpdate stream).

### O2 — `detect_pin` and key-GPIO ownership ▸ FP#3, FB#2, FU#7
Files: `firmware/main/protocol/protocol.c` (~551), `firmware/main/input/keypad.c` (~223, ~504), `firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c` (~19)
- [x] Clamp `DetectPinRequest.timeout_ms` (suggest `≤ 30000`, `0` → default).
- [x] The pin scan must not run on the CDC task: move `keypad_detect_pressed_pin` execution into the keypad task (request/response via the existing staging mechanism or a small queue), so `board_keys_set_gpio` has a **single owning task** as its header requires. The CDC task only enqueues and later sends `DetectPinResult`. Keep RX serviced meanwhile.
- [x] In `keypad.c` staged-config apply: commit `s_config = applied` only **after** `board_keys_set_gpio` succeeds; on failure keep the previous config, re-arm the previous pins, log + `diag_record`.
- [x] Same for the `keypad_init` fallback path (FU#7 mentions line ~270): whatever pins are actually armed must be what `keypad_get_config` reports.
- Verify: `make firmware`; on the pad: detect_pin from the app while sending HostStatus (protocol must stay responsive); detect → apply config sequence; deliberately invalid pin to exercise the failure path.

### O3 — HID report pending races ▸ FU#2, FU#6
Files: `firmware/main/usb/usb_hid.c` (~89, ~110)
- [x] `usb_hid_set_touch_retry` must not blindly clear `s_report_pending`. Use the same CAS/exchange discipline as the keypad path: only clear a pending flag that this call itself observed and satisfied, or make the submit path a single helper both callers use (`try_submit_or_mark_pending`).
- [x] `s_pending_edge_us`: only arm it if `s_report_pending` is still `true` after the failed submit (re-check with an atomic load / CAS on the pending flag); if `complete_cb` already delivered, drop the edge instead of leaking it into the next report.
- [x] Document the ownership: which tasks/cores touch `s_report_pending`, `s_pending_edge_us`, `s_key*_pressed`.
- Verify: `make firmware`; bench with `scripts/bench_latency.sh` for outliers; play a map with touch retry active, watch for stuck/missed keys and latency outliers in diag.

### O4 — Keycode/config application path ▸ FU#3, FU#4, FU#5
Files: `firmware/main/input/keypad.c` (~292), `firmware/main/config/device_config.c` (~128), `firmware/main/app_main.c` (~101, ~194)
- [x] `keypad_set_input_enabled(false)` must also stop `keypad_task` from reporting sampled levels: when disabled, force reported state = released (send one release report if anything was down) and skip steps 2/3 re-reads. HE module must never produce a held key.
- [x] Remove the direct `usb_hid_set_keycodes` call from `device_config_apply` (and the redundant one at `app_main.c:~101`); keycodes are applied **only** by the keypad task's staged apply.
- [x] `app_main.c:~194`: re-read the current config with `device_config_get()` before applying, or apply only brightness/sleep there. Do not push the stale snapshot.
- Verify: `make firmware`; pad with HE module → no key reports; change key1 while holding key1 from the app → no phantom press/release; connect app during boot and push config → pad keeps the new config.

### O5 — `ui_store` shared state and dirty-bit ▸ FI#4, FI#5
Files: `firmware/main/ui/ui_store.c/.h`, `firmware/main/runtime/runtime.c` (~49, ~54)
- [ ] Protect `s_pending_layouts[]`, `s_dirty_save_mask`, `s_dirty_erase_mask` with a mutex (FreeRTOS semaphore) or make the masks atomic **and** copy the pending layout under the same lock. Protocol task (core 0) and runtime task (core 1) both touch them.
- [ ] `ui_store_flush_dirty`: clear a dirty bit **only after** its save/erase succeeded; on failure keep the bit (retry next flush) and `diag_record` the error. Have `runtime.c` log the returned error instead of discarding it.
- Verify: `make firmware`; push a layout during gameplay, finish the map, reboot → layout persists. Host test if `ui_store` logic can be split out; otherwise pad test.

### O6 — Headless-mode UI guards ▸ FI#1, FI#6
Files: `firmware/main/protocol/protocol.c` (~522), `firmware/main/ui/ui.h` (~30), `firmware/main/ui/ui_port.c` (~229, ~338)
- [ ] `protocol.c` data_update source 67 → call `ui_trigger_easter_egg()` (which has the `s_ui_ok` guard), not `easter_egg_trigger()`.
- [ ] Make `ui_lock()` return `bool` (false when `!s_ui_ok`) and have every caller skip the LVGL work when it returns false; or audit every `ui_lock()` caller and guard each. Pick one and apply consistently.
- [ ] `ui_init`: if `s_screens[UI_SCREEN_IDLE] == NULL` after `rebuild_screen`, return `ESP_ERR_NO_MEM` (headless fallback) instead of `lv_screen_load(NULL)`.
- Verify: `make firmware`; force headless (disconnect LCD or stub `board_display_init` to fail) and send the easter-egg trigger (`scripts/trigger_easter_egg.py`) → no panic.

### O7 — Daemon foreign-pad guards ▸ DA#1, DA#2, DA#4, DA#6
Files: `desktop/daemon/src/runtime.rs` (~438, ~625), `desktop/daemon/src/ipc_handlers.rs` (~374, ~708), `desktop/daemon/src/firmware_update.rs` (~208), `desktop/daemon/src/sync.rs`, `desktop/crates/opad-ipc/src/lib.rs` (Status response)
- [ ] Introduce one predicate (e.g. `DaemonState::may_touch_pad()` = `!foreign_pad && pending_takeover.is_none() && pending_replacement.is_none()`) and use it everywhere a sync/push is triggered: the COOLDOWN→SYNC `TriggerSync` in `runtime.rs`, `ForceSync`, `UpdateConfig`, `SetLayout`/`ResetLayout`, `ResetCounters`, `RestoreDeviceFromPc`, `ImportPcFromDevice`, `ImportBackup`, and `firmware_update::install`'s pre-sync. Handlers return a clear error (`Error("Pad belongs to another install; resolve the takeover prompt first")`).
- [ ] Add `foreign_pad: bool` to `IpcResponse::Status` so GUI/CLI can gate. IPC change → bump `IPC_PROTOCOL_VERSION` only if the wire format is not backward compatible under the current serde settings; check how unknown/missing fields are handled and state the decision in the commit.
- [ ] After a successful takeover (`ipc_handlers.rs:~708`), also push the custom layouts that `DeviceConnected` suppressed (mirror the `SendLayout` loop at `runtime.rs:~438`).
- [ ] `firmware_update::install`: re-check `mode == IDLE` (and `may_touch_pad()`) **immediately before** `pause_and_release`/flash, after the download and stage complete; abort with an error if the user started playing.
- Verify: `make check`; unit tests for the predicate on the runtime state machine; manual: foreign pad + play a map → no sync; "leave it alone" then ForceSync → refused.

### O8 — Daemon connect/sync correctness ▸ DA#3, DA#5
Files: `desktop/daemon/src/runtime.rs` (~303–345), `desktop/daemon/src/sync.rs` (~340)
- [ ] `DeviceConnected`: perform the `protocol_version` check **first**. For an incompatible pad: do not set `device_connected`, do not adopt/save its config, do not set `counters_source`; set an `incompatible_device` marker (already exists for the CLI message) and return. `Tick` must not send `RequestDeviceStatus`/`HostStatus`/`TriggerSync` to it.
- [ ] `perform_sync` failure path: do not overwrite `st.counters` with the rejected reconciled values. Leave the in-memory counters as the pad reported them (or restore the pre-sync snapshot), keep `esp_counters`/`counters_source` consistent, and make the failure `SyncCompleted` carry the unchanged counters.
- Verify: `make check`; add a unit test feeding a `protocol_version = 2` `DeviceConnected` through `apply_event` and asserting no `SaveDeviceConfig` effect; sync-failure test asserting counters unchanged.

### O9 — `opad-device` serial worker re-Hello ▸ DD#2, DD#3
Files: `desktop/crates/opad-device/src/lib.rs` (~283–363)
- [ ] After the connection is established (`is_connected == true`), do **not** gate frame dispatch on `has_hello_ack`; only the pre-connection state drops non-HelloAck frames. A re-Hello re-confirms framing but must not discard `CounterSyncResp`/`ConfigAck`/`LayoutAck` in flight.
- [ ] Apply the `HELLOS_BEFORE_REOPEN` reopen/DTR-drop fallback regardless of `framing.is_some()`. If a re-Hello goes unanswered N times: reopen the port; if still unanswered: emit `Disconnected` so daemon/GUI stop showing "connected" with frozen counters.
- Verify: `make check`; existing worker tests; manual: with the pad connected, trigger `rehandshake()` during a sync (or simulate lag) and confirm the sync completes; unplug/replug and desync scenarios.

### O10 — 🪟 Windows pipe accept + bootloader port pick ▸ DD#5, DD#6
Files: `desktop/crates/opad-ipc/src/transport/windows.rs` (~105), `desktop/crates/opad-device/src/flash.rs` (~148)
- [ ] `IpcListener::accept`: after `connect()` succeeds, return the connected `server` **even if** creating the replacement instance fails (log it; create lazily on the next `accept`). In the `connect()` error/cancel path, ensure `next` is repopulated so the pipe name always has a listening instance.
- [ ] `enter_bootloader`: after a trigger fires and `pick` misses within the window, before running the next trigger's `wait_until_openable(app_port)`, re-check `bootloader_ports()` for the pad; if found, return it. Never let `PortBusy(app_port)` overwrite a `NoBootloader`-class error when the app port has vanished (prefer the more specific error).
- Verify: `make check`; cross-check `cargo check --target x86_64-pc-windows-gnu`; on the Windows VM: rapid GUI reconnects; first flash with slow driver install (or simulate by extending the window).

### O11 — tosu supervisor pause/resume ▸ DM#1, DM#4
Files: `desktop/crates/opad-tosu/src/lib.rs` (~435, ~541)
- [ ] `pause()` must await the child actually exiting: replace `drop(child)` with `child.kill().await` + `child.wait().await`, and signal completion with a `tokio::sync::Notify`/`watch` that `pause()` awaits (with a bounded timeout as a last resort, logged). Handle the "mid-`launch_tosu`" window: re-check `paused` after spawn and kill immediately if set.
- [ ] A stop caused by `paused` is not a crash: do not increase the backoff, and reset backoff on `resume()`.
- Verify: `make check`; add a test with a fake child (e.g. `sleep`) asserting `pause()` returns only after the pid is gone and that `resume()` relaunches promptly. Manual: run an app update with tosu running on Linux; Windows VM if available.

### O12 — (optional, last) `apply_event` state cloning ▸ DA#9
Files: `desktop/daemon/src/runtime.rs` (~799)
- [ ] Only if budget remains after phases 1–5. Remove the full `DaemonState` clone per event: let the controller borrow the state under the lock, or copy only the fields IPC handlers may write. Behaviour-preserving; existing daemon tests must pass unchanged.

---

## Claude Sonnet checklist (42 findings, 13 clusters)

Kickoff prompt:

> Read `docs/claude-code-review/FIX-PLAN.md`, follow "Rules for every session", and work through the **Sonnet checklist** in order. Phase 2 (S1–S4) only after Opus phase 1 is committed; phase 4 (S5–S13) only after Opus phase 3 is committed. Budget cap for this session: $__.

### S1 — Key-GPIO allow-list and debounce bounds ▸ FU#1, FU#8
Files: `firmware/main/input/keypad.c` (`SCAN_PINS`, ~433), `firmware/main/config/config_validate.c` (~8, ~15), `firmware/main/input/debounce.h`, `firmware/test/host/test_config.c`
- [ ] Remove GPIO8 (module-ID ADC divider, see `board.c:~212`) from both `SCAN_PINS` and `KEY_GPIO_ALLOWED`.
- [ ] Replace the two lists with **one** shared, host-safe array (e.g. `config_validate.h` exporting `const uint8_t KEY_GPIO_ALLOWED[]` + count) used by `keypad.c` for scanning and by `config_validate.c` for validation. Apply the `CONFIG_OSUPAD_BENCH_DEBUG_GPIO` exclusion in that single place.
- [ ] Delete the redefined `DEBOUNCE_MIN_US`/`DEBOUNCE_MAX_US` in `config_validate.c`; include `input/debounce.h`.
- [ ] Extend `test_config.c`: GPIO8 rejected; debug GPIO rejected when configured; all listed pins accepted.
- Verify: host tests, `make firmware`. Coordinate: O2 also edits `keypad.c` — rebase on it.

### S2 — Frame parser resync + protocol hygiene ▸ FP#4, FP#5, FP#6, FP#7
Files: `firmware/main/protocol/frame_parser.c`, `firmware/main/protocol/protocol.c` (~362, ~579–605), `firmware/test/host/test_frame_parser.c`
- [ ] Resync: scan with a head index (or `memchr` for `0xAA` / `00 00` pairs) and compact **once** after the loop instead of `drop_front(parser, 1)` per byte. Same accept/reject results as before (O1's locked-framing rules included) — the tests from O1 must still pass. Add a test feeding 8 KB of junk and asserting the parser resyncs on the frame that follows.
- [ ] Report resync: expose `resync_bytes` deltas and record them (e.g. `DIAG_EVENT_FRAME_TOO_LARGE` with `arg1`, or a new `DIAG_EVENT_FRAME_RESYNC`). Firmware only; no protocol change.
- [ ] Single `static void expire_stale_partial(int64_t now)` holding the `RX_STALE_US` check, reset, log and diag; call it from both `protocol_feed_cdc_bytes` and `protocol_rx_idle`.
- [ ] `layout_from_proto`: out-of-range `flags` → clamp to a **rejected** value (e.g. `UINT8_MAX` if the validator rejects it, otherwise reject the frame explicitly), consistent with the neighbouring `w`/`h` handling (fix those too if they clamp to 0).
- Verify: host tests, `make firmware`, send an oversized/junk stream to the pad and confirm the diag event appears in `opadctl`.

### S3 — Brightness/sleep ownership ▸ FI#2, FB#1
Files: `firmware/main/ui/ui_port.c` (~117, sleep/wake path), `firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c` (~42), `firmware/main/config/device_config.c` (~131)
- [ ] Make `ui_port`'s `s_brightness` the single source of truth. On wake, call `board_display_set_brightness(s_brightness)` (not the board's stale private copy).
- [ ] `device_config_apply` must **not** call `board_backlight_set` directly; route brightness through `ui_set_brightness`.
- [ ] `ui_set_brightness` must not read `s_asleep` unlocked from the protocol task: either take the LVGL lock, or set a "brightness changed" flag that `pad_timer_cb` applies on the LVGL task.
- Verify: `make firmware`; sleep the display, change brightness from the app, wake by key → new brightness; no backlight flash over a `disp_off` panel.

### S4 — UI small correctness ▸ FI#3, FI#8
Files: `firmware/main/ui/ui_port.c` (~87), `firmware/main/ui/ui_store.c` (~126)
- [ ] KPS ring: compute `total - oldest` as signed and clamp at 0; reset the ring when `total < oldest` (counter rebase/reset); track fill count instead of using 0 as "unfilled".
- [ ] `ui_store_erase`: when `nvs_erase_key` returns `ESP_ERR_NVS_NOT_FOUND`, return `ESP_OK` without `nvs_commit` and without `counters_record_nvs_write()`.
- Verify: `make firmware`; force-restore counters to a lower value while KPS shows → no `2147483647`; "Reset to default" twice → NVS write counter increments once.

### S5 — `opad-device` platform bugs ▸ DD#1, DD#4, DD#7
Files: `desktop/crates/opad-device/src/lib.rs` (~268, ~291, ~296, ~1086), `desktop/crates/opad-ipc/src/transport/unix.rs` (~14)
- [ ] 🪟 Replace every `Instant::now() - Duration::from_secs(10)` with `Option<Instant>` (`None` = "send Hello now") or `checked_sub`. Grep for other `Instant - Duration` in the crate.
- [ ] `locate_pad`: try `serial.and_then(|s| s.strip_prefix("OSUPAD-"))`, then fall back to `device_id` when that yields None (not only when serial is absent).
- [ ] `get_socket_path`: `std::env::var("XDG_RUNTIME_DIR").ok().filter(|v| !v.is_empty())`.
- Verify: `make check`; unit tests for `locate_pad` with a non-OSUPAD serial and for the empty-var case.

### S6 — Updater correctness ▸ DU#1, DU#2, DU#3, DU#4, DU#5, DU#6, DU#8
Files: `desktop/crates/opad-update/src/{client.rs, tosu.rs, version.rs, download.rs, bin/opad-manifest.rs}`, `desktop/daemon/src/updater.rs`, `desktop/daemon/src/backup.rs`
- [ ] `fetch_manifest`: send `If-None-Match` only when the caller holds a cached manifest (pass `Option<&ReleaseManifest>` or a `have_cached: bool`); on daemon start with no manifest in memory, do a full fetch. Alternatively clear `schedule.etag` when the manifest is not held — pick one and test the restart path.
- [ ] `tosu::install`: write VERSION/NOTICE **after** `install_to` succeeds (or roll them back on Err). Fix the doc comment.
- [ ] `opad-manifest` `app_artifact()`: add `.AppImage` (case-insensitive) → (`linux-x86_64`, `ArtifactKind::Binary`) + unit test.
- [ ] `fetch_artifact`: hash the in-memory bytes (`sha256_bytes`) — add a bytes variant of `check_hash`; delete `tempdir_for_check` and the temp-file round trip.
- [ ] `tosu::plan`: use `is_newer` (normalise a leading `v` on both sides) — never downgrade, never reinstall on a cosmetic mismatch.
- [ ] `version.rs`: semver-correct pre-release compare — split on `.`, numeric identifiers numerically, alphanumeric lexically, numeric < alphanumeric; strip `+build` before comparing. Tests: `rc10 > rc9`, `rc.10 > rc.9`, `1.0.0+build == 1.0.0` for ordering, release > pre-release.
- [ ] Extract `write_atomic(target, bytes)` in `download.rs` (temp + fsync + rename + dir sync); `stage_bytes` and `daemon/src/backup.rs` call it; use it for VERSION/NOTICE.
- Verify: `make check`; targeted unit tests for each bullet.

### S7 — Daemon updater/reset leftovers ▸ DA#7, DA#8
Files: `desktop/daemon/src/updater.rs` (~343, ~390), `desktop/daemon/src/ipc_handlers.rs` (~430), `desktop/daemon/src/sync.rs`
- [ ] AppImage self-replace: on `rename` failing with `EXDEV`, fall back to copy + set permissions + rename within the target dir; don't delete the staged file until the replacement succeeded.
- [ ] Run the package-manager `Command` via `tokio::process::Command` (or `spawn_blocking`) so a polkit prompt does not pin a runtime worker.
- [ ] `pending_device_push`: either delete the field and its two writes, or have `perform_sync` honour it with `force_restore = true`. Prefer honouring it (the reset is user intent); add a test that a `ResetCounters` while disconnected survives the next connect.
- Verify: `make check`.

### S8 — Model/tosu correctness ▸ DM#2, DM#5, DM#7
Files: `desktop/crates/opad-model/src/lib.rs` (~428–451), `desktop/crates/opad-tosu/src/lib.rs` (~268, ~460)
- [ ] `JsonBackup::validate`: build the `DeviceConfig` and delegate to `DeviceConfig::validate()`; remove the duplicated range checks. Test: `"0x00"` key rejected.
- [ ] `find_tosu_binary`: if `$OPAD_TOSU_PATH` is set but missing, log a warning naming the bad path and fall through to PATH/bundled candidates.
- [ ] `strip_ansi`: proper CSI (`ESC [` … final byte `0x40..=0x7E`) and OSC (`ESC ]` … `BEL` or `ESC \`) handling. Tests with chalk/ora-style sequences.
- Verify: `make check`.

### S9 — GUI main/designer bugs ▸ DG#1, DG#2, DG#4, DG#5, DG#7
Files: `desktop/gui/src/main.rs` (~476, ~791, ~1472, ~1493, ~2597), `desktop/gui/src/designer/mod.rs` (~380)
- [ ] `ShowRequested`: handle `"logs"` and `"diagnostics"` tokens (and any other `Page` variant `parse_page_arg` can produce — derive the mapping from one table used by both sides).
- [ ] Separate the local log counter from the daemon fetch cursor: `latest_log_seq` advances **only** from the daemon's returned `latest_seq`/last entry; local `log_event` uses its own counter.
- [ ] `ImportCompleted`: copy `gameplay_display_hz` too; audit for any other field missing versus the adopt path (make both use one `fn adopt_config(&mut self, &DeviceConfig)`).
- [ ] Designer `Apply`: carry `(screen, layout)` through `apply_layout` into `Message::Applied` and mark exactly that pair.
- [ ] `CloseRequested`: while `tray_available == None`, hide to tray (or defer the close until the probe resolves) instead of exiting.
- Verify: `make check`; run the GUI (`cargo run -p opad-gui`), test each scenario in its finding.

### S10 — GUI tray + single instance ▸ DG#3, DG#6, DG#8
Files: `desktop/gui/src/tray.rs` (~328), `desktop/gui/src/single_instance.rs` (~34, ~133)
- [ ] Tray: retry the `StatusNotifierWatcher` probe (e.g. every 5 s for the first minute, then every 30 s) or skip the pre-check and let ksni handle late registration; emit `Available` when it appears so the window can hide.
- [ ] 🪟 Windows: create the first pipe instance synchronously in `claim()` (before iced starts) so a second launch always finds a listener; surface the error instead of `if let Ok` swallowing it.
- [ ] Unix: bind first; only `remove_file` when `connect` fails with `ECONNREFUSED` (stale socket). Test with two near-simultaneous launches.
- Verify: `make check`; manual on Linux; Windows VM for the pipe.

### S11 — CLI monitor/status correctness ▸ DC#1, DC#10, DC#4, DC#5, DC#6, DC#7
Files: `desktop/cli/src/main.rs` (~227, ~435, ~465, ~477, ~478, ~527), `desktop/crates/opad-ipc/src/lib.rs` (`GetLogEntries`), `desktop/daemon/src/ipc_handlers.rs`
- [ ] `monitor --follow`: advance `since_seq` from the last **received** entry's seq (`entries.last()`), never from `latest_seq`; drop the `latest_seq > 0` guard. Fixes both DC#1 and DC#10.
- [ ] `--level`/`--source`: derive `clap::ValueEnum` on `LogLevel`/`LogSource` (in `opad-model`) so invalid values error out.
- [ ] Filtering: add optional `level`/`source` filters to `GetLogEntries` so the daemon filters before applying `limit`; CLI passes them through. Keep the request backward compatible (optional fields).
- [ ] Post-flash verification failure: same exit status with and without daemon — return an error in both branches.
- [ ] Incompatibility message: report the device-protocol version the daemon expects (add it to `IncompatibleDevice` or a shared constant in `opad-protocol`), not `IPC_PROTOCOL_VERSION`.
- Verify: `make check`; `opadctl monitor --follow` through a pad connect burst; `--level err` errors out.

### S12 — Flash flow resume ▸ DC#2, DC#3
Files: `desktop/cli/src/main.rs` (~509, ~554), `desktop/daemon/src/ipc_handlers.rs` (~1023–1040), `desktop/daemon/src/main.rs` (connection lifecycle), `desktop/crates/opad-ipc/src/lib.rs`
- [ ] Add an IPC request `ResumeDevice` (resume without waiting for reconnect). `opadctl bootloader` uses it instead of `FinishFlash`; a failed `opadctl flash` also uses it before surfacing the flash error.
- [ ] Daemon: track which IPC connection sent `PrepareFlash`; if that connection drops before `FinishFlash`/`ResumeDevice`, call `resume()` automatically and log it.
- [ ] CLI: `tokio::signal::ctrl_c` guard around the flash so Ctrl-C sends `ResumeDevice` before exiting.
- Verify: `make check`; `opadctl bootloader` returns immediately; Ctrl-C during `opadctl flash` → `opadctl status` shows the pad reconnecting.

### S13 — Release manifest signing ▸ PK#1
Files: `scripts/release/build_release.sh` (~17, ~128), `.github/workflows/release.yml`, `scripts/release/build_packages.sh`
- [ ] The manifest must be generated and signed over the **exact artifacts that are published**. Restructure so `build_release.sh` (or a new `sign_release.sh`) takes an existing `dist/` (the zigbuild outputs + packages) and never `rm -rf dist` before signing; the workflow runs build → packages → manifest+sign → publish in that order on the same files.
- [ ] Add a CI assertion that every artifact listed in the manifest exists in the upload set with a matching sha256.
- Verify: run the script locally against a `dist/` populated by `make packages`; `opad-manifest` verify step passes; dry-run the workflow with `act` if available, otherwise reason through it in the commit message.

---

## Claude Haiku checklist (8 findings)

Kickoff prompt:

> Read `docs/claude-code-review/FIX-PLAN.md`, follow "Rules for every session", and do the **Haiku checklist**. Each item is a small mechanical edit; do not change behaviour beyond what is listed. One commit for all items is fine (`chore: small review cleanups`). Budget cap for this session: $__.

- [ ] **H1 ▸ FP#8** `firmware/main/protocol/protocol.c:~36` — remove `volatile` from `s_host_framing_known` / `s_host_framing`; add a comment that protocol state is owned by the CDC task. Verify `make firmware`.
- [ ] **H2 ▸ DD#10** `desktop/crates/opad-ipc/Cargo.toml:~17`, `src/lib.rs:~22` — remove unused `bytes` and `byteorder` deps and the `MAX_IPC_FRAME_SIZE` alias (confirm zero references with grep first). Verify `make check`.
- [ ] **H3 ▸ DU#7** `desktop/crates/opad-update/src/http.rs:~43` — after `send()`, assert `resp.url().scheme() == "https"` and return an error otherwise, so the comment becomes true. Verify `make check`.
- [ ] **H4 ▸ DM#3** `desktop/crates/opad-model/src/log.rs` — `Display` impls for `LogSource`/`LogLevel` use `f.pad(..)` instead of `write!`. Add a test `format!("[{:<7}]", LogSource::Tosu) == "[TOSU   ]"`. Verify `make check`.
- [ ] **H5 ▸ PK#2** `packaging/linux/appimage/AppRun:~6` — `LD_LIBRARY_PATH="...${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"`. Verify with `shellcheck` if available and by `LD_LIBRARY_PATH= sh -x AppRun` showing no trailing colon.
- [ ] **H6 ▸ FI#10** `firmware/main/ui/easter_egg.c:~29, ~73–74, ~119–120`, `firmware/main/ui/core/ui_screen.c:~243` — delete `easter_egg_is_active()` (confirm no callers), drop `lv_obj_set_user_data(count, card)`, keep a single opa setter. Verify `make firmware`.
- [ ] **H7 ▸ DM#6** `desktop/crates/opad-ui-preview/build.rs:~54, ~97` — move the `rerun-if-changed` for `firmware/main/ui/core` above the stub early-return. Verify `cargo build -p opad-ui-preview` twice after touching `ui_ids.h` rebuilds.
- [ ] **H8 ▸ DM#9** `desktop/crates/opad-model/src/paths.rs:~97` — `install_lib_dir()` composes `install_prefix()`: `let prefix = install_prefix()?; Ok(if cfg!(windows) { prefix } else { prefix.join("lib").join("opad") })`. Existing test `the_install_lib_dir_sits_under_the_prefix` must pass. Verify `make check`.

---

## Antigravity checklist (16 findings, 9 clusters)

Kickoff prompt (paste into Antigravity):

> This repo has a code-review fix plan at `docs/claude-code-review/FIX-PLAN.md`. Follow its "Rules for every session" exactly (verify each finding before fixing, update the status marker in the findings file, one commit per cluster, author GFerreiroS <info@gferreiro.com>, no trailers, `make check` / `make firmware` must pass). Do the **Antigravity checklist** only, in order; skip A4 until cluster O9 is committed and A7 until S9 is committed (check `git log`). All items are behaviour-preserving refactors, efficiency fixes or packaging fixes — if you find you need to change behaviour, stop and report instead. When the checklist is done, stop and print a summary.

### A1 — Firmware dead/duplicated code ▸ FU#9
Files: `firmware/main/usb/usb_cdc.c` (~140–147), `firmware/main/config/device_config.c` (~207), `firmware/main/counters/counters.c` (~148), `firmware/main/input/latency_stats.c`
- [ ] Delete the stale comment block superseded by the one that follows it.
- [ ] Remove `device_config_set`'s IDLE check (it duplicates `write_to_nvs`'s).
- [ ] Move the mid-file `#include` to the top of `counters.c`.
- [ ] Remove the unread `s_samples` counter from `latency_stats`.
- Verify: `./firmware/test/host/run_tests.sh`, `make firmware`.

### A2 — Touch retry boot scan and idle polling ▸ FU#10
Files: `firmware/main/input/touch_retry.c` (~150, task loop), `firmware/main/Kconfig.projbuild`
- [ ] Gate the 126-address I2C scan behind a new Kconfig option (default off).
- [ ] In the task loop, read the CST816 register only when INT is active or a touch was seen in the last ~100 ms; otherwise sleep without an I2C transaction.
- Verify: `make firmware`; on the pad, touch retry still works (tap the screen during a map), boot log no longer shows the scan, no periodic NACK errors.

### A3 — UI efficiency + latent lifetime ▸ FI#7, FI#9
Files: `firmware/main/ui/core/ui_screen.c` (~341), `firmware/main/ui/ui_port.c` (~92, `rebuild_screen`)
- [ ] Free the per-screen `ui_layout_t` copy **after** `lv_obj_delete(old)` returns in `rebuild_screen` (and on final teardown), not in the screen's `LV_EVENT_DELETE` handler. Observers must never see a dangling `user_data`.
- [ ] Clock/date: reformat only when `t / 60` changes (or use a 1 s `lv_timer`); keep `lv_subject_copy_string` behaviour identical.
- Verify: `make firmware`; layouts switch Idle↔Playing repeatedly without crash; clock still updates on the minute.

### A4 — `opad-device` dedup (after O9) ▸ DD#8, DD#9
Files: `desktop/crates/opad-device/src/lib.rs` (~300–363, ~731–749, ~783–802, ~955)
- [ ] Extract `fn device_config_from(c: &proto::ConfigPayload) -> DeviceConfig` (including the `== 0 → DEFAULT_KEYn_GPIO` and `tosu_endpoint` defaults) and use it in both the HelloAck and ConfigAck arms.
- [ ] Extract one `handshake(port, window) -> Option<(HelloAck, Framing)>` used by both `probe_port` and the worker loop. **Behaviour-preserving**: the worker's timing rules win (they are the ones the O9 fix tested); document any difference from the old `probe_port` timing in the commit and confirm `opadctl status`/GUI still finds the pad on cold plug.
- Verify: `make check`; plug/unplug the pad, probe finds it; connect with the current firmware.

### A5 — Daemon main.rs dedup ▸ DA#10
Files: `desktop/daemon/src/main.rs` (~175–209)
- [ ] One `fn classify_tosu_line(&str) -> LogLevel` used by both the startup log-tail and the live callback (input is already ANSI-stripped by the supervisor; do not strip twice).
- [ ] Startup banner uses `env!("CARGO_PKG_VERSION")`.
- Verify: `make check`.

### A6 — ui-preview / tosu dedup ▸ DM#8, DM#10
Files: `desktop/crates/opad-ui-preview/src/lib.rs` (~8, `sample_values`), `desktop/crates/opad-ui-preview/Cargo.toml`, `desktop/crates/opad-tosu/src/lib.rs` (~620)
- [ ] `opad-ui-preview`: depend on `opad-model` normally; use `ui_source::*` names in `sample_values()`; derive `SCREEN_W`/`SCREEN_H`/`MAX_WIDGETS`/`LABEL_MAX`/`SUFFIX_MAX` from `opad_layout` (note `LABEL_MAX_BYTES = 31` vs `LABEL_MAX = 32` — determine which is correct from the firmware header and use one).
- [ ] `launch_tosu`: single generic `pump(reader, cb, file)` for stdout and stderr.
- Verify: `make check` (includes `source_ids_match_firmware`).

### A7 — GUI polling efficiency (after S9) ▸ DG#9, DG#10
Files: `desktop/gui/src/main.rs` (~476, ~1601), diagnostics page module
- [ ] Diagnostics: move evdev/`GetAsyncKeyState` polling into a `Subscription` stream on its own thread emitting `KeyEvent` only on state change; cache the pretty-printed bundle string in state and regenerate only when its inputs change.
- [ ] Poll only what the visible page needs: `GetStatus` always; `GetUiValues` on Dashboard/Designer; `GetUpdateStatus`/`GetFirmwareUpdate` on navigating to Settings plus a 60 s timer.
- Verify: `make check`; run the GUI; CPU at idle on Diagnostics drops; Settings→Updates still shows fresh state.

### A8 — CLI UX cleanups ▸ DC#8, DC#9
Files: `desktop/cli/src/main.rs` (~366–380, ~602–616, ~839)
- [ ] `fn confirm(prompt: &str, non_tty_msg: &str) -> Result<bool>` used by `Import` and `FirmwareUpdate`.
- [ ] `opadctl setup`: compare the embedded `70-opad.rules` (`include_str!`) with `/etc/udev/rules.d/70-opad.rules`; print "already installed" if identical, else a `sudo tee` command that writes the embedded content (no repo-relative path).
- Verify: `make check`; run `opadctl setup` on a machine with and without the rule.

### A9 — Packaging/CI ▸ PK#3, PK#4, PK#5
Files: `scripts/release/test_packages.sh` (~26), `desktop/gui/Cargo.toml` (`[package.metadata.deb] depends` ~53 and `[package.metadata.generate-rpm.requires]` ~122 — packages are built by `cargo-deb` / `cargo-generate-rpm` from `scripts/release/build_packages.sh`), `packaging/linux/deb/postinst`, `packaging/linux/arch/PKGBUILD` (~41), `.github/workflows/release.yml` (~154)
- [ ] .deb: add `libcap2-bin` to `depends` in `desktop/gui/Cargo.toml`; .rpm: add `libcap` to `generate-rpm.requires`; make `test_packages.sh` **not** pre-install `libcap2-bin`/`procps` so the test proves the dependency. Leave `postinst`'s `setcap ... || true` as is (it is correct once the dependency is declared).
- [ ] PKGBUILD: run `make espflash` in `build()` and install `/usr/lib/opad/bin/espflash`; add the needed `makedepends`.
- [ ] release.yml: include `usr/lib/opad/tosu/tosu` in the glibc assertion with the 2.28 target; add a distro with glibc ≤ 2.31 (e.g. Debian 11) to the matrix or assert via `objdump -T | grep GLIBC_` max version.
- Verify: `make deb rpm` locally if the toolchain exists; `scripts/release/test_packages.sh`; workflow lint (`actionlint` if available).

---

## Progress log

Append a line per completed cluster: `YYYY-MM-DD  <cluster>  <commit sha>  <executor>  notes`.

```
2026-09-24  O1  aa05333  Claude Opus  FP#1 FP#2 fixed; parser locks to the first recognised frame's framing; pad test pending
2026-09-24  O2  abe93c6  Claude Opus  FP#3 FB#2 FU#7 fixed; pin scan is a keypad-task state machine, timeout clamped to 30 s; pad test pending
2026-09-24  O3  240d115  Claude Opus  FU#2 FU#6 fixed; pending flag replaced by change/sent sequence numbers; bench + pad test pending
2026-09-24  O4  (this commit)  Claude Opus  FU#3 FU#4 FU#5 fixed; all runtime keypad config goes through the keypad task; pad test pending
```
