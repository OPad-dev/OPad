# Claude Code review findings

Findings from `/code-review` runs, saved so they survive the session. Each file
covers one run; findings are ranked most severe first and carry a status
(`open` / `fixed` / `wontfix` / `invalid`) that should be updated as they are
worked through. Unless a finding is marked *(confirmed)* or *(verified)*, the
reviewer did not re-verify it, so check each against the code before acting on it.

All runs on 2026-09-23 were against `main` @ `b0e0a37` with a clean working tree,
so each covers the current code of its path rather than a diff.

| Scope | File | Findings |
| --- | --- | --- |
| `firmware/main/protocol` | [2026-09-23-firmware-protocol.md](2026-09-23-firmware-protocol.md) | 8 |
| `firmware/main` usb / input / config / counters / runtime / diag / app_main | [2026-09-23-firmware-usb-input-config.md](2026-09-23-firmware-usb-input-config.md) | 10 |
| `firmware/main/ui` | [2026-09-23-firmware-ui.md](2026-09-23-firmware-ui.md) | 10 |
| `firmware/test`, `firmware/boards` (medium) | [2026-09-23-firmware-test-boards.md](2026-09-23-firmware-test-boards.md) | 2 |
| `desktop/crates/opad-protocol`, `opad-device`, `opad-ipc` | [2026-09-23-desktop-opad-protocol-device-ipc.md](2026-09-23-desktop-opad-protocol-device-ipc.md) | 10 |
| `desktop/crates/opad-update` | [2026-09-23-desktop-opad-update.md](2026-09-23-desktop-opad-update.md) | 8 |
| `desktop/crates/opad-model`, `opad-layout`, `opad-storage`, `opad-tosu`, `opad-ui-preview` | [2026-09-23-desktop-model-layout-storage-tosu-preview.md](2026-09-23-desktop-model-layout-storage-tosu-preview.md) | 10 |
| `desktop/daemon` | [2026-09-23-desktop-daemon.md](2026-09-23-desktop-daemon.md) | 10 |
| `desktop/gui` | [2026-09-23-desktop-gui.md](2026-09-23-desktop-gui.md) | 10 |
| `desktop/cli` | [2026-09-23-desktop-cli.md](2026-09-23-desktop-cli.md) | 10 |
| `packaging`, `scripts`, `.github`, `protocol` (medium) | [2026-09-23-packaging-scripts-ci-protocol.md](2026-09-23-packaging-scripts-ci-protocol.md) | 5 |

93 findings total. `opad-layout`, `opad-storage` and `protocol/` came out clean.

**Fixing them:** see [FIX-PLAN.md](FIX-PLAN.md) — every finding assigned to one
executor (Opus / Sonnet / Haiku / Antigravity), clustered by file, with run order,
session rules and paste-able kickoff prompts.

## Status: closed (2026-09-24)

All 93 findings are closed: **92 fixed, 1 invalid** (DC#10). Every cluster in
FIX-PLAN.md is ticked. Every commit passed `make firmware`, the firmware host
tests and `make check` (fmt, clippy `-D warnings`, 328 tests).

### Hardware validation record

Run by the owner on the real pad (Waveshare ESP32-S3 Touch LCD 2, new firmware
flashed via `opadctl flash`) on 2026-09-24:

| Step | Covers | Result |
| --- | --- | --- |
| Flash + reconnect | S12 (daemon pause/resume path) | ✅ released port, 12 s flash, reconnected 3 s later, no 15 s wait |
| Boot | O6, S1, A2 | ✅ no I2C scan, quiet idle log |
| Touch as third key (tap / hold / lift / double tap / 30 s idle) | FU#10 | ✅ |
| Keys, remap while held, detect-pin, protocol responsive during detect | O2, O3, O4, S1 | ✅ |
| Full map with live tosu data, unplug/replug mid-map | O1, S2 (pad side) | ✅ |
| Sleep, set brightness asleep, wake | S3, FB#1 | ✅ |
| Layout pushed during gameplay, reboot | O5 | ✅ |

**Signed off by the owner without a hardware run** (unit/integration tests and
code review only): foreign-pad guards and incompatible-pad handling (O7, O8),
headless mode (O6 — the LCD is always fitted), desktop reconnect logic (O9, tested
on the pad only against the pre-fix daemon), Windows pipe/bootloader-port picks
(O10, `cargo check` for the Windows target), tosu supervisor pause (O11), the
updater fixes (S6, S7), the GUI fixes (S9, S10, DG#10), the CLI fixes (S11,
DC#8), release signing (S13), and the Windows cold-plug with a legacy-firmware
pad (DD#9). If any of these misbehaves, the FIX-PLAN progress log names the
commit to look at.

## Top items across runs (2026-09-23)

### Firmware
1. Framing lock: legacy headers accepted after marked lock, and `s_host_framing` re-latched per frame (protocol #1, #2).
2. `detect_pin` `timeout_ms` unbounded, blocks the CDC task (protocol #3).
3. GPIO8 (module-ID divider) in the key-pin lists → pin detect returns it instantly (usb/input #1, #8).
4. HID `s_report_pending` race between touch-retry (core 1) and keypad (core 0) (usb/input #2).
5. HE module reported as a held key; keycode changes bypass the staged apply (usb/input #3, #4, #5).
6. Headless mode: `data_update` source 67 runs LVGL without `lv_init` → panic loop; `ui_init` derefs a NULL screen (ui #1, #6).
7. `ui_store` pending layouts / dirty masks shared across cores without a lock; dirty bit cleared before the NVS write (ui #4, #5).
8. Brightness set while asleep is lost on wake (ui #2, boards #1).

### Desktop
1. Foreign-pad guards bypassed: cooldown→sync trigger, every IPC handler, and the firmware installer (daemon #1, #2, #4).
2. `opad-device` serial worker: Windows `Instant` subtraction panic kills the worker silently; frames dropped and no reopen after a re-Hello (device #1, #2, #3).
3. Updater: persisted ETag with no cached manifest disables all updates after a restart; AppImage never classified in the manifest; tosu VERSION written before the binary; `pause()` doesn't wait for tosu to exit (update #1, #2, #3; tosu #1).
4. Incompatible-protocol pad adopted as connected, counters saved under the previous pad's id (daemon #3).
5. Version compare: `rc10 < rc9`, and tosu plan ignores `is_newer` (update #5, #6).
6. Flash flow: CLI death between `PrepareFlash`/`FinishFlash` leaves the daemon paused forever; `bootloader` waits 15 s for an impossible reconnect (cli #2, #3).
7. GUI log cursor skips a daemon entry per local log line; single-instance races on both platforms; tray probed once (gui #2, #3, #6, #8).

### Release / packaging
1. `build_release.sh` signs a manifest over locally rebuilt artifacts, not the ones `release.yml` publishes (packaging #1).
2. `AppRun` `LD_LIBRARY_PATH` trailing-colon lets a planted `.so` in the CWD load (packaging #2).
3. Arch package ships no espflash; glibc gate skips the tosu binary (packaging #4, #5).

## Cross-cutting themes

- **Framing negotiation lives in three places** (firmware parser, `opad-device` worker, `opad-device::probe_port`) with different rules; the firmware side re-latches per frame while the host locks once. See firmware protocol #1/#2 and device #9.
- **Log sequence cursors** are mishandled the same way in the GUI (`latest_log_seq` doubles as local counter) and the CLI (`since_seq` advanced to `latest_seq`, `> 0` guard). A shared "advance from last received entry" helper in `opad-model` would fix both.
- **Cross-core shared state without a lock** in firmware: `ui_store` masks/pending layouts, `s_report_pending` / `s_pending_edge_us` in `usb_hid`, `s_asleep` read from the protocol task.
- **Duplicated tables/logic that have already drifted:** key-pin allow-list and debounce bounds (firmware), `ConfigPayload → DeviceConfig` (device), atomic-write sequence (update/backup), tosu log classifier (daemon), confirm prompt (cli), stdout/stderr pump (tosu), `install_prefix` (model), wire constants (ui-preview).
- **Brightness / sleep ownership** is split between `board_display.c`, `ui_port.c` and `device_config.c`, each with its own copy of the current brightness.
