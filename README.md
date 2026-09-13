# osu!pad

**Low-Latency ESP32-S3 osu! Keypad & Telemetry Display**

A deterministic, ultra-low-latency two-key mechanical keypad and live telemetry HUD designed for competitive osu!lazer gameplay on Linux *(Windows support planned after Linux v1.0)*.

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
|     osupad-gui     |   |      osupad-daemon      |
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
- `osupad-daemon`: Background service.
- `osupad-gui`: Graphical configuration, layout designer, and system tray.
- `osupadctl`: Command-line management tool.

### 3. Firmware Build (ESP-IDF v5.5.2)
```bash
cd firmware
idf.py set-target esp32s3
idf.py build
```

---

## ⚡ Flashing the Firmware

### Option A: Via `osupadctl` (Recommended)
`osupadctl` integrates native USB flashing via `espflash` and requires no external toolchain:
```bash
osupadctl flash firmware/build/osupad-firmware.bin
```

### Option B: Via ESP-IDF
```bash
cd firmware
idf.py -p /dev/ttyACM0 flash
```

---

## 🚀 Linux Setup & Installation

### 1. Udev Rules
Allow non-root user access to USB CDC and ROM bootloader devices:
```bash
sudo cp packaging/linux/udev/99-osupad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

### 2. Systemd User Service (Daemon)
Run the headless daemon automatically in your user session:
```bash
mkdir -p ~/.config/systemd/user/
cp packaging/linux/systemd-user/osupad-daemon.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now osupad-daemon.service
```

### 3. Desktop Application & Autostart
```bash
mkdir -p ~/.local/share/applications ~/.config/autostart
cp packaging/linux/desktop/osupad-gui.desktop ~/.local/share/applications/
# For background tray autostart on login:
cp packaging/linux/desktop/osupad-gui.desktop ~/.config/autostart/
```

---

## 💻 CLI Quick Reference (`osupadctl`)

```bash
# Check device, daemon, and tosu state
osupadctl status

# Trigger safe reconciliation
osupadctl sync

# Export portable JSON backup
osupadctl export backup.json

# Import validated backup
osupadctl import backup.json

# Flash firmware image directly over USB
osupadctl flash firmware/build/osupad-firmware.bin

# Stream real-time diagnostic logs
osupadctl monitor
```

---

## 📄 Documentation
- [System Architecture](docs/architecture.md)
- [USB Framing & Protocol](docs/protocol.md)
- [Counter Reconciliation & Disaster Recovery](docs/recovery.md)
- [Latency Testing Methodology](docs/latency-testing.md)
- [Testing & Hardware Checklist](docs/testing-checklist.md)
- [Technical Specification](osupad_technical_spec_v1.md)

---

## 📜 License
Licensed under the [MIT License](LICENSE).

