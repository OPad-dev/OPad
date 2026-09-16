# osu!pad — Packaging, Distribution and Device Pairing

**Companion to:** `osupad_technical_spec_v1.md`, `osupad_remaining_work_v1.md`, `docs/windows-portability.md`
**Audience:** the project owner, Antigravity and other coding agents
**Baseline:** `main` at `0a5c13e` (v1.0.0 tagged, Linux complete)
**Supersedes:** amendment **A3** ("Windows support is deferred") in `osupad_remaining_work_v1.md`

---

## 0. Scope and decisions

### Goal

Ship osu!pad on Windows 10/11 and on the major Linux distros as real, installable packages, and make the pad and the app feel like **one product** without ever locking the user out of their own hardware.

### Decided (owner, 2026-09-16)

| Decision | Choice |
|---|---|
| Binding model | **Pairing / ownership claim.** The pad records which host install owns it. A different install must explicitly take over. Nothing cryptographic; this protects counter integrity, not exclusivity. |
| Windows transport | **Keep CDC-ACM.** Already cross-platform in the existing code; zero firmware change. |
| Installer | **Inno Setup `.exe`**, with a *strong* uninstall that leaves nothing behind. |
| Unbind path | **Documented full reflash only.** No GUI unpair button, no on-device factory reset. |
| Version | Stays **v1.0.0**. The release is not finished until these details are done. |
| Code signing | **Ship unsigned initially.** Apply to SignPath Foundation (free for OSS) once the repo is public. |
| Linux distribution | Native **`.deb` / `.rpm` / AUR** packages built in CI. Not targeting official distro repositories. |
| tosu | **Never bundled.** Optional runtime dependency, fetched on request by the app. |

### Version

Everything in this document is part of **v1.0.0**, not a follow-up release. The repository has **no git remote and nothing has ever been pushed** — the existing `v1.0.0` tag is local-only, so it carries no compatibility promise to anyone and costs nothing to move.

**Action:** delete the local tag (`git tag -d v1.0.0`) and re-cut it when W4 passes. Until then the version in `Cargo.toml` should read `1.0.0-rc`.

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

### W2-2. Code signing — ship unsigned, then apply to SignPath

**Decision:** ship unsigned for the first release, and pursue free signing afterwards.

**Free signing is genuinely available for this project.** [SignPath Foundation](https://signpath.org/) provides free OV code signing to open-source projects; osu!pad is MIT, so it qualifies. The private key lives on SignPath's HSM and signing runs as a step in the release pipeline, which also certifies that the signed binary was built from the public source tree.

**Prerequisites, in order — note that the first one does not exist yet:**
1. **A public repository.** `git remote -v` is currently empty; nothing has ever been pushed. SignPath requires a publicly available codebase, so publishing the repo is a hard prerequisite, not a nice-to-have.
2. **CI-based releases.** SignPath signs from the pipeline, not from a developer machine. This pairs with W4-1.
3. Application and review by the Foundation.

**What signing does and does not fix.** An OV certificate does **not** instantly clear SmartScreen — reputation accrues as downloads accumulate. Only EV certificates get immediate SmartScreen trust, and those are not free. So expect the warning to fade rather than vanish.

**Not worth doing:** a self-signed certificate. It is free and it does nothing for SmartScreen; the user must manually install the certificate into their trust store, which is worse UX than shipping unsigned.

**Paid fallbacks** if SignPath does not work out: Certum's open-source code signing certificate (roughly €25-30/yr) or Microsoft Trusted Signing (about $10/month, subject to identity-validation requirements). Note that since June 2023 all code signing keys must live on hardware or an HSM, which is why cheap file-based certificates no longer exist.

**Interim requirement.** While unsigned, the README and download page must explain the SmartScreen prompt honestly and tell the user how to proceed ("More info" → "Run anyway"), and publish **SHA-256 checksums** for every artifact so a careful user can verify what they downloaded.

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

## 6. L: Linux distribution packages

The current `packaging/linux/install.sh` is a per-user script (`~/.local/bin`). That stays as the from-source path, but it is not a distributable package.

### L-0. Scope: your own packages, not the official repositories

**Be clear about the target.** Getting into Debian's or Fedora's official archives is not realistic here and should not be attempted: both require every Rust dependency to be packaged separately as a distro package, which is impractical for a workspace this size, and both forbid vendored pre-built binaries.

**What is realistic, and what this section means by "support for major distros":** build `.deb`, `.rpm` and an AUR `PKGBUILD` in CI and publish them as release artifacts, so users install with `apt install ./osupad.deb`, `dnf install ./osupad.rpm` or `yay -S osupad-bin`. That covers Debian/Ubuntu/Mint/Pop, Fedora/RHEL/openSUSE and Arch/Manjaro respectively.

### L-1. System-wide vs per-user layout

**Problem.** The current assets are per-user and **hardcode a per-user path**: `packaging/linux/systemd-user/osupad-daemon.service` has `ExecStart=%h/.local/bin/osupad-daemon`. A distro package installs binaries to `/usr/bin`, so that unit is wrong inside a package and the daemon will fail to start.

**Required change.**
- Binaries → `/usr/bin/{osupad-daemon,osupad-gui,osupadctl}`
- systemd **user** unit → `/usr/lib/systemd/user/osupad-daemon.service`, with `ExecStart=/usr/bin/osupad-daemon`
- `.desktop` files → `/usr/share/applications/`
- udev rules → `/usr/lib/udev/rules.d/99-osupad.rules` (**not** `/etc/udev/rules.d/`, which is reserved for local administrator overrides)
- Icons → `/usr/share/icons/hicolor/...`

Keep the unit a **user** unit, not a system one: the daemon is per-user, needs the session bus for the tray, and owns a per-user IPC socket. Template the `ExecStart` path so the same source file produces both the `~/.local/bin` and `/usr/bin` variants rather than maintaining two divergent copies — this is exactly the R7 failure mode (a second, divergent unit file) and it must not be repeated.

### L-2. Fix the udev rule before shipping it

**Problem.** `packaging/linux/udev/99-osupad.rules` is Arch-centric and looser than it needs to be:

```
SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", MODE="0666", GROUP="uucp", TAG+="uaccess"
```

- `GROUP="uucp"` is the Arch convention. Debian, Ubuntu and Fedora use `dialout`. A group that does not exist on the target distro makes the rule silently ineffective.
- `MODE="0666"` grants **every user and every process on the machine** read/write access to the pad's serial interface. Combined with `TAG+="uaccess"` it is also redundant: `uaccess` already grants the physically-logged-in user access via systemd-logind, which is the modern, correct mechanism and is distro-independent.

**Required change.** Drop `MODE` and `GROUP`, keep `uaccess`:

```
SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="303a", TAG+="uaccess"
```

Narrow `ATTRS{idVendor}=="303a"` to also match the specific PIDs the code already knows (`OSUPAD_APP_PID` and `ESP_ROM_BOOTLOADER_PID` in `desktop/crates/osupad-device/src/lib.rs`) so the rule does not claim every Espressif device the user owns.

**Acceptance.** A normal desktop user can open the pad on Debian, Fedora and Arch with no group membership change and no logout, and no other user on the machine can. Verify on all three.

### L-3. Build the packages

| Target | Tool | Notes |
|---|---|---|
| `.deb` | `cargo-deb` | Per-crate metadata in `Cargo.toml`; `maintainer-scripts` for `udevadm control --reload` and `systemctl --user daemon-reload` |
| `.rpm` | `cargo-generate-rpm` | `%post`/`%postun` scriptlets for the same |
| Arch | hand-written `PKGBUILD` | Publish as `osupad-bin` (prebuilt) and optionally `osupad-git` |

**Runtime dependencies differ per distro and must be declared explicitly.** `iced`/wgpu needs Vulkan plus X11/Wayland client libraries, and `ksni` needs D-Bus. Get the per-distro package names right (`libvulkan1` vs `vulkan-loader`, `libwayland-client0` vs `wayland`, and so on) — a missing dependency here surfaces as a GUI that fails to start with an opaque wgpu error.

**Acceptance.** Install, launch, use and remove cleanly in a fresh container or VM for **Debian stable, Ubuntu LTS, Fedora and Arch**. Removal leaves no files behind, matching the W2-3 standard applied to Windows. Package installation must **not** enable the user service automatically without consent — `systemctl --user enable` is the user's decision, prompted by the GUI as it is today.

### L-4. AppImage (optional)

An AppImage covers every other distro with one artifact and is cheap to add once the binaries build. Flatpak is **not** recommended: the sandbox complicates raw serial access, the per-user IPC socket, and spawning tosu as a subprocess, for little benefit to this particular application.

---

## 7. T: tosu integration and redistribution

### T-1. The licensing position

**tosu is LGPL-3.0** (Mikhail Babynichev). osu!pad is MIT. These interact cleanly here, for a specific reason worth writing down so it is not re-litigated later:

**osu!pad does not link tosu in any way.** `desktop/crates/osupad-tosu/src/lib.rs` talks to it over a WebSocket at `ws://127.0.0.1:24050/websocket/v2` (`DEFAULT_TOSU_ENDPOINT`), and `spawn_tosu_supervisor` (`:275`) launches it as a **separate process**. Separate programs communicating over a socket are not a derivative work, and executing a program is not linking. **No copyleft obligation reaches osu!pad's own MIT-licensed code.** This is true whether or not tosu is bundled.

**Redistribution is the only thing that creates obligations.** If an installer or package *ships the tosu binary*, that is conveying an LGPL-3.0 work, which requires shipping the license text and copyright notice, and providing the corresponding source (or a valid written offer / access from the same place the binary is offered). All of this is satisfiable — it is permitted, not forbidden — but it is an ongoing maintenance burden: every tosu version bump means re-checking the source offer.

### T-2. Decision: never bundle tosu

Not because of licensing, but because bundling is worse on every axis that matters:

- It puts you on the hook for redistributing someone else's project and keeping its source offer current.
- It pins a tosu version that will go stale, while tosu tracks osu! client changes and needs to stay current to keep working.
- Distro packaging rejects vendored third-party binaries outright, so `.deb`/`.rpm` could not carry it anyway.
- The pad is **fully useful without it** — it is a 1000 Hz keyboard, and only the gameplay telemetry view depends on tosu.

**The code is already designed for this.** `find_tosu_binary` (`:256`) resolves `$OSUPAD_TOSU_PATH` → `~/.local/opt/tosu/tosu` → `tosu` on `$PATH`, warns once if absent, and retries. The supervisor also skips launching when something already listens on port 24050, so a hand-started tosu is respected. Nothing about that needs to change.

### T-3. Required change: an "Install tosu" helper in the GUI

Make the existing design explicit to the user instead of leaving it to an environment variable.

**Do:**
- GUI Device (or Settings) page shows tosu status: *not installed* / *installed, not running* / *connected*.
- When not installed, offer **"Download tosu"**: fetch the current release from tosu's official GitHub releases, verify the checksum, and install to `~/.local/opt/tosu/tosu` — **the path `find_tosu_binary` already looks for**. On Windows, the equivalent per-user location.
- The download must be clearly attributed: name the upstream project, show its LGPL-3.0 license, and link to its repository. The user is obtaining tosu from its authors; osu!pad is only automating the fetch.
- Always allow pointing at an existing install instead (`$OSUPAD_TOSU_PATH`, or a file picker).
- Never download anything without the user asking. No silent fetch on first run.

**Do not:** include the tosu binary in `osupad.iss`, the `.deb`, the `.rpm` or the `PKGBUILD`. For the AUR package, an `optdepends=('tosu')` entry is the correct expression of the relationship.

**Acceptance.**
- Fresh install with no tosu: pad works as a keyboard, display shows the idle clock, GUI states plainly that gameplay telemetry needs tosu and offers to fetch it.
- After the helper runs, the daemon connects with no restart and no manual configuration.
- `COM-05` in `docs/testing-checklist.md` (tosu killed mid-play) still passes: PLAYING → COOLDOWN → IDLE with no stuck UI.

### T-4. Attribution

Add a **Third-party software** section to the README and an About entry in the GUI naming tosu, its author, its LGPL-3.0 license and its repository — regardless of the fact that it is never bundled. Correct attribution is cheap and this project depends on their work for its headline feature.

---

## 8. Order of work

Three tracks that only converge at W4. They can be worked in parallel.

```
Windows        W0-1 ─┬─ W0-2          (IPC abstraction: all Windows work blocks on this)
                     └─ W0-3
               W0-4, W0-5, W0-6       (parallel, independent)
                  ↓
               W1-1, W1-2, W1-3
                  ↓
               W2-1 ─ W2-3            (installer + strong uninstall)

Linux          L-2 ─ L-1 ─ L-3        (udev fix first: it is a correctness bug today)
                            └─ L-4    (optional)

Cross-platform T-3                    (tosu helper; independent of everything)
               W3-1 → W3-2 → W3-3 → W3-4   (pairing; ships on Linux too)

                  ↓ all tracks
               W4-1 … W4-4 → re-cut v1.0.0
```

**Start here.** Two items are worth doing before anything else because they are live defects rather than new features:

1. **L-2** — the udev rule names a group that does not exist on Debian or Fedora, and grants `0666` to every process on the machine. That is wrong on the platform you already shipped.
2. **P3-1** — the latency table in `docs/latency-testing.md` is still empty. It is the last open v1 item and it is the baseline every other platform gets compared against.

Then **W0-1**, which gates every remaining Windows task.

**Deferred until needed:** W2-2 (signing) cannot start until the repository is public, so it trails the rest.

---

## 9. Risks

| Risk | Mitigation |
|---|---|
| `tray-icon` fights `iced`'s event loop on Windows | Dedicated thread for the tray; A5/P2-8 already require tray failure to be non-fatal |
| `usbser.sys` COM assignment is unstable across replugs | Discovery is by VID/PID, not by port name, so this is already handled — but verify with several pads and across reboots |
| Windows exclusive serial handles break recovery flashing | W1-3: daemon must release the port; test explicitly with the GUI running |
| SmartScreen suppresses adoption | W2-2 signing decision |
| Pairing reintroduces an R3-class ordering bug | W3-3 requires the owner check in the `HelloAck` arm with dedicated daemon tests |
| Windows storage writes violate P1-3 during play | W4-2 re-runs STR-02 on Windows rather than assuming it |
| Distro package misses a runtime library; GUI dies with an opaque wgpu error | L-3 requires a clean-container install test on all four distros |
| Packaged systemd unit keeps the `%h/.local/bin` path and the daemon never starts | L-1 templates one source unit into both variants (the R7 lesson) |
| A tosu update breaks telemetry and users blame osu!pad | tosu is never pinned or bundled (T-2); the GUI reports tosu status explicitly (T-3) |
| SmartScreen suppresses Windows adoption while unsigned | W2-2: publish SHA-256 checksums and document the prompt honestly; pursue SignPath once public |

---

## 10. Out of scope

- macOS.
- MSIX packaging and Store-style "plug in the pad → Windows offers the app". That needs a Store-signed MSIX; the Inno decision rules it out. The daemon-at-login plus hotplug detection (W1-2) delivers nearly the same feel.
- Any cryptographic enforcement of the pairing model (§0).
- Windows Service hosting for the daemon (W1-1).
- Inclusion in official Debian/Fedora/Arch repositories (L-0). Own-built packages only.
- Flatpak (L-4), and bundling tosu in any artifact (T-2).
- EV code signing (W2-2).
- v2 rapid trigger — see `osupad_v2_rapid_trigger_plan.md`.
