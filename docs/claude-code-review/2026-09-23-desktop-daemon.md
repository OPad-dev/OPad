# Code review: `desktop/daemon`

- Date: 2026-09-23
- Command: `/code-review high desktop/daemon`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched the daemon, so the review covers its current code.
- Verification: the reviewer confirmed that `pending_device_push` is written but never read, that `foreign_pad` is not consulted by any IPC handler / `perform_sync` / the firmware installer and is not carried by `IpcResponse::Status`, that the device crate emits `Ownership` → `Counters` → `Connected` in that order, and that the tosu supervisor already strips ANSI before the callback. Other details were not re-verified.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. COOLDOWN→SYNC transition fires `TriggerSync` without the foreign-pad guards — `fixed`

**File:** `desktop/daemon/src/runtime.rs:625`
**Category:** correctness

The COOLDOWN→SYNC transition fires `TriggerSync` unconditionally, bypassing the `foreign_pad` / `pending_takeover` / `pending_replacement` guards that every other sync trigger honours.

**Failure scenario:** A pad owned by another install connects (`Ownership::Someone` → `foreign_pad = true`, prompt pending, connect-time sync correctly suppressed). The user plays one map. On cooldown expiry `Tick` pushes `TriggerSync`; `perform_sync` checks only storage, reconciles against a zeroed stored row, calls `save_device_state` for the foreign pad, sends `CounterSync` to it, and `SyncCompleted` inserts it into `known_devices` and arms the auto-backup with its counters. §W3-3 ("no counter may ever be written under the wrong owner", "nothing is synced from it") is violated by the single most common path: playing.

## 2. No IPC handler or the firmware installer checks `foreign_pad`; status does not expose it — `fixed`

**File:** `desktop/daemon/src/ipc_handlers.rs:374`
**Category:** correctness

No IPC handler (`ForceSync`, `UpdateConfig`, `SetLayout`/`ResetLayout`, `ResetCounters`, `RestoreDeviceFromPc`, `ImportPcFromDevice`, `ImportBackup`) nor `firmware_update::install` checks `foreign_pad`, and `IpcResponse::Status` does not expose the flag, so nothing upstream can gate on it.

**Failure scenario:** User answers the takeover prompt with "leave it alone" (`ResolveTakeover take_over = false`): `pending_takeover` is cleared but `foreign_pad` stays true. `GetStatus` now looks like an ordinary connected pad. The GUI/CLI issues `ForceSync` (or `UpdateConfig` / `SetLayout`) → `perform_sync` writes the foreign pad's counters to SQLite and pushes `CounterSync`/config/layout to a pad the user just said to leave untouched.

## 3. Incompatible-protocol pad is adopted as connected before the version check — `open`

**File:** `desktop/daemon/src/runtime.rs:326`
**Category:** correctness

On `DeviceConnected` the pad's config is adopted and persisted, and `device_connected`/`counters_source` are set, before the `protocol_version` check; the early return for an incompatible pad then leaves the daemon treating it as connected with a stale `device_info`.

**Failure scenario:** A pad with `protocol_version` 2 connects. Lines 303-338 set `device_connected = true` and emit `SaveDeviceConfig(cfg)` from the incompatible firmware, overwriting the stored config. The early return at 345 skips setting `device_info`, so `state.device_info` is still the previous pad loaded from SQLite at startup. `Tick` then sends `RequestDeviceStatus`/`HostStatus` to the incompatible pad every second, and after 300 s (`last_synced_counters` is None) fires `TriggerSync`: `perform_sync` uses the old pad's `device_info` with the new pad's `Counters` (emitted before `Connected`) and saves them under the previous pad's `device_id`.

## 4. `firmware_update::install` checks IDLE once, then flashes without re-checking mode — `fixed`

**File:** `desktop/daemon/src/firmware_update.rs:208`
**Category:** correctness

`install()` checks IDLE once up front, then runs a `perform_sync` (up to ~10 s), an HTTP download and a stage before releasing the port and flashing, with no re-check of mode; it also calls `perform_sync` without any takeover/`foreign_pad` check.

**Failure scenario:** GUI sends `InstallFirmwareUpdate` while IDLE. The user starts a map during the sync/download (mode → PLAYING, `SetStorageWritesAllowed(false)`). `install()` proceeds to `pause_and_release` and `flash::flash`: the pad is rebooted mid-map (P1-3 / §U-0.1 violation) and the keyboard goes away during play. The updater's `tick()` explicitly re-reads mode for exactly this reason; the firmware path does not.

## 5. On sync failure the in-memory counters are overwritten with values the pad rejected — `open`

**File:** `desktop/daemon/src/sync.rs:340`
**Category:** correctness

On sync failure `perform_sync` overwrites the shared in-memory counters with the PC-reconciled values the pad just rejected, while leaving `esp_counters` and `counters_source = Device` untouched.

**Failure scenario:** Pad rejects `CounterSync` three times (e.g. it counted presses after the snapshot and the retry re-reconcile did not converge). `st.counters` becomes the reconciled PC view; `GetStatus` reports counters (source = Device) that the pad does not hold, contradicting `esp_counters` in the same response, until the next `DeviceCounters` event happens to arrive. The failure `SyncCompleted` in `main.rs` then feeds `controller.state.counters` (now the wrong values) back through `apply_event`.

## 6. After takeover only the suppressed config is re-sent, not the suppressed layouts — `fixed`

**File:** `desktop/daemon/src/ipc_handlers.rs:708`
**Category:** correctness

After a successful takeover only the suppressed config is re-sent; the custom layouts that `DeviceConnected` withheld because `foreign_pad` was true (`runtime.rs:438`) are never pushed.

**Failure scenario:** Foreign pad connects with custom Idle/Playing layouts stored on this PC; `SendLayout` is skipped at connect. User accepts the takeover: `send_config` goes out, `foreign_pad = false`, but the pad keeps its default layouts until it is physically re-plugged. The comment says "the config we suppressed while it was foreign goes out" but layouts were suppressed under the same condition.

## 7. AppImage self-replace uses cross-filesystem `rename`; package install blocks a tokio worker — `open`

**File:** `desktop/daemon/src/updater.rs:390`
**Category:** correctness / efficiency

AppImage self-replacement uses `std::fs::rename` from the state directory to `$APPIMAGE`, which fails with `EXDEV` across filesystems, and `apply_downloaded` blocks a tokio worker on a synchronous `Command::status()` that can wait minutes on a polkit prompt.

**Failure scenario:** AppImage lives on `/opt` or a second drive while `state_dir` is under `$HOME`: rename returns "Invalid cross-device link", the staged file is deleted at line 343, and the user sees an error after clicking Install even though the download verified. Separately, `pkexec apt/dnf` holding for a password pins one runtime worker thread for the whole prompt.

**Suggested fix:** Copy+rename (or `fs::copy` then set permissions) when `rename` fails with `EXDEV`; use `tokio::process::Command` or `spawn_blocking` for the package manager call.

## 8. `pending_device_push` is written but never read — `open` (confirmed)

**File:** `desktop/daemon/src/ipc_handlers.rs:430`
**Category:** correctness

`PendingOperations::pending_device_push` is set in two places in `ResetCounters` but never read anywhere in the codebase (`perform_sync` drains config, layouts and `last_seen` only).

**Failure scenario:** `ResetCounters` while disconnected with no `device_info` (never-connected install) zeroes `state.counters` and sets the flag; on connect the pad's `DeviceCounters` event overwrites `state.counters` and nothing consumes the flag, so the reset silently evaporates. Where `device_info` exists the reset survives only by accident of generation reconciliation.

**Suggested fix:** Either drop the field or have `perform_sync` honour it with `force_restore = true`.

## 9. `apply_event` deep-clones the full `DaemonState` on every event — `open`

**File:** `desktop/daemon/src/runtime.rs:799`
**Category:** efficiency

`apply_event` deep-clones the full `DaemonState` (`custom_layouts: HashMap<Screen, Layout>`, `ui_values: Vec`, all `Option<String>`s) into the controller and back out on every event, i.e. at least 20×/s on the 50 ms tick plus every tosu frame and every device event.

**Failure scenario:** With two custom layouts and live telemetry, every tick allocates and frees two copies of the layout trees and value vectors while holding the shared lock that `GetStatus`/`GetUiValues` contend on.

**Suggested fix:** Keep the state only behind the `Arc<Mutex>` (controller borrows it under the lock) or diff just the fields IPC handlers may write.

## 10. Duplicated tosu log classifier; hardcoded `v1.0.0` banner — `open`

**File:** `desktop/daemon/src/main.rs:175`
**Category:** simplification

The tosu line → `LogLevel` classification is copy-pasted twice (startup log-tail at 175-194 and live callback at 197-209), and the startup banner hardcodes `v1.0.0` instead of `env!("CARGO_PKG_VERSION")` used everywhere else.

**Failure scenario:** Any tweak to the heuristic (e.g. adding `ERR`/`WARNING`, or matching case-insensitively) must be made in two places and the two copies already differ (one calls `strip_ansi`, the other relies on the supervisor doing it — which it does, so this is duplication rather than a bug). After the next version bump the daemon log still announces 1.0.0 while `Handshake` rejects clients that are not `CARGO_PKG_VERSION`, which makes the startup line actively misleading when debugging a version-mismatch report.
