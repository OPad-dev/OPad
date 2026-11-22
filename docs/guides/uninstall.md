---
title: Uninstalling the Software
description: How to uninstall the OPad desktop suite on Linux and Windows, what data is removed, and what stays on the pad.
---

Uninstalling the desktop suite removes the background daemon, GUI, `opadctl`, bundled `tosu`, autostart entries, and desktop data.

> [!NOTE]
> Uninstalling the desktop software has **no effect on the pad hardware**. OPad keeps working as a standalone 1000 Hz USB HID keyboard, and its lifetime counters, key settings, ownership record, and Hall Effect calibration stay intact in the pad's NVS flash. To erase or unbind the pad itself, see [Recovery, Reflashing & Unbinding](../recovery.md). <!-- src: /AGENTS.md --> <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

> [!WARNING]
> Both the Linux package uninstallers (`.deb`, `.rpm`, Arch) and the Windows uninstaller delete the desktop OPad data directory (including `opad.db`, automatic backups, settings, layouts, and `install.id`) without prompting. Export a backup with `opadctl export backup.json` or **Device** -> **Export backup** first if you want to keep a copy on disk. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Linux Uninstallation

### Debian / Ubuntu (`.deb`)
```bash
sudo apt remove opad
# Or to also remove system conffiles:
sudo apt purge opad
```
During package removal (never on upgrade), `prerm` runs `/usr/lib/opad/uninstall-cleanup` (`packaging/linux/uninstall-cleanup.sh`) and `postrm` cleans up system files: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/linux/deb/postrm -->
- Stops running `opad-gui`, `opad-daemon`, `opadctl`, and `/usr/lib/opad/tosu/tosu` processes.
- Removes `/usr/lib/opad`, `/etc/opad`, and `/etc/xdg/autostart/opad-gui.desktop`, and reloads `udevadm`.
- Deletes `$HOME/.local/share/opad`, `$HOME/.local/state/opad`, `~/.config/autostart/opad-gui.desktop`, the enabled `~/.config/systemd/user/default.target.wants/opad-daemon.service` symlink, and any `~/.config/tosu`, `~/.local/share/tosu`, or `~/.cache/tosu` directory containing OPad's `.created-by-opad` marker (leaving user-installed `tosu` directories untouched).

### Fedora / RHEL (`.rpm`)
```bash
sudo dnf remove opad
```
Runs the same `uninstall-cleanup` script on removal (not on upgrade). <!-- src: /packaging/linux/uninstall-cleanup.sh -->

### Arch Linux (`PKGBUILD`)
```bash
sudo pacman -R opad
```
(Use `opad-bin` if installed from an AUR binary package.) <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /packaging/linux/uninstall-cleanup.sh -->

### AppImage, Source Installs, or Custom XDG Directories
If you used the AppImage, installed from source (`make install-user`), or set custom `$XDG_DATA_HOME` / `$XDG_STATE_HOME` paths, remove them manually: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->
```bash
# Stop running processes
pkill -x opad-gui opad-daemon || true

# Remove AppImage or user-local binaries, and desktop data/logs
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
rm -f ~/.config/autostart/opad-gui.desktop

# Optional: remove udev rule installed via AppImage or source
sudo rm -f /etc/udev/rules.d/70-opad.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Windows Uninstallation

1. Open **Windows Settings** -> **Apps** -> **Installed apps** (or **Control Panel** -> **Programs and Features**).
2. Select **OPad** and click **Uninstall**.
3. The Inno Setup uninstaller automatically: <!-- src: /packaging/windows/installer.iss -->
   - Stops running `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe`, and `tosu.exe` processes (`taskkill /F /T`).
   - Deletes the installation directory (`%LOCALAPPDATA%\Programs\opad`).
   - Removes the `opad-daemon` and `opad-gui` autostart entries under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
   - Deletes `%APPDATA%\opad` (containing `opad.db`, automatic backups, `daemon.log`, `tosu.log`, and update staging).

(Named pipes and Windows inbox COM/HID drivers require no manual cleanup.) <!-- src: /packaging/windows/installer.iss -->

## How to check

- Confirm no `opad-daemon` or `opad-gui` process is running.
- Plug in your OPad and type into a text editor: Key 1 and Key 2 continue typing normally as a standalone USB HID keyboard.
