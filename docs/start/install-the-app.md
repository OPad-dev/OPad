---
title: Installing the Desktop Suite
description: How to install the OPad desktop suite (opad-daemon, opad-gui, opadctl, and bundled tosu) on Linux and Windows.
sidebar:
  order: 4
---

The optional OPad desktop suite adds live osu! gameplay telemetry, a screen layout designer, lifetime key counter sync, interactive diagnostics, firmware updates, and Hall Effect tuning. It consists of: <!-- src: /README.md --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->

- `opad-daemon`: background service handling USB CDC communication, SQLite persistence, and `tosu` supervision.
- `opad-gui`: graphical configuration, layout designer, diagnostics, and system tray app (built with `iced`).
- `opadctl`: command-line management and firmware flashing tool.
- `tosu`: bundled memory reader providing live osu! telemetry (LGPL-3.0; you can substitute your own via `OPAD_TOSU_PATH` or **Settings** -> **ℹ About & tosu**). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

## Linux Installation

Pre-built Linux packages are published on the [GitHub Releases](https://github.com/OPad-dev/OPad/releases) page (targeting glibc 2.28+, covering Ubuntu 22.04/24.04/26.04, Debian 12/13, RHEL/AlmaLinux/Rocky 9–10, Fedora, and Arch). <!-- src: /README.md -->

### Debian / Ubuntu / Pop!_OS (`.deb`)
```bash
sudo apt install ./opad_<version>_amd64.deb
```
Or with `dpkg`:
```bash
sudo dpkg -i opad_<version>_amd64.deb
sudo apt-get install -f
```
The package installs: <!-- src: /packaging/linux/deb/postinst --> <!-- src: /Makefile -->
- Binaries into `/usr/bin/`, bundled `tosu` into `/usr/lib/opad/tosu/`, and pinned `espflash` into `/usr/lib/opad/bin/`.
- `udev` rules at `/etc/udev/rules.d/70-opad.rules` (and reloads `udevadm`).
- `cap_sys_ptrace=eip` on `/usr/lib/opad/tosu/tosu` so `tosu` can read `osu!` memory under Yama `ptrace_scope` 1–2.
- The `opad-daemon.service` systemd user unit and desktop/tray autostart entry.

### Fedora / RHEL / openSUSE (`.rpm`)
```bash
sudo dnf install ./opad-<version>.x86_64.rpm
```
Sets up the same binaries, `udev` rule, `cap_sys_ptrace=eip` capability, and systemd user unit as the `.deb` package. <!-- src: /README.md -->

### Arch Linux (`PKGBUILD`)
Build and install from the repository's `packaging/linux/arch/PKGBUILD` (package name `opad`): <!-- src: /packaging/linux/arch/PKGBUILD -->
```bash
makepkg -si
```
On Arch, `tosu` runs under the system `nodejs` (`TOSU_STANDALONE=0`), so file capabilities are not applied to `node`; see [Troubleshooting §2](../troubleshooting.md) if `ptrace_scope > 0`. <!-- src: /packaging/linux/arch/PKGBUILD -->

### Standalone AppImage
```bash
chmod +x opad-x86_64.AppImage
./opad-x86_64.AppImage
```
On first run, the AppImage offers to install the `udev` rule, or you can install it explicitly via `pkexec`: <!-- src: /packaging/linux/appimage/AppRun -->
```bash
./opad-x86_64.AppImage install-udev
```
You can also invoke the bundled daemon or CLI through the AppImage: `./opad-x86_64.AppImage daemon` or `./opad-x86_64.AppImage ctl <command>`. Because an AppImage mounts `nosuid`, `cap_sys_ptrace` does not apply to its bundled `tosu`; see [Troubleshooting §2](../troubleshooting.md). <!-- src: /packaging/linux/appimage/AppRun --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Manual `udev` Rule (Source Installs)
If you installed from source with `make && sudo make install` or `make install-user`, install `70-opad.rules` so non-root users can open the CDC port, USB node, and input node and so ModemManager does not probe the pad: <!-- src: /packaging/linux/udev/70-opad.rules -->
```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```
Run `opadctl setup` to verify `udev` rules and `ptrace` configuration. <!-- src: /desktop/cli/src/main.rs#run_setup -->

## Windows Installation

Download `opad-setup.exe` (or `opad-setup-<version>.exe`) from GitHub Releases and run it. <!-- src: /packaging/windows/installer.iss -->

1. **SmartScreen**: Until code signing is enabled, Windows displays "Windows protected your PC". Click **More info** -> **Run anyway** (you can verify the file against `SHA256SUMS` on the release page first). <!-- src: /README.md -->
2. **Per-user install**: Installs without administrator rights (`PrivilegesRequired=lowest`) into `%LOCALAPPDATA%\Programs\opad` (`DefaultDirName={localappdata}\Programs\opad`). <!-- src: /packaging/windows/installer.iss -->
3. **Installed files**: `opad-gui.exe`, `opad-daemon.exe`, `opadctl.exe`, `espflash.exe`, `install-origin`, and `tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->
4. **Startup**: The optional task **Start OPad when I log in** (checked by default) adds `opad-daemon` and `opad-gui` (`--tray`) under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. <!-- src: /packaging/windows/installer.iss -->
5. **No third-party drivers**: Windows 10/11 uses built-in `hidclass.sys` and `usbser.sys`. <!-- src: /README.md -->

## Starting the Software

Launch **OPad** from your application menu or Start Menu:
- `opad-gui` connects to `opad-daemon` over local IPC (`$XDG_RUNTIME_DIR/opad/daemon.sock` on Linux, `\\.\pipe\opad-ipc-<SID>` on Windows). <!-- src: /desktop/crates/opad-model/src/paths.rs --> <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- Verify everything is connected with `opadctl status` or on the `opad-gui` **Dashboard**. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
