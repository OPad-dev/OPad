# osu!pad — Remaining Work to v1.0 (Linux)

**Companion to:** `osupad_technical_spec_v1.md` (the contract)
**Audience:** Antigravity and other coding agents
**Baseline:** commit `79622d7` plus the uncommitted working tree as of 2026-09-13. Line numbers refer to that state and may drift; search for the quoted symbol if a line no longer matches.

---

## 0. How to use this document

1. Read `osupad_technical_spec_v1.md` completely first. This document does **not** replace it. It lists (a) spec amendments the project owner has decided, and (b) the concrete gaps between the spec and the current code.
2. Work in priority order: **P0 → P1 → P2 → P3**. Do not start a P2 item while a P0/P1 item is open unless it is trivially independent.
3. Each task has: **Problem**, **Where**, **Spec**, **Required change**, **Acceptance**. A task is done only when every acceptance point is met.
4. The latency invariant (spec §3) still overrides everything. Nothing in this list may add work to the key ISR, the keypad task, or the TinyUSB task on core 0.
5. Keep changes small and per-task. One task (or a tight group of related tasks) per commit.
6. Commits are authored as `GFerreiroS <info@gferreiro.com>`. Do not add co-author or session trailers.
7. If something here conflicts with the spec, this document wins for the items listed in section 1. For anything not covered, follow the spec, and ask the owner before changing a closed decision.

---

## 1. Spec amendments (closed decisions)

These override the named sections of the spec. Update the docs (task P3-6) to reflect them.

### A1. LVGL is accepted (overrides §14.1 "Do not use LVGL")

The owner tested the LVGL UI on hardware and noticed no latency regression. LVGL stays. The following constraints are now part of the contract and **must not be broken**:

- LVGL task, rendering, and SPI/DMA interrupts stay on **core 1** (`ui_port.c` `task_affinity = 1`, `board_display.c` `isr_cpu_id = ESP_INTR_CPU_AFFINITY_1`).
- LVGL tick comes from `lv_tick_set_cb(esp_timer_get_time)`, not from the esp_lvgl_port periodic esp_timer (which runs on core 0). Keep `lvgl_port_stop()`.
- LVGL uses its own fixed memory pool (`CONFIG_LV_MEM_SIZE_KILOBYTES`), never the general heap on the hot path.
- Only the widgets actually used are enabled in `sdkconfig.defaults`.
- The on-device latency stats must still be recorded for the release gate (task P3-1), so "no noticeable regression" becomes a measured number.

### A2. Screen layout designer and data-source protocol are in scope

The PC layout designer (`osupad-layout`, `osupad-ui-preview`, GUI Designer page) and the protocol messages `DataUpdate`, `SetLayout`, `LayoutAck`, `reset_layout`, `HostStatus` are accepted v1.0 features. They must follow the same persistence rules as everything else (no NVS/SQLite writes during PLAYING/COOLDOWN, see P1-3).

### A3. Windows support is deferred (Phase 10 postponed) — **SUPERSEDED 2026-09-16**

> **SUPERSEDED by `osupad_packaging_distribution_plan.md`.** Windows support is
> in v1.0, not after it: named pipes (§W0-1, §W0-2), COM port discovery (§W1-3),
> login startup (§W1-1) and an Inno Setup installer (§W2-1) are all done. The
> paragraph below is kept for the record and no longer describes the plan.
>
> What survives of it is the *reason* it was written: platform-specific code
> still lives in `osupad-ipc`, `osupad-device` and `packaging/`, and nothing
> outside those places opens a `UnixStream` or a `/dev/...` path directly. That
> discipline is why the port was a `#[cfg]` alias rather than a rewrite.

Do **not** implement Windows support now: no named pipes, COM port discovery, or login-startup work. Linux must be fully working and released first.

To avoid making the later port harder:
- Keep platform-specific code inside `osupad-ipc` (socket transport), `osupad-device` (port discovery), and `packaging/`.
- Do not add new direct `tokio::net::UnixStream` / `/dev/...` / `systemctl` usage outside those places. The GUI's `ipc.rs` and the CLI should call an `osupad-ipc` helper (for example `osupad_ipc::connect()`) instead of opening `UnixStream` themselves.

### A4. `press_color_rgb` is legacy

The firmware ignores `ConfigPayload.press_color_rgb`; key highlight colors now come from layouts. Keep the proto field number reserved, but stop treating it as a live setting (task P3-5).

### A5. The tray lives in the GUI, not the daemon (overrides §6, §16.1, §19, §29, §40 "Owns tray")

The owner chose to keep the current design: `osupad-gui` hosts the system tray (ksni, StatusNotifierItem) and keeps running in the background with no window open (Discord-style, `iced::daemon`, `--tray` starts hidden). The daemon does **not** own a tray.

What still holds from the spec:
- The daemon stays the single owner of the serial port, tosu, SQLite, runtime state, sync, logs, and IPC. The GUI and tray only talk to it over IPC.
- Everything keeps working with the GUI closed or crashed: HID, sync, display data, persistence. The tray is an optional front end.
- The tray is device-focused (§19): pad status, counters, sync. No osu!/tosu details beyond a short connection line.
- No tray host (for example GNOME without AppIndicator) must never break the GUI or daemon; the GUI then behaves as a normal windowed app (close = quit).

The concrete work is task P2-8.

---

## 2. P0: keyboard safety (do these first)

### P0-1. A display or NVS failure must not kill the keyboard

**Problem:** `app_main.c` wraps every init in `ESP_ERROR_CHECK`. If `counters_init()` (NVS) or `ui_init()` (LCD/LVGL) fails, the firmware aborts and reboot-loops, so the pad stops working as a keyboard.

**Where:** `firmware/main/app_main.c:31-67`, `firmware/main/counters/counters.c` (`counters_init`), `firmware/main/ui/ui_port.c` (`ui_init`).

**Spec:** §8 (boot priority, "An LCD failure MUST NOT prevent HID operation"), §31 (ESP-side failure isolation).

**Required change:**
1. Reorder boot to match §8:
   1. `board_init()` for **key GPIOs only**. Split backlight/LEDC setup into a separate function, because LEDC failure must not be fatal.
   2. `keypad_init()`, `usb_hid_init()`, `tinyusb_driver_install()`: HID usable.
   3. `usb_cdc_init()`, `usb_cdc_start_task()`.
   4. `counters_init()` and device config load (P0-3).
   5. `runtime_init()`.
   6. Backlight + `ui_init()`.

   Counters must be loaded before any key could be counted against stale values. The simplest correct approach: `keypad_init` starts with RAM counters at 0, and `counters_init` **adds** the NVS values rather than overwriting (`keypad_add_lifetime_presses`), so presses between HID-ready and NVS-load are not lost. Alternatively, load NVS before `tinyusb_driver_install` but tolerate failure. Pick one and document it in a comment.
2. Only failures in steps (i)–(ii) may be fatal. Everything else must log, record a diagnostic event (P2-1), and continue:
   - `counters_init` failure: counters stay in RAM from 0. Set a `counters_nvs_ok = false` flag, and skip all later checkpoints and host syncs that would write NVS. Report it in `DeviceStatus` (add `bool nvs_ok` to the proto, a new field number).
   - `ui_init` failure: no display. `ui_*` setters must become no-ops (guard every public `ui_*` function with an `s_ui_ok` flag, **including** `ui_notify_activity` and `ui_lock`/`ui_unlock`, which currently call `lvgl_port_lock` unconditionally and would hang or crash). Report `display_ok` in `DeviceStatus`.
   - `runtime_init` / `usb_cdc_start_task` failure: log and continue.
3. `nvs_flash_init()` erase-and-retry is acceptable, but the erase must be logged as a diagnostic event because it wipes lifetime counters.

**Acceptance:**
- Build a test variant with `board_display_init` forced to return `ESP_FAIL` (a temporary `#define` or Kconfig `OSUPAD_TEST_FAIL_LCD`). The pad enumerates, Z/X work, CDC handshake works, and `DeviceStatus.display_ok == false`.
- Same with `nvs_flash_init` forced to fail: keyboard works, counters count in RAM, `nvs_ok == false`, and no crash when the host sends `CounterSync`, `SetLayout`, or `SetConfig`.
- No `ESP_ERROR_CHECK` remains on non-essential init paths.

---

### P0-2. Eager debounce must re-sample at the end of the lockout

**Problem:** The ISR accepts the first edge, then **ignores every edge** inside the lockout, and nothing re-reads the pin when the lockout ends. If the pin's final level differs from the accepted state when bouncing stops, no further edge arrives, so the key stays in the wrong state (stuck down or stuck up) until the switch is touched again. The trigger can be a noise glitch on Dupont wiring, or a tap/release shorter than the lockout. The default is also 5 ms (spec: 2–3 ms), and µs are truncated to ms.

**Where:** `firmware/main/input/keypad.c:16` (`debounce_ms = 5`), `:33-71` (`gpio_isr_handler`), `:73-102` (`keypad_task`); `keypad.h` (`keypad_config_t.debounce_ms`); `protocol.c` `set_config` handler (`c->debounce_us / 1000`).

**Spec:** §9.2 (eager debounce, step 4 re-sample), §34 (debounce bounce patterns, no stuck keys), §37 (2–3 ms default).

**Required change:**
1. Change `keypad_config_t.debounce_ms` (uint16 ms) to `debounce_us` (uint32 µs). Default **5000 µs**. Clamp accepted values to `500..=20000` µs. Update `protocol.c` to pass µs through without division, and `protocol_send_config_ack` to report µs directly.
2. Implement the re-sample in `keypad_task` (core 0, already highest priority). This avoids adding an esp_timer:
   - The task keeps a per-key `lockout_end_us`. Instead of `ulTaskNotifyTake(pdTRUE, portMAX_DELAY)`, it waits with a timeout equal to the smallest remaining lockout among keys still in lockout (or `portMAX_DELAY` when none are).
   - When a key's lockout has expired and it has not been re-sampled yet, read the GPIO. If the level differs from the accepted state, apply it as a new accepted transition: update state, start a new lockout, submit HID, and count it if it is a press.
   - The ISR and task both touch `s_key_state` / `s_last_transition_us`. Protect the compare-and-update with a `portMUX_TYPE` spinlock (`portENTER_CRITICAL_ISR` in the ISR, `portENTER_CRITICAL` in the task). Keep the critical sections to a few instructions.
   - FreeRTOS tick is 1 ms (`CONFIG_FREERTOS_HZ=1000`), so the wait resolves to ≥1 ms. That is acceptable: re-sampling only corrects an already-wrong state and never delays an eagerly accepted edge.
3. Extract the pure decision logic into a function with no ESP-IDF dependencies, for example `debounce_step(state*, now_us, level, source={EDGE|RESAMPLE}) -> action`, in `input/debounce.c/.h`, so it can be unit-tested on the host (P3-2).
4. Counting rule stays: only accepted press transitions increment counters, including presses applied by the re-sample. A glitch press that is later corrected will have been counted once; that is acceptable and must be documented in a comment.
5. Update GUI Settings slider range and daemon validation to the same `500..=20000` µs range.

**Acceptance:**
- Unit tests (host-compiled) cover: clean press/release; bounce burst on press; bounce burst on release; glitch shorter than lockout (must end in the correct final state); tap shorter than lockout; both keys interleaved.
- On hardware: 2 minutes of fast alternating taps with no stuck key and no missed press. `osupadctl latency` p99.9 unchanged within measurement resolution (10 µs buckets) compared to before the change.
- Default debounce everywhere (firmware, proto comment, SQLite default, `DeviceConfig::default`, GUI) is 5000 µs.

---

### P0-3. Persist device-side configuration in NVS

**Problem:** Key mapping, debounce, brightness, and sleep timeout live only in RAM. On boot without the daemon, the pad reverts to Z/X, 5 ms, 80% brightness. A user who remapped keys gets different keys when the PC software is not running, which breaks "the keypad must remain a good two-key USB keyboard even if every host-side component is missing".

**Where:** `firmware/main/protocol/protocol.c` (`set_config` handler), `firmware/main/input/keypad.c`, `firmware/main/usb/usb_hid.c`, `firmware/main/ui/ui_port.c:31` (`s_brightness = 80`), `firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c:7` (`s_backlight_percent = 80`).

**Spec:** §7.1 ("NVS for nonvolatile counters/configuration that must live on device"), §2.1, §37 (brightness default 100%).

**Required change:**
1. New module `firmware/main/config/device_config.c/.h`, NVS namespace `osupad_cfg`, one versioned blob `{version, key1_usage, key2_usage, debounce_us, brightness, sleep_s}`. Validate on load and fall back to defaults on any mismatch.
2. Load at boot, before `usb_hid_init` / `keypad_init` / `ui_init` (or apply immediately after, if P0-1 ordering requires it).
3. On `SetConfig`: validate every field (HID usage `0x04..=0xE7`, debounce range from P0-2, brightness `0..=100`, sleep `0` or `10..=86400`). Reject invalid values with `ConfigAck{success=false, message}` instead of silently ignoring them. Apply valid values to RAM immediately **only if safe** (see 4). Persist to NVS **only in IDLE**; otherwise mark dirty and let the runtime supervisor persist when state becomes IDLE (the same pattern as counter checkpoints).
4. Key mapping and debounce changes while a key is held could leave a stuck keycode on the host. Apply mapping changes only when both keys are released (check `keypad_is_pressed`); otherwise stage them and apply on the next all-released moment in `keypad_task`.
5. Fill in all fields of `ConfigAck.current_config` (it currently omits sleep timeout and uses ms).
6. Defaults: brightness **100**, sleep **600 s**, debounce **5000 µs**, Z (`0x1D`) / X (`0x1B`).

**Acceptance:**
- Set K1=A, K2=S via GUI, unplug, stop the daemon (`systemctl --user stop osupad-daemon`), plug in: A/S are produced.
- Sending `SetConfig` during PLAYING/COOLDOWN performs zero NVS writes (verify with a log line in the NVS write function and a gameplay session).
- Invalid values produce `ConfigAck.success=false`; the daemon surfaces the message to the GUI/CLI.

---

## 3. P1: synchronization, persistence, and recovery correctness

### P1-1. Post-cooldown counter sync is rejected by the device (race)

**Problem:** The daemon enters COOLDOWN and sends `HostStatus{playing=false}`. Exactly 5.0 s later it runs `perform_sync` and sends `CounterSync`. The device starts its own 5 s cooldown when it *receives* that HostStatus, and its supervisor only checks once per second (`runtime.c:28`), so it reaches IDLE 5–6 s later. `counters_sync_from_host` rejects anything outside IDLE (`counters.c:90`), so the sync almost always arrives during device COOLDOWN and is rejected. The daemon ignores `CounterSyncResponse.success` (`osupad-device/src/lib.rs` `handle_device_message`) and marks the sync "completed successfully". The "PC restores ESP" path therefore only works through a manual Sync.

**Where:** `desktop/daemon/src/main.rs` `perform_sync` (`send_counter_sync` at ~409), `firmware/main/runtime/runtime.c:28`, `firmware/main/counters/counters.c:90`, `desktop/crates/osupad-device/src/lib.rs` (`CounterSyncResp` handling).

**Spec:** §11.3, §13, §40 ("ESP can restore PC and PC can restore ESP").

**Required change:**
1. Firmware: reduce the supervisor period to **100 ms** (still core 1, low priority). The state machine must stay host-driven; do not add timing work on core 0.
2. Firmware: `DeviceStatus.state` must reflect the real device state (it already does). Make sure the daemon's status poll reads it.
3. Daemon: make `perform_sync` a sequence with timeouts, not fire-and-forget:
   1. Wait until the latest `DeviceStatus.state == IDLE` (poll `request_status`, 100 ms interval, 3 s timeout). On timeout, log a warning, stay in `Sync`, and retry on the next tick.
   2. Send `CounterSync` and **await** the matching `CounterSyncResponse` (match by `sequence_number`; the device already echoes the request's sequence). Timeout 2 s.
   3. `success=false`: log it with the device's reported state, keep SQLite as reconciled, and retry up to 3 times with backoff; after that, surface `last_sync_error` in `GetStatus`.
   4. Only on success: set `last_sync_time`, persist `last_sync_at`, and go to IDLE.
4. Add `DeviceEvent::CounterSyncResult { seq, success, state }` instead of folding the response into a generic `Counters` event.
5. Firmware `CounterSyncResponse` should carry a reason when rejected. Add `string message` (new field number) with "not idle", "stale generation", "non-monotonic", or "nvs unavailable".

**Acceptance:**
- Scenario: ESP counters at gen 1 / 100, 100; SQLite at gen 1 / 5000, 5000 for the same device_id. Play one map, exit, and wait 7 s: ESP shows ≥5000 on the idle screen, NVS contains it (verify after a power cycle), and the Monitor shows one successful sync.
- A forced rejection (for example, keep the device in COOLDOWN via a test hook) produces a visible error, no false "Synchronization completed successfully" log, and a retry.

---

### P1-2. Reconcile on device connect, and handle ESP replacement

**Problem:** Reconciliation only runs after a cooldown or a manual Sync. Recovery cases 1 and 2 (§13) do not happen until the user plays a map. A **replaced** ESP has a new `device_id`, so `load_device_state(new_id)` returns nothing, and the PC would silently adopt the new board's zero counters instead of offering a restore.

**Where:** `desktop/daemon/src/main.rs` `DeviceEvent::Connected` handler and `perform_sync`; `desktop/crates/osupad-storage/src/lib.rs`.

**Spec:** §13 (recovery cases 1–4), §11.3, §18 (Device actions).

**Required change:**
1. After `HelloAck`, and when the daemon mode is `Idle`, run the P1-1 sync sequence automatically. If the mode is PLAYING/COOLDOWN, schedule it for the next SYNC.
2. Same `device_id` exists in SQLite: normal §13 reconciliation (the newer generation wins; same generation takes the per-key max) and push to the ESP. This covers case 2 (ESP reflashed, NVS erased).
3. No row for this `device_id`:
   - If SQLite has **no** device rows: import the ESP state into SQLite (case 1).
   - If SQLite has other device rows **and** the ESP looks fresh (generation 1, both counters below a small threshold such as 1000): do **not** auto-merge. Set `pending_replacement: Some(previous_device_id)` in daemon state, import nothing yet, and expose it in `GetStatus`. The GUI shows "This looks like a new pad. Restore counters from <old id>?" with two explicit actions (P2-4): *Restore from previous pad* (force restore, generation = max(old, esp) + 1) or *Treat as new pad*.
   - Otherwise (a new board with real counts): import it as a separate device row.
4. Storage: add `list_device_states()` and `load_latest_device_state()` (by `last_seen_at`). Also update `last_seen_at` on connect. That write is allowed only in IDLE; when not in IDLE, keep it in memory and write it at the next SYNC.

**Acceptance:**
- Case 1: delete `~/.local/share/osupad/osupad.db`, start the daemon, and connect the pad. Within 3 s SQLite contains the ESP counters and the GUI shows them.
- Case 2: `idf.py erase-flash` then flash the firmware, and connect. Within 3 s the ESP shows the PC counters, and they survive a power cycle.
- Replacement: change the device_id (for example, a test build with a fake MAC suffix) and connect. The GUI prompts, and nothing is overwritten until a choice is made.
- Host integration tests for all three (P3-2).

---

### P1-3. Zero NVS/SQLite writes during PLAYING and COOLDOWN, enforced everywhere

**Problem:** Several paths write storage while gameplay or cooldown is active:
- Daemon `UpdateConfig` rejects only PLAYING, so it **writes SQLite during COOLDOWN** (`main.rs` ~523-533).
- Daemon `SetLayout` / `ResetLayout` write SQLite in **any** mode (`main.rs` ~476, ~494).
- Firmware `set_layout` saves to NVS whenever the state is not PLAYING, so it **writes NVS during COOLDOWN** (`protocol.c:305`). Same for `reset_layout` (`protocol.c:321`).

**Spec:** §11.1, §11.2, §12, §20.3 ("SQLite writes MUST be zero"), §30 operation table, §40.

**Required change:**
1. **Storage write guard (daemon).** Give `Storage` a shared `Arc<AtomicBool> writes_allowed` controlled by the runtime state machine (false in Playing/Cooldown). Every mutating `Storage` method checks it and returns `StorageError::WritesBlocked` if false, logging at `warn`. This is both the enforcement and the test hook.
2. **Pending-operation queue (daemon).** Operations requested during PLAYING/COOLDOWN follow the §30 table:
   - Change brightness / sleep timeout: apply to the device RAM immediately (proven harmless on hardware; the device defers its own NVS write per P0-3). Queue the SQLite write for SYNC. Response: `ConfigUpdated { deferred_persist: true }`.
   - Change key mapping / debounce: **defer** entirely until IDLE. Response: `OperationDeferred { reason }`.
   - Set/reset layout: apply to the device (the device keeps it in RAM only). Queue the SQLite save **and** a re-send after SYNC so the device persists it in IDLE. Response: `LayoutApplied { message: "applied, will be saved after gameplay" }`.
   - Reset counters, import backup, force sync, restore actions: **reject** with a clear message (existing behaviour, extended to COOLDOWN everywhere).
   - Flash firmware: reject in PLAYING **and** COOLDOWN.
   - Drain the pending queue at the end of a successful SYNC.
3. **Firmware.** Replace every `== OSUPAD_STATE_PLAYING` persistence check with `!= OSUPAD_STATE_IDLE`. Keep a "dirty layouts" bitmask and persist in the runtime supervisor when the state becomes IDLE. Log (not per press) when a write is deferred.
4. Add a debug counter of NVS writes to `DeviceStatus` (`uint32 nvs_writes`, new field) so tests can assert it did not change across a play session.

**Acceptance:**
- Integration test: drive Playing → Cooldown and issue UpdateConfig, SetLayout, ResetLayout, ImportBackup, ResetCounters, ForceSync. `Storage` records zero successful writes while blocked, and the queued ones are applied after SYNC.
- Hardware: note `DeviceStatus.nvs_writes`, play a map while changing brightness and layout from the GUI, and confirm it is unchanged until ≥5 s after leaving the map.

---

### P1-4. Daemon must load PC state at startup (offline GUI, export, reset)

**Problem:** `DaemonState.counters` starts as `CounterState::default()` and is only filled from the device. With no pad connected, the GUI shows 0/0, `ExportBackup` writes zeros with `device_id: "unknown"`, and `ResetCounters` is not persisted (it only saves when `device_info` is `Some`).

**Where:** `desktop/daemon/src/main.rs` startup (~62-86), `ExportBackup` (~588), `ResetCounters` (~562).

**Spec:** §31 ("ESP disconnected → GUI remains usable for stored backup/config"), §21.

**Required change:**
1. At startup, load `load_latest_device_state()` plus its `DeviceInfo` (board profile, firmware version, `last_sync_at`) into `DaemonState`. Add `counters_source: Pc | Device` to the status so the GUI can label values.
2. `ExportBackup` with no device connected exports the stored PC state. If there is no stored state at all, return `OperationRejected { "No counters known yet: connect the pad once" }`.
3. `ResetCounters` with no device: persist to SQLite (generation+1, zeros) for the last known device, and mark `pending_device_push`. On the next connect, reconciliation naturally pushes it because the PC generation is higher.
4. Keep `GetStatus.last_sync_time` populated from SQLite after a restart.

**Acceptance:**
- Stop the daemon, unplug the pad, start the daemon: GUI and `osupadctl status` show the last synced counters, labelled as the PC copy.
- `osupadctl export x.json` with the pad unplugged produces the real counters and device_id.

---

### P1-5. JSON import: validation, preview, and confirmation

**Problem:** `ImportBackup` applies immediately. It does not reject out-of-range debounce/sleep/hz values or counters that overflow SQLite `INTEGER` (i64), has no device_id mismatch check, shows no diff, and overwrites `tosu_endpoint` with a hard-coded default (`main.rs` ~616).

**Where:** `desktop/crates/osupad-model/src/lib.rs:220` (`JsonBackup::validate`), `desktop/daemon/src/main.rs` `ImportBackup`, `desktop/cli/src/main.rs` `Import`.

**Spec:** §13 case 4, §21 (validate, reject malformed/overflowing, show what will change, confirm rollback/reset).

**Required change:**
1. `JsonBackup::validate` also checks: `debounce_us` in the P0-2 range; `display_sleep_seconds` 0 or `10..=86400`; `gameplay_display_hz` `1..=30`; `lifetime_*` ≤ `i64::MAX`; `board_profile == "waveshare_esp32s3_touch_lcd_2"`; `exported_at` not in the future by more than a day. Use `#[serde(deny_unknown_fields)]` only if forward compatibility is not needed; otherwise ignore unknown fields.
2. New IPC request `PreviewImport(JsonBackup)` → `ImportPreview { current: {counters, config}, incoming: {counters, config}, device_id_matches: bool, is_counter_rollback: bool, warnings: Vec<String> }`. No side effects.
3. `ImportBackup { backup, confirm: bool }`: the daemon refuses unless `confirm == true`. Preserve the current `tosu_endpoint`. Keep `press_color_rgb` as is (legacy).
4. CLI `import`: print the preview diff, then require `--yes` (or an interactive `y/N` if stdin is a TTY).
5. GUI import flow is part of P2-3.

**Acceptance:** Unit tests for each validation rule, and an IPC test showing that `ImportBackup{confirm:false}` is rejected and that preview has no side effects.

---

### P1-6. Reset lifetime counters requires strong confirmation in the GUI

**Problem:** `Message::ResetCounters` (`desktop/gui/src/main.rs:276`) sends the IPC request immediately on a single click.

**Spec:** §13 ("MUST require deliberate confirmation in the GUI"), §18 Device.

**Required change:** Add a modal confirmation that shows the current K1/K2/total and requires typing `RESET` (or an equivalent two-step confirmation) before the request is sent. The daemon's `ResetCounters` also takes `confirm: bool` and rejects if false, so no client can reset by accident.

**Acceptance:** A single click cannot reset. Cancelling leaves the counters untouched. The CLI `--yes` path still works.

---

### P1-7. Protocol version checks (device and IPC) and post-flash validation

**Problem:**
- The host never checks `HelloAck.protocol_version`.
- The daemon ignores the IPC `Handshake.client_protocol`.
- `FinishFlash` does not verify that the device came back or which firmware it runs.
- Versions are hard-coded as `"1.0.0"` in `protocol.c:99`, the daemon, the device crate `Hello.client_version`, and `DeviceInfo::default`.

**Spec:** §18.1, §22, §23.4, §26 step 5.

**Required change:**
1. Firmware: take the version from `esp_app_get_description()->version`, set via `PROJECT_VER` in `firmware/CMakeLists.txt` (for example from `git describe` or a `version.txt`).
2. Host binaries: use `env!("CARGO_PKG_VERSION")`. Set the workspace version to the real release version when tagging.
3. Daemon: if `HelloAck.protocol_version != 1`, mark the device as `incompatible { firmware_version, protocol_version }`. Send nothing except Hello/status requests, show it in GetStatus/GUI/tray/CLI with a clear "update firmware or host" message, and do not reconcile counters.
4. IPC: on a `client_protocol` mismatch, reply `HandshakeRejected { daemon_protocol, reason }` and close. The GUI and CLI must perform the handshake first on every connection they open. Currently the GUI's `ipc::request` skips it; move the handshake into the shared `osupad-ipc` connect helper.
5. Flash flow: `FinishFlash` resumes discovery, waits up to 15 s for `HelloAck`, and responds `FlashFinished { firmware_version, protocol_version, compatible }` or an error. The CLI prints the result. A failed flash must never modify SQLite counters (already true; keep it covered by a test).

**Acceptance:** Flashing a build with the protocol version bumped to 2 shows "incompatible" in the GUI and CLI, and no counter sync happens. A normal flash ends with the CLI printing the new firmware version.

---

### P1-8. Device command queue must not block the daemon or replay stale commands

**Problem:** `DeviceManager` uses `mpsc::channel(64)`, which is only drained while a port is open. Unguarded `send_host_status(...).await` calls in the daemon main loop (tosu connect/disconnect, playing transitions) block forever once the queue fills with no pad attached, freezing the daemon (IPC, tosu, and so on). Commands queued while disconnected are replayed to the device on reconnect: stale time syncs, counter syncs, possibly for a different board.

**Where:** `desktop/crates/osupad-device/src/lib.rs:56`, all `device_manager.send_*` call sites in `desktop/daemon/src/main.rs`.

**Required change:**
1. The `send_*` methods return `DeviceError::NotConnected` immediately when not connected, and use `try_send`: a full queue returns `DeviceError::Busy`. They never await queue capacity.
2. On disconnect, drain and drop the queue.
3. After `HelloAck` the daemon re-sends the full state explicitly (it already sends time/config/host status/layouts/data). Keep that, and add the P1-2 sync.
4. Consider splitting `DeviceManager` behind a trait (`DeviceLink`) so the daemon runtime can be tested with a fake (needed by P3-2).

**Acceptance:** With no pad attached, the daemon survives 1000 simulated tosu connect/disconnect cycles and IPC stays responsive. A test using the fake `DeviceLink` covers it.

---

## 4. P2: missing features

### P2-1. ESP diagnostic ring buffer and `LogEventBatch`

**Problem:** The firmware never sends `LogEventBatch`, so the Monitor has no ESP events.

**Spec:** §24.2, §23.3, §40 GUI ("Monitor displays host and ESP diagnostic events").

**Required change:**
1. `firmware/main/diag/diag.c/.h`: a fixed-size RAM ring buffer (for example 64 entries) of `{uint32 timestamp_ms, uint16 event_id, uint8 level, uint32 arg0, uint32 arg1}`. `diag_record()` is lock-free or uses a short spinlock, has no string formatting, and is safe from any task (not required from ISR; do not call it in the key ISR).
2. Events at minimum: boot (reset reason), HID mounted/unmounted/suspended, CDC opened/closed, NVS init failed / erased, LCD init failed, frame too large / decode failed, config rejected, counter sync rejected (reason), layout rejected, deferred NVS write flushed, display sleep/wake, latency outlier (a keypad-task sample > 1000 µs; the record happens after HID submit and can be sampled into a counter plus a single deferred event), watchdog/brownout reset reason on boot.
3. Proto: extend `LogEvent` with `uint32 event_id = 5; uint32 arg0 = 6; uint32 arg1 = 7;` (additive, compatible). The firmware leaves `tag`/`message` empty. The daemon maps `event_id` to human-readable text in one table (`osupad-model::diag`).
4. Draining: the CDC protocol task (core 1) sends batches **only in IDLE** when CDC is connected, at most every 1 s, and also on `request_logs` (new `HostToDevice` field `bool request_logs`). During PLAYING/COOLDOWN, events stay buffered; on overflow the oldest are dropped, with a `dropped` count in the batch.
5. Replace the most important `ESP_LOGx` calls on hot-ish paths (protocol decode errors, CDC write drops) with `diag_record`. Keep `ESP_LOG` for boot/init only.

**Acceptance:** Unplugging the tosu-connected pad and plugging it back shows ESP events (boot, CDC opened, HID mounted) in the GUI Monitor with ESP as the source. No events are sent during a map, and the buffered ones arrive after cooldown.

---

### P2-2. Structured LogHub and a full Monitor

**Problem:** The daemon log is a `VecDeque<String>` fed only by explicit `log_info` calls (no levels, no source). `tracing` output never reaches it. The GUI polls the last 200 strings and has no controls.

**Where:** `desktop/daemon/src/main.rs` (`log_info`, `GetLogEntries`), `desktop/gui/src/pages.rs` `monitor`, `desktop/cli/src/main.rs` `Monitor`.

**Spec:** §24.1, §24.3, §29 LogHub.

**Required change:**
1. Move the LogHub into `desktop/daemon/src/log_hub.rs`: a ring of `LogEntry { seq: u64, ts: DateTime<Local>, source: Host|Esp, level, target, message }`, capacity 2000.
2. Implement a `tracing_subscriber::Layer` that pushes `INFO`+ events into the hub, so all existing `info!`/`warn!`/`error!` appear. Remove the duplicate `log_info` strings or turn them into `info!` calls.
3. IPC: `GetLogEntries { since_seq: Option<u64>, limit }` → `LogEntries { entries: Vec<LogEntry>, latest_seq }`. Polling by `since_seq` is enough; a push subscription is optional.
4. No disk logging during PLAYING/COOLDOWN. Optionally, rotate a log file under `$XDG_STATE_HOME/osupad/` in IDLE only (not required).
5. GUI Monitor: severity filter (Debug/Info/Warn/Error), source filter (All/HOST/ESP), **Clear** (hides entries up to the current seq on the client), **Copy** (visible entries to clipboard), **Save log** (rfd save dialog, plain text), auto-scroll toggle. Format lines as in spec §24.3: `HH:MM:SS SOURCE LEVEL message`.
6. CLI: `osupadctl monitor --follow` streams via `since_seq` polling, with `--level` and `--source` filters.

**Acceptance:** A GUI filter set to ESP+Warn shows only ESP warnings. Save writes a file identical to the visible list. The daemon's own `tracing` warnings (for example a tosu reconnect) appear.

---

### P2-3. GUI Backup section (export / import with preview)

**Problem:** The GUI Device page only prints CLI commands for backup.

**Spec:** §18 Backup (export, import, validate, show summary/diff, confirm), §21.

**Required change:** Add a Backup card or page:
- **Export:** rfd save dialog (default name `osupad-backup-YYYYMMDD.json`) → `ExportBackup` → pretty JSON written by the GUI.
- **Import:** rfd open dialog → parse → `PreviewImport` → modal showing current vs incoming for each field (highlight counter decreases and generation change, and warn on device_id mismatch) → **Apply** requires ticking "I understand this replaces my counters" when `is_counter_rollback` → `ImportBackup{confirm:true}`.
- Disable both buttons during PLAYING/COOLDOWN, with the reason shown.

**Acceptance:** A round trip (export, then import the same file) shows "no changes except generation +1". A malformed file shows a validation error without calling ImportBackup.

---

### P2-4. GUI Device page: PC vs ESP counters and recovery actions

**Problem:** The Device page lacks the spec's comparison and recovery actions.

**Spec:** §18 Device (ESP counters, PC counters, sync state; actions: sync, restore ESP from PC, import PC from ESP, update firmware, reset).

**Required change:**
1. Daemon status gains `pc_counters: Option<CounterState>` (SQLite), `esp_counters: Option<CounterState>` (last HelloAck/Status), `last_sync_error`, `pending_replacement` (P1-2), and `device_compat` (P1-7).
2. New IPC requests (all rejected in PLAYING/COOLDOWN, all require `confirm: true`):
   - `RestoreDeviceFromPc`: force-restore the ESP with PC counters, generation = max(pc, esp) + 1; also persist to SQLite.
   - `ImportPcFromDevice`: overwrite SQLite with the ESP state, generation = max(pc, esp) + 1, pushed back to the ESP so both match.
   - `ResolveReplacement { restore_from: Option<String> }` for P1-2.
3. GUI shows a two-column PC | PAD table (K1, K2, total, generation) with mismatches highlighted, plus the actions above, each with a confirmation modal describing exactly what will be overwritten.
4. **Update firmware** button: rfd picker for `.bin` → run `osupadctl flash <file>` as a child process, stream its stdout into a modal, then refresh status. The GUI must not open the serial port itself (§26).

**Acceptance:** With the ESP and PC deliberately different (edit SQLite while the daemon is stopped), both restore directions work from the GUI and survive a daemon restart and pad power cycle.

---

### P2-5. GUI: daemon-offline recovery action

**Problem:** The GUI shows "Daemon offline" but offers no action.

**Spec:** §18.1 (clear recovery action such as Start Daemon / Install or Repair).

**Required change:** When the handshake fails, show a banner with:
- **Start daemon:** `systemctl --user start osupad-daemon.service`; if the unit is missing or systemd user is unavailable, spawn `osupad-daemon` detached (`setsid`, stdout to `$XDG_STATE_HOME/osupad/daemon.log`).
- **Install service:** if the unit file is missing, copy the bundled unit (embed `packaging/linux/systemd-user/osupad-daemon.service` with `include_str!`) into `~/.config/systemd/user/`, then `daemon-reload` and `enable --now`.

Put this Linux-specific code in one module (`gui/src/platform_linux.rs`) to keep A3 in mind. Re-handshake automatically every 2 s while offline.

**Acceptance:** Stop the daemon with the GUI open, click Start daemon, and the GUI reconnects without a restart.

---

### P2-6. IPC hardening and single-daemon guarantee

**Problem:**
- The socket and its directory get default permissions.
- The fallback path `/tmp/osupad-<pid>.sock` uses the *process* id, so without `XDG_RUNTIME_DIR` clients can never find the daemon.
- `create_listener` deletes any existing socket, so a second daemon silently steals the socket from a running one while both fight over the serial port.
- `read_request` allocates whatever length the peer claims (up to 4 GiB).

**Where:** `desktop/crates/osupad-ipc/src/lib.rs:109-113`, `:138`, `:162-170`.

**Spec:** §22, §32 (socket permissions appropriate to the logged-in user).

**Required change:**
1. Path: `$XDG_RUNTIME_DIR/osupad/daemon.sock`; fallback `/tmp/osupad-$UID/daemon.sock` (real UID via `rustix::process::getuid()` or `libc::getuid()`).
2. Create the directory with mode `0700` and verify ownership if it already exists; refuse to use a directory owned by someone else. Set socket mode `0600` after bind.
3. Before removing an existing socket, try to connect to it. If a daemon answers, exit with `"osupad-daemon is already running"` (exit code 0 under systemd is fine). Only remove it if the connect fails (stale socket).
4. Cap the frame length at 1 MiB for requests and 8 MiB for responses; larger frames close the connection with an error.
5. Provide `osupad_ipc::connect_and_handshake()` for GUI and CLI (see P1-7 and A3).

**Acceptance:** Starting a second daemon exits with a clear message and the first keeps working. `stat` shows `0700` on the directory and `0600` on the socket. A test sending a 2 GiB length header does not allocate.

---

### P2-7. Gameplay display update rate uses the configured value

**Problem:** `gameplay_display_hz` is stored and sent but unused. The daemon hard-codes a 100 ms playing flush (`telemetry.rs:8`) and the firmware ignores the field.

**Spec:** §14.2 (rate-limited; ship the highest rate that passes the gate), §18 Display (expose only if useful).

**Required change:** The daemon derives `FLUSH_INTERVAL_PLAYING` from `config.gameplay_display_hz` (clamp `1..=30`). Change the default to **10 Hz** to match the current, owner-tested behaviour. Update the SQLite v1 default through a new migration that only changes rows still at the old default of 5, plus `DeviceConfig::default` and the proto comment. Expose it in GUI Settings under an "Advanced" section. The firmware LVGL refresh stays as is (A1).

**Acceptance:** Setting 2 Hz visibly slows PP/progress updates on the pad; 30 Hz does not change latency stats beyond measurement resolution (record both in P3-1).

---

### P2-8. Finish the GUI-hosted tray (decision A5)

**Problem:** The GUI tray (`desktop/gui/src/tray.rs`) exists, but:
- The menu only has *Open osu!pad*, *Sync pad now*, *Quit*. It lacks the §19 status lines and an *Open Monitor* item.
- Status is only in the tooltip, which many StatusNotifier hosts (KDE, waybar, AppIndicator) show inconsistently.
- Nothing starts the GUI in the background at login, so after a reboot there is no tray until the user opens the app.
- The daemon still depends on `tray-icon` and `winit` without using them.

**Spec:** §19 (content), amended by A5 (location).

**Required change:**
1. **Menu content.** Rebuild the ksni menu from `TrayStatus` on every status update:
   ```text
   osu!pad
   --------------------------
   Pad: Connected            (disabled item; "Disconnected" / "Incompatible firmware" / "Daemon offline")
   Firmware: 1.0.0           (disabled; hidden when unknown)
   Key 1: 1,284,391          (disabled; thousands separators)
   Key 2: 1,176,822          (disabled)
   Last sync: 00:47          (disabled; local time, "Never" if none; "Sync failed" if last_sync_error)
   --------------------------
   Open osu!pad
   Open Monitor
   Sync pad now              (disabled while PLAYING/COOLDOWN or pad offline)
   --------------------------
   Quit osu!pad app
   ```
   Keep the tooltip as a short summary. Use a status-dependent icon (connected / disconnected / daemon offline) if a suitable themed icon exists; otherwise keep one icon.
2. **Open Monitor.** Shows the window and switches to `Page::Monitor`. Also accept `osupad-gui --page monitor` (and `--page device`, etc.) so a second launch forwards the page to the running instance through the existing `single_instance` mechanism.
3. **Quit semantics.** *Quit* exits the GUI only. It must **not** stop the daemon; the label must make that clear. A separate "Stop daemon" action is not needed.
4. **Status source.** The tray updates from the same IPC status poll as the window, and keeps polling while the window is hidden (at a slower rate, for example every 3 s, to save CPU). When the daemon is offline, show "Daemon offline" and an *Start daemon* item that runs the P2-5 action.
5. **Background autostart.** Add `packaging/linux/xdg-autostart/osupad-gui.desktop` with `Exec=osupad-gui --tray` (starts hidden in the tray). `install.sh` installs it into `~/.config/autostart/`. Add a GUI setting "Start in tray at login" that creates or removes that file (Linux-only module, see A3).
6. **No tray host.** Keep the current `TrayEvent::Unavailable` behaviour (close window = quit) and show a one-time hint in the GUI: "No system tray found; the app will quit when closed. The pad keeps working."
7. **Daemon cleanup.** Remove `tray-icon` and `winit` from `desktop/daemon/Cargo.toml` and from `[workspace.dependencies]` if nothing else uses them.

**Acceptance:**
- After login (with the autostart entry installed) the tray icon appears without opening a window, and its menu shows live pad status and counters that update within 3 s of a change.
- *Open Monitor* opens the window on the Monitor page; `osupad-gui --page monitor` with the app already running does the same.
- *Quit* closes the GUI; `osupadctl status` still reports the daemon running and the pad connected.
- Killing the GUI process does not affect HID, display data, or the next post-cooldown sync.
- On a session with no StatusNotifier host, the GUI starts and works as a normal window.

---

### P2-9. XDG autostart fallback

**Spec:** §17.1.

**Required change:** Add `packaging/linux/xdg-autostart/osupad-daemon.desktop` (`Exec=osupad-daemon`, `X-GNOME-Autostart-enabled=true`). `install.sh` detects whether `systemctl --user` works (`systemctl --user is-system-running` or `show-environment`). If it does, install the unit; otherwise copy the autostart entry to `~/.config/autostart/`. Also make sure the unit imports the graphical environment: document `systemctl --user import-environment DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS` or rely on `graphical-session.target`, and verify on the owner's desktop.

**Acceptance:** On a session without systemd user, the daemon starts at login via XDG autostart.

---

### P2-10. Periodic idle sync and clock resync

**Problem:** Presses made outside osu! (IDLE) reach SQLite only after the next map or a manual sync. Time is synced only on connect and after a sync.

**Spec:** §11.3, §14.3 ("periodically while idle if appropriate"), §25.

**Required change:** While the daemon is IDLE and the device is connected, run the P1-1 sync sequence every **5 minutes if counters changed**, and send `TimeSync` every **10 minutes** and on a detected wall-clock jump (> 2 s drift between `Instant` and `SystemTime` deltas, which covers suspend/resume and DST). Never during PLAYING/COOLDOWN.

**Acceptance:** Type 50 presses while idle and wait 5 minutes: SQLite reflects them. After a laptop suspend/resume, the pad clock is corrected within 10 s of reconnect.

---

### P2-11. BSP display API

**Problem:** `ui_port.c` calls `esp_lcd_panel_disp_on_off` and `board_backlight_set` directly. The spec's narrow board API (`board_display_set_brightness`, `board_display_sleep`, `board_display_wake`) does not exist.

**Spec:** §5.

**Required change:** Add those three functions to `boards/waveshare_esp32s3_touch_lcd_2/board.h/.c` (and `board_display.c`), move the panel handle ownership into the board layer, and have `ui_port.c` call only board functions. Keep the rename to `board_get_key1_gpio()` / `board_get_key2_gpio()` as an optional cleanup.

**Acceptance:** `grep esp_lcd_panel firmware/main` returns nothing outside `board_*` files (LVGL port config excepted).

---

### P2-12. SQLite failure isolation in the daemon

**Problem:** `Storage::open(&db_path)?` (`main.rs:62`) exits the daemon if the database cannot be opened or migrated.

**Spec:** §31 ("SQLite failure → do not modify ESP counters destructively; surface error").

**Required change:** On an open/migration failure, start with `storage: None` and `storage_error: Some(msg)` in the status. Device connection, tosu, display data, and layouts pushed from memory keep working. Counter sync must **not** push to the ESP (no PC authority) and restore/import/reset are rejected. The GUI shows the error with the DB path. Retry opening every 60 s.

**Acceptance:** `chmod 000` the DB file: the daemon runs, the pad works, the GUI shows the error, and the ESP counters are untouched.

---

## 5. P3: verification, cleanup, release (Linux)

### P3-1. Latency release gate with recorded numbers

**Spec:** §33 (release gate), §37 ("If a default changes after benchmarking, document the measured reason").

**Required change:**
1. Add a Kconfig menu `osu!pad benchmark` in `firmware/main/Kconfig.projbuild`:
   - `OSUPAD_BENCH_HID_ONLY`: skips `ui_init`, the CDC protocol task, and the runtime supervisor (stage A).
   - `OSUPAD_BENCH_DEBUG_GPIO` + `OSUPAD_BENCH_DEBUG_GPIO_NUM`: toggle a spare GPIO in `keypad_task` right after the HID submit, for logic-analyzer correlation. Verify the pin is free on the Waveshare header; it must not be the key pins, LCD, USB, or strapping pins.
2. Stage A (HID-only) does not have CDC, so the stats cannot be read over the protocol. Print the latency stats on the USB-Serial-JTAG console every 10 s in that build only, or read them via a GPIO-triggered log.
3. Procedure (write it in `docs/latency-testing.md`): reset stats (`osupadctl latency --reset`), 2 minutes of alternating taps at > 15 presses/s, then read p50/p99/p99.9/max/deferred. Repeat for stages A, B (daemon connected, no display updates: add `OSUPAD_BENCH_NO_DISPLAY`), C (display + gameplay data at the chosen Hz, using a recorded tosu replay or a real map), and D (full stack including sync transitions).
4. Record a results table (date, firmware commit, stage, samples, p50, p99, p99.9, max, deferred) in `docs/latency-testing.md`. Gate: p99.9 delta from stage A < 0.1 ms, no new > 1 ms outliers, no stuck or missed keys (§33.3).
5. Also run `scripts/bench_latency.py` (host-side evdev jitter) for stages A and D, and record the numbers.

**Acceptance:** A filled table for A–D exists in the docs and the gate passes.

---

### P3-2. Automated tests

**Spec:** §34, §35 ("each phase should end with tests").

**Required change:**
1. **Refactor for testability (daemon).** Split `desktop/daemon/src/main.rs` (~750 lines) into:
   - `runtime.rs`: a pure state machine `RuntimeController::on_event(Event, now) -> Vec<Action>` with no I/O.
   - `sync.rs`: the P1-1/P1-2 sequences over a `DeviceLink` trait and `Storage`.
   - `ipc_handlers.rs`: request handling over the same traits.
   - `log_hub.rs` (P2-2).
   - `main.rs`: wiring only.
2. **Host tests** (in `desktop/daemon/tests/` or module tests), with a fake `DeviceLink` and in-memory `Storage`:
   - daemon with no ESP; daemon with ESP but no tosu; tosu reconnect
   - PLAYING → COOLDOWN → PLAYING (no sync, no writes)
   - PLAYING → COOLDOWN → SYNC → IDLE (exactly one sync)
   - zero Storage writes during PLAYING/COOLDOWN for every IPC operation (P1-3 guard)
   - reconcile PC→ESP, ESP→PC, stale generation, replacement prompt (P1-2)
   - device rejects sync → retry and error surfaced (P1-1)
   - JSON validation rules and preview/confirm (P1-5)
   - IPC handshake mismatch (P1-7); oversized frame (P2-6); second daemon refused (P2-6)
   - LogHub `since_seq` paging (P2-2)
   - queue does not block without a device (P1-8)
3. **Firmware unit tests** for pure logic, built and run on the host with plain `gcc` + Unity or a minimal test runner under `firmware/test/host/`:
   - `debounce_step` (P0-2)
   - counter sync acceptance rules (extract from `counters_sync_from_host`)
   - config validation (P0-3)
   - diag ring buffer overflow (P2-1)
   - protocol frame parser: split frames, oversized length, garbage, back-to-back frames (extract framing from `protocol_feed_cdc_bytes`)
4. **Hardware checklist** in `docs/testing-checklist.md`, run manually before release: key 1/2 press/release, simultaneous keys, rapid streams, display asleep then press (HID first), CDC absent, malformed frame (a script that sends garbage over the port), LCD fail build, NVS fail build, power-cycle counter persistence, long session (≥ 2 h of real play), reconnect (unplug/replug × 20), daemon kill during play, tosu kill during play.

**Acceptance:** `cargo test --workspace` and the firmware host tests pass in CI (P3-3). The checklist is filled in once for the release commit.

---

### P3-3. CI

**Required change:** `.github/workflows/ci.yml` with:
- Rust: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`. Install the system dependencies needed by iced/rfd/ksni on Ubuntu; the `osupad-ui-preview` C build needs a C compiler.
- Firmware: `espressif/idf:v5.5.2` container, `idf.py set-target esp32s3 build`, upload `build/*.bin` as an artifact.
- Firmware host unit tests.
- Proto drift check: regenerate `osupad.pb.c/.h` with the pinned nanopb generator and fail if `git diff` is non-empty. Add a `scripts/gen_proto.sh` used by both developers and CI.

**Acceptance:** CI is green on `main`.

---

### P3-4. Version and identity cleanup

**Required change:**
- Single version source (P1-7): firmware `PROJECT_VER`; host `CARGO_PKG_VERSION`; set the workspace version to `1.0.0` at release.
- USB serial string is hard-coded `"OSUPAD-S3-0001"` (`usb_descriptors.c`). Generate it from the MAC at runtime (same format as `device_id`) so two pads do not collide in `/dev/serial/by-id/`.
- `DeviceInfo::default()` should not invent `firmware_version: "1.0.0"` / `device_id: "unknown"`. Use `Option` fields or empty strings, and let the GUI render "—".
- Hard-coded `firmware_version: "1.0.0"` in the daemon's `ImportBackup` (`main.rs` ~630): use the last known value.

---

### P3-5. Dead code and config cleanup

- `firmware/partitions.csv` is unused because `sdkconfig.defaults` selects `CONFIG_PARTITION_TABLE_SINGLE_APP_LARGE`. Delete it; OTA is not needed (§26: local USB flashing). Do not change the active partition table, because NVS must stay at `0x9000`.
- Daemon `Cargo.toml`: remove the unused `tray-icon`/`winit` dependencies (done as part of P2-8).
- `press_color_rgb` (A4): stop exposing it in `DeviceConfig`/IPC/JSON, leave the SQLite column (harmless) and the proto field (keep the number, mark `deprecated = true`).
- SQLite v1 default `tosu_endpoint 'ws://127.0.0.1:24050/ws'` differs from `DeviceConfig::default` (`/websocket/v2`). Add a migration to normalise it, or confirm `normalize_endpoint` handles it and add a test.
- `firmware/main/protocol/protocol.c` `handle_host_message`: unknown payloads log at debug and are ignored. Keep that, but count them in diag (P2-1).
- Remove the leftover `usb_hid_send_keyboard_report` if unused.

---

### P3-6. Documentation updates

Update these to match reality after the tasks above:
- `docs/architecture.md`: LVGL (A1) and its core/timer constraints, the layout designer and data-source pipeline (A2), tray hosted by the GUI and why (A5), including the GUI's background lifetime and login autostart, task/core map (core 0: key ISR, keypad task, TinyUSB; core 1: CDC protocol, runtime supervisor, LVGL, SPI ISR).
- `docs/protocol.md`: all current messages including `HostStatus`, `DataUpdate`, `SetLayout`, `LayoutAck`, `reset_layout`, `request_status`, `reset_latency_stats`, the BOOTLOADER / 1200-baud reboot mechanisms, new fields added by these tasks, and the version negotiation rules.
- `docs/recovery.md`: connect-time reconciliation, the replacement flow, restore actions, deferred operations.
- `docs/latency-testing.md`: procedure and results table (P3-1).
- `README.md`: build/install/flash steps verified from a clean machine; the Windows note says "planned after Linux v1.0".
- The spec's §39 open decisions that are now fixed (GPIO 14/9, LVGL, core affinity, gameplay Hz, espflash): list them in `docs/architecture.md` with the reason.

---

### P3-7. Linux release

- `scripts/release/build_release.sh`: builds the firmware `.bin` and release host binaries, and produces `SHA256SUMS`.
- `osupadctl flash` verifies the file is an ESP32-S3 app image (magic byte `0xE9`, chip id in the extended header) before flashing (§32 "firmware update files should be validated").
- `Cargo.lock` committed and dependencies not bumped after tagging (§16.2).
- Tag `v1.0.0` once the §40 Definition of Done (Appendix A) is all checked.

---

## Appendix A: §40 Definition of Done status (Linux)

Legend: ✅ done · 🟡 partial · ❌ missing. Task IDs show what closes each item.

**Keyboard**
- ✅ Enumerates as a keyboard without host software
- 🟡 KEY_1/KEY_2 produce configured usages. Lost on reboot without the daemon → **P0-3**
- ✅ 1 ms HID endpoint interval (`bInterval = 1`)
- 🟡 Fast streams / simultaneous presses do not miss or stick. Debounce re-sample missing → **P0-2**
- 🟡 Full stack passes the latency gate. Owner reports good feel; numbers not recorded → **P3-1**

**Device display**
- ✅ Idle clock from host time
- ✅ Lifetime counters shown
- ✅ Display sleeps after the configured interval
- ✅ Wakes on key press after the HID submit
- ✅ Gameplay view: title, artist, PP, progress, map key counts
- 🟡 LCD failure does not break the keyboard → **P0-1**

**Persistence / recovery**
- ✅ ESP lifetime counters survive restart
- ✅ PC counters/config survive restart in SQLite. Not loaded at daemon startup → **P1-4**
- ❌ No NVS/SQLite writes in PLAYING/COOLDOWN → **P1-3**
- ❌ ESP restores PC and PC restores ESP → **P1-1**, **P1-2**, **P2-4**
- 🟡 JSON export/import is version-validated. No range checks, preview, or confirmation; offline export gives zeros → **P1-4**, **P1-5**, **P2-3**

**Daemon**
- ✅ Runs without GUI
- 🟡 Auto connect/reconnect to device and tosu. Can hang without a device → **P1-8**
- 🟡 PLAYING/COOLDOWN/SYNC/IDLE. Sync is fire-and-forget and rejected by the device → **P1-1**
- ✅ Owns tray → amended by **A5**: the GUI hosts the tray; the daemon owns everything else
- 🟡 Owns IPC. No version check, permissions, or single-instance guard → **P1-7**, **P2-6**
- ✅ Tray failure does not kill core. The tray is in the GUI process, so it cannot take down the daemon; keep it covered by the P2-8 acceptance checks

**GUI**
- 🟡 Detects daemon via IPC. No handshake on requests, no recovery action → **P1-7**, **P2-5**
- ✅ Required configuration (keys, debounce, brightness, sleep)
- 🟡 Lifetime/device sync state. No PC vs ESP comparison → **P2-4**
- ❌ Backup/recovery actions through daemon → **P2-3**, **P2-4**
- ❌ Monitor shows host **and ESP** events with filters → **P2-1**, **P2-2**
- 🟡 Tray (GUI-hosted, A5). Missing §19 status lines, Open Monitor, and login autostart → **P2-8**

**Maintenance**
- 🟡 README build/install instructions → **P3-6**
- 🟡 Architecture and protocol documented. Out of date → **P3-6**
- 🟡 Dependencies pinned for release → **P3-7**
- ✅ Mature dependencies used (TinyUSB, NVS, esp_lcd, LVGL, nanopb, prost, tokio, rusqlite, iced, espflash)
- 🟡 Linux release usable → all of the above; Windows deferred (**A3**)

---

## Appendix B: explicitly out of scope for this round

- ~~Windows support (named pipes, COM discovery, login startup): **after** Linux v1.0 (A3).~~
  **Reversed 2026-09-16** by `osupad_packaging_distribution_plan.md` — it is in
  v1.0. See the note on A3 above.
- ESP-NOW wireless dongle, battery, and heavy-ballast case V2 (`docs/roadmap.md`): post-v1.0.
- Touch UI, Wi-Fi, Bluetooth, NTP, macros, RGB, more than two keys (spec §2.2).
- ~~A custom OTA subsystem (spec §26; flashing stays espflash over USB).~~
  **Partially reversed 2026-09-16** by §U-3 of the packaging plan, and the
  reversal is deliberate rather than scope creep:
  - **In v1.0 (§U-3a):** the *partition layout* changes to two OTA slots now.
    Rewriting the table at `0x8000` cannot be done by an OTA update — it needs a
    serial reflash of every pad in the field. The field is currently about one
    pad, and that will never be truer than it is today.
  - **In v1.0 (§U-3b):** firmware updates are host-driven flashes over USB,
    which is still espflash, just driven by the daemon with a verified image and
    explicit consent. No new firmware attack surface.
  - **Still out of scope (§U-3c):** the OTA subsystem itself — streaming into
    the inactive slot over CDC, slot switching, and bootloader rollback. That is
    what the layout is being put in place for, and it is post-v1.0.

---

## Appendix C: review fixes and work split (added 2026-09-13)

A review of `v1.0-work` up to `c04fef0` found the issues below. Two agents are now working in parallel. **Stay inside your own column to avoid merge conflicts.**

**Status (2026-09-13):** R1–R9 and P3-5–P3-7 are done and merged on `v1.0-work` (R1–R7 in `3ecf6ca`, `2a44e3c`, `f306a3a`). While fixing R4–R6, one more bug turned up and was fixed in `2a44e3c`: the daemon main loop overwrote the shared state after every event, dropping IPC changes (config, layouts, replacement choice). `fmt`, `clippy -D warnings`, 65 Rust tests, the firmware host tests, the normal firmware build and the Stage A build all pass. Still open: hardware-only work (latency table in `docs/latency-testing.md`, `docs/testing-checklist.md` run).

### Claude (branch `v1.0-claude`, separate worktree; merged into `v1.0-work` when done)

| ID | Problem | Files |
|---|---|---|
| R1 | `docs/latency-testing.md` results table is not real data (Stage A cannot compile; firmware percentiles are 10 µs bucket edges, table shows 34/48/58; debounce description is wrong). Replace it with an empty table to be filled from real hardware runs. | `docs/latency-testing.md` (results section only) |
| R2 | Stage A benchmark build does not compile (`keypad_get_latency_stats`, `sample_count`, `deferred_count` do not exist). | `firmware/main/app_main.c` (bench block only) |
| R3 | HelloAck emits `Connected` **before** `Counters`, so the connect handler sees the previous pad's counters: replacement prompt never fires and old counters get saved under the new device_id. | `osupad-device` `handle_device_message` (HelloAck arm only), `daemon/src/runtime.rs` connect handling, daemon tests |
| R4 | A failed sync (device not IDLE in 3 s, or 3 rejected attempts) leaves the daemon in `Sync` and may leave storage writes blocked; no retry. | `daemon/src/sync.rs`, `daemon/src/runtime.rs` |
| R5 | Possible self-deadlock on SQLite recovery (`if let Some(..) = &state.lock()` then re-locking inside). | `daemon/src/main.rs` |
| R6 | `perform_sync` is awaited inside the main select loop (stalls tosu/status/display for up to ~10 s); `ForceSync` returns `success: true` even when the sync failed. | `daemon/src/main.rs`, `daemon/src/ipc_handlers.rs` (ForceSync arm only) |
| R7 | GUI "Install service" writes a second unit `osupad.service` (network.target/default.target) instead of the packaged `osupad-daemon.service`; "Start daemon" spawns the binary with output to /dev/null instead of `systemctl --user start`. | `gui/src/platform_linux.rs`, `gui/src/main.rs` (`start_daemon_process` only) |

### Antigravity (branch `v1.0-work`)

- Finish **P3-5, P3-6, P3-7** as planned.
- **R8** (fits P3-5): `device_config_init` can `nvs_flash_erase()` without a `DIAG_EVENT_NVS_ERASED` record (it also wipes counters); `protocol_send_config_ack` hard-codes `gameplay_display_hz = 10`. Files: `firmware/main/config/device_config.c`, `firmware/main/protocol/protocol.c`.
- **R9**: `firmware/test/host/test_frame_parser` binary must not be committed (add `firmware/test/host/test_*` binaries to `.gitignore`); make sure `run_tests.sh` passes, including the overflow test.
- In **P3-6**, do **not** write latency numbers into `docs/latency-testing.md`. Only the owner fills them from real hardware runs.
- Avoid editing the files listed in Claude's column. If you must, keep the hunk small and mention it in the commit message.
- Commit often (small commits). Claude rebases onto your latest commit before merging.
