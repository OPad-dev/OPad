# osu!pad — Windows Support, Installer and Device Pairing

**Companion to:** `osupad_technical_spec_v1.md`, `osupad_remaining_work_v1.md`, `docs/windows-portability.md`
**Audience:** the project owner, Antigravity and other coding agents
**Baseline:** `main` at `0a5c13e` (v1.0.0 tagged, Linux complete)
**Supersedes:** amendment **A3** ("Windows support is deferred") in `osupad_remaining_work_v1.md`

---

## 0. Scope and decisions

### Goal

Ship osu!pad on Windows 10/11 with a real installer, and make the pad and the app feel like **one product** without ever locking the user out of their own hardware.

### Decided (owner, 2026-09-16)

| Decision | Choice |
|---|---|
| Binding model | **Pairing / ownership claim.** The pad records which host install owns it. A different install must explicitly take over. Nothing cryptographic; this protects counter integrity, not exclusivity. |
| Windows transport | **Keep CDC-ACM.** Already cross-platform in the existing code; zero firmware change. |
| Installer | **Inno Setup `.exe`**, with a *strong* uninstall that leaves nothing behind. |
| Unbind path | **Documented full reflash only.** No GUI unpair button, no on-device factory reset. |

### Open decision

- **Version number.** `v1.0.0` is already tagged and published. This plan assumes Windows ships as **`v1.1.0`**; re-cutting a published tag is not recommended. Owner to confirm.

### The one invariant that overrides everything

Spec §3 and task P0-1 still hold on Windows. **The HID keyboard must work on any PC with no software, no install, and no pairing.** Every mechanism in this document lives on the CDC/protocol side. Nothing here may gate, delay, or add work to the key ISR, the keypad task, or the TinyUSB task on core 0. A pad plugged into a machine that has never seen the installer is a working 1000 Hz keyboard.

### What "vendor lock, but not really a lock" means here

**In scope — the product feel:**
- One installer sets up everything; no manual steps, no driver hunt, no Zadig.
- The daemon runs at login and notices the pad on hotplug; the tray reflects it immediately.
- The app is the obvious and only supported way to configure the pad.
- The pad knows which install owns it and refuses to *silently* hand its counters to a different one.

**Explicitly NOT in scope — real enforcement:**
- No cryptographic attestation, no signed handshake, no anti-tamper.
- The firmware stays flashable over USB by design. Anything baked into firmware plus app is extractable from either.
- Third-party software talking to the CDC interface is not prevented. It is simply not supported.

This is an **ownership model**, not a DRM scheme. Stating that plainly here so no later task tries to "harden" it into one.

---

## 1. What already works on Windows (verify, don't rewrite)

Confirmed by reading the tree at `0a5c13e`:

- **Firmware: zero changes.** The HID interface binds to `hidclass.sys` and CDC-ACM to inbox `usbser.sys` on Win10 1709+. No `.inf`, no WinUSB, no Zadig.
- **Device discovery: already portable.** `desktop/crates/osupad-device/src/lib.rs:577` filters `serialport::available_ports()` by VID/PID and returns `/dev/ttyACM0` on Linux and `COM3` on Windows from the same code path. `find_target_port` (`:568`) and `find_bootloader_port` (`:573`) both work unchanged.
- **Platform-agnostic crates:** `osupad-protocol`, `osupad-model`, `osupad-layout`, `osupad-tosu`, `osupad-storage`, `osupad-ui-preview`. SQLite/`rusqlite` WAL behaves the same on NTFS.
- **`iced` 0.13** renders on Windows via wgpu DX12/Vulkan.

**Latency note:** the transport choice is latency-irrelevant. CDC is a separate USB endpoint from the HID interface; protocol traffic never touches the keystroke path. The only option that would have risked the invariant was a vendor-HID protocol interface sharing the HID stack, and it is rejected for that reason.

---

## 2. W0: make the workspace build on Windows (do these first)

Nothing else can be tested until `cargo build --target x86_64-pc-windows-msvc` succeeds.

### W0-1. Abstract the IPC transport

**Problem.** `osupad-ipc` does not merely *use* Unix sockets internally — the concrete type is in the **public API**, so it leaks into every consumer and cannot be fixed with a `cfg` swap at the call sites.

**Where.**
- `desktop/crates/osupad-ipc/src/lib.rs:11` — `use tokio::net::{UnixListener, UnixStream}`
- `:206` `connect_and_handshake() -> (UnixStream, IpcResponse)`
- `:213` `connect_and_handshake_at(..) -> (UnixStream, IpcResponse)`
- `:245` `send_request(stream: &mut UnixStream, ..)`
- `:280` `read_request(stream: &mut UnixStream)`
- `:299` `send_response(stream: &mut UnixStream, ..)`
- `:317` `create_listener` — uses `std::os::unix::fs::{MetadataExt, PermissionsExt}`
- `:356` stale-socket probe via `std::os::unix::net::UnixStream::connect`
- Consumers: `desktop/daemon/src/main.rs:45`, `desktop/cli/src/main.rs:10,491`, `desktop/gui/src/ipc.rs`

**Required change.**
1. Introduce `IpcStream` and `IpcListener` type aliases in `osupad-ipc`, selected by `#[cfg]`:
   - unix → `tokio::net::{UnixStream, UnixListener}`
   - windows → `tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer}`
2. Change every public signature above to take `&mut IpcStream` / return `IpcStream`. The framing (`[len: u32 LE][json]`) and the `IpcRequest`/`IpcResponse` enums do **not** change — they are already transport-agnostic.
3. Named pipe: `\\.\pipe\osupad-ipc-{user_sid}`. Include the SID so two users on one machine get separate pipes, mirroring the current per-uid socket path.
4. Accept loop differs by platform and must be written per-platform, not shared: a `NamedPipeServer` is consumed on connect and a fresh instance must be created for the next client. Do not try to force this into the Unix accept-loop shape.

**Acceptance.**
- `osupad-ipc` builds for both targets.
- `crates/osupad-ipc/tests/ipc_test.rs` passes on both. The permissions assertion at `:335` becomes the Windows ACL check below, `#[cfg]`-gated.
- Daemon, GUI and CLI compile against the new signatures with no behaviour change on Linux.

---

### W0-2. IPC hardening on Windows (the P2-6 equivalent)

**Problem.** P2-6 hardened the Unix socket with `0700` permissions and a peer check (`lib.rs:317-362`). A named pipe created with default security is reachable by any process on the machine, including across sessions. The Linux guarantee must not silently weaken.

**Required change.**
- Create the pipe with a security descriptor granting access only to the creating user's SID (and `SYSTEM`).
- Do **not** set `first_pipe_instance(false)`. Use `first_pipe_instance(true)` on the first server instance so a second daemon fails loudly rather than squatting the pipe — this is the Windows half of the single-daemon guarantee.
- Map `ERROR_PIPE_BUSY` / `ERROR_ACCESS_DENIED` to the same "another daemon is running" error the Unix path returns.

**Acceptance.** A second daemon instance refuses to start with a clear message. A process running as a different user cannot open the pipe.

---

### W0-3. Single-instance guard for the GUI

**Problem.** `desktop/gui/src/single_instance.rs:5-54` is a Unix socket lock (`PermissionsExt`, `UnixListener`, `UnixStream`) that also carries the "raise the existing window" handoff.

**Required change.** On Windows use a named mutex (`CreateMutexW` with a `Local\osupad-gui` name) for the lock, plus the existing IPC pipe (or a `WM_COPYDATA` broadcast) for the raise-window handoff. Keep the `OnceLock` shape so the Linux path is untouched.

**Acceptance.** Launching the GUI twice on Windows focuses the existing window instead of opening a second one, matching Linux.

---

### W0-4. Storage and config paths

**Problem.** `desktop/daemon/src/main.rs:437-446` resolves the DB from `XDG_DATA_HOME`, then `HOME`, then falls back to a bare relative `osupad.db` — on Windows that fallback lands in the working directory, which for a login-launched daemon is unpredictable (often `C:\Windows\System32`).

**Required change.** Replace with `dirs::data_dir()`:
- Linux: `~/.local/share/osupad/osupad.db` (unchanged — keep honouring `XDG_DATA_HOME` first so existing installs do not migrate)
- Windows: `%APPDATA%\osupad\osupad.db`

Remove the bare-relative fallback; if no directory can be resolved, fail loudly. Audit every other path construction in the daemon and GUI for the same pattern (logs, layout exports, JSON backup default directory).

**Acceptance.** Fresh Windows install creates exactly `%APPDATA%\osupad\`. Existing Linux installs keep using their current DB with no migration step. **Record every path written, for W2-3.**

---

### W0-5. Cross-platform tray

**Problem.** `desktop/gui/Cargo.toml` depends on `ksni = "0.3"`, a Linux/D-Bus StatusNotifierItem implementation. It does not build on Windows. This is a hard compile failure, not a runtime degradation.

**Required change.**
- Make `ksni` a `[target.'cfg(target_os = "linux")'.dependencies]` entry.
- Add `tray-icon` under `[target.'cfg(windows)'.dependencies]`.
- Extract the tray model (menu items, status lines, click actions) from `desktop/gui/src/tray.rs` into a backend-agnostic struct, with two thin backends behind it.
- Preserve the A5 decision and the P2-8 acceptance checks: the tray is GUI-hosted, and tray failure must not take down the GUI, let alone the daemon.
- `tray-icon` on Windows needs a running win32 event loop. Verify it cooperates with `iced`'s loop; if it does not, run it on a dedicated thread.

**Acceptance.** Tray shows the same §19 status lines on both platforms. Killing the tray leaves the GUI and daemon running.

---

### W0-6. Resolve the remaining `cfg(target_os = "linux")` sites

**Where.** `desktop/gui/src/main.rs:5, 278, 706, 743, 1008, 1734`

**Required change.** Each currently compiles to nothing on Windows. Audit all six: give each either a Windows counterpart or an explicit, commented "Linux-only, not applicable" `#[cfg]`. Do not leave silent no-ops — a missing Windows branch here is a feature that quietly does nothing.

**Acceptance.** No silently-empty Windows branch remains. `cargo build` for both targets is warning-clean under `-D warnings`.

---

## 3. W1: Windows platform integration

### W1-1. `platform_windows.rs`

**Problem.** `desktop/gui/src/platform_linux.rs` is entirely `systemctl --user` (service install, enable, start, status) — see R7, which fixed this for Linux. Windows has no equivalent.

**Required change.** New `desktop/gui/src/platform_windows.rs` exposing the same functions the GUI already calls:
- **Autostart:** write `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\osupad-daemon`. Prefer this over Task Scheduler — it is trivially inspectable by the user and trivially removable by the uninstaller, which matters for W2-3.
- **Start/stop daemon:** spawn/terminate the process directly. Do **not** build a Windows Service; the daemon is per-user, needs the user's session for tray and IPC, and a service adds an uninstall footprint for no benefit.
- **Status:** detect a running daemon by attempting the IPC pipe connection, not by process enumeration.

Note the R7 lesson: the Linux version was broken precisely by writing a *second*, divergent unit file instead of using the packaged one. Windows has one autostart mechanism — use exactly that one, and let the installer and the GUI agree on the value byte-for-byte.

**Acceptance.** "Install service" / "Start daemon" / status indicator behave identically to Linux from the user's point of view. The registry value matches what the installer writes.

---

### W1-2. Hotplug detection

**Problem.** The Linux daemon relies on udev (`packaging/linux/udev/99-osupad.rules`) plus reconnect polling. Windows has no udev.

**Required change.** Register for `WM_DEVICECHANGE` / `RegisterDeviceNotification` on the daemon side, or — if that forces an unwanted message pump — fall back to the existing reconnect poll, which already works. Poll interval must respect P1-3: **no storage writes during PLAYING or COOLDOWN**, and hotplug polling must not become a write trigger.

**Acceptance.** Plugging the pad in with the daemon already running connects within 2 s. Unplugging mid-session does not lose counters or wedge the state machine (P1-8).

---

### W1-3. Flashing on Windows

**Problem.** `desktop/cli/src/esp_rom.rs` and `scripts/flash_board.sh` handle recovery flashing. The `.sh` script is Linux-only, and the ESP32-S3 USB-download reboot sequence is known to be fiddly (see the recorded USJ re-attach / watchdog behaviour).

**Required change.** Verify `esp_rom.rs` bootloader entry works on Windows (`find_bootloader_port` is already portable). Provide `scripts/flash_board.ps1` or fold the logic into the CLI so no shell script is needed. **The daemon must release the COM port before flashing** — Windows serial handles are exclusive, unlike Linux, so a held port fails the flash outright rather than merely being impolite.

**Acceptance.** Recovery flash works on Windows with the GUI running, without manually stopping the daemon first.

---

## 4. W2: Installer and uninstaller

### W2-1. Inno Setup script

**Where.** New `packaging/windows/osupad.iss`, plus a build script alongside `scripts/release`.

**Installs:**
- `osupad-gui.exe`, `osupad-daemon.exe`, `osupad-cli.exe` → `%LOCALAPPDATA%\Programs\osupad\`

  Per-user install, not `Program Files`. This avoids requiring admin rights, matches the per-user daemon model, and keeps the uninstall entirely inside the user's own profile.
- Start Menu shortcut for the GUI. **No desktop shortcut by default** — offer it as an unchecked checkbox.
- The `HKCU\...\Run` autostart value from W1-1, as an opt-in checkbox on the final page ("Start osu!pad when I log in", default checked).
- Uninstall entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\osupad`.

**Does not install:** any driver, any `.inf`, any service, any redistributable. If a VC++ runtime turns out to be needed, prefer static CRT linking in the MSVC build over shipping a redist.

**Upgrade behaviour:** `AppId` GUID fixed across versions so upgrades replace rather than stack. Must stop a running daemon and GUI before replacing binaries (`CloseApplications`), and must **not** touch `%APPDATA%\osupad\` — counters survive upgrades unconditionally.

---

### W2-2. Code signing

Unsigned installers trigger SmartScreen ("Windows protected your PC"), which for a keyboard-adjacent download is a real adoption problem — users are right to be suspicious of unsigned software that installs a keyboard tool at login.

**Options, owner's call:**
- Sign with an OV/EV certificate (~$200-400/yr; EV clears SmartScreen immediately, OV builds reputation over time).
- Ship unsigned and document the SmartScreen bypass in the README.

Not a blocker for a first release; it is a blocker for a comfortable one. Flagging it now so it is a decision, not a surprise.

---

### W2-3. Strong uninstall — no garbage left

This is an explicit owner requirement. The uninstaller must be *provably* complete, which means an inventory, not an intention.

**Must remove unconditionally:**
- `%LOCALAPPDATA%\Programs\osupad\` — all binaries, including any file the app wrote into its own install dir
- `HKCU\...\Run\osupad-daemon`
- `HKCU\...\Uninstall\osupad`
- Start Menu and (if created) desktop shortcuts
- Any `%TEMP%` scratch files the app created

**Must prompt (a clear, explicit dialog, default = keep):**
- `%APPDATA%\osupad\` — the SQLite DB with **lifetime counters**, config, layouts, logs

  > "Also delete your osu!pad settings and lifetime key counters? This cannot be undone."

  Offer a "Export backup first" button that invokes the existing JSON export (P1-5/P2-3) before deleting. Counters are the emotionally valuable data in this product; deleting them silently would be the single worst thing the uninstaller could do.

**Needs no cleanup (document why, so nobody adds bogus cleanup code later):**
- The named pipe — kernel object, disappears with the process
- The COM port assignment — owned by Windows' `usbser.sys` enumeration, not by us
- The pad's own NVS — deliberately untouched; see W3-4

**Acceptance.** Install → use → uninstall (choosing "delete everything") leaves **zero** osu!pad files and **zero** registry values. Verify with a filesystem+registry diff across the whole cycle, and record the procedure in `docs/testing-checklist.md` as a new section. Run the same check for the "keep my data" path and confirm that `%APPDATA%\osupad\` is the *only* thing left.

---

## 5. W3: Device pairing / ownership claim

Cross-platform — this ships on Linux too, not just Windows.

### W3-1. Install identity

**Required change.** On first run the daemon generates a random `install_id` (UUIDv4) and stores it in the SQLite DB. It is per-install, not per-machine: reinstalling after the "delete everything" uninstall produces a new identity, and that is correct and intended.

---

### W3-2. Firmware: owner field in NVS

**Required change.**
- Store `owner_id` (16 bytes, zero = unclaimed) in the existing NVS config namespace (`firmware/main/config/device_config.c`).
- Add `owner_id` to `HelloAck` in `protocol/osupad.proto` (next free field number — do not renumber existing fields; `device_id` is 4 and `counter_generation` is 5).
- Add a `ClaimOwnership { owner_id }` command that writes the field.
- **Respect P1-3:** the claim write is an NVS write, so it is permitted **only** in IDLE. Never during PLAYING or COOLDOWN. It happens at connect time, which is already an IDLE-only moment.
- An unclaimed pad (all-zero owner) is claimed silently on first connect. No prompt for the common case.

**Acceptance.** A pad with no owner pairs invisibly. Firmware host tests cover claim, re-claim, and the IDLE-only write guard.

---

### W3-3. Daemon and GUI: the takeover prompt

**Problem.** `desktop/daemon/src/runtime.rs:35,226,260,533` already has `pending_replacement` for "a different pad appeared". Ownership is the mirror image: "this pad belongs to a different install".

**Required change.** Extend the existing mechanism rather than adding a parallel one. On connect, compare `HelloAck.owner_id` with the local `install_id`:

| Case | Behaviour |
|---|---|
| Match | Normal operation. Silent. |
| Owner is all-zero | Claim silently, proceed. |
| Owner differs | **Prompt**, and block counter sync until the user answers. |

The prompt must be friction-light, because per the owner's decision the *only* alternative is reflashing:

> "This osu!pad is paired with another installation. Its lifetime counters are 1,234,567 / 1,234,567.
> **[Take over and keep the pad's counters]** · [Take over and use this PC's counters] · [Leave it alone]"

**Critical ordering constraint (R3 applies directly here).** R3 was exactly this class of bug: `HelloAck` emitted `Connected` before `Counters`, so the connect handler saw the *previous* pad's counters, the replacement prompt never fired, and old counters were saved under the new `device_id`. The owner check must happen in the **same** `HelloAck` arm, before any counter reconciliation, and it must be covered by a daemon test that would have caught R3.

**Acceptance.**
- Moving a pad between two PCs prompts exactly once per takeover, and never again on that PC.
- "Leave it alone" keeps the keyboard fully working, with telemetry and config disabled — the pad is still a keyboard.
- No counter is ever written under the wrong `device_id` or the wrong owner. Daemon test coverage for all four cases in the table above.

---

### W3-4. Documented reflash (the nuke path)

Per the owner's decision this is the **only** unbind mechanism: no GUI unpair button, no on-device factory reset.

**Required change.** A `docs/recovery.md` section, linked from the README and from the takeover prompt itself:
- `espflash erase-flash` followed by a normal firmware flash returns the pad to fully unclaimed stock.
- State plainly that this **also erases the ESP-side lifetime counters**, and point at JSON export first.
- Cover both Linux and Windows, including the USB-download-mode reboot quirk and releasing the COM port first (W1-3).

**Acceptance.** A user following the doc on a clean machine gets an unclaimed pad. The takeover prompt links to this doc, so nobody who hits the prompt is ever left without an exit.

---

## 6. W4: Verification and release

### W4-1. CI

Add `x86_64-pc-windows-msvc` to the CI matrix: `cargo build`, `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`. **`cargo check` alone is not sufficient** — the `ksni`/`tray-icon` split and the IPC transport aliases fail at link time, not check time.

### W4-2. Windows hardware checklist

Extend `docs/testing-checklist.md` with a Windows column. Re-run at minimum: HW-01..HW-05, COM-01, COM-02, STR-01, STR-02. **HW-05 (HID-first on display wake) and STR-02 (zero storage writes) are the invariant-critical ones** and must be re-verified on Windows rather than assumed from the Linux run.

Add a new section for W2-3's install/uninstall filesystem+registry diff.

### W4-3. Latency on Windows

Re-run the `docs/latency-testing.md` stages on Windows and add rows to the results table. Note that the Linux v1.0 table is **still empty** (P3-1 is the last open v1 item) — that should be filled first, so there is a baseline to compare Windows against. `scripts/bench_latency.py` uses evdev and is Linux-only; the Windows host-side equivalent needs a raw-input or ETW-based approach, or the firmware-side percentiles alone for stages A-C.

### W4-4. Docs

- Rewrite `docs/windows-portability.md` from aspirational architecture notes into the actual as-built description.
- README: Windows install instructions, SmartScreen note if unsigned, and the reflash/unbind section.
- `docs/architecture.md`: the IPC transport abstraction and the pairing model.
- `docs/protocol.md`: `owner_id` in `HelloAck`, the `ClaimOwnership` command.
- Record the pairing model as an amendment appendix in `osupad_technical_spec_v1.md`, and mark **A3 superseded** in `osupad_remaining_work_v1.md`.

---

## 7. Order of work

```
W0-1 ─┬─ W0-2                     (IPC: everything blocks on this)
      └─ W0-3
W0-4, W0-5, W0-6                  (parallel, independent)
   ↓
W1-1, W1-2, W1-3                  (platform integration)
   ↓
W2-1 ─ W2-3                       (installer; W2-2 signing is a side decision)
   ↓
W3-1 → W3-2 → W3-3 → W3-4         (pairing; cross-platform, ships on Linux too)
   ↓
W4-1 … W4-4
```

W3 (pairing) is independent of W0-W2 and could be done first on Linux if Windows stalls. W0-1 blocks every other Windows task and should be the first commit.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| `tray-icon` fights `iced`'s event loop on Windows | Dedicated thread for the tray; A5/P2-8 already require tray failure to be non-fatal |
| `usbser.sys` COM assignment is unstable across replugs | Discovery is by VID/PID, not by port name, so this is already handled — but verify with several pads and across reboots |
| Windows exclusive serial handles break recovery flashing | W1-3: daemon must release the port; test explicitly with the GUI running |
| SmartScreen suppresses adoption | W2-2 signing decision |
| Pairing reintroduces an R3-class ordering bug | W3-3 requires the owner check in the `HelloAck` arm with dedicated daemon tests |
| Windows storage writes violate P1-3 during play | W4-2 re-runs STR-02 on Windows rather than assuming it |

---

## 9. Out of scope

- macOS.
- MSIX packaging and Store-style "plug in the pad → Windows offers the app". That needs a Store-signed MSIX; the Inno decision rules it out. The daemon-at-login plus hotplug detection (W1-2) delivers nearly the same feel.
- Any cryptographic enforcement of the pairing model (§0).
- Windows Service hosting for the daemon (W1-1).
- v2 rapid trigger — see `osupad_v2_rapid_trigger_plan.md`.
