# Code review: `desktop/gui`

- Date: 2026-09-23
- Command: `/code-review high desktop/gui`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched `desktop/gui`, so the review covers all 12 source files (~8.4k lines) plus the daemon/IPC/layout code they call into.
- Verification: finding 2 was confirmed via daemon source (`LogHub::get_entries` filters `e.seq > since`, exclusive). Others were not re-verified.
- Dropped after cross-checking: "designer Delete/arrow keys fire while typing in a text field" — iced 0.14's `keyboard::listen()` only forwards `Status::Ignored` events, so a focused `text_input` swallows them first. `idx()` in the designer is correct (`Idle = 0`, `Playing = 1`).
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `ShowRequested` never matches the `"logs"` / `"diagnostics"` page tokens main() sends — `fixed`

**File:** `desktop/gui/src/main.rs:1493`
**Category:** correctness

`ShowRequested` never matches the `"logs"` (or `"diagnostics"`) page token that `main()` itself sends, so a second launch with `--page logs`/`monitor` cannot switch the running instance to the Logs page.

**Failure scenario:** Instance A is running. User runs `opad-gui --page monitor` (or `--page logs`): `parse_page_arg` maps both to `Page::Logs`, `main()` sends `show logs` over the single-instance socket, the running instance's match has arms for `"monitor"`/`"device"`/... but not `"logs"`, falls into `_ => {}`, and just focuses the window on whatever page it was already showing. Same for `--page diagnostics`.

## 2. `log_event()` bumps the daemon fetch cursor, skipping one real entry per local log — `fixed` (confirmed)

**File:** `desktop/gui/src/main.rs:2597`
**Category:** correctness

`log_event()` bumps `latest_log_seq`, which is also the daemon fetch cursor, so every GUI-local log entry makes the next `GetLogEntries` skip one real daemon entry forever (daemon filters `e.seq > since_seq`).

**Failure scenario:** At startup `log_event("OPad application initialized")` sets `latest_log_seq = 1`, so the first poll asks `since_seq = Some(1)` and the daemon's entry seq 1 is never fetched. Clicking "Start daemon" logs two local entries → the next two daemon entries (the ones describing the daemon starting) are silently dropped from the Logs page and diagnostic bundle.

**Suggested fix:** Keep a separate local seq counter or only advance the cursor from the daemon's `latest_seq`.

## 3. Tray watcher is probed exactly once; a late watcher is never used — `fixed`

**File:** `desktop/gui/src/tray.rs:328`
**Category:** correctness

The tray stream probes for `org.kde.StatusNotifierWatcher` exactly once (2 s timeout) and then terminates; the subscription is never restarted, so a watcher that appears a few seconds later is never used for the life of the process.

**Failure scenario:** GUI autostarts at login with `--tray` via `/etc/xdg/autostart` before the AppIndicator extension / SNI host has registered on the session bus. `name_has_owner` returns false, `TrayEvent::Unavailable` is emitted, `tray_available` becomes `Some(false)` permanently: the window pops up at every login despite `--tray`, the "No system tray found" banner shows, and closing the window exits the app instead of hiding it — even though the tray host was ready 1-2 s later. ksni itself would have handled late watcher registration if the pre-check were retried or skipped.

## 4. `ImportCompleted` skips `gameplay_display_hz` and blocks re-adoption — `fixed`

**File:** `desktop/gui/src/main.rs:791`
**Category:** correctness

`ImportCompleted` copies every form field from the imported config except `gameplay_display_hz`, and because it also sets `self.config = config` the next Status poll sees no change and never re-adopts it.

**Failure scenario:** Backup has `gameplay_display_hz = 20`; the current form shows 10. After import, `self.config.gameplay_display_hz = 20` but the Settings slider stays at 10 (the adopt condition `!config_loaded || self.config != config || device_just_connected` is false on the next poll). User later presses "Save settings" for an unrelated change and `SaveConfig` writes `gameplay_display_hz = 10` back over the value the import just restored.

## 5. `Message::Applied` marks the current (screen, layout), not the one that was sent — `fixed`

**File:** `desktop/gui/src/designer/mod.rs:380`
**Category:** correctness

`Message::Applied` marks `applied[idx(self.screen)] = self.layout().clone()` using the screen and working layout at response time, not the (screen, layout) that was actually sent in `Message::Apply`.

**Failure scenario:** User presses Apply on the Playing screen, then clicks the "Idle screen" tab while the request is in flight (`busy` only disables Apply, not `SelectScreen` or edits). `LayoutApplied` arrives: `applied[Idle]` is overwritten with the unsent Idle working copy, and `applied[Playing]` is never updated. The Playing tab still shows the "Apply to pad •" dirty marker, the Idle tab shows clean although the pad has a different layout, and Revert on Idle "reverts" to something the pad never received. Same corruption if the user edits a widget during the in-flight apply.

**Suggested fix:** Carry (screen, layout) through `apply_layout` into `Applied`.

## 6. Windows single-instance pipe is created lazily; a second launch can exit silently — `fixed` (tested on the Windows VM at the pipe level: back-to-back handoffs before iced starts; the GUI window itself not launched)

**File:** `desktop/gui/src/single_instance.rs:133`
**Category:** correctness

On Windows the single-instance pipe is created lazily inside the `show_requests` subscription, so a second launch that wins the mutex race but finds no (or a busy) pipe drops its show request and exits silently.

**Failure scenario:** User double-clicks the launcher and a second process starts while the first is still initializing iced (before its subscription has called `create_pipe_instance`), or between `server.connect()` completing and the next `create_pipe_instance(pipe_name, false)`. `CreateMutexW` reports `ERROR_ALREADY_EXISTS`, `OpenOptions::open(pipe)` fails (`ERROR_FILE_NOT_FOUND` / `ERROR_PIPE_BUSY`), the error is ignored with `if let Ok`, and the second process returns `Ok(())` — the window never appears. On unix the listener is bound synchronously in `claim()`, so only the Windows path has this gap.

## 7. `CloseRequested` exits while the tray is still being probed — `fixed`

**File:** `desktop/gui/src/main.rs:1472`
**Category:** correctness

`CloseRequested` exits the whole app whenever `tray_available != Some(true)`, which includes the `None` state while the tray is still being probed, so closing the window in the first seconds after launch quits instead of hiding to tray.

**Failure scenario:** Launch `opad-gui` normally; the Linux tray stream can take up to ~4 s (2 s watcher probe + 2 s spawn timeout). User closes the window at t = 1.5 s: `tray_available` is `None`, `!= Some(true)` is true, `iced::exit()` runs and the app is gone even though the tray icon would have appeared a moment later. The fallback timer at 2 s then also flips it to `Some(false)` until `Started` arrives, extending the window in which close == quit.

## 8. Unconditional unlink of `gui.sock` lets two simultaneous launches both become "the" instance — `fixed`

**File:** `desktop/gui/src/single_instance.rs:34`
**Category:** correctness

Unconditionally unlinking `gui.sock` before bind lets two near-simultaneous launches both become "the" instance: the second removes the first's freshly bound socket.

**Failure scenario:** A launcher double-click starts two processes ~10 ms apart. Both `connect()` attempts fail (no listener yet), process A binds `gui.sock`, process B then runs `remove_file(gui.sock)` and binds its own. Both proceed to run a full GUI with two tray icons; A's listener is orphaned so later `opad-gui` launches only ever reach B.

**Suggested fix:** Bind first and only remove the file when `connect` fails with `ECONNREFUSED` (a genuinely stale socket).

## 9. 4 ms `DiagnosticsPoll` drives ~250 update+view cycles/s with heavy view work — `fixed` (bundle string not cached)

**File:** `desktop/gui/src/main.rs:1601`
**Category:** efficiency

A 4 ms `DiagnosticsPoll` timer drives ~250 update+view cycles per second while the Diagnostics page is open, and the view rebuilds heavy content (pretty-printed JSON bundle of 50 logs, ptrace cache lock) on every one of them.

**Failure scenario:** Open Diagnostics → Export Bundle tab: every 4 ms iced runs `update()` (mutex lock + evdev ioctl), then `view()` which calls `generate_diagnostic_bundle()` → `serde_json::to_string_pretty` over the whole bundle, every frame — sustained CPU on a page meant to diagnose latency.

**Suggested fix:** Move evdev/`GetAsyncKeyState` polling into a `Subscription` stream on its own thread that emits a `DiagnosticsMessage::KeyEvent` only on state change (`handle_key_event` is already edge-triggered), and cache the bundle string in state.

**Status note (A7, 2026-09-24):** The key polling now runs on its own thread, still every 4 ms. It emits `KeyEvent`s only on a change, each stamped with the `Instant` it was seen, so the chatter/flutter timing is not delayed by the UI queue. With the 250 Hz tick gone, `view()` and the bundle only rebuild on real messages. Measured on Linux (debug build, Diagnostics page, 8 s): 28.2% of a core before, 0.8% after. The bundle string itself is **not** cached. It embeds `chrono::Local::now()` with sub-second precision, which the preview shows live, so a cache would freeze it. That is a visible change, and not needed now that the view runs about once a second.

## 10. Four IPC connections per poll tick regardless of visible page — `open` (partly fixed)

**File:** `desktop/gui/src/main.rs:476`
**Category:** efficiency

Every poll tick opens four separate IPC connections with full handshakes (`GetStatus`, `GetUiValues`, `GetUpdateStatus`, `GetFirmwareUpdate`) regardless of which page is visible; update and firmware status are re-fetched every second although they change on the order of hours.

**Failure scenario:** Window open, daemon online: 4 connect+handshake+request round trips per second (plus a 5th on the Logs page), 3 per 3 s in the tray. `GetUiValues` is fetched even on Settings/Device/About where nothing reads `ui_values`, and `GetUpdateStatus`/`GetFirmwareUpdate` are hammered once a second though the Settings → Updates panel is the only consumer.

**Suggested fix:** Poll update/firmware on Navigate to Settings plus a slow (e.g. 60 s) timer, and only fetch `UiValues` on Dashboard/Designer.

**Status note (A7, 2026-09-24):** Now polled:
- `GetStatus`: always.
- `GetUiValues`: only on Dashboard/Designer.
- `GetFirmwareUpdate`: only on the pages that read it (Settings' firmware card, Device's "Running slot" fallback, the Diagnostics bundle).
- Tray only (no window): `GetStatus` alone.

Navigating and opening the window poll at once. The window is 3 connections per tick on every page except About, which is 2 (it was 4 everywhere, 5 on Logs). The tray is 1 per 3 s.

**Not done:** moving `GetUpdateStatus` to a 60 s timer. Its `restart_required` drives the "OPad was updated. Restart…" banner on every page, and the Settings → Updates panel shows live state. A 60 s timer would delay both by up to a minute, which is a behaviour change and needs a decision (e.g. a daemon push, or accepting the delay).
