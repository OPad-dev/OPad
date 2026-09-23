# Code review: `desktop/crates/opad-protocol`, `opad-device`, `opad-ipc`

- Date: 2026-09-23
- Command: `/code-review high desktop/crates/opad-protocol desktop/crates/opad-device desktop/crates/opad-ipc`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched these crates, so the review covers their current code.
- Verification: findings were not re-verified after the review; treat each as *plausible* until checked against the code.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `Instant::now() - Duration::from_secs(10)` panics early after boot on Windows — `open`

**File:** `desktop/crates/opad-device/src/lib.rs:268` (also lines 291 and 296)
**Category:** correctness

`Instant::now() - Duration::from_secs(10)` panics with "overflow when subtracting duration from instant" on Windows when the process starts less than 10 s after boot, and the panic happens inside the `spawn_blocking` worker whose `JoinHandle` is dropped, so the serial worker dies silently and the daemon never connects to a pad.

**Failure scenario:** Windows autostarts `opad-daemon` at login on a fast-booting machine; QPC-based `Instant` is < 10 s since boot → `Instant::sub` panics on the first `last_hello` initialisation → worker thread gone, `is_connected` stays false forever, no log beyond the swallowed panic.

**Suggested fix:** Use `Option<Instant>` / `checked_sub`, or a "send Hello now" flag instead of a backdated `Instant`.

## 2. Frames in flight are dropped after a re-Hello while `is_connected` stays true — `fixed`

**File:** `desktop/crates/opad-device/src/lib.rs:360`
**Category:** correctness

After a re-Hello (`rehandshake()` at 294-297 or the stale-partial-frame drop at 283-292) `has_hello_ack` is cleared while `is_connected` stays true, so every non-HelloAck frame already in flight (`CounterSyncResp`, `ConfigAck`, `LayoutAck`) is silently discarded by `else if !has_hello_ack { continue }` until the pad answers Hello again.

**Failure scenario:** Daemon's broadcast receiver lags → `main.rs:310` calls `rehandshake()` while `sync.rs` step 2 is awaiting `CounterSyncResult` for seq N → the pad's reply arrives before its HelloAck and is dropped → sync times out and retries (or fails after 3 tries) although the pad answered correctly. Same for `ipc_handlers.rs:1138` waiting on `LayoutAck`.

**Suggested fix:** Keep dispatching frames once the connection has been established; only the pre-connection state should gate them.

## 3. Reopen-after-unanswered-Hellos fallback is disabled once framing is known — `fixed`

**File:** `desktop/crates/opad-device/src/lib.rs:301`
**Category:** correctness

The "reopen the port after `HELLOS_BEFORE_REOPEN` unanswered Hellos" fallback is guarded by `framing.is_none()`, so once a pad has answered once, a later re-Hello that goes unanswered retries every 400 ms forever with no reopen and no disconnect while `is_connected` reports true.

**Failure scenario:** Pad's frame parser desyncs mid-session (the very case the comment on `HELLOS_BEFORE_REOPEN` says a DTR drop fixes) → host drops the stale partial frame, clears `has_hello_ack`, re-sends Hello → pad never answers → worker loops indefinitely, all incoming frames dropped per finding 2, daemon and GUI show "connected" with frozen counters until the pad is physically replugged.

**Suggested fix:** Apply the reopen threshold regardless of whether framing is known.

## 4. `locate_pad` ignores `device_id` whenever the platform reports any serial — `open`

**File:** `desktop/crates/opad-device/src/lib.rs:1086`
**Category:** correctness

`locate_pad` falls back to `device_id` only when the platform reports no USB serial at all (`serial.or(device_id)`), so a reported serial that is not an `OSUPAD-` string suppresses the MAC even though the HelloAck `device_id` carries it.

**Failure scenario:** Windows reports a composite-device instance id such as `7&2ABC&0&0000` as the CDC port's serial → `.or(device_id)` never consults `device_id` → `strip_prefix("OSUPAD-")` fails → `mac: None`, and `usb_path` is None off-Linux → `select_bootloader_port` degrades to "the only new 303a:1001 port"; with a second ESP dev board plugged in, the flash goes Ambiguous or to the wrong chip.

**Suggested fix:** Apply `strip_prefix` to the serial and fall back to `device_id` when that yields None.

## 5. Windows `IpcListener::accept` drops a connected client if the replacement instance fails — `fixed` (untested on Windows)

**File:** `desktop/crates/opad-ipc/src/transport/windows.rs:105`
**Category:** correctness

`IpcListener::accept` takes the ready instance out of `next` before awaiting `connect()`; if creating the replacement instance fails the already-connected `server` is dropped with the error, and if `connect()` errs or the future is cancelled `next` stays `None` so the pipe name has no listening instance until the next `accept` call.

**Failure scenario:** Transient `CreateNamedPipe` failure (handle pressure, DACL conversion error) right after a GUI connects → `?` returns Err and drops `server` → the GUI's freshly opened pipe is closed before the handshake → "daemon offline" while the daemon is running.

**Suggested fix:** Return the connected `server` regardless of replacement outcome (log the failure, recreate lazily on the next accept), and recreate the pending instance in the error path.

## 6. Bootloader-port miss is reported as `PortBusy` while the pad sits in download mode — `fixed` (untested on Windows)

**File:** `desktop/crates/opad-device/src/flash.rs:148`
**Category:** correctness

When a boot trigger fires but the bootloader port is not matched within the 3 s window, the next trigger's `wait_until_openable(app_port)` fails (the app port is gone) and its `PortBusy` overwrites `last_err`, so `enter_bootloader` returns "close anything else using it" while the pad is sitting in download mode.

**Failure scenario:** First flash on Windows: the pad re-enumerates as 303a:1001 but driver installation takes > 3 s → `pick` sees nothing → loop moves to BaudTouch → `wait_until_openable` on the vanished COM port spends 2 s and yields `PortBusy(app_port)` → user is told to close a serial monitor; the pad is left in the ROM bootloader and the GUI/CLI never re-checks `bootloader_ports()` for it.

**Suggested fix:** Re-check for the pad's bootloader port before/after the wait and prefer `NoBootloader` (or a fresh `pick`) over the openability error.

## 7. Empty `XDG_RUNTIME_DIR` yields a CWD-relative socket path — `open`

**File:** `desktop/crates/opad-ipc/src/transport/unix.rs:14`
**Category:** correctness

`get_socket_path` treats an empty `XDG_RUNTIME_DIR` as set (`std::env::var` returns `Ok("")`), producing the relative path `opad/daemon.sock` that depends on each process's CWD.

**Failure scenario:** A systemd user unit or a shell with `XDG_RUNTIME_DIR=` exported empty → daemon binds `./opad/daemon.sock` in its CWD (and `create_listener` creates an `opad/` dir there), while the GUI launched from another directory resolves a different relative path → connect fails with "Daemon offline" although it is running.

**Suggested fix:** Filter the var with `.filter(|v| !v.is_empty())` before using it.

## 8. `ConfigPayload → DeviceConfig` mapping duplicated between HelloAck and ConfigAck arms — `open`

**File:** `desktop/crates/opad-device/src/lib.rs:783`
**Category:** reuse

The `proto::ConfigPayload → DeviceConfig` mapping (including the `== 0 → DEFAULT_KEYn_GPIO` fallbacks and the `tosu_endpoint` default) is copy-pasted between the HelloAck arm (731-749) and the ConfigAck arm (783-802).

**Failure scenario:** Maintenance cost: a new config field or a changed GPIO-default rule added in one arm and not the other makes the config reported at connect time disagree with the config reported after a ConfigAck, with no compiler help.

**Suggested fix:** Extract one `fn device_config_from(c: &proto::ConfigPayload) -> DeviceConfig` and call it from both arms.

## 9. `probe_port` re-implements the dual-framing Hello handshake — `open`

**File:** `desktop/crates/opad-device/src/lib.rs:955`
**Category:** reuse

`probe_port` re-implements the dual-framing Hello handshake (send Marked, then Legacy, decode until an OSUPAD HelloAck) that the worker loop also implements at 300-363 with different timing rules (`hello_framing` alternation vs "legacy at half the window") and its own buffer/decode loop.

**Failure scenario:** Two copies of the framing-negotiation policy drift: a change to how the host tries framings (e.g. adding a third framing or changing the order for old-app↔new-pad compatibility) has to be made in both places, and the probe and the worker can disagree about which pads are found.

**Suggested fix:** A single `fn handshake(port, window) -> Option<(HelloAck, Framing)>` used by both.

## 10. Unused deps and unreferenced export in `opad-ipc` — `fixed`

**File:** `desktop/crates/opad-ipc/Cargo.toml:17`
**Category:** simplification

`opad-ipc` declares `bytes` and `byteorder` dependencies that nothing in `src/` or `tests/` uses, and `lib.rs:22` exports `MAX_IPC_FRAME_SIZE` as an alias that no crate in the workspace references.

**Failure scenario:** Dead surface: two crates compiled and linked into every consumer for nothing, and an exported constant that duplicates `MAX_REQUEST_FRAME_SIZE` under a name suggesting a single frame cap when the request and response caps differ.

**Suggested fix:** Remove the two dependencies and the alias.
