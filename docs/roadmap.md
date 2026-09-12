# osu!pad Future Roadmap

This document outlines planned future hardware and firmware extensions for the osu!pad ecosystem.

---

## 📡 Milestone: ESP-NOW Ultra-Low-Latency Wireless Dongle (Dual-Mode)

### 1. Concept & Rationale
While standard Bluetooth LE (BLE) introduces 7.5–15 ms latency and Wi-Fi introduces jitter spikes from home routers, **ESP-NOW** operates via raw 2.4 GHz 802.11 vendor action frames directly between two ESP32 devices without a router.
- **Latency**: ~1.0 ms peer-to-peer transmission time.
- **Competitiveness**: Matches the latency profile of commercial 2.4 GHz wireless gaming peripherals (e.g. Logitech Lightspeed / Razer Hyperspeed).
- **Driverless Host Experience**: The USB receiver dongle registers as a standard 1000 Hz USB HID keyboard on the PC.

---

### 2. Architecture

```text
┌────────────────────────────────────────┐                      ┌────────────────────────────────────────┐
│         osu!pad (Transmitter)          │                      │          USB Dongle (Receiver)         │
│                                        │       ESP-NOW        │                                        │
│   [2x MX Keys] ──> ESP32-S3 (Pad)      ├─ (1ms Peer-to-Peer) ─┤──> ESP32-S3 (Dongle) ──> [Host PC]     │
│   [3.7V LiPo]                          │                      │    (Hardware 1000Hz USB HID Keyboard)  │
└───────────────────┬────────────────────┘                      └────────────────────────────────────────┘
                    │
                    │ (Plugged in via USB-C)
                    └───────────────────────────────────────────────────> Direct 1000Hz Wired USB to [Host PC]
```

---

### 3. Key Technical Specifications

1. **Hardware**:
   - **Keypad**: Waveshare ESP32-S3-Touch-LCD-2 with flat 3.7V LiPo battery (housed in the lower cavity).
   - **Dongle**: Second ESP32-S3 dev board or USB stick dongle plugged into the host PC.

2. **Automatic Dual-Mode Switch**:
   - **Wired Mode**: When USB-C is connected to the pad, VBUS is detected (`tud_mounted()`). The pad transmits keystrokes over hardware USB HID at 1000 Hz and charges the internal LiPo battery. ESP-NOW radio is put into low-power idle.
   - **Wireless Mode**: When unplugged, the pad immediately activates ESP-NOW and transmits encrypted key event frames (press/release timestamps, key IDs, battery level) to the dongle receiver.

3. **Dongle Receiver Firmware**:
   - Runs TinyUSB composite device (1000 Hz USB HID Keyboard + CDC Telemetry/Configuration channel).
   - Instantaneous ISR-driven key event forwarding.
   - Telemetry passthrough (forwards tosu stats and clock updates from PC to pad display over ESP-NOW).

4. **Pairing**:
   - Out-of-the-box hardcoded default channel/MAC pairing.
   - Optional touchscreen "Pair New Dongle" utility on the keypad's LCD.
