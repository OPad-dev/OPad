---
title: Installing the Desktop App
description: Install the optional OPad desktop app (opad-daemon, opad-gui, opadctl and the bundled tosu) on Linux or Windows 10/11.
sidebar:
  order: 3
---

The desktop app is optional: the pad types without it. Install it for live osu! data on the pad's screen, the screen designer, lifetime counters on your PC, swipe actions, Hall Effect tuning and firmware updates. <!-- src: /AGENTS.md#1. Product rules -->

It has four parts: <!-- src: /packaging/windows/installer.iss --> <!-- src: /desktop/gui/Cargo.toml -->

| Program | What it does |
|---|---|
| `opad-daemon` | Runs in the background. Talks to the pad over its serial port, keeps your counters and settings, and runs tosu. |
| `opad-gui` | The OPad window and tray icon: settings, Designer, Stats, Device and Diagnostics. |
| `opadctl` | The same features from a terminal, plus flashing and recovery. See the [`opadctl` reference](../reference/opadctl.md). |
| tosu | Reads osu!'s memory for map and play data. Bundled (LGPL-3.0); you can use your own, see [Using OPad with tosu](../guides/use-with-tosu.md). |

## Download

Every release on the [GitHub Releases](https://github.com/OPad-dev/OPad/releases) page lists, at the top of its notes, which file is for your system: <!-- src: /scripts/release/release_notes.sh -->

| System | File |
|---|---|
| Windows 10 or 11 | `OPad-Windows-Setup-<version>.exe` |
| Ubuntu or Debian | `OPad-Linux-Debian-Ubuntu-<version>.deb` |
| Fedora, RHEL, AlmaLinux, Rocky | `OPad-Linux-Fedora-<version>.rpm` |
| Arch Linux | `OPad-Linux-Arch-<version>.pkg.tar.zst` |
| Any other Linux | `OPad-Linux-<version>.AppImage` |

The packages are built for x86-64 and target: <!-- src: /README.md --> <!-- src: /scripts/release/release_notes.sh -->

- Ubuntu 22.04, 24.04 and 26.04; Debian 12 and 13 (`.deb`);
- Fedora and RHEL / AlmaLinux / Rocky 9 and 10 (`.rpm`);
- Arch Linux (`.pkg.tar.zst`);
- any other Linux (AppImage);
- Windows 10 and 11 (installer).

The Linux binaries are built against glibc 2.31 and the bundled tosu against glibc 2.28, so they run on every distribution above. <!-- src: /.github/workflows/release-app.yml --> <!-- src: /Makefile#TOSU_GLIBC -->

## Linux

### Debian and Ubuntu

```bash
sudo apt install ./OPad-Linux-Debian-Ubuntu-<version>.deb
```

### Fedora, RHEL, AlmaLinux and Rocky

```bash
sudo dnf install ./OPad-Linux-Fedora-<version>.rpm
```

### Arch Linux

```bash
sudo pacman -U ./OPad-Linux-Arch-<version>.pkg.tar.zst
```

This installs the `opad-bin` package: the same binaries and bundled tosu as the `.deb`. <!-- src: /packaging/linux/arch/PKGBUILD-bin.in --> <!-- src: /.github/workflows/release-app.yml -->

### What the packages set up

The `.deb`, `.rpm` and Arch packages all install the same things: <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install -->

- `opad-daemon`, `opad-gui` and `opadctl` in `/usr/bin`, with tosu and a pinned `espflash` under `/usr/lib/opad/`;
- the udev rule `/usr/lib/udev/rules.d/70-opad.rules`, so you can use the pad without root and ModemManager leaves it alone;
- the `cap_sys_ptrace` capability on `/usr/lib/opad/tosu/tosu`, so tosu can read osu! without you lowering the system-wide `ptrace_scope` (see [Troubleshooting §2](../troubleshooting.md#2-linux-tosu-cannot-read-osu-yama-ptrace_scope));
- a login autostart entry that starts the OPad tray icon, and an `opad-daemon` systemd user unit.

You do not need to start the daemon yourself: when `opad-gui` starts and finds no daemon running, it starts one (through the systemd user unit when that is available). <!-- src: /desktop/gui/src/platform_linux.rs#start_daemon --> <!-- src: /desktop/gui/src/main.rs#App -->

These packages update themselves from the app's **Settings** → **🔄 Updates & Firmware** tab. <!-- src: /desktop/crates/opad-update/src/origin.rs -->

### AppImage

```bash
chmod +x OPad-Linux-<version>.AppImage
./OPad-Linux-<version>.AppImage
```

The AppImage installs nothing system-wide, so the first time it runs it offers to install the udev rule (it asks for your password through `pkexec`). You can also install the rule yourself: <!-- src: /desktop/gui/src/platform_linux.rs#appimage_needing_udev_rule --> <!-- src: /packaging/linux/appimage/AppRun -->

```bash
./OPad-Linux-<version>.AppImage install-udev
```

The same file also runs the other programs: `./OPad-Linux-<version>.AppImage daemon` starts the daemon, and `./OPad-Linux-<version>.AppImage ctl <command>` runs `opadctl`. <!-- src: /packaging/linux/appimage/AppRun -->

> [!NOTE]
> An AppImage cannot give tosu the `cap_sys_ptrace` capability, so on a system with Yama `ptrace_scope` above 0 you have to lower it for live osu! data. See [Troubleshooting §2](../troubleshooting.md#2-linux-tosu-cannot-read-osu-yama-ptrace_scope). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

To build the app yourself and install it from a checkout of the repository instead, see [Building from source](../develop/building.md#installing-a-source-build).

### Check your Linux setup

```bash
opadctl setup
```

It reports whether the udev rule is installed and matches this version, and whether tosu can read osu!'s memory, with the command that fixes it if not. <!-- src: /desktop/cli/src/main.rs#run_setup -->

## Windows

1. Download `OPad-Windows-Setup-<version>.exe` and run it.
2. Windows may show **Windows protected your PC**, because the installer is not code-signed yet. Click **More info**, then **Run anyway**. If you want, first compare the file with the SHA-256 checksum shown next to it on the release page. <!-- src: /README.md -->
3. Keep **Start OPad when I log in** checked (the default) if you want the daemon and tray icon to start with Windows.
4. Finish the installer. It starts OPad for you.
<!-- src: /packaging/windows/installer.iss -->

The installer does not need administrator rights. It installs for your user only, into `%LOCALAPPDATA%\Programs\opad`, with `opad-gui.exe`, `opad-daemon.exe`, `opadctl.exe`, `espflash.exe` and `tosu\tosu.exe`. **Start OPad when I log in** adds `opad-daemon` and `opad-gui --tray` to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Windows uses its own `hidclass.sys` and `usbser.sys` drivers for the pad; nothing else is installed. <!-- src: /packaging/windows/installer.iss -->

## Check that it works

Open **OPad** from your application menu or Start menu, or run:

```bash
opadctl status
```

You should see `ESP32 Device:     Connected`, the pad's firmware version, and `tosu (osu!):      Active` once tosu has started. In the app, the **Device** page shows the same. <!-- src: /desktop/cli/src/main.rs#Commands::Status --> <!-- src: /desktop/gui/src/pages.rs#device -->

If the pad is not detected, see [Troubleshooting §3](../troubleshooting.md#3-linux-usb-permissions-and-the-serial-port).
