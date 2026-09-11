# osu!pad

**Low-Latency ESP32-S3 osu! Keypad & Telemetry Display**

A deterministic, ultra-low-latency two-key mechanical keypad and live telemetry HUD designed for competitive osu!lazer gameplay.

---

## ⚡ Core Principle: Latency Always Wins

> **If a feature measurably worsens keyboard latency or latency jitter, the feature is reduced, deferred, frozen during gameplay, or removed.**

The keypad functions as a standalone 1000 Hz USB HID keyboard out of the box with zero host software running. Auxiliary features (LCD telemetry, SQLite persistence, system tray, and tosu integration) are completely isolated from the keypress critical path.

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
|  - Configuration   |IPC|  - State Machine        |
|  - Diagnostics     |   |  - SQLite Storage       |
|  - Backup Restore  |   |  - System Tray          |
+--------------------+   |  - tosu & Device Owner  |
                         +------------+------------+
                                      |
                           USB CDC    |    USB HID (1000 Hz)
                                      |
                                      v
                         +-------------------------+
                         |  ESP32-S3 Touch LCD 2   |
                         |  - Eager Debounce ISR   |
                         |  - USB Composite Device |
                         |  - RAM Lifetime Press   |
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

---

## 📦 Host Software Suite

The host stack is built with Rust and split into modular crates:
- `osupad-daemon`: Background daemon owning SQLite, tosu, and the USB CDC device.
- `osupad-gui`: Modern desktop configuration and live diagnostic monitor written in `iced`.
- `osupadctl`: Command-line management tool for automation and backups.

### Building
```bash
cd desktop
cargo build --release
```

### CLI Quick Reference
```bash
# Check device and daemon state
osupadctl status

# Trigger safe synchronization
osupadctl sync

# Export portable JSON backup
osupadctl export backup.json

# Import validated backup
osupadctl import backup.json

# Stream live device & host logs
osupadctl monitor
```

---

## 🚀 Linux Setup & Installation

1. **Udev Rules** (allow non-root access to USB CDC / serial):
   ```bash
   sudo cp packaging/linux/udev/99-osupad.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

2. **Systemd User Service**:
   ```bash
   mkdir -p ~/.config/systemd/user/
   cp packaging/linux/systemd-user/osupad-daemon.service ~/.config/systemd/user/
   systemctl --user enable --now osupad-daemon.service
   ```

---

## 📄 Documentation
- [Architecture & State Machine](docs/architecture.md)
- [USB Framing & Protobuf Protocol](docs/protocol.md)
- [Disaster Recovery & Counter Reconciliation](docs/recovery.md)
- [Latency Regression Testing Gate](docs/latency-testing.md)
- [Formal Technical Specification](osupad_technical_spec_v1.md)

## 📜 License
Licensed under the [MIT License](LICENSE).
