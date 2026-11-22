---
title: Uninstalling the Software
description: Complete uninstallation procedure for the OPad desktop suite on Linux and Windows, cleanup of database records, services, and udev rules.
category: how-to
source_files:
  - /packaging/windows/installer.iss
  - /packaging/linux/uninstall-cleanup.sh
  - /packaging/linux/deb/postrm
  - /desktop/crates/opad-model/src/paths.rs
---

If you choose to uninstall the OPad desktop suite, you can remove all application binaries, background services, autostart entries, and local database state cleanly from your system. <!-- src: /packaging/linux/uninstall-cleanup.sh -->

> [!NOTE]
> Uninstalling the desktop suite has **zero effect on the keypad hardware**. The pad continues functioning as a standalone 1000 Hz USB HID keyboard out of the box (Product Rule 2). The pad's internal lifetime counters, switch calibration, and device settings stored in NVS remain intact on the device. <!-- src: /packaging/linux/uninstall-cleanup.sh -->

## Linux Uninstallation

### Debian / Ubuntu (`.deb`)
To remove the application packages:
```bash
sudo apt remove opad
```
To purge configuration files and udev rules completely:
```bash
sudo apt purge opad
```
The packaging scripts execute `packaging/linux/uninstall-cleanup.sh` and `postrm`: <!-- src: /packaging/linux/deb/postrm -->
- Stops running instances of `opad-daemon`, `opad-gui`, and bundled `tosu`.
- Removes `/usr/lib/opad` and `/etc/xdg/autostart/opad-gui.desktop`. <!-- src: /packaging/linux/deb/postrm -->
- Reloads udev rules to release device permissions.

### Fedora / RHEL (`.rpm`)
```bash
sudo dnf remove opad
```

### Arch Linux (AUR)
```bash
sudo pacman -R opad-bin
```

### Manual Linux Data Cleanup
If you wish to purge all local databases, install identity files, and logs created by the daemon: <!-- src: /desktop/crates/opad-model/src/paths.rs -->
```bash
# Delete persistent database and backups
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"

# Delete logs
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
```

## Windows Uninstallation

On Windows 10 and 11, OPad provides a clean Inno Setup uninstaller: <!-- src: /packaging/windows/installer.iss -->

1. Open **Windows Settings > Apps > Installed apps** (or **Control Panel > Programs and Features**).
2. Locate **OPad** in the list and click **Uninstall**.
3. **Data Retention Prompt**:
   - The uninstaller will ask whether you want to **"Delete all user settings, database history, and logs?"**.
   - If you click **Yes**, the uninstaller removes `%APPDATA%\opad` (including `opad.db` and the install UUID). Reinstalling later will generate a fresh install identity. <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
   - If you click **No**, your local keypress counters and screen layout backups are preserved in `%APPDATA%\opad` for future reinstalls.
4. The uninstaller removes binaries from `C:\Program Files\OPad`, stops any running daemon background processes, and unregisters the user logon registry entry. <!-- src: /packaging/windows/installer.iss -->
