---
title: Troubleshooting Guide
description: Common issues with OPad, keyboard layouts, Linux Yama ptrace for tosu, USB permissions, Windows SmartScreen, and firmware recovery.
sidebar:
  order: 3
---

This guide covers common issues with OPad key mappings, `tosu` telemetry, USB serial permissions, Windows SmartScreen/antivirus prompts, and firmware recovery.

---

## 1. Keyboard Layouts & Scancode Mappings (QWERTY, AZERTY, QWERTZ)

### How Key Scancodes Work in OPad
OPad is a 1000 Hz USB HID composite keyboard. USB HID keyboards transmit **USB HID Keyboard Usage IDs** (hardware scancodes), not localized letters; your operating system's keyboard layout translates each usage ID into a character. <!-- src: /firmware/main/usb/usb_descriptors.c --> <!-- src: /firmware/main/usb/usb_hid.c -->

Default firmware scancodes: <!-- src: /firmware/main/usb/usb_hid.c -->
- **Key 1**: `0x1D` (`Z` on US QWERTY)
- **Key 2**: `0x1B` (`X` on US QWERTY)
- **Key 3** (3-key V3 modules): `0x06` (`C` on US QWERTY)
- **Touchscreen Quick Retry**: `0x35` (`` ` `` / `~` on US QWERTY)

### Behavior on Non-QWERTY Layouts

| Operating System Layout | Physical Key 1 Sends | OS Interprets As | Recommended Action |
|---|---|---|---|
| **US / UK QWERTY** | Usage `0x1D` | **`Z`** | None needed (default `Z` / `X` in osu!) |
| **French AZERTY** | Usage `0x1D` | **`W`** | In osu!, bind Left Key to `W`, or rebind Key 1 in OPad |
| **German QWERTZ** | Usage `0x1D` | **`Y`** | In osu!, bind Left Key to `Y`, or rebind Key 1 in OPad |
| **Dvorak / Colemak** | Usage `0x1D` | Layout-dependent | Rebind in osu! or in OPad |

### Adjusting Key Mappings
1. **In osu! / osu!lazer (fastest)**: Open osu! **Options -> Key Bindings** and tap Key 1 and Key 2 on your OPad to bind whatever character your OS layout produces.
2. **In `opad-gui`**: Open **Settings** -> **⌨ Keypad**, edit **KEY 1** / **KEY 2** (and **KEY 3** on 3-key V3 pads), and click **Save settings**. The keycodes are stored in pad NVS so they persist across PCs. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/config/device_config.c -->

---

## 2. Linux Yama ptrace Scope & tosu Memory Telemetry

### The Issue
OPad displays live accuracy, combo, unstable rate (UR), and beatmap telemetry on its screen via `tosu` (`ws://127.0.0.1:24050/websocket/v2`). On Linux, `tosu` reads the memory of the running `osu!` process (Wine/Proton or native `osu!lazer`). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT -->

Linux distributions with the **Yama Linux Security Module** control process memory inspection through `/proc/sys/kernel/yama/ptrace_scope`: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->
- **Scope 0**: Processes running as the same UID can inspect each other.
- **Scope 1 (default on Ubuntu/Debian)**: Processes can only `ptrace` direct children (`osu!` is not a child of `tosu`).
- **Scope 2**: Only processes with `CAP_SYS_PTRACE` can inspect memory.
- **Scope 3**: No `ptrace` allowed until reboot (cannot be lowered at runtime; set `0` in `/etc/sysctl.d/` and reboot).

When `ptrace_scope` is `1` or higher and `tosu` lacks `cap_sys_ptrace`, `tosu` cannot read `osu!` memory and live telemetry stays empty. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Solution 1: File Capability (Recommended for `.deb` / `.rpm` / local ELF installs)
Grant `cap_sys_ptrace=eip` only to the `tosu` binary so system-wide `ptrace` protection stays intact: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access --> <!-- src: /packaging/linux/deb/postinst -->

```bash
# If installed via .deb or .rpm package:
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# If installed locally (~/.local):
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```

> [!NOTE]
> The `.deb` and `.rpm` packages apply `setcap cap_sys_ptrace=eip` automatically on install. The **AppImage** cannot use file capabilities because its FUSE/squashfs mount is `nosuid`, and the **Arch package** runs `tosu` via the system `node` executable (giving `node` the capability would grant it to every Node program). On those two, use Solution 2. Both `opadctl setup` and the `opad-gui` Diagnostics page report which case applies to your install. <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Solution 2: System-Wide ptrace Scope
Allow same-user `ptrace` directly: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

**Temporary (until reboot):**
```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
```

**Persistent (across reboots):**
Create or edit `/etc/sysctl.d/10-ptrace.conf`:
```ini
kernel.yama.ptrace_scope = 0
```
Then apply:
```bash
sudo sysctl --system
```

### `tosu` Exits Immediately with `GLIBC_2.xx not found`
Upstream `tosu`'s prebuilt `@tosuapp/lazer-calculator` requires glibc 2.38, which fails on older distributions such as Ubuntu 22.04, Debian 12, and RHEL 9. OPad's release builds recompile it against glibc 2.28 (`TOSU_PORTABLE=1` by default in `Makefile`), so the bundled `tosu` runs on all supported distributions. You will only hit this error with a custom `tosu` (`OPAD_TOSU_PATH`) or one built with `make tosu TOSU_PORTABLE=0`. <!-- src: /Makefile --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

---

## 3. USB Permissions & Serial Port Access (Linux)

### Symptom: `Permission denied (os error 13)` or Pad Not Detected by Daemon
On Linux, standard users need a `udev` rule to access the pad's CDC serial device (`/dev/ttyACM*`), USB device node, and input node (needed by the `opad-gui` switch chatter tester). <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/gui/src/diagnostics.rs#open_opad_evdev -->

### Fix
The `.deb` and `.rpm` packages install `/etc/udev/rules.d/70-opad.rules` automatically. For the AppImage, accept the first-run prompt or run `./opad-x86_64.AppImage install-udev`. From a source checkout: <!-- src: /packaging/linux/appimage/AppRun --> <!-- src: /packaging/linux/udev/70-opad.rules -->

```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Keep the filename prefixed `70-` so it sorts before `73-seat-late.rules`, where `systemd` applies `TAG+="uaccess"` ACLs to the active user session. Run `opadctl setup` to verify the installed rule matches the embedded rule. <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/cli/src/main.rs#run_setup -->

### ModemManager Interference
ModemManager probes newly attached CDC ACM ports with `AT` commands, which can interfere with OPad's serial framing. `70-opad.rules` disables probing for both application (`303a:4001`) and ROM bootloader (`303a:1001`) PIDs: <!-- src: /packaging/linux/udev/70-opad.rules -->

```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
```

### Serial Port Busy
While `opad-daemon` is running, it holds the pad's CDC port (`/dev/ttyACM0` on Linux, `COMx` on Windows). Use `opadctl flash` (which asks the daemon to release the port automatically) or stop `opad-daemon` before opening the port with another tool. <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

### Keys Do Nothing After a Pin Change
Compare the physical GPIO silkscreen label the switch wire is connected to against **Key Pins** in `opadctl status`. The GUI pin selector labels schematic header positions (for example, `GPIO14 (P2 pin 11)`), which are easy to miscount on the board header. <!-- src: /AGENTS.md --> <!-- src: /desktop/gui/src/pages.rs#settings -->

---

## 4. Windows SmartScreen & Permissions

### Windows Defender SmartScreen
The Windows installer (`opad-setup.exe`) is built by GitHub Actions and is unsigned until code signing is enabled. If SmartScreen shows "Windows protected your PC": <!-- src: /packaging/windows/installer.iss --> <!-- src: /README.md -->
1. Click **More info**.
2. Click **Run anyway**.
3. Optionally verify the download against `SHA256SUMS` published alongside the GitHub Release.

Windows 10/11 uses built-in USB HID and CDC (`usbser.sys`) drivers with no driver install; OPad's MS OS 2.0 descriptor labels the serial port **OPad (COMx)** in Device Manager. <!-- src: /firmware/main/usb/usb_descriptors.c -->

### Antivirus False Positives on `tosu.exe`
Because `tosu.exe` reads the virtual memory of `osu!.exe`, some antivirus products flag or quarantine it. If telemetry stops updating on Windows: <!-- src: /packaging/windows/installer.iss -->
1. Check Windows Security Protection History / antivirus quarantine.
2. Restore and exclude `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->

---

## 5. Firmware Recovery & Re-flashing

If the pad becomes unresponsive or stops enumerating as OPad (`303a:4001`): <!-- src: /desktop/cli/src/main.rs#Commands::Recover --> <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->
1. Run the hands-free recovery command with an ESP-IDF build directory or unpacked firmware release:
   ```bash
   opadctl recover firmware/build
   ```
2. Unplug and replug the USB cable when prompted. `opadctl` catches the ESP32-S3 ROM bootloader (`303a:1001`) at power-on and flashes the full image set over USB.
3. Never press the physical BOOT or RESET buttons.

See [Recovery, Reflashing & Unbinding](recovery.md) for complete recovery, OTA rollback, counter reconciliation, and unbinding procedures.
