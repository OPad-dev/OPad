# OPad

**Low-Latency ESP32-S3 Rhythm Gaming Keypad & Telemetry Display**

A deterministic, ultra-low-latency mechanical keypad and live telemetry HUD designed for competitive osu!, osu!lazer, and rhythm games on **Linux and Windows 10/11**.

---

## ⚡ Core Principle: Latency Always Wins

> **"If a feature measurably worsens keyboard latency or latency jitter, the feature is reduced, deferred, frozen during gameplay, or removed."**

The keypad functions as a standalone 1000 Hz USB HID keyboard out of the box with zero host software running. Auxiliary features (LVGL telemetry, SQLite persistence, system tray, and tosu integration) are completely isolated from the keypress critical path.

---

## 🏗️ System Architecture

```text
                      +-------------------+
                      |    osu!lazer      |
                      +---------+---------+
                                | (observes)
                                v
                      +-------------------+
                      |       tosu        |
                      +---------+---------+
                                | (WebSocket v2)
                                v
+--------------------+   +-------------------------+
|      opad-gui      |   |       opad-daemon       |
|     (Rust/iced)    |<->|         (Rust)          |
|  - System Tray     |IPC|  - State Machine        |
|  - Layout Designer |   |  - SQLite Storage       |
|  - Configuration   |   |  - Headless Service     |
|  - Diagnostics     |   |  - tosu & Device Owner  |
+--------------------+   +------------+------------+
                                      |
                           USB CDC    |    USB HID (1000 Hz)
                                      |
                                      v
                         +-------------------------+
                         |  ESP32-S3 Touch LCD 2   |
                         |  - Eager Debounce ISR   |
                         |  - Core 0: Input & HID  |
                         |  - Core 1: LVGL & CDC   |
                         |  - ST7789 Telemetry HUD |
                         +-------------------------+
```

---

## 🔌 Hardware Target & Switch Wiring

### Supported Hardware
- **Board**: **Waveshare ESP32-S3-Touch-LCD-2** (ESP32-S3, 16MB Flash, 8MB PSRAM, 2.0" ST7789 LCD)
- **Switches**: Any two standard MX-compatible mechanical switches.

### Switch Pinout (Header P2)
Standard MX mechanical switches have no polarity. Connect each switch between its designated GPIO and ground:

| Input | Board Header & Pin | GPIO | Switch Connection |
|---|---|---|---|
| **Key 1** (Default `Z`) | **Header P2, Pin 11** | `GPIO 14` | Pin A -> Pin 11, Pin B -> GND (Pin 13) |
| **Key 2** (Default `X`) | **Header P2, Pin 12** | `GPIO 9` | Pin A -> Pin 12, Pin B -> GND (Pin 2) |
| **Ground** | **Header P2, Pin 13 / Pin 2** | `GND` | Ground Reference |

*Note: Pins 11, 12, and 13 are directly adjacent on Header P2 for simple breadboard/Dupont jumper wiring.*

These are the defaults. Each key's pin can be changed in the app under **Settings → Keys**. The list only offers header pins that work with a switch to GND:

| Header | Pins (GPIO) |
|---|---|
| **P1** | 1 (`GPIO 2`), 2 (`GPIO 4`), 3 (`GPIO 6`), 4 (`GPIO 16`), 6 (`GPIO 18`), 7 (`GPIO 21`), 8 (`GPIO 8`), 9 (`GPIO 7`), 10 (`GPIO 10`) |
| **P2** | 7 (`GPIO 15`), 8 (`GPIO 13`), 9 (`GPIO 11`), 10 (`GPIO 12`), 11 (`GPIO 14`), 12 (`GPIO 9`) |

Left out: `GPIO 19/20` (USB), `GPIO 43/44` (UART0 console), `GPIO 47/48` (touch and IMU I2C) and `GPIO 17` (pulled down on the board). Most listed pins are also wired to the camera connector, so don't use them with a camera fitted.

---

## 🛠️ Building From Source

### 1. Prerequisites (Ubuntu / Debian)
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libasound2-dev libudev-dev \
    libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev libdbus-1-dev
```

### 2. Desktop Software Suite (Rust)
```bash
cd desktop
cargo build --release
```
The compiled binaries are produced in `desktop/target/release/`:
- `opad-daemon`: Background service.
- `opad-gui`: Graphical configuration, layout designer, and system tray.
- `opadctl`: Command-line management tool.

### 3. Firmware Build (ESP-IDF v5.5.2)
```bash
cd firmware
idf.py set-target esp32s3
idf.py build
```

---

## ⚡ Flashing the Firmware

### Option A: Via `opadctl` (Recommended)
`opadctl` drives `espflash` directly and needs no shell script and no ESP-IDF
toolchain. It asks the daemon to release the serial port first, reboots the pad
into the ROM download bootloader hands-free, writes the image and reboots back
into the app — on Linux and on Windows, with the app running:

```bash
# Update the app image only (ota_0 at 0x20000)
opadctl flash firmware/build/opad-firmware.bin

# Recovery flash: bootloader + partition table + OTA data + app
opadctl flash --full firmware/build
```

It also works with no daemon running at all, which is the state a recovery
flash usually happens in.

### Option B: Via ESP-IDF
```bash
cd firmware
idf.py -p /dev/ttyACM0 flash
```

If the pad will not enter download mode or has to be returned to stock, see
[Recovery, Reflashing & Unbinding](docs/recovery.md).

---

## 🪟 Windows Setup & Installation

**The pad needs none of this to work as a keyboard.** Plug it into any Windows
10 or 11 machine and it types immediately: HID binds to the inbox
`hidclass.sys` and the CDC port to the inbox `usbser.sys`. No `.inf`, no
WinUSB, no Zadig, no install. The software below is for configuring it, the
telemetry HUD and the lifetime counters.

1. Download `opad-setup-<version>.exe` from the GitHub Releases page and run it.
2. It installs the daemon, the GUI and `opadctl`, bundles tosu, and configures
   both to start at login. There is nothing else to do.

> **SmartScreen will warn you.** The installer is **not code-signed** yet:
> OPad is applying to SignPath Foundation for free OSS signing, which
> requires the repository to be public first. Until then, Windows shows
> "Windows protected your PC" — choose **More info → Run anyway**. Check the
> download against `SHA256SUMS` on the release page if you would rather verify
> it than trust the dialog.

Uninstall from Settings → Apps. It removes everything it installed, including
the two `Run` registry values, and asks before it touches your `opad.db` —
so a reinstall keeps your lifetime counters unless you say otherwise.

**To reset a pad completely, or hand it to someone else**, see
[Recovery, Reflashing & Unbinding](docs/recovery.md). There is no unpair button
in the app, deliberately: the only unbind is a documented reflash, and it erases
the pad's lifetime counters along with the owner record.

---

## 🚀 Linux Setup & Installation

### Option A: Pre-built Packages (Recommended)

Pre-built packages are on the GitHub Releases page. The `.deb` and `.rpm` set
up everything: the udev rule, the daemon's systemd user service, tray autostart,
the bundled `espflash`, and the ptrace capability tosu needs to read osu!'s
memory (see [docs/troubleshooting.md](docs/troubleshooting.md) §2).

OPad runs on glibc 2.31 or newer (Ubuntu 20.04, Debian 11, and later). The
bundled tosu, which feeds the pad's live gameplay screen, needs glibc 2.38
(Ubuntu 24.04, Debian 13, Fedora 39, and later); see
[troubleshooting §2](docs/troubleshooting.md).

#### Debian / Ubuntu / Pop!_OS (`.deb`):
```bash
sudo apt install ./opad_<version>_amd64.deb
# Or via dpkg:
sudo dpkg -i opad_<version>_amd64.deb
```

#### Fedora / RHEL / openSUSE (`.rpm`):
```bash
sudo dnf install ./opad-<version>.x86_64.rpm
```

#### Standalone AppImage:
```bash
chmod +x opad-x86_64.AppImage
./opad-x86_64.AppImage
```
The AppImage installs nothing system-wide. On first run it offers to install
the udev rule (it asks for your password), or run
`./opad-x86_64.AppImage install-udev` yourself. It cannot give tosu the ptrace
capability; `opadctl setup` and the Diagnostics page explain the alternative.

#### Arch Linux:
The repository's `PKGBUILD` builds from source (tosu runs on the system
`nodejs`, so see troubleshooting §2 about ptrace):
```bash
makepkg -si
```

---

### Option B: Installing From Source

#### 1. System-wide Install:
```bash
make
sudo make install
```

#### 2. User-local Install (`~/.local`):
```bash
make
make install-user
systemctl --user daemon-reload
systemctl --user enable --now opad-daemon.service
```

#### 3. Manual Udev Rules:
If not installing from package, allow non-root user access to USB CDC and ROM bootloader devices:
```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

---

## 💻 CLI Quick Reference (`opadctl`)

```bash
# Check device, daemon, and tosu state
opadctl status

# Trigger safe reconciliation
opadctl sync

# Export portable JSON backup
opadctl export backup.json

# Import validated backup
opadctl import backup.json

# Flash firmware image directly over USB
opadctl flash firmware/build/opad-firmware.bin

# Recovery flash (bootloader, partition table, OTA data and app)
opadctl flash --full firmware/build

# Reboot the pad into the ROM download bootloader and leave it there
opadctl bootloader

# Stream real-time diagnostic logs
opadctl monitor
```

---

## 📄 Documentation
- [Documentation Hub & Wiki](docs/README.md)
- [System Architecture](docs/architecture.md)
- [Project Roadmap & Hardware Variants](docs/roadmap.md)
- [OPad on Windows — as built](docs/windows-portability.md)
- [USB Framing & Protocol](docs/protocol.md)
- [Counter Reconciliation, Recovery & Unbinding](docs/recovery.md) — including the reflash / unbind path
- [Latency Testing Methodology](docs/latency-testing.md)
- [Testing & Hardware Checklist](docs/testing-checklist.md)
- [Technical Specification v1](docs/specs/technical-spec-v1.md)
- [Rapid Trigger v2 Specification](docs/specs/v2-rapid-trigger.md)


---

## 📦 Third-Party Software

OPad bundles or interacts with the following third-party software:

### tosu
- **Project**: [tosu](https://github.com/KotRikD/tosu)
- **Author**: Mikhail Babynichev and the tosu contributors
- **License**: [GNU Lesser General Public License v3.0 (LGPL-3.0)](licenses/tosu/LICENSE)
- **Redistribution Notice**: See [licenses/tosu/NOTICE](licenses/tosu/NOTICE)
- **User Replacement Rights**: Under the terms of LGPL-3.0, users are entitled to replace the bundled tosu binary with their own version. You can point the `OPAD_TOSU_PATH` (or legacy `OSUPAD_TOSU_PATH`) environment variable to a custom tosu executable, or configure an external path in the OPad desktop GUI (**Settings → tosu**).

---

## ⚖️ Trademark Disclaimer
OPad is an independent open-source hardware and software project. OPad is not affiliated with, endorsed by, or sponsored by ppy Pty Ltd or osu!. "osu!" is a registered trademark of ppy Pty Ltd.

---

## 📜 License
Everything original to OPad is licensed under the [MIT License](LICENSE): the firmware, the desktop app (daemon, GUI, CLI), the PCB designs and footprints, the enclosure CAD, and the documentation.

Third-party components keep their own open licenses and ship with their notices:
- [tosu](licenses/tosu/LICENSE) (LGPL-3.0), built from upstream source and bundled with the desktop packages, see the section above. Not stored in this repository.
- [Montserrat](desktop/gui/assets/fonts/Montserrat-OFL.txt) fonts embedded in the GUI (SIL Open Font License 1.1).
- [nanopb](firmware/main/protocol/nanopb/LICENSE.txt) protobuf runtime vendored in the firmware (zlib license).

Libraries pulled in at build time (Rust crates, ESP-IDF components such as LVGL and TinyUSB) are not stored here and are used under their own permissive licenses.


