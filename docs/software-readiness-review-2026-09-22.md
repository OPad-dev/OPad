# Software readiness review (2026-09-22)

**Author:** Claude (Opus 5.5, Anthropic), AI code reviewer
**Scope:** the whole repository, with a focus on why OPad breaks when it is installed on a machine other than the one it was built on, and on the differences between Linux and Windows.
**Method:** read the desktop workspace, packaging, release scripts, CI and the relevant firmware USB code. Ran `cargo test --workspace` locally (all pass on Linux). Checked the built binaries with `objdump`/`ldd`. Read the logs of the last CI run on `main` (`35763954377`).
**Not changed:** no code was changed. This document is the only file added.

Severity: **P0** = blocks a release or breaks a fresh install. **P1** = breaks a common setup or a feature. **P2** = robustness, polish, or cleanup.

---

## TL;DR: what to fix before calling it ready

| # | Sev | Problem | Where |
|---|---|---|---|
| 1 | P0 | Linux binaries are built on Arch and need **glibc 2.44** (`opad-gui`) / 2.39 (`opad-daemon`, `opadctl`). They do not start on Ubuntu 22.04/24.04, Debian 12/13, or Fedora before 2.44. | release build process |
| 2 | P0 | The deb/rpm **never start the daemon at login**: no enabled user unit, no autostart entry. After a reboot nothing runs until the GUI is opened by hand. | `packaging/linux/deb/postinst`, rpm scripts |
| 3 | P0 | **Leftover `osupad` names** break real features: `opadctl flash <dir>` and `--full` look for `osupad-firmware.bin` (the build makes `opad-firmware.bin`), and the manifest generator ignores every artifact the release scripts produce, so **no update is ever offered**. | `cli/src/main.rs:743`, `opad-manifest.rs:232-251` |
| 4 | P0 | **Flashing needs `espflash`, which no package ships.** The in-app firmware update and `opadctl flash` fail on any machine without it. | `opad-device/src/flash.rs:144-166` |
| 5 | P0 | **The Windows installer ships no tosu.** Nothing builds `tosu.exe`, and the installer skips the missing file without any message. | `Makefile:82`, `installer.iss:95` |
| 6 | P0 | **CI on `main` is red:** Windows GUI job uses `-p osupad-gui`, and `gen_proto.sh` points at `protocol/opad.proto`, which does not exist. | `.github/workflows/ci.yml:136`, `scripts/gen_proto.sh` |
| 7 | P1 | Pad detection accepts **any** `303a:4001` device (Espressif's stock TinyUSB CDC PID), and flashing accepts **any** `303a:1001` device (every ESP32-S3/C3/C6 USB-Serial-JTAG). A second Espressif board on the desk can be opened, or even flashed, by mistake. | `opad-device/src/lib.rs:712-727`, `flash.rs:115` |
| 8 | P1 | Designer preview is a **stub** in any build made from a clean checkout (LVGL comes from `managed_components/`, which is gitignored and only exists after `idf.py build`). | `opad-ui-preview/build.rs:22-37` |
| 9 | P1 | Linux-only GUI gaps: autostart writes `Exec=opad-gui` with no path, the GUI-launched tosu writes `tosu.env` to the wrong place, file dialogs need an xdg-desktop-portal, the Wayland app id doesn't match the `.desktop` file, and the AppImage cannot register a service or autostart that survives. | see §3 |
| 10 | P1 | tosu on Linux probably cannot read osu! memory under the default Yama `ptrace_scope=1` (Ubuntu, Arch). The app shows "tosu connected" but gets no gameplay data. **Needs a check on real hardware.** | §3.6 |

---

## 1. Serial port detection (no pinned ports)

**Short answer: the app already finds the pad on its own.** The daemon, `opadctl` and the flasher all find it by USB VID/PID through `serialport::available_ports()` (`opad-device/src/lib.rs:712-727`). No port is stored in config. `--port` on `opadctl flash`/`bootloader` is only an optional override. Windows was tested on COM3, COM4 and COM15 (WIN-08).

The problems are with **what** it matches and with a few leftovers that still name a port:

### 1.1 P1: the match is too broad

- `find_target_port()` returns the **first** port with `303a:4001`. That is the PID esp_tinyusb gives any CDC-only device by default, so an ESP32-S2/S3 dev board running a TinyUSB example matches too. The daemon then opens it, sets DTR/RTS, and sends Hello to it forever.
- `find_bootloader_port()` matches `303a:1001`. That is **not** unique to the ROM bootloader: it is the USB-Serial-JTAG of every ESP32-S3/C3/C6/H2 running normal firmware. `enter_bootloader()` returns such a port straight away (`flash.rs:115-117`), so `opadctl flash` / the in-app update can **write OPad firmware to another board** that happens to be plugged in.
- `DeviceManager` sets `is_connected = true` as soon as the port opens (`lib.rs:203`), before any `HelloAck`. Commands can be queued to a device that never answers, and a non-OPad device looks "connected" in the UI.
- With two pads connected, the first one listed wins and the other is ignored without a word.

**Fix:**
- Match on VID/PID **and** the USB serial prefix `OSUPAD-` (or the product string `OPad ESP32-S3`). `serialport`'s `UsbPortInfo` already has `serial_number` and `product` on both OSes.
- Only report "connected" after `HelloAck`. Drop the port and move to the next candidate if no `HelloAck` arrives within about 3 s.
- For the bootloader: note the app port's USB location (serial number, or `/sys` path / Windows location path) before rebooting it, and accept only a `303a:1001` port at the same location. If more than one `303a:1001` is present and none can be matched, refuse and ask for `--port`.
- When moving to `1209:4F50` (pid.codes, see the 2026-09-21 todo), change `find_target_port`, `70-opad.rules`, `trigger_easter_egg.py` and the docs in the same commit, and keep accepting `303a:4001` for one release.

### 1.2 P2: leftover pinned ports

| Where | What | Fix |
|---|---|---|
| `scripts/trigger_easter_egg.py:133-145` | Matches any `0x303A` device (any PID), then falls back to `COM3` / `/dev/ttyACM0`. Also has a hard-coded `C:\Users\paella\.espressif\...` Python path (~line 23). | Match VID+PID+serial prefix like the Rust code. Fail if nothing is found instead of guessing. Remove the personal path. |
| `README.md:131` | `idf.py -p /dev/ttyACM0 flash` | Drop `-p` (idf.py auto-detects) or use `opadctl flash`. |
| `docs/recovery.md:221,228` | `espflash ... -p /dev/ttyACM0` / `-p COM3` | Say "the port from `espflash list-ports`", or drop `-p` (espflash asks). |
| `docs/testing-checklist.md` COM-02 | names `/dev/ttyACM0` | Cosmetic; `test_com02` already auto-detects. |

`opad-ipc/tests/ipc_test.rs:94` uses `/dev/ttyACM0` only as a test string, which is fine.

### 1.3 P2: udev rule hardening

`packaging/linux/udev/70-opad.rules` is correct now (sorts before 73, `uaccess` only). Also add `ENV{ID_MM_DEVICE_IGNORE}="1"` so ModemManager never probes the pad's ttyACM. This matters because the firmware reboots into download mode on an RTS falling edge while DTR is high (`firmware/main/usb/usb_cdc.c:270-275`), so any program that probes the port and toggles the lines can knock the pad into the ROM bootloader.

---

## 2. Why it breaks on another machine (build portability)

### 2.1 P0: glibc too new

Measured on the current `desktop/target/release` build (built on this Arch machine, glibc 2.44):

| Binary | Highest glibc symbol | Why |
|---|---|---|
| `opad-gui` | **GLIBC_2.44** (`sinh`, `cosh`), 2.43 (`acosf`, `atan2f`, ...) | libm symbols pulled in by iced/wgpu/lyon |
| `opad-daemon` | GLIBC_2.39 | `pidfd_spawnp` (std process spawning) |
| `opadctl` | GLIBC_2.39 | same |

A `.deb`/`.rpm`/AppImage built here fails with `version 'GLIBC_2.44' not found` everywhere except rolling distros. Ubuntu 24.04 has 2.39, Debian 13 has 2.41, Ubuntu 22.04 has 2.35.

**Fix:** build release binaries in an old-glibc container: `ubuntu:22.04`, or a manylinux-style image, or `cargo zigbuild --target x86_64-unknown-linux-gnu.2.31`. Do it in a CI release job, not on a developer machine. Add a CI step that fails if `objdump -T` shows a glibc newer than the chosen floor.

### 2.2 P0: releases are built by hand on one machine

- `scripts/build_all.sh:18`, `scripts/gen_proto.sh:7`, `scripts/release/build_release.sh:23` source `~/Documents/projects/esp32/v5.5.2/esp-idf/export.sh`, a path on one developer's machine.
- `build_packages.sh` rewrites tracked files (`packaging/linux/deb/opad-daemon.service`, `install-origin` markers) every time it runs.
- No CI job produces release artifacts, so what ships depends on the state of one workstation: its glibc, whether `managed_components/` exists (§2.3), whether `espflash` is on `PATH`, and whether `build/tosu` is stale.

**Fix:** add a tag-triggered `release.yml`. Linux job in an old-glibc container: firmware (`espressif/idf:v5.5.2`), then deb/rpm/AppImage/tarball. Windows job: installer with `tosu.exe` and `espflash.exe`. Then manifest + signature. Scripts should use `$IDF_PATH` or `idf.py` on `PATH` and fail clearly if neither exists.

### 2.3 P1: Designer preview is silently a stub

`opad-ui-preview/build.rs` compiles the real LVGL UI only if `firmware/managed_components/lvgl__lvgl` exists. That directory is gitignored and only created by `idf.py build`/`reconfigure`. On a clean checkout (CI, AUR `build()`, another developer) it prints a `cargo:warning` and links `preview_stub.c`, so the Designer shows mock frames. It also uses `firmware/sdkconfig` when present, so the preview can differ between machines.

**Fix:** in release builds, run `idf.py reconfigure` (or `python -m idf_component_manager`) before `cargo build`. Or vendor the pinned LVGL 9.5 sources for the host build. Make the stub opt-in (`OPAD_PREVIEW_STUB=1`) and have the release build fail without the real sources. Also **commit `firmware/dependencies.lock`**: it is gitignored now, so `lvgl ~9.5.0` / `esp_tinyusb ^2.0.0` can resolve to different versions on each machine.

### 2.4 P1: undeclared runtime tools

| Tool | Used for | Shipped? |
|---|---|---|
| `espflash` | every flash (`flash.rs:144-166`) | No (deb/rpm/AppImage). Windows only if `espflash.exe` happens to be in `target/release`. |
| `systemctl --user` | GUI "Start daemon"/"Install user service" | assumed |
| `pkexec` + a polkit agent | deb/rpm self-update (`opad-update/src/app.rs`) | assumed (no agent on many bare WMs) |
| `xdg-desktop-portal` + a FileChooser backend | every file dialog (rfd 0.15 uses the portal via `ashpd`) | assumed |
| Node 24 (Arch only) | `TOSU_STANDALONE=0` wrapper | `depends=('nodejs')`, but Arch `nodejs` is newer than 24 and tosu needs `>=24.14 <25` |

**Fix:** bundle `espflash` (pinned version) next to the binaries in every package, or implement the write in-process with the `espflash` crate, since there is already a SLIP client in `flash.rs`. Arch: depend on `nodejs-lts-krypton` (Node 24), or use the standalone tosu too. Document or depend on `xdg-desktop-portal` (and `-gtk`/`-kde`).

---

## 3. Linux-specific gaps (why Windows works and Linux doesn't)

Windows has a single installer that sets up everything: the Run keys, tosu (when present) and absolute paths. On Linux these jobs are split across several package formats, and some of them are missing.

### 3.1 P0: nothing starts at login (deb/rpm)

- `postinst` reloads udev only. The user unit is installed to `/usr/lib/systemd/user/` but never enabled, and no `/etc/xdg/autostart/opad-gui.desktop` is shipped.
- The unit says `WantedBy=graphical-session.target` / `PartOf=graphical-session.target`. Many sessions never reach that target (i3, bspwm, Hyprland/Sway without uwsm, startx). There the unit does not start even after `enable`, and `PartOf` stops the daemon when the target stops.

**Fix:** ship `/etc/xdg/autostart/opad-gui.desktop` (`--tray`) and run `systemctl --global enable opad-daemon.service` in postinst. Change the unit to `WantedBy=default.target` and drop `PartOf`/`After=graphical-session.target`, since the daemon needs no display. The GUI already starts the daemon if it's offline, which covers sessions without systemd.

### 3.2 P1: AppImage cannot run on its own

- No udev rule. A user who isn't in `dialout`/`uucp` gets `Permission denied`, and the hint says to check `70-opad.rules`, which the AppImage doesn't have.
- The GUI starts the daemon from inside the FUSE mount (`find_sibling_executable`). When the GUI exits, the AppImage runtime unmounts and the daemon's files (and the bundled tosu) disappear.
- "Install user service" writes `ExecStart=/tmp/.mount_XXXX/usr/bin/opad-daemon` (`platform_linux.rs:72-77`), which is gone after a reboot.
- `owns_bundled_tosu()` is true for AppImage (`origin.rs`), but the bundle is read-only squashfs, so a tosu update cannot be written.
- The updater wants `ArtifactKind::Binary` for AppImage, but `opad-manifest` never emits one, so AppImage updates always fail with "no artifact".

**Fix:** when `$APPIMAGE` is set, use `"$APPIMAGE" daemon` (AppRun already dispatches it) for the service `ExecStart`, the detached spawn and the autostart `Exec`. Add a first-run "Install udev rule" action (`pkexec` copying the embedded rule). Treat the AppImage's tosu as not owned. Classify `*.AppImage` as `Binary` in the manifest.

### 3.3 P1: GUI autostart points at a bare name

`platform_linux::set_gui_autostart_enabled` writes `Exec=opad-gui --tray` (`platform_linux.rs:156`). With `make install-user`, `~/.local/bin` is often not on the graphical session's `PATH`, so autostart silently does nothing. Windows writes an absolute, quoted path (`platform_windows.rs:33-41`). **Fix:** write `current_exe()` (or `$APPIMAGE`), like Windows does.

### 3.4 P1: the GUI's "Start tosu" writes to the wrong config

`gui/src/main.rs:2614-2660` has its own copy of the tosu launch code. It writes `tosu.env` **next to the binary**. That is the bug the daemon fixed in `opad-tosu` (`tosu_config_dir`, `lib.rs:425-431`): on Linux tosu reads `~/.config/tosu/tosu.env`. On a deb/rpm install `/usr/lib/opad/tosu` isn't writable anyway. So a GUI-started tosu opens the browser dashboard and logs to `/dev/null`. **Fix:** remove the copy and ask the daemon (IPC) to start/resume its supervisor, or call a shared `opad_tosu::launch_tosu`.

### 3.5 P2: name and path mismatches

- Wayland `application_id: "osupad"` (`gui/src/main.rs:434`) vs `StartupWMClass=opad` in every `.desktop` file. GNOME/KDE show a generic icon and don't group the window with its launcher.
- A detached daemon logs to `$XDG_STATE_HOME/osupad/daemon.log` (`platform_linux.rs:99-103`). Everything else uses `.../opad/` (`paths::daemon_log_path`).
- IPC socket: `XDG_RUNTIME_DIR/opad/daemon.sock`, else `/tmp/opad-<uid>/` (`opad-ipc/src/transport/unix.rs:13-19`). If the GUI or `opadctl` runs without `XDG_RUNTIME_DIR` (su, some cron/ssh contexts) while the daemon has it, they can't find each other and the GUI starts a second daemon. **Fix:** fall back to `/run/user/<uid>` when it exists before using `/tmp`.
- `opadctl setup` tells the user to `sudo cp packaging/linux/udev/70-opad.rules ...`. That relative path only exists in a source checkout. It already embeds the file: write it out through `sudo tee` instead.

### 3.6 P1 (to verify): tosu memory access on Linux

tosu reads osu! (stable under Wine, or lazer) process memory. On Linux that needs ptrace-level access to another process. With Yama `kernel.yama.ptrace_scope=1` (default on Ubuntu and Arch, 0 on Fedora), a process that isn't the target's parent is refused. In that case tosu can run and serve its WebSocket (the pad shows "tosu connected") but never report gameplay. That fits the "works on Windows, not on Linux" pattern. **Check:** on Ubuntu, `cat /proc/sys/kernel/yama/ptrace_scope`, then play a map and read `tosu.log`. **Likely fix:** `setcap cap_sys_ptrace=eip` on the bundled tosu in postinst (deb/rpm; not possible for AppImage), plus a clear diagnostic in the GUI when tosu is up but sees no osu! process. Also confirm which osu! builds tosu 4.26.2 supports on Linux (stable/Wine vs native lazer) and say so in the README.

### 3.7 P2: tray on GNOME

No StatusNotifier host means no tray. This is handled: the window opens and quit-on-close is used (`main.rs:1449-1457`). Document that GNOME needs the AppIndicator extension to keep OPad in the tray.

---

## 4. Windows-specific gaps

- **P0, no tosu:** see TL;DR 5. `make tosu` only runs `compile:linux`. Add `compile:win` in a Windows CI job and fail the installer build if `tosu.exe` is missing (drop `skipifsourcedoesntexist` for it).
- **P0, no espflash:** `espflash.exe` is copied only if it's already in `target/release`. Fetch a pinned release in CI.
- **P2:** `find_tosu_binary()` looks in `~/.local/opt/tosu/tosu.exe` on Windows, a Linux path. Harmless, but use `%LOCALAPPDATA%` or drop it.
- **P2:** the installer runs `taskkill /F /IM tosu.exe`, which also kills a tosu the user runs on their own for overlays. Kill only the one under `{app}\tosu` (e.g. PowerShell filtering on `Path`).
- **P2:** `OutputBaseFilename=opad-setup` has no version, and the manifest expects `osupad-setup-<ver>.exe` (see §5).
- WIN-06 (flash with the GUI running) passed on bare metal. Keep it on the release checklist, because it's the most fragile Windows path (exclusive COM handles).

---

## 5. Leftovers from the `osupad` → `opad` rename that break things

| Where | Expects | Actually produced | Effect |
|---|---|---|---|
| `cli/src/main.rs:743` (`resolve_flash_set`) | `<dir>/osupad-firmware.bin` | `firmware/build/opad-firmware.bin` | `opadctl flash firmware/build` and `--full` fail "App image not found". The tests pass only because they create `osupad-firmware.bin` themselves. |
| `opad-manifest.rs:251` | `osupad-firmware.bin` | `dist/opad-firmware.bin` | Firmware never in manifest → no in-app firmware updates. |
| `opad-manifest.rs:232` | `osupad-setup-*.exe` | `opad-setup.exe` | No Windows app updates. |
| `opad-manifest.rs:241` | `osupad-linux-x86_64-*.tar.gz` | `opad-linux-x86_64-<ver>.tar.gz` | No `install-user` updates. |
| `.github/workflows/ci.yml:98-144` | package `osupad-gui` | `opad-gui` | Windows GUI job fails (`package ID specification 'osupad-gui' did not match`). |
| `scripts/gen_proto.sh` | `protocol/opad.proto` | `protocol/osupad.proto` | Protobuf drift job fails. |
| `README.md` 31, 96-222 | `osupadctl`, `osupad-daemon.service`, `packaging/linux/desktop/...` | `opadctl`, `opad-daemon.service` | Install instructions don't work. |
| `opad-tosu` / GUI | `$OSUPAD_TOSU_PATH`, `$OSUPAD_MANIFEST_URL` | — | Works, but inconsistent. Accept both, document `OPAD_*`. |
| `opad-update/src/http.rs:12` | User-Agent `osupad/…` | — | Cosmetic. |
| `daemon/src/backup.rs:73`, GUI export names | `osupad-backup-*`, `osupad-*.json`, `osupad.log` | — | Cosmetic. Keep reading the old prefix. |

**Fix:** one pass with `git grep -n osupad`. Decide for each hit whether it is a *compat alias to keep reading* (DB, data dir, env vars, old backups) or a *name to change*. Add a test that runs `build_release.sh`'s naming through `app_artifact`/`firmware_artifact`, so the two can't drift again.

---

## 6. Versioning

- The workspace is `1.0.0-rc`, but the daemon logs `v1.0.0` (`daemon/src/main.rs:47`), sends Hello with `client_version: "1.0.0"` (`opad-device/src/lib.rs:224`), and PKGBUILDs say `pkgver=1.0.0` with `sha256sums=('SKIP')` against a `v1.0.0` tag that doesn't exist yet. The release script defaults `FIRMWARE_VERSION` to `1.0.0` instead of reading `firmware/CMakeLists.txt`.
- **Fix:** use `env!("CARGO_PKG_VERSION")` everywhere, read `PROJECT_VER` from CMake in the release script, and generate the PKGBUILD `pkgver`/checksum at release time.
- There are two identical PKGBUILDs (`/PKGBUILD` and `packaging/linux/arch/PKGBUILD`) plus `PKGBUILD-git`. Keep one.

---

## 7. Robustness issues in the daemon and device layer

- **P2, daemon side effects before the single-instance check.** `main.rs` starts the tosu supervisor (line 197) and the `DeviceManager` (line 207) *before* `create_listener` (line 212) can report `AlreadyRunning`. A second daemon (GUI auto-spawn racing systemd) can briefly launch tosu and grab the serial port before it exits. Move `create_listener` up to right after the first `connect` check.
- **P2, write latency.** The device loop blocks in `read()` for up to 100 ms and then sleeps 5 ms (`lib.rs:174, 265`), so a queued command can wait about 105 ms before it's written. That's fine for config, but noticeable for HUD `DataUpdate` values. Use a shorter read timeout (10 ms) or a separate writer thread.
- **P2, `paths.rs` side effects.** `data_dir()`/`state_dir()`/`database_path()` try the legacy→modern rename on every call. Do the migration once at daemon startup, and log a failed rename (for example across filesystems) instead of ignoring it.
- **P2, `paths.rs` docs** still say `osupad` in the table (lines 12-14).

---

## 8. Documentation drift

- `README.md` install and CLI sections: old names and paths (see §5).
- `docs/specs/packaging-distribution.md` still quotes `osupad-device/src/lib.rs:577` and other pre-rename paths.
- The testing checklist says WIN-06 passed with `osupad-firmware.bin`, a file the current build no longer produces.
- Add a "Linux requirements" section to the README: udev rule (or `dialout`/`uucp`), xdg-desktop-portal, the AppIndicator extension on GNOME, ptrace note for tosu (§3.6), and the minimum glibc once §2.1 is fixed.

---

## 9. Suggested order of work

1. **Make CI green** (§5 rows for `ci.yml` and `gen_proto.sh`). Every later step relies on it.
2. **Rename pass** (§5), with the naming test.
3. **Release pipeline in CI** with an old-glibc Linux container and a Windows runner, bundling `tosu`/`tosu.exe` and `espflash` (§2.1, §2.2, §2.4, §4).
4. **Real Designer preview in release builds** and a committed `dependencies.lock` (§2.3).
5. **Linux start-at-login** for deb/rpm (§3.1), absolute autostart paths (§3.3), a single tosu launch path (§3.4).
6. **Stricter device matching and safer bootloader selection** (§1.1).
7. **AppImage self-sufficiency** (§3.2), or drop the AppImage for 1.0 and say so.
8. **Check tosu on stock Ubuntu** (§3.6) and document the result.
9. Versioning cleanup (§6), robustness items (§7), docs (§8).

### Acceptance test for "ready"

Test on **fresh VMs** (not the dev machine): Ubuntu 24.04 (GNOME, Wayland), Fedora (KDE), Arch (a WM without `graphical-session.target`), Windows 11 with no dev tools.

- [ ] Install the package from CI artifacts, reboot, and plug the pad in. The daemon is running, the pad connects within 2 s, and there are no permission errors.
- [ ] Plug in a second Espressif board as well (TinyUSB CDC example and/or a USJ console board). The daemon still picks the OPad, and `opadctl flash` refuses or picks the right one.
- [ ] tosu starts without opening a browser. Playing a map updates the pad HUD.
- [ ] Designer preview shows real LVGL frames, not the stub.
- [ ] `opadctl flash --full <unpacked release>` and the in-app firmware update both work with nothing extra installed.
- [ ] The update check finds the app and firmware artifacts in a signed test manifest.
- [ ] GUI autostart and "Start daemon" work after a reboot, including from the AppImage if it ships.

---

## What this review did not cover in depth

- Firmware internals beyond USB descriptors and CDC line handling (input timing, LVGL, NVS, OTA slot selection).
- GUI page logic and the Designer's layout rules.
- Hardware (KiCad, enclosure) and the licensing items already tracked in `docs/todo-2026-09-21-evening.md`.
- Anything that needs the pad or the Windows VM to confirm. §3.6 in particular is a likely cause, not a verified one.
