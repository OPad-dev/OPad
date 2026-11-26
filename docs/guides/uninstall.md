---
title: Uninstalling the Software
description: Remove the OPad desktop app on Linux or Windows, what the uninstaller deletes, and why the pad keeps working afterwards.
sidebar:
  order: 8
---

Uninstalling removes the daemon, the app, `opadctl`, the bundled tosu, the autostart entries and OPad's data on this PC.

> [!NOTE]
> The pad itself is not affected. It keeps working as a plain USB keyboard, and its lifetime counters, key settings, Hall Effect calibration and owner record stay on the pad. To erase or hand over the pad itself, see [Recovery §7](../recovery.md#7-reflashing-and-unbinding-a-pad). <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

> [!WARNING]
> The uninstaller deletes OPad's data folder without asking: the database with your settings, layouts and play statistics, the automatic backups, and the logs. If you want to keep a copy, export a backup first (`opadctl export ~/opad-backup.json`, or **Device** → **Export backup**) and save it outside that folder. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Linux

Use the command for the package you installed:

```bash
sudo apt remove opad        # .deb
sudo dnf remove opad        # .rpm
sudo pacman -R opad-bin     # Arch release package (opad-bin)
sudo pacman -R opad         # Arch package built from the repository's PKGBUILD
```
<!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/linux/arch/PKGBUILD-bin.in --> <!-- src: /packaging/linux/arch/PKGBUILD -->

On a real uninstall (never on an upgrade), the package runs `/usr/lib/opad/uninstall-cleanup`, which: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/linux/deb/prerm --> <!-- src: /packaging/linux/arch/opad-bin.install -->

- stops `opad-gui`, `opad-daemon`, `opadctl` and the bundled tosu in every session;
- for every user, deletes `~/.local/share/opad` and `~/.local/state/opad`, the app's autostart file `~/.config/autostart/opad-gui.desktop`, and an enabled `opad-daemon` user-service link;
- deletes tosu's `~/.config/tosu`, `~/.local/share/tosu` and `~/.cache/tosu` only where OPad created them, never a tosu you installed yourself;
- deletes the updater's cache `/var/cache/opad-update`.

The package removal also deletes `/usr/lib/opad` and `/etc/opad`, removes the udev rule and the system autostart entry, and reloads udev. <!-- src: /packaging/linux/deb/postrm --> <!-- src: /desktop/gui/Cargo.toml#package.metadata.generate-rpm -->

> [!NOTE]
> The cleanup only knows the default folders. If you set your own `$XDG_DATA_HOME` or `$XDG_STATE_HOME`, delete `$XDG_DATA_HOME/opad` and `$XDG_STATE_HOME/opad` yourself. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->

### AppImage

The AppImage has no uninstaller. Remove it by hand: <!-- src: /desktop/crates/opad-model/src/paths.rs --> <!-- src: /desktop/gui/src/platform_linux.rs --> <!-- src: /packaging/linux/appimage/AppRun -->

```bash
# Stop OPad
pkill -x opad-gui; pkill -x opad-daemon

# If you let the app install its systemd user service
systemctl --user disable --now opad-daemon.service
rm -f ~/.config/systemd/user/opad-daemon.service

# Data, logs and autostart
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
rm -f ~/.config/autostart/opad-gui.desktop

# The udev rule the AppImage installed
sudo rm -f /etc/udev/rules.d/70-opad.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Then delete the `.AppImage` file.

### Installed from source

From the same checkout, run `make uninstall-user` for a `make install-user` install, or `sudo make uninstall` for `sudo make install`. These remove the programs, the service and autostart files and `lib/opad`; delete the data folders and, for `install-user`, the udev rule you copied, as in the AppImage steps above. <!-- src: /Makefile#uninstall --> <!-- src: /Makefile#uninstall-user -->

## Windows

1. Open **Settings** → **Apps** → **Installed apps** (or **Control Panel** → **Programs and Features**).
2. Find **OPad** and click **Uninstall**.

The uninstaller: <!-- src: /packaging/windows/installer.iss -->

- stops `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe` and `tosu.exe`;
- deletes the install folder `%LOCALAPPDATA%\Programs\opad`;
- removes the `opad-daemon` and `opad-gui` login entries from `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`;
- deletes `%APPDATA%\opad`: the database, automatic backups, `daemon.log`, `tosu.log` and update downloads.

There are no drivers to remove: the pad only ever used Windows' own.

## Check that it worked

1. Check that no `opad-daemon` or `opad-gui` process is running.
2. Plug the pad in and type in a text editor: the keys still work, as a standalone USB keyboard.
