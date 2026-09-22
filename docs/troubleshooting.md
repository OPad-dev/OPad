# OPad Troubleshooting Guide

This guide covers common questions and troubleshooting steps for the OPad keypad, desktop integration, key mappings, and overlay telemetry on Linux and Windows.

---

## 1. Keyboard Layouts & Scancode Mappings (QWERTY, AZERTY, QWERTZ)

### How Key Scancodes Work in OPad
OPad is a 1000 Hz USB HID composite keyboard. USB HID keyboards do **not** transmit localized letters (such as "Z" or "W"); they transmit **USB HID Key Usage IDs** (hardware scancodes). The operating system's keyboard layout then translates these usage IDs into the corresponding character for your language.

Default firmware scancodes:
* **Key 1**: `0x1D` (`HID_USAGE_KEY_KEYBOARD_Z`)
* **Key 2**: `0x1B` (`HID_USAGE_KEY_KEYBOARD_X`)

### Behavior on Non-QWERTY Layouts

| Operating System Layout | Physical Key 1 Sends | OS Interprets As | Recommended Action |
|---|---|---|---|
| **US / UK QWERTY** | Usage `0x1D` | **`Z`** | None needed (default `Z` / `X` in osu!) |
| **French AZERTY** | Usage `0x1D` | **`W`** | In osu!, bind Left Key to `W` (or rebind in OPad) |
| **German QWERTZ** | Usage `0x1D` | **`Y`** | In osu!, bind Left Key to `Y` (or rebind in OPad) |
| **Dvorak / Colemak** | Usage `0x1D` | Layout-dependent | Rebind in osu! or OPad |

### Adjusting Key Mappings
You have two options:
1. **In osu! / osu!lazer (Fastest)**: Open osu! **Options → Key Bindings** and tap Key 1 and Key 2 on your OPad to bind whatever scancode the pad sends.
2. **In OPad**: open the desktop app, **Settings → Keys**, and choose the
   letter each key sends. The pad stores it, so it holds on every PC.

---

## 2. Linux Yama ptrace Scope & tosu Memory Telemetry

### The Issue
OPad displays real-time accuracy, combo, unstable rate (UR), and beatmap details on its built-in display using `tosu`. Under Linux, `tosu` reads the memory of the running `osu!` process (whether running via Wine/Proton or native Linux `osu!lazer`).

Modern Linux distributions (Ubuntu, Debian, Fedora, Arch) protect process memory using the **Yama Linux Security Module** via `/proc/sys/kernel/yama/ptrace_scope`:
* **Scope 0**: Classic ptrace permissions (processes with the same UID can inspect each other).
* **Scope 1 (Default on Ubuntu/Debian)**: Restricted ptrace (processes can only ptrace direct children).
* **Scope 2**: Admin-only ptrace (only processes with `CAP_SYS_PTRACE` can inspect memory).
* **Scope 3**: No ptrace allowed until reboot.

When `ptrace_scope` is set to `1` or higher, `tosu` cannot attach to `osu!` and live telemetry on the OPad screen will remain paused or show zero.

### Solution 1: File Capability (Recommended)
Granting `CAP_SYS_PTRACE` specifically to the `tosu` binary allows it to read process memory without weakening system-wide security:

```bash
# If installed via package:
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# If installed locally (~/.local):
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```
> [!NOTE]
> The `.deb` and `.rpm` packages apply this capability when they install. The
> **AppImage** cannot: it runs from a `nosuid` mount, which ignores file
> capabilities. The **Arch package** runs tosu with the system `node`, and
> giving `node` this capability would hand it to every Node program. On those
> two, use Solution 2. `opadctl setup` and the Diagnostics page tell you which
> case you are in.

### Solution 2: System-wide ptrace Scope
If you prefer to allow ptrace across your user session:

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

---

### tosu exits at once with a `GLIBC_2.xx not found` error
Upstream tosu's prebuilt pp calculator (`@tosuapp/lazer-calculator`) needs
glibc 2.38, so an upstream tosu build fails to start on Ubuntu 22.04, Debian 12
and RHEL 9. OPad's release packages rebuild it (and tosu's own native addon)
for glibc 2.28, so the bundled tosu runs on every supported distribution. You
will only see this with a tosu built another way: your own (`OPAD_TOSU_PATH`),
or `make tosu TOSU_PORTABLE=0`.

## 3. USB Permissions & Serial Port Access (Linux)

### Symptom: `Permission denied (os error 13)` or Device Not Detected
On Linux, standard users do not have access to raw USB serial devices (`/dev/ttyACM*`) or input nodes by default.

### Fix
The `.deb` and `.rpm` install the rule. The AppImage offers an **Install**
button on first run (or run `./opad-x86_64.AppImage install-udev`). From a
source checkout:
```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```
The rule also lets the Diagnostics page read the pad's key events.

### ModemManager Interference
ModemManager probes new serial devices by sending `AT` Hayes commands. This can corrupt CDC protocol handshakes. The official `70-opad.rules` rule includes:
```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
```
Ensure you are using the latest `70-opad.rules`.

---

## 4. Windows SmartScreen & Permissions

### Windows Defender SmartScreen
The OPad installer (`opad-setup.exe`) is built from this repository by the
release workflow on GitHub Actions, and is not code-signed yet. If Windows displays "Windows protected your PC":
1. Click **More info**.
2. Click **Run anyway**.
3. You can verify the installer checksum against `SHA256SUMS` published on the GitHub Releases page.

### Antivirus False Positives on `tosu.exe`
Because `tosu` reads the virtual memory of another process (`osu!.exe`), certain overly aggressive antivirus heuristics may flag or quarantine `tosu.exe`. If telemetry stops updating on Windows:
1. Check your Antivirus Quarantine / Protection History.
2. Whitelist `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`.

---

## 5. Firmware Recovery & Re-flashing

If the pad becomes unresponsive, corrupted, or needs a complete factory unbind:
1. Unplug the USB cable.
2. Hold down the **BOOT** button on the ESP32-S3 board.
3. Plug in the USB cable while holding BOOT, then release after 2 seconds.
4. Run recovery flash:
   ```bash
   opadctl flash --full firmware/build
   ```
See [docs/recovery.md](recovery.md) for full hardware recovery steps.
