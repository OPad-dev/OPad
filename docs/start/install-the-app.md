---
title: Installing the Desktop Suite
description: Installation guide for the OPad desktop application suite (daemon, GUI, opadctl, tosu) on Linux (.deb, .rpm, Arch AUR, AppImage) and Windows.
category: tutorial
source_files:
  - /packaging/linux/deb/postinst
  - /packaging/linux/udev/70-opad.rules
  - /packaging/linux/appimage/AppRun
  - /packaging/linux/arch/PKGBUILD
  - /packaging/windows/installer.iss
  - /desktop/crates/opad-model/src/paths.rs
---

The OPad desktop software suite provides real-time gameplay HUD telemetry, visual screen layout designing, interactive hardware diagnostics, and Hall Effect magnetic switch tuning. The suite comprises four core components: <!-- src: /desktop/crates/opad-model/src/paths.rs -->
- `opad-daemon`: Background service managing serial communication, counter persistence, and tosu supervision.
- `opad-gui`: Graphical interface built with iced.
- `opadctl`: Command-line management and flashing utility.
- `tosu`: Bundled memory reader streaming live beatmap statistics from osu!.

## Linux Installation

OPad is distributed in multiple native packaging formats for Linux. <!-- src: /packaging/linux/deb/postinst -->

### Ubuntu / Debian (`.deb`)
Download the latest `.deb` package from the [Releases](https://github.com/OPad-dev/OPad/releases) page:
```bash
sudo dpkg -i opad_<version>_amd64.deb
sudo apt-get install -f
```
The package automatically installs:
- Binaries into `/usr/bin/` and bundled tosu into `/usr/lib/opad/tosu/`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->
- Hardware udev rules (`/etc/udev/rules.d/70-opad.rules`). <!-- src: /packaging/linux/udev/70-opad.rules -->
- Sets `cap_sys_ptrace=eip` on `/usr/lib/opad/tosu/tosu` so tosu can read osu!'s process memory under Yama ptrace restrictions without requiring root execution. <!-- src: /packaging/linux/deb/postinst -->
- Installs user systemd service files (`opad-daemon.service`).

### Fedora / RHEL (`.rpm`)
Download the `.rpm` package and install via dnf:
```bash
sudo dnf install ./opad-<version>.x86_64.rpm
```

### Arch Linux (AUR)
Install using your preferred AUR helper:
```bash
yay -S opad-bin
```

### Standalone AppImage
Download the `.AppImage` file and make it executable:
```bash
chmod +x OPad-<version>-x86_64.AppImage
```
Because AppImages run in userspace, install the required hardware udev rule by running: <!-- src: /packaging/linux/appimage/AppRun -->
```bash
./OPad-<version>-x86_64.AppImage install-udev
```
Then run the AppImage directly to launch `opad-gui`.

### Linux udev Rules (`70-opad.rules`)
To grant regular logged-in users permission to communicate with the pad's serial CDC port without `sudo`, the following rule is installed: <!-- src: /packaging/linux/udev/70-opad.rules -->
```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
SUBSYSTEM=="input", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
```
*Note: ModemManager is explicitly instructed to ignore the pad (`ENV{ID_MM_DEVICE_IGNORE}="1"`), preventing AT modem probe traffic from corrupting the serial connection.* <!-- src: /packaging/linux/udev/70-opad.rules -->

## Windows Installation

Download the official Windows installer (`OPad-Setup-<version>.exe`) from GitHub Releases. <!-- src: /packaging/windows/installer.iss -->

1. **Run Installer**: Execute `OPad-Setup-<version>.exe`.
2. **Installation Directory**: Installed by default to `C:\Program Files\OPad`. <!-- src: /packaging/windows/installer.iss -->
3. **Components**: Installs `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe`, and bundled `tosu.exe`.
4. **Startup**: Offers to launch `opad-daemon` silently on Windows user logon via the Registry Run key (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
5. **No Third-Party Drivers**: Windows 10 and 11 use built-in USB HID and USB Serial (CDC ACM) drivers out of the box.

## Starting the Software

Once installed:
- Launch **OPad** from your application menu or start menu.
- `opad-gui` will connect to `opad-daemon` over local IPC (`daemon.sock` on Linux, `\\.\pipe\opad-ipc-<sid>` on Windows). <!-- src: /desktop/crates/opad-model/src/paths.rs -->
- If the daemon is not already running, `opad-gui` will start it automatically in the background.
