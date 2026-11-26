---
title: Troubleshooting
description: Fixes for wrong characters on non-QWERTY layouts, missing tosu data on Linux, USB permissions, Windows SmartScreen and antivirus warnings, and a pad that stops responding.
sidebar:
  order: 3
---

Each section starts with the symptom. The section numbers are fixed, because `opadctl` and the app point to them.

## 1. Keyboard layouts (QWERTY, AZERTY, QWERTZ)

**Symptom:** the keys type the wrong letters, for example `w` instead of `z`.

A USB keyboard sends key *positions* (HID usage IDs), not letters, and your OS keyboard layout turns each position into a character. OPad's defaults are the positions of `z`, `x`, `c` and `` ` `` on a US keyboard: <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/config/device_config.h -->

| Control | HID usage | US / UK QWERTY | French AZERTY | German QWERTZ |
|---|---|---|---|---|
| Key 1 | `0x1D` | `z` | `w` | `y` |
| Key 2 | `0x1B` | `x` | `x` | `x` |
| Key 3 (V3 modules) | `0x06` | `c` | `c` | `c` |
| Touchscreen (Quick Retry) | `0x35` | `` ` `` | `²` | `^` |

Nothing is broken. Fix it in either place:

- **In osu!** (quickest): open the key binding settings and press each OPad key to bind whatever character your layout gives.
- **In the OPad app:** open **Settings** → **⌨ Keypad**, type the keys you want in **KEY 1** and **KEY 2** (and **KEY 3** on a V3 module), and click **Save settings**. The pad stores them, so they follow it to any PC. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/config/device_config.c -->

## 2. Linux: tosu cannot read osu! (Yama ptrace_scope)

**Symptom:** tosu runs and `opadctl status` shows `tosu (osu!): Active`, but the pad shows no map or play data.

tosu reads osu!'s memory. On Linux, the Yama security module (`/proc/sys/kernel/yama/ptrace_scope`) blocks that when the value is above 0, because osu! is not a child of tosu. A tosu binary with the `cap_sys_ptrace` capability is exempt at scopes 1 and 2. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

First, find out which case you are in:

```bash
opadctl setup
```

It prints your `ptrace_scope`, whether tosu has the capability, and the exact command to run. The Diagnostics page in `opad-gui` says the same. <!-- src: /desktop/cli/src/main.rs#run_setup -->

### Fix A: give tosu the capability (packages and `~/.local` installs)

The `.deb`, `.rpm` and Arch release package already do this when they install. If it was lost, or for a `make install-user` install, run the line for your install: <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install --> <!-- src: /Makefile#install-user -->

```bash
# .deb, .rpm or Arch release package
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# make install-user
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```

This keeps the system-wide protection on and exempts only tosu.

### Fix B: lower ptrace_scope (AppImage and source-built Arch package)

The capability cannot work in two cases, and `opadctl setup` tells you so: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access --> <!-- src: /packaging/linux/arch/PKGBUILD -->

- the **AppImage**, because its files are mounted `nosuid`, which ignores file capabilities;
- the Arch package built from the repository's **source `PKGBUILD`**, which runs tosu with the system `node`; giving `node` the capability would give it to every Node program.

There, lower the scope to 0. Until the next reboot:

```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
```

To keep it after reboots, put this line in `/etc/sysctl.d/10-ptrace.conf`:

```ini
kernel.yama.ptrace_scope = 0
```

and apply it with `sudo sysctl --system`.

> [!NOTE]
> At `ptrace_scope` 3 nothing can read another process's memory, with or without the capability, and the value cannot be lowered until you reboot. Set it to 0 in `/etc/sysctl.d/` and reboot. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### tosu exits at once with `GLIBC_2.xx not found`

The tosu that OPad bundles is rebuilt to run on glibc 2.28 and newer, so it starts on every supported distribution. Upstream tosu's own Linux build needs glibc 2.38, which Ubuntu 22.04, Debian 12 and RHEL 9 do not have. You only see this error with a tosu you supplied yourself (`OPAD_TOSU_PATH`) or one built with `make tosu TOSU_PORTABLE=0`. Use the bundled one instead. <!-- src: /Makefile#TOSU_PORTABLE -->

## 3. Linux: USB permissions and the serial port

### The daemon does not find the pad, or reports `Permission denied`

Your user needs the udev rule `70-opad.rules` to open the pad's serial port (`/dev/ttyACM*`), its USB device and its input device (used by the app's switch tester). The rule also stops ModemManager probing the pad. <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/gui/src/diagnostics.rs#open_opad_evdev -->

1. Check it:

   ```bash
   opadctl setup
   ```

   It says whether the rule is installed (in `/etc/udev/rules.d/` or `/usr/lib/udev/rules.d/`) and whether it matches this version. <!-- src: /desktop/cli/src/main.rs#run_setup -->
2. If it is missing:
   - **`.deb`, `.rpm` or Arch package:** reinstall the package; it ships the rule in `/usr/lib/udev/rules.d/`. <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb -->
   - **AppImage:** accept the prompt when the app starts, or run `./OPad-Linux-<version>.AppImage install-udev`. <!-- src: /packaging/linux/appimage/AppRun -->
   - **Source checkout:**

     ```bash
     sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
     sudo udevadm control --reload-rules && sudo udevadm trigger
     ```
3. Unplug the pad and plug it back in.

Keep the file name starting with `70-`. The rule grants access with systemd's `uaccess` tag, which only works in a rule that sorts before `73-seat-late.rules`. <!-- src: /packaging/linux/udev/70-opad.rules -->

The rule covers the app's USB ID `303a:8415`, the ESP32-S3 bootloader's `303a:1001`, and `303a:4001` used by release-candidate firmware. <!-- src: /packaging/linux/udev/70-opad.rules -->

### The serial port is busy

While `opad-daemon` runs, it holds the pad's serial port (`/dev/ttyACM0` on Linux, a `COM` port on Windows). `opadctl flash`, `opadctl bootloader` and the app's firmware update ask the daemon to let go of it for you. For any other serial tool, stop the daemon first. <!-- src: /desktop/cli/src/main.rs#prepare_flash -->

### A key does nothing after a pin change

Compare the GPIO label printed on the board next to the wire with **Key Pins** in `opadctl status`. The app's pin list names schematic header positions such as `GPIO14 (P2 pin 11)`, which are easy to count wrong on the header. See [Changing key pins](guides/change-key-pins.md). <!-- src: /AGENTS.md#6. Hardware and debugging facts --> <!-- src: /desktop/crates/opad-model/src/lib.rs#KEY_PINS -->

## 4. Windows: SmartScreen and antivirus

### "Windows protected your PC"

The installer is not code-signed yet, so SmartScreen warns about it. <!-- src: /README.md -->

1. Click **More info**.
2. Click **Run anyway**.

If you want to be sure the file is the real one, compare it first with the SHA-256 checksum shown next to it on the GitHub release page.

You never need to install a driver: Windows uses its own `hidclass.sys` and `usbser.sys`, and the pad's serial port shows up in Device Manager as **OPad (COMx)**. <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_ms_os_20_desc -->

### Live osu! data stops: antivirus and `tosu.exe`

tosu reads osu!'s memory, and some antivirus products flag or quarantine it for that. If the pad stops showing map data:

1. Check your antivirus quarantine (in Windows Security: **Protection history**).
2. Restore `tosu.exe` and add an exclusion for `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->

## 5. Firmware recovery

**Symptom:** the pad stopped responding, or no longer shows up as OPad (`303a:8415`).

You never need the board's BOOT or RESET buttons.

1. Run the recovery command with a folder that holds the firmware: an ESP-IDF build folder, or a folder with a firmware release's `.bin` files: <!-- src: /desktop/cli/src/main.rs#Commands::Recover -->

   ```bash
   opadctl recover firmware/build
   ```

2. When it asks, unplug the pad's USB cable and plug it back in. `opadctl` catches the ESP32-S3's built-in bootloader in the moment it appears at power-on (`303a:1001`) and writes the full firmware over USB. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->

[Recovery §7](recovery.md#7-reflashing-and-unbinding-a-pad) has the details, plus what happens when an update is interrupted and how to reflash or unbind a pad.
