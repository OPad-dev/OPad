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
| Linux distribution | Native **`.deb` / `.rpm`** published as **GitHub release artifacts only**; **AUR** for Arch. No official distro repositories, ever. |
| tosu | **Bundled everywhere.** Prebuilt from upstream releases on Windows/deb/rpm; **built from source** on AUR. |
| Build system | A top-level **`Makefile`** with `PREFIX`/`DESTDIR`, used by the AUR package and every from-source install. |
| Updates | Auto-update for **tosu** and the **app** on every platform (Linux needs one polkit prompt); **explicitly consented** update for the **firmware**. All new work. |
| Firmware layout | Switch to a **two-slot OTA partition table in v1.0**, even though OTA code lands later. |

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

## 6. B: Build from source — the Makefile

**Owner decision (2026-09-16):** a top-level `Makefile` builds osu!pad and, on request, tosu, for the machine it runs on. The AUR package is a thin wrapper around it, and so is every other from-source install.

This supersedes `packaging/linux/install.sh`, which is a per-user copy script with the install layout hardcoded.

### B-1. Requirements that make it usable by packagers

The Makefile is only useful to packaging if it behaves like a normal autotools-style build. Two variables are non-negotiable:

- **`PREFIX`** (default `/usr/local`) — where things are installed
- **`DESTDIR`** (default empty) — a staging root prepended to every install path

`DESTDIR` is what lets a package build install into a fake root and capture the result. Without it the `PKGBUILD`, `cargo-deb` and `cargo-generate-rpm` all have to reimplement the layout by hand, which is how the layout drifts.

```make
make                                    # build desktop binaries
make DESTDIR=/tmp/pkg PREFIX=/usr install   # staged system install (packagers)
make install-user                       # ~/.local layout (replaces install.sh)
```

### B-2. Targets

| Target | Does |
|---|---|
| `all` | Release-build `osupad-daemon`, `osupad-gui`, `osupadctl` |
| `tosu` | Build tosu from source into `build/tosu/` (B-3) |
| `firmware` | `idf.py build`; skipped with a clear message if ESP-IDF is absent |
| `install` | Install into `$(DESTDIR)$(PREFIX)` using the L-1 system layout |
| `install-user` | Install into `~/.local`, the current `install.sh` behaviour |
| `uninstall` | Remove everything `install` placed |
| `check` | `cargo fmt --check`, `clippy -D warnings`, Rust tests, firmware host tests |
| `clean` | Drop build artifacts |

**`install` must not do anything that requires root beyond writing to `$(DESTDIR)`.** No `udevadm control --reload`, no `systemctl --user enable`, no group changes. Those belong in package post-install scriptlets and in the GUI, never in `make install` — a staged package build runs unprivileged and must not attempt them.

### B-3. Solves L-1's templating problem

L-1 needs one systemd unit source producing both `%h/.local/bin/osupad-daemon` and `/usr/bin/osupad-daemon`. The Makefile is where that happens: keep a single `osupad-daemon.service.in` with an `@BINDIR@` placeholder and substitute `$(PREFIX)/bin` (or `~/.local/bin` for `install-user`) at install time. One source file, no divergent copies — the R7 failure mode closed structurally rather than by discipline.

Apply the same treatment to the `.desktop` files and the udev rule.

### B-4. Building tosu from source

Verified against tosu's manifests:

- **Toolchain:** `pnpm` (`packageManager: pnpm@10.10.0`), **Node.js `>=24.14.0 <25.0.0`**, TypeScript, `rolldown` for bundling.
- **Release binaries are produced with `@yao-pkg/pkg`** (`compile:linux` → `pkg --output dist/tosu --compress brotli dist/index.js`), a maintained fork of `vercel/pkg`.
- **`tsprocess`**, the workspace package that does the actual process memory reading, is a **native addon** — building it needs a C/C++ toolchain and Python (`base-devel` on Arch).

**Do not use `pkg` for the source build.** `@yao-pkg/pkg` works by downloading a prebuilt Node base binary from GitHub at build time, which means network access during `build()` — against Arch packaging guidelines, which require every source to be declared in `source=()`, and fragile in any sandboxed or offline build.

**It is also unnecessary.** `pkg` exists to produce a self-contained binary for users who have no Node. Arch has `nodejs` packaged. So:

1. `pnpm install --frozen-lockfile`
2. `pnpm run genver && pnpm run ts:compile` → `dist/index.js` (plus the built `tsprocess` addon)
3. Install `dist/` to `$(DESTDIR)$(PREFIX)/lib/osupad/tosu/`
4. Install a small wrapper script named **`tosu`** next to it that `exec`s `node index.js "$@"`

`find_tosu_binary` (`desktop/crates/osupad-tosu/src/lib.rs:256`) only checks that the path is a file and then spawns it as a process, so a wrapper script works with **no code change**.

**Node version risk, flag it in the PKGBUILD.** tosu pins `engines.node` to the 24.x series. Arch's `nodejs` tracks current and will move past 24, so the package may need to depend on a specific `nodejs-lts-*` rather than `nodejs`. Test this before publishing, and pick the dependency that actually satisfies the engine constraint on the day you ship.

**LGPL note, in your favour:** distributing a build recipe is not distributing the work. For the AUR package you convey no tosu binary at all — the user's machine builds it from upstream source — so the T-3 conveying obligations do not apply there. They still apply in full to the Windows installer and the `.deb`/`.rpm`, which do ship a binary.

**Acceptance.**
- `make tosu` produces a working tosu on a clean Arch container with only `base-devel`, `nodejs`, `pnpm` and `git` installed.
- No network access during `build()` beyond the declared sources.
- The daemon connects to the resulting tosu with no configuration.

---

## 7. L: Linux distribution packages

The current `packaging/linux/install.sh` is a per-user script (`~/.local/bin`). That stays as the from-source path, but it is not a distributable package.

### L-0. Distribution channel: GitHub releases (owner decision)

**Decided:** `.deb` and `.rpm` are built in CI and published as **GitHub release artifacts**. They are never submitted to Debian, Ubuntu, Fedora or openSUSE. This is not a deferral to revisit later — it is the distribution model.

It is also the only workable one. Both archives require every Rust dependency to be packaged separately as a distro package, which is impractical for a workspace this size, and both forbid vendored prebuilt binaries — which the Windows and deb/rpm artifacts deliberately contain (bundled tosu, T-2).

**The AUR is a separate matter and is still the plan for Arch.** The AUR is not an official Arch repository — it is a recipe index, and publishing a `PKGBUILD` there is the ordinary way to distribute Arch software. Nothing about the GitHub-only decision affects it. (What *is* out of scope for Arch is `extra`/`core`.)

**So users install with:**

| Distro | Command |
|---|---|
| Debian / Ubuntu / Mint / Pop | `apt install ./osupad_<ver>_amd64.deb` |
| Fedora / RHEL / openSUSE | `dnf install ./osupad-<ver>.x86_64.rpm` |
| Arch / Manjaro | `yay -S osupad` |

**Three consequences that follow directly, and must be handled rather than discovered:**

1. **Dependency declarations matter more, not less.** `apt install ./file.deb` and `dnf install ./file.rpm` both still resolve declared dependencies against the user's configured repositories. Getting the per-distro package names right in L-3 is what makes a local install work at all — a missing dependency here is a hard failure with no repo to fall back on.
2. **No repository signature.** Packages installed from a file are not verified against repo metadata; `dnf` will warn about an unsigned package. GPG-sign the `.rpm` with `rpmsign` and publish the public key in the README. The minisign-signed release manifest (**U-0.3**) is the primary integrity mechanism for every artifact on every platform, and it covers this case too.
3. **Discovery is entirely on you.** There is no `apt search osupad`. The README, the releases page and the project site are the only ways anyone finds this, so the install instructions above need to be prominent and copy-pasteable.

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
| Arch | hand-written `PKGBUILD` | Wraps the **B** Makefile: `make DESTDIR="$pkgdir" PREFIX=/usr install tosu`. Builds tosu from source (B-4), so no vendored binary. Publish `osupad` (source) and optionally `osupad-git` |

**Runtime dependencies differ per distro and must be declared explicitly.** `iced`/wgpu needs Vulkan plus X11/Wayland client libraries, and `ksni` needs D-Bus. Get the per-distro package names right (`libvulkan1` vs `vulkan-loader`, `libwayland-client0` vs `wayland`, and so on) — a missing dependency here surfaces as a GUI that fails to start with an opaque wgpu error.

**Acceptance.** Install, launch, use and remove cleanly in a fresh container or VM for **Debian stable, Ubuntu LTS, Fedora and Arch**. Removal leaves no files behind, matching the W2-3 standard applied to Windows. Package installation must **not** enable the user service automatically without consent — `systemctl --user enable` is the user's decision, prompted by the GUI as it is today.

### L-4. AppImage (optional)

An AppImage covers every other distro with one artifact and is cheap to add once the binaries build. Flatpak is **not** recommended: the sandbox complicates raw serial access, the per-user IPC socket, and spawning tosu as a subprocess, for little benefit to this particular application.

---

## 8. T: tosu integration and redistribution

### T-1. The licensing position

**tosu is LGPL-3.0** (Mikhail Babynichev). osu!pad is MIT. These interact cleanly here, for a specific reason worth writing down so it is not re-litigated later:

**osu!pad does not link tosu in any way.** `desktop/crates/osupad-tosu/src/lib.rs` talks to it over a WebSocket at `ws://127.0.0.1:24050/websocket/v2` (`DEFAULT_TOSU_ENDPOINT`), and `spawn_tosu_supervisor` (`:275`) launches it as a **separate process**. Separate programs communicating over a socket are not a derivative work, and executing a program is not linking. **No copyleft obligation reaches osu!pad's own MIT-licensed code.** This is true whether or not tosu is bundled.

**Redistribution is the only thing that creates obligations.** If an installer or package *ships the tosu binary*, that is conveying an LGPL-3.0 work, which requires shipping the license text and copyright notice, and providing the corresponding source (or a valid written offer / access from the same place the binary is offered). All of this is satisfiable — it is permitted, not forbidden — but it is an ongoing maintenance burden: every tosu version bump means re-checking the source offer.

### T-2. Decision: bundle tosu, pinned to its stable release

**Owner decision (2026-09-16), overriding the earlier "never bundle" recommendation.** tosu ships inside the installer and the Linux packages, and the app keeps it current by tracking tosu's **stable** (non-prerelease) GitHub release — see **U-1**.

The auto-update requirement resolves the main practical objection to bundling. A bundled-and-pinned tosu goes stale as osu! changes and silently stops working; a bundled tosu that updates itself does not. What remains true is that bundling means **conveying an LGPL-3.0 work**, which carries the obligations in T-3. Those are satisfiable and routine, but they must actually be in the artifacts, not just intended.

**What ships:**
- Windows: `tosu.exe` inside the Inno package, installed to `%LOCALAPPDATA%\Programs\osupad\tosu\`.
- Linux `.deb`/`.rpm`: the tosu binary under `/usr/lib/osupad/tosu/`, **not** `/usr/bin` — it is a private, auto-updating component, not a system command, and it must not collide with a tosu the user installed themselves.
- **AUR: bundled too, but built from source** (see **B-3**). Arch policy rejects vendored *prebuilt* binaries, but building from source is the normal AUR path, so the package compiles tosu on the user's machine and installs it to `/usr/lib/osupad/tosu/`. Same end result as the other platforms, arrived at the Arch-correct way.

**Auto-update is disabled for package-manager-owned builds.** On AUR (and on any `.deb`/`.rpm` installed system-wide), the package manager owns the tosu binary, so U-1 must not replace it — `pacman`/`apt`/`dnf` would be overwritten behind their back and the file would be reported as modified. The rule generalises cleanly: **osu!pad only ever auto-updates a tosu it owns and installed into a user-writable location.** Everything else is reported, not touched.

**The existing resolution order stays and gains one step at the end.** `find_tosu_binary` (`desktop/crates/osupad-tosu/src/lib.rs:256`) resolves `$OSUPAD_TOSU_PATH` → `~/.local/opt/tosu/tosu` → `$PATH`. Append the bundled location **last**, so a tosu the user installed deliberately always wins over the bundled copy. Never overwrite or auto-update a tosu found outside the bundled directory — that binary is not yours to manage.

### T-3. LGPL-3.0 compliance for the bundled binary

Conveying tosu requires three things per artifact. None are difficult; all must be verifiable at release time.

**Do:**
1. **License text and notice.** Ship `licenses/tosu/LICENSE` (the full LGPL-3.0 text) and a `NOTICE` recording the upstream project, author (Mikhail Babynichev), copyright, the exact bundled version, and the release URL it came from. Installed alongside the binary on every platform.
2. **Corresponding source.** LGPL-3.0 conveying obligations are satisfied by offering source from the same place the binary is offered. Publish, next to each osu!pad release artifact, the matching tosu source tarball or an explicit written offer naming the exact upstream tag. **Generate this automatically from the version the build pulled**, so it cannot drift from the binary actually shipped.
3. **Distro metadata.** `.deb` needs `debian/copyright` listing LGPL-3.0 for the bundled component; `.rpm` needs the composite `License:` field. A package whose metadata claims MIT while shipping an LGPL binary is simply incorrect.

**Also required, and easy to forget:** LGPL-3.0 grants the user the right to **replace** the bundled component with their own version. The `$OSUPAD_TOSU_PATH` override and the "use my own tosu install" setting satisfy this in practice, so keep both working and mention them in the NOTICE.

**Acceptance.** A release script check fails the build if the bundled tosu version does not match the shipped NOTICE and source offer. Verify on all artifacts, including the Windows installer.

### T-4. tosu status and manual override in the GUI

**Do:**
- Device or Settings page shows tosu status: *bundled (version)* / *using your install at PATH* / *running* / *connected*.
- A setting to use an external tosu instead of the bundled one, with a file picker, writing the same value `$OSUPAD_TOSU_PATH` provides.
- A setting to pin the bundled tosu to its current version and disable U-1 updates.
- The supervisor already skips launching when something is listening on port 24050 (`spawn_tosu_supervisor`, `:275`), so a hand-started tosu keeps working untouched. Do not change this.

**Acceptance.**
- Fresh install: telemetry works with no user action, because tosu is bundled.
- A user with their own tosu on `$PATH` keeps using it; the bundled copy is never launched and never updated.
- `COM-05` in `docs/testing-checklist.md` (tosu killed mid-play) still passes: PLAYING → COOLDOWN → IDLE with no stuck UI.

### T-5. Attribution

Add a **Third-party software** section to the README and an About entry in the GUI naming tosu, its author, its LGPL-3.0 license and its repository. Required by the license now that it is bundled, and correct regardless — this project depends on their work for its headline feature.

---

## 9. U: Updates

Three separate updaters with three different risk profiles. **None of them existed before this section; all three are new work.**

### U-0. Rules that apply to all three

1. **Never during PLAYING or COOLDOWN.** This is P1-3 extended to updates. Replacing a binary, restarting a process, or writing flash mid-map is exactly the class of thing P1-3 exists to prevent. All three updaters check state and defer.
2. **An updater is a remote code execution channel into the user's machine.** It is the highest-risk component in this plan — higher than the installer, because it runs unattended and repeatedly. Every downloaded artifact is verified before execution (U-0.3). Treat a shortcut here as a security bug, not a convenience.
3. **Signed manifests, independent of code signing.** Publish a release manifest (versions, URLs, SHA-256 per artifact) signed with **minisign/ed25519**, with the public key compiled into the app. This is free, takes an afternoon, and does not depend on SignPath or W2-2. Verify the manifest signature, then verify each artifact's hash against it. HTTPS and "it came from GitHub" are **not** sufficient on their own.
4. **Always user-disablable**, per updater, with the current version and last-check time visible in the GUI.
5. **No update may leave the pad unable to act as a keyboard.** If an update fails at any point, the previous working state must survive.
6. **Check on a schedule, not aggressively.** Once per day, with ETag caching. The unauthenticated GitHub API allows 60 requests/hour per IP; a naive poll across many users looks like abuse and gets rate-limited.

### U-1. tosu auto-update (tracks stable)

**Do:**
- Query GitHub Releases for tosu, selecting the newest release with `prerelease == false` and `draft == false`. That is the definition of "stable" here; write it down so it is not reinterpreted later.
- Compare against the installed bundled version. If newer: download the platform asset, verify its SHA-256, and replace atomically (download to a temp file, fsync, rename).
- **Only when the daemon is IDLE.** Stop the supervised tosu, swap, restart. A pending update waits rather than interrupting.
- Never touch a tosu outside the bundled directory (T-2).
- Regenerate the NOTICE and source-offer reference on update (T-3), so a self-updated install stays compliant.
- On failure: keep the current binary, log it, surface it in the GUI, retry next cycle. Never leave the directory without a working binary.

**Acceptance.** A stale bundled tosu updates itself within a day of a new stable release. Killing the app mid-download leaves the previous version intact and working. An update never happens during a map.

### U-2. osu!pad app auto-update

**Every install checks GitHub and downloads the update.** What differs between platforms is only how the update is **applied**, and that difference comes from who owns the installed files.

| Install origin | Check | Download | Apply |
|---|---|---|---|
| **Windows installer** | auto | auto | Run the new signed installer `/SILENT /NORESTART`; Inno's `CloseApplications` stops the running processes. No prompt if the user opted in. |
| **`.deb` / `.rpm`** (GitHub) | auto | auto | `pkexec apt-get install -y <file>` / `pkexec dnf install -y <file>` → **one polkit password prompt**, then the real package manager applies it. |
| **AUR** | auto | **no** | **Notify only.** `pacman` owns these files and `yay`/`paru` update them. osu!pad must not touch them. |
| **`make install-user`** (`~/.local`) | auto | auto | Direct file replacement. No prompt — the user already owns every file. |
| **AppImage** (if L-4) | auto | auto | Direct replacement. |

**Why `.deb`/`.rpm` needs that one prompt.** The binaries live in `/usr/bin`, owned by root and recorded in the dpkg/rpm database. The daemon runs as the user and cannot write there. Requiring authentication is not a limitation to engineer around — it is the OS working correctly, and every native Linux updater behaves this way.

**The thing that must not be done:** overwriting `/usr/bin/osupad-daemon` directly, even if a way is found to get write access. That desynchronises the package database — `dpkg -V` / `rpm -V` then report modified files, and the next reinstall or `apt upgrade` silently reverts the update. **Always apply through the package manager**, never around it. Because these packages come from GitHub rather than a repository, `apt install ./new.deb` records the new version cleanly and nothing will later contradict it.

`pkexec` plus the native package manager is the baseline. PackageKit's D-Bus `InstallFiles` is the more "correct" desktop API and handles polkit natively, but it is not installed everywhere (notably not on most Arch systems), so treat it as an optional nicety rather than the mechanism.

### U-2a. Detecting install origin

The updater must know which row of that table it is in, and **must not guess**.

**Do:** have each packaging path drop a marker file at `$(PREFIX)/lib/osupad/install-origin` containing exactly one of `windows`, `deb`, `rpm`, `aur`, `appimage`, `user`, `source`. The Makefile (**B-2**) writes `user` or `source`; `cargo-deb`, `cargo-generate-rpm`, the `PKGBUILD` and the Inno script each write their own.

Probing the system instead (`dpkg -S`, `rpm -qf`, `pacman -Qo`, path prefix checks) is slower, needs those tools present, and gets ambiguous cases wrong — a `.deb` that a user unpacked by hand, for example. A marker written at package time is unambiguous and costs one line per packaging path.

**Fallback:** if the marker is missing or unrecognised, degrade to notify-only. Never guess and then modify files.

**Acceptance.** Each packaging path produces the right marker, verified in the clean-container tests from L-3. An install with no marker never attempts to apply an update.

**What the GitHub-only decision (L-0) actually costs Linux** is not self-update — it is the *unattended* part. Updates still arrive in the app and still install from the app; they just need one authentication per update instead of none.

**If you later want fully unattended `apt`/`dnf` updates without leaving GitHub**, it is achievable: an APT or DNF repository is just a static file tree, and **GitHub Pages can host one**. APT needs `dists/` + `pool/` with a GPG-signed `Release`/`InRelease`; DNF needs `repodata/` from `createrepo_c`. Users add the repo once, then updates flow through their normal package manager with no prompt from you. The cost is a signing key you must keep and metadata regenerated every release.

**Out of scope for v1.0.** Recorded so the option is not rediscovered from scratch.

**Do:**
- Daemon checks the signed manifest daily; GUI shows "update available" with release notes and an explicit "Install now".
- **Prompt; never auto-install by default.** The user may opt in to automatic install on Windows.
- Defer while PLAYING or COOLDOWN.
- Coordinate daemon and GUI restart; do not leave a stale daemon talking to a new GUI. The IPC handshake (P1-7) must reject a version mismatch loudly rather than misbehave.
- An app update **must not** trigger a firmware update. They are separate decisions (U-3).
- Replacing a running binary is safe on Linux (the open inode survives), so the order is: apply, then restart the daemon and GUI. Do not stop the daemon before the package manager has actually succeeded — a failed update must leave a running app behind.

**Acceptance.**
- Windows updates in place, keeps counters, and the pad stays a working keyboard throughout.
- `.deb`/`.rpm` update from inside the app via one polkit prompt; afterwards `dpkg -V` / `rpm -V` report **no** modified files, and the recorded package version matches what is installed.
- Cancelling the polkit prompt leaves the running version fully working and the update still pending.
- An AUR install never modifies anything on disk.
- A corrupted or wrongly-signed download is rejected and the running version is untouched.

### U-3. ESP32-S3 firmware update

This reverses **Appendix B** of `osupad_remaining_work_v1.md` ("a custom OTA subsystem is out of scope; flashing stays espflash over USB"). Note the reversal explicitly when updating that document.

**The device is a keyboard. A firmware update is the only operation in this entire project that can stop it being one.** Everything below follows from that.

#### U-3a. Change the partition table now — do this even though OTA comes later

**Problem.** `firmware/sdkconfig.defaults` sets `CONFIG_PARTITION_TABLE_SINGLE_APP_LARGE=y`: one 1.5 MB factory app, no OTA slots. The board has **16 MB of flash**, so roughly 14 MB is currently unallocated. There is no space problem — only a layout problem.

**Why now.** Adding OTA slots later rewrites the partition table at `0x8000`, which cannot be done by an OTA update — it needs a full serial reflash of **every device in the field**. Right now the field is approximately zero devices. This is the cheapest this change will ever be, and it gets strictly more expensive with every pad shipped.

**Required change.** Custom `firmware/partitions.csv` with two OTA slots, **keeping `nvs` at offset `0x9000` at its current size** so existing lifetime counters survive (the existing comment in `sdkconfig.defaults` already relies on this property):

```
nvs,      data, nvs,     0x9000,  0x6000
otadata,  data, ota,     0xf000,  0x2000
phy_init, data, phy,     0x11000, 0x1000
ota_0,    app,  ota_0,   0x20000, 0x200000
ota_1,    app,  ota_1,   0x220000,0x200000
```

Ship v1.0 on this layout and flash `ota_0` with espflash as today. The OTA *code* can come later; the *layout* cannot.

**Acceptance.** A pad flashed with the new table keeps its lifetime counters across the change. `idf.py flash` and the recovery path both still work. Firmware reports its running partition in `HelloAck`.

#### U-3b. v1.0 mechanism: host-driven flash over USB

Use the path that already exists rather than adding firmware attack surface for v1.0.

**Do:**
- Daemon downloads the firmware image, verifies it against the signed manifest (U-0.3), and checks the target chip and `firmware_version` before doing anything.
- Reboot the pad into the ROM bootloader, flash the app partition **only** (never `erase-flash` — that wipes NVS and the lifetime counters), and return to the app using the existing `reset_to_app` in `desktop/cli/src/esp_rom.rs`, which already handles the ESP32-S3 USB-Serial-JTAG reboot quirk.
- **Explicit consent every time. Never automatic, never silent, not even opt-in.** Show exactly what will happen: *"Your pad will be unusable as a keyboard for about N seconds. Do not unplug it."*
- Refuse to start unless the daemon is IDLE, the pad is connected directly (not through a hub the user is about to disturb), and counters have been synced to the host first.
- On Windows, release the COM port before flashing (W1-3) — Windows serial handles are exclusive.
- Post-flash validation already exists as part of P1-7; reuse it. If the pad does not come back with the expected version, say so loudly and link to `docs/recovery.md`.

**The failure mode to document honestly:** if flashing is interrupted, the app partition is incomplete and the pad will not run as a keyboard until re-flashed. It is **not bricked** — the ESP32-S3 ROM bootloader is in mask ROM and cannot be erased — but it does need manual recovery. This is the strongest argument for U-3c.

#### U-3c. OTA A/B with rollback — the target design

Once U-3a has shipped and the layout is in place, move to real OTA:

- The daemon streams the image over the existing CDC protocol into the inactive slot; the app keeps running, so **the keyboard stays alive during the transfer**.
- Switch slots on a single reboot; mark the new image valid only after it boots and enumerates successfully, otherwise the bootloader rolls back automatically (`esp_ota_mark_app_valid_cancel_rollback`).
- That rollback is the whole point: **a failed firmware update can no longer leave the user without a working keyboard.**

**The latency constraint that governs the implementation.** Writing to SPI flash disables the instruction cache, stalling any code executing from flash — potentially for milliseconds during an erase. This is precisely why P1-3 blocks NVS writes during play, and it applies to OTA writes with far more force given the volume. Therefore: OTA writes are **IDLE-only**, chunked, and yield between blocks. Re-run the `docs/latency-testing.md` stages with an OTA transfer in progress and treat any regression as a release blocker.

**Not in v1.0.** U-3a is in v1.0; U-3b is v1.0's mechanism; U-3c follows. Sequencing it this way means the OTA code can be written without stranding anyone.

---

## 10. W4: Verification and release

### W4-1. CI

Add `x86_64-pc-windows-msvc` to the CI matrix: `cargo build`, `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`. **`cargo check` alone is not sufficient** — the `ksni`/`tray-icon` split and the IPC transport aliases fail at link time, not check time.

### W4-2. Windows hardware checklist

Extend `docs/testing-checklist.md` with a Windows column. Re-run at minimum: HW-01..HW-05, COM-01, COM-02, STR-01, STR-02. **HW-05 (HID-first on display wake) and STR-02 (zero storage writes) are the invariant-critical ones** and must be re-verified on Windows rather than assumed from the Linux run.

Add a new section for W2-3's install/uninstall filesystem+registry diff.

### W4-2b. Update verification

Each updater needs its own checklist entries, because these are the paths that can break a working install:

- Tampered/wrongly-signed manifest is **rejected**; the running version is untouched (U-0.3).
- Download interrupted mid-transfer for each of the three updaters: previous working state survives in all cases.
- No updater fires during PLAYING or COOLDOWN; a pending update defers and applies afterwards (U-0.1).
- Firmware update preserves lifetime counters (U-3b flashes the app partition only, never `erase-flash`).
- Post-update, the pad still enumerates as a keyboard on a machine with no osu!pad software installed.

### W4-3. Latency on Windows

Re-run the `docs/latency-testing.md` stages on Windows and add rows to the results table. Note that the Linux v1.0 table is **still empty** (P3-1 is the last open v1 item) — that should be filled first, so there is a baseline to compare Windows against. `scripts/bench_latency.py` uses evdev and is Linux-only; the Windows host-side equivalent needs a raw-input or ETW-based approach, or the firmware-side percentiles alone for stages A-C.

### W4-4. Docs

- Rewrite `docs/windows-portability.md` from aspirational architecture notes into the actual as-built description.
- README: Windows install instructions, SmartScreen note if unsigned, and the reflash/unbind section.
- `docs/architecture.md`: the IPC transport abstraction and the pairing model.
- `docs/protocol.md`: `owner_id` in `HelloAck`, the `ClaimOwnership` command.
- Record the pairing model as an amendment appendix in `osupad_technical_spec_v1.md`, and mark **A3 superseded** in `osupad_remaining_work_v1.md`.

---

## 11. Order of work

Three tracks that only converge at W4. They can be worked in parallel.

```
Windows        W0-1 ─┬─ W0-2          (IPC abstraction: all Windows work blocks on this)
                     └─ W0-3
               W0-4, W0-5, W0-6       (parallel, independent)
                  ↓
               W1-1, W1-2, W1-3
                  ↓
               W2-1 ─ W2-3            (installer + strong uninstall)

Linux          L-2 ─ B-1 … B-4 ─ L-1 ─ L-3   (udev fix first: it is a correctness bug today;
                                  │            the Makefile then carries the install layout)
                                  └─ L-4       (optional)

Firmware       U-3a                   (OTA partition table — do this early, see below)

Cross-platform T-2 … T-5               (bundle tosu + LGPL compliance)
               W3-1 → W3-2 → W3-3 → W3-4   (pairing; ships on Linux too)
               U-0 → U-1, U-2, U-3b    (updaters; U-0 signing gates all three)

                  ↓ all tracks
               W4-1 … W4-4 → re-cut v1.0.0
```

**Start here.** Two items are worth doing before anything else because they are live defects rather than new features:

1. **L-2** — the udev rule names a group that does not exist on Debian or Fedora, and grants `0666` to every process on the machine. That is wrong on the platform you already shipped.
2. **U-3a** — the OTA partition table. It costs almost nothing today and costs a manual reflash of every pad in the field once you have users. It is the one item here whose price only goes up.
3. **P3-1** — the latency table in `docs/latency-testing.md` is still empty. It is the last open v1 item and it is the baseline every other platform gets compared against.

Then **W0-1**, which gates every remaining Windows task, and **U-0**, which gates all three updaters.

**Deferred until needed:** W2-2 (signing) cannot start until the repository is public, so it trails the rest.

---

## 12. Risks

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
| **An updater becomes a remote code execution channel** | U-0.3: minisign-signed manifest, hash-verified artifacts, public key in the binary. The single highest-risk item in this plan |
| A firmware update is interrupted and the pad stops being a keyboard | U-3a ships the OTA layout in v1.0 so U-3c's automatic rollback becomes possible; until then U-3b requires explicit consent and documents recovery |
| OTA flash writes stall the CPU and break the latency gate | U-3c: IDLE-only, chunked writes; re-run the latency stages with a transfer in flight |
| Bundled tosu ships without its license or source offer | T-3: release script fails the build on a version/NOTICE mismatch |
| tosu auto-update overwrites a tosu the user installed themselves | T-2: bundled directory only; user installs always win resolution order |
| Shipping an OTA partition table later strands every pad in the field | U-3a: do it now, while the field is ~zero devices |
| Arch's `nodejs` moves past the 24.x that tosu's `engines` pins | B-4: depend on the `nodejs-lts-*` that actually satisfies it, and verify before publishing |
| `make install` tries a privileged action and breaks staged package builds | B-2: `install` writes only under `$(DESTDIR)`; reloads and enables live in scriptlets |
| U-1 overwrites a tosu owned by `pacman`/`apt`/`dnf` | T-2: only auto-update a tosu osu!pad installed into a user-writable location |
| Updater writes `/usr/bin` directly and desynchronises the package database | U-2: apply only through the package manager; U-2a marker decides which path is legal |

---

## 13. Out of scope

- macOS.
- MSIX packaging and Store-style "plug in the pad → Windows offers the app". That needs a Store-signed MSIX; the Inno decision rules it out. The daemon-at-login plus hotplug detection (W1-2) delivers nearly the same feel.
- Any cryptographic enforcement of the pairing model (§0).
- Windows Service hosting for the daemon (W1-1).
- Submission to Debian/Ubuntu/Fedora/openSUSE archives, or to Arch `extra`/`core` (L-0). GitHub releases only — **the AUR is still in scope**.
- Flatpak (L-4).
- Using `@yao-pkg/pkg` in the source build (B-4) — the system `nodejs` plus a wrapper script replaces it.
- Hosted APT/DNF repositories (including the GitHub Pages route in U-2), and therefore *unattended* `.deb`/`.rpm` updates. v1.0 updates from inside the app with one polkit prompt.
- OTA firmware transfer code (U-3c). The **partition layout** for it is in scope (U-3a).
- EV code signing (W2-2).
- v2 rapid trigger — see `osupad_v2_rapid_trigger_plan.md`.

---

## Appendix A: parallel work split (added 2026-09-16)

Two agents work this plan at the same time: **Claude** and **Antigravity**. They cannot message each other. **The repository is the only channel** — commits, and the status block in A.7.

### A.1. Branches and worktrees

```bash
git worktree add -b v1.0-agy ../osupad-agy main   # Antigravity opens this folder
git checkout -b v1.0-claude                        # Claude works in the main checkout
```

`main` is the integration target. Worktrees share one `.git`, so each side sees the other's commits immediately with no fetch: `git log v1.0-agy` works from either directory.

**There is no git remote.** Nothing has ever been pushed. Do not add a remote, do not push, do not open a PR.

### A.2. Columns

| Column | Owner | Tasks, in order | Needs the pad? |
|---|---|---|---|
| **A** | **Claude** | W0-1 → W0-2 → W0-4 → W1-1 → W1-2 → U-0 → U-1 → U-2 → U-2a → W3-1, W3-3 | no |
| **B** | **Antigravity** | L-2 → B-1…B-4 → L-1 → L-3 → W0-3 → W0-5 → W0-6 → W2-1 → W2-3 → T-2…T-5 | no |
| **C** | whoever the owner is supervising | U-3a → W3-2 → U-3b | **yes** |

Column C is hardware-gated (A.5) and is scheduled by the owner, not claimed by an agent.

### A.3. File ownership — do not edit outside your column

| Path | Owner |
|---|---|
| `desktop/crates/osupad-ipc/**` | A |
| `desktop/daemon/**` | A |
| `desktop/cli/**` | A |
| `desktop/crates/osupad-tosu/**` | A |
| `desktop/gui/**` | B |
| `packaging/**`, `Makefile`, `*.iss`, `PKGBUILD` | B |
| `firmware/**`, `protocol/*.proto` | C |
| `docs/**`, `*.md` | whoever owns the task; keep the hunk small |

`desktop/crates/osupad-model`, `-protocol`, `-storage`, `-layout` are shared. Touch them only when your task requires it, keep the change minimal, and say so in the commit subject.

**The one cross-column dependency.** W0-1 changes public signatures in `osupad-ipc` (`connect_and_handshake`, `send_request`, `read_request`, `send_response` move from `UnixStream` to an `IpcStream` alias). `desktop/gui/src/ipc.rs` and `desktop/gui/src/single_instance.rs` are **B's files** and must adapt afterwards.

Protocol: **A lands W0-1 first** and writes `BREAKING: osupad-ipc signatures` in the commit subject. B rebases and adapts. B does not start W0-3 before that commit exists. Until then B has L-2, B-1…B-4, L-1 and L-3, none of which touch Rust.

### A.4. Read before starting

1. `osupad_technical_spec_v1.md` — the contract
2. `osupad_packaging_distribution_plan.md` — this document, especially §0
3. `osupad_remaining_work_v1.md` §0 and Appendix A — conventions and v1 status
4. `docs/architecture.md`

### A.5. Hardware is exclusive — one agent at a time

There is **one pad**. `/dev/ttyACM0` takes one process, `osupad-daemon` has a single-instance guard, and flashing obviously cannot overlap.

- Never run `osupad-daemon` or open the serial port unless the owner has handed you the pad.
- Never flash without being told to.
- If a task needs the device, **stop and say so** rather than taking it. The owner serialises column C.
- Everything in columns A and B is pure code and needs no device.

### A.6. Rules that override everything

1. **The pad is a keyboard first.** Spec §3 and P0-1: it must work as a 1000 Hz HID keyboard with no software, no install, no pairing, on any machine. Nothing may add work to the key ISR, the keypad task, or the TinyUSB task on core 0.
2. **No storage writes during PLAYING or COOLDOWN** (P1-3). This now covers updates too (U-0.1).
3. **Commits are authored `GFerreiroS <info@gferreiro.com>`.** No `Co-Authored-By`, no session trailers, no agent attribution. Conventional-commit subjects, committed on your own branch.
4. **Small commits, one task each.** The other agent reads your commits to know what changed.
5. **Do not edit the other column's files.** If you genuinely must, keep the hunk minimal and name it in the commit subject.
6. Before merging to `main`: rebase onto the other column's latest, then confirm `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test` and the firmware host tests all pass.

### A.7. Status — each side updates its own row when a task closes

Keep it to one line per task. This is how the other agent learns what landed.

| Date | Column | Task | Commit | Notes |
|---|---|---|---|---|
| 2026-09-16 | A | W0-1 | `3bfbbc7` | **B is unblocked for W0-3.** `osupad-ipc` public types are now `IpcStream` (client), `IpcServerStream` (accepted server end) and `IpcListener`; `create_listener` returns `IpcListener` and `accept()` yields one stream, not a tuple. The framing helpers are generic over `IpcTransport`, so `&mut IpcStream` call sites are unchanged. `gui/src/ipc.rs` needed no edit. |
| 2026-09-16 | A | W0-2 | `84ff5d8` | Windows pipe instances get a protected DACL (SYSTEM + the calling user only); `create_listener` fails rather than creating an unrestricted pipe if the SID is unknown. The two-account "another user cannot open the pipe" check is a W4-2 manual item. |
| 2026-09-16 | A | W0-4 | `5074a6b` | New `osupad_model::paths` resolves every app path via `dirs` and fails loudly instead of falling back to a relative one. **B: use it for W2-3's uninstall list** — everything written is `<data>/osupad.db`, `<state>/tosu.log`, `<state>/daemon.log`, plus the socket/pipe. Also `install_lib_dir()` / `install_prefix()` / `bundled_tosu_binary()` / `install_origin_path()`. |
| 2026-09-16 | A | W1-1 | `c30aeaf` | **Touches `gui/` (B's).** New `gui/src/platform_windows.rs`; autostart is `HKCU\…\Run`. **The installer must write these byte-for-byte:** `osupad-daemon` → `"<dir>\osupad-daemon.exe"`, `osupad-gui` → `"<dir>\osupad-gui.exe" --tray`. Five of W0-6's six `cfg(linux)` sites now have Windows branches — treat them as done. `gui/Cargo.toml` gained a windows-only `winreg`. |
| 2026-09-16 | A | W1-2 | `9610481` | Reconnect poll, not `WM_DEVICECHANGE` (§W1-2 allows this): 400 ms scan + 300 ms settle keeps the 2 s acceptance with margin, and a headless daemon needs no message pump. |
| 2026-09-16 | A | U-0 | `395c5f6` | New `osupad-update` crate: signed manifest, minisign + SHA-256 verification, atomic staged installs, daily ETag-cached schedule, IDLE-only gate. **Updates are inert until the owner generates a signing key** (added to A.8). tosu artifacts must be recorded in our signed manifest to be installable — see the manifest module docs. |
| 2026-09-16 | A | U-2a | `8320676` | **Contract for B:** each packaging path writes `$(PREFIX)/lib/osupad/install-origin` (Windows: `install-origin` in the install dir) containing exactly one of `windows`/`deb`/`rpm`/`aur`/`appimage`/`user`/`source`. Newlines, spaces and case are tolerated. Missing or unknown ⇒ notify-only, never modify. Verify in L-3's clean-container tests. |
| 2026-09-16 | A | U-1 | `1a25c0c` | Bundled tosu updates itself, IDLE-only, and only where we own the binary (`windows`/`user`/`appimage`). NOTICE and VERSION are rewritten on every swap, so a self-updated install stays §T-3 compliant. `spawn_tosu_supervisor` now returns a `TosuSupervisor` handle — **signature change, B has no call sites**. Storage migration v7 adds `app_state`. |
| 2026-09-16 | A | U-2 | `f53b048` | App updates apply through whatever owns the files (installer / pkexec+apt/dnf / direct / notify-only). Never automatic: a person presses Install. **The IPC handshake now rejects a version mismatch**, since GUI/CLI/daemon always ship together. **B: the GUI side of §U-0.4 is unbuilt** — `GetUpdateStatus`, `SetUpdateEnabled`, `InstallUpdate` and `ComponentUpdate` are the surface it needs. |
| 2026-09-16 | A | W3-1 | `a52e7ae` | Per-install UUIDv4 in `app_state`, generated on first run. No storage ⇒ no identity ⇒ claims nothing, prompts about nothing. |
| 2026-09-16 | A | W3-3 | `723b3ef` | **Touches `protocol/` (C's).** Added `HelloAck.owner_id` (field 8) and `ClaimOwnership` (HostToDevice 14); nothing renumbered. **W3-2 is still C's** — NVS storage, the IDLE-only claim write, firmware tests. Until then every pad reports no owner and is claimed silently. "Leave it alone" suppresses config, layouts, telemetry and all syncing while the pad stays a keyboard. **B: the prompt UI is unbuilt**; `Status.pending_takeover` + `ResolveTakeover` are the surface. |
| 2026-09-16 | A | audit fix | `aaf6ca9` | Taking a pad over never wrote the new owner to it, so it would have re-prompted on every reconnect; the test drove a method production never called. Fixed, plus the applied `.deb`/installer is now deleted and `osupadctl setup` no longer prints Linux udev instructions on Windows. **B, for T-2/T-3:** U-1 reads the installed tosu version from `<bundled tosu dir>/VERSION` (one line, e.g. `4.26.2`) — packaging must write it alongside `NOTICE`, or the updater cannot tell what is installed. Paths already match: `$(PREFIX)/lib/osupad/tosu/tosu`. |
| 2026-09-16 | A | U-2 / W3-3 GUI | `61d3da1` | **Touches `gui/` (B's).** The GUI halves of two A tasks I had wrongly left to B: the §W3-3 takeover prompt (three buttons, both counter pairs, a pointer to `docs/recovery.md`) and the §U-0.4 update panel on Settings (installed/available versions, last check, per-updater switch, Install button only when something is ready, notify-only installs say to use the package manager). Plus a restart banner after an in-place update. `pages::grouped` is now `pub(crate)`; `tray.rs` still has its own copy as `format_grouped`, worth folding together during W0-5. |
| 2026-09-16 | C | W3-2 | `22f00ca` | **Touches `firmware/` (C's), done at the owner's request.** `owner_id[16]` in the NVS config blob as v3; v1/v2 blobs migrate and read as unclaimed. Decision logic is in `config/owner.c` with no IDF dependencies, covered by 7 host tests: claim, re-claim (no write), takeover, IDLE-only, and an all-zero claim refused so there is no wire path to unpairing (§W3-4). `osupad.pb.c/.h` regenerated with nanopb 0.4.9.1. **Not compiled** — no ESP-IDF on the machine it was written on — and the hardware acceptance is still open. **Still unstarted in column C: U-3a (partition table) and U-3b.** |
| 2026-09-16 | B | L-2 | `f1ccbfd` | Drop Arch-specific `GROUP="uucp"` and insecure `MODE="0666"`, keep `TAG+="uaccess"`, narrow `ATTRS{idProduct}` to `4001|1001` (app and bootloader). |
| 2026-09-16 | B | B-1…B-4 | `21bbab3` | Top-level Makefile with GNU PREFIX/DESTDIR, all/tosu/firmware/install/install-user/check targets, install-origin marker, and `@BINDIR@` templating for unit and desktop entries. |
| 2026-09-16 | B | L-1 | `f5e7860` | Template user unit and desktop launcher with `@BINDIR@`, install to `/usr/bin`, `/usr/lib/systemd/user`, `/usr/share/applications`, `/usr/lib/udev/rules.d`. |
| 2026-09-16 | B | L-3 | `64d5c56` | Add PKGBUILD (source build wrapping Makefile), `cargo-deb` metadata with maintainer scripts and `debian/copyright`, and `cargo-generate-rpm` metadata with udev scriptlets. |
| 2026-09-17 | B | W0-3 | `8596568` | Single-instance guard on Windows using `Local\osupad-gui` named mutex, plus handoff via per-user named pipe `\\.\pipe\osupad-gui-{sid}`. |
| 2026-09-17 | B | W0-5 | `fc7cf7f` | Cross-platform tray: ksni scoped to Linux, tray-icon on Windows with dedicated thread and Win32 pump, shared TrayViewModel and unified status formatting via `pages::grouped`. |

### A.8. Open owner decisions

These are not for an agent to settle:

- **Publish the repository?** Required before SignPath free code signing (W2-2).
- **`v1.0.0` tag.** It is local-only and should be deleted and re-cut when W4 passes; workspace version becomes `1.0.0-rc` until then.
- **Update signing key.** §U-0.3 requires a minisign/ed25519 keypair whose public half is compiled into the app (`osupad_update::verify::MANIFEST_PUBLIC_KEY`, currently empty). Generate with `minisign -G`, decide where the secret key lives, and keep it off the build machines. Until it is set, all three updaters fail closed and nothing can be downloaded and applied.
- **Latency table** (`docs/latency-testing.md`, P3-1) — hardware runs, owner only. Still the last open v1 item.
