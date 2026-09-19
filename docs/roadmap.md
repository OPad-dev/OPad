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

---

## 🪨 Milestone: Heavy Ballast & Deskmat Anti-Slip Grip (Case V2)

### 1. Increased Weight & Mass
- **Requirement**: Add heavy ballast/weight to the enclosure so high-BPM streaming (240+ BPM) doesn't cause the keypad to budge or lift.
- **Design Approaches**:
  - Internal ballast chambers in the bottom plate for standard hardware weights (e.g. steel M8/M10 nuts, lead sinkers, or steel coins/washers).
  - High-density solid ballast base option (100% solid perimeter/infill base floor).

### 2. Deskmat Anti-Slip High-Traction Bottom Pattern
- **Requirement**: Molded textured pattern on the bottom plate that physically locks into cloth and hybrid mousepads/deskmats.
- **Design Approaches**:
  - Molded herringbone / knurled diamond pyramid micro-tread pattern across the bottom surface.
  - Dual hybrid system: Molded textured tooth grid + perimeter recess pockets for silicone/rubber grip pads.

---

## 🎹 Milestone: Multi-Key Catalog Expansion (3-Key Standard & 4k–10k Mania)

### 1. Catalog Variants

| Variant | Target Game Mode | Key Configuration | Display Type | Sensing Options |
| :--- | :--- | :--- | :--- | :--- |
| **osu!pad Standard (2k)** | osu! Standard / Taiko | 2× Primary Keys | 2.0" Touch IPS (320×240) | MX Hot-Swap / Hall Effect (Rapid Trigger) |
| **osu!pad Standard Plus (3k)** | osu! Standard | 2× Hall Effect Keys + 1× Quick Retry / Reset key + Aux Tactile Buttons | 2.0" Touch IPS (320×240) | 2× Hall Effect + 1× Mechanical + Tactile Aux |
| **osu!pad Mania (4k)** | 4-Key Mania, Quaver, Etterna | 4× Linear Keys | 3.4" or 4.3" Bar Display | Multi-channel Hall Effect (Rapid Trigger) |
| **osu!pad Mania (5k / 6k / 7k)** | 7K Mania, BMS, O2Jam | 5, 6, or 7× Keys + Spacebar option | Ultra-wide Bar Display (e.g. 480×120 / 800×320) | Multi-channel Hall Effect (Rapid Trigger) |
| **osu!pad Mania Pro (10k)** | 10K Mania, Pop'n, Dual-Hand | 10× Keys | Ultra-wide Bar Display | Multi-channel Hall Effect via on-board SPI ADC |

### 2. 3-Key Standard Layout Details
- **Primary Keys (Z, X)**: Magnetic Hall Effect switches with continuous sub-0.1 mm Rapid Trigger.
- **Quick Retry / Reset Key (`~` or `Esc`)**: Dedicated mechanical switch or low-profile tactile switch. Does not require Hall Effect—simple GPIO interrupt with eager zero-overhead debouncing.
- **Auxiliary Controls**: Rotary encoder knob or side tactile buttons for Master/Music Volume, Song Select scrolling, and Pause/Menu skip.

---

## 🔌 Milestone: Modular Swappable Switch PCBs & Universal Bus

The swappable switch PCB philosophy established in V1 is extended to the entire product catalog:

### 1. The Architecture: ADC Chip on the Keys PCB
Rather than routing delicate analog signals across multi-wire ribbon cables, **the analog sensing chip lives directly on the switch daughterboard**:

```text
┌────────────────────────────────────────────────────────┐
│                   MAIN CONTROLLER (Brain)              │
│       ESP32-S3 (or RP2350)  +  Screen  +  USB-C        │
└──────────────────────────┬─────────────────────────────┘
                           │
                           │ Universal 8-wire JST-SH Cable
                           │ (3V3, GND, SCK, MOSI, MISO, CS, INT, ID)
                           ▼
┌────────────────────────────────────────────────────────┐
│              SWAPPABLE SWITCH DAUGHTERBOARD            │
│                                                        │
│  [Hall 1] ──┐                                          │
│  [Hall 2] ──┤                                          │
│  [Hall 3] ──┼──> [On-Board SPI ADC] ──> SPI Bus out   │
│  [Hall ...] ┘    (e.g. TI ADS7953)                     │
│                                                        │
│  [ID Resistor Divider] ────────────────> ID Net out    │
└────────────────────────────────────────────────────────┘
```

#### Why This Architecture Wins:
1. **Pristine Signal Integrity (Noise Immunity)**:
   Hall effect analog voltages travel only 2–4 mm on the PCB directly into the ADC. No electrical noise from the screen or USB power rail can induce chatter across the cable.
2. **Universal Cable & Connector**:
   The exact same 8-pin connector carries power, SPI, interrupt, and identification whether the daughterboard has 2 keys, 4 keys, or 10 keys.
3. **Motherboard Reusability**:
   The main controller board needs zero redesign to support 10 keys. It simply polls the SPI bus.

### 2. Automatic Hardware Auto-Detection
The mainboard automatically identifies the connected daughterboard at boot:

* **Hardware ID Net (GPIO8 ADC)**:
  Each switch PCB populates a resistor divider yielding a unique voltage:
  * `0.30 V`: 2-Key MX (V1 Mechanical)
  * `1.06 V`: 2-Key Hall Effect (Analog)
  * `1.40 V`: 3-Key Standard (2 Hall + 1 Reset)
  * `1.80 V`: 4-Key Mania (Hall SPI)
  * `2.20 V`: 7-Key Mania (Hall SPI)
  * `2.60 V`: 10-Key Mania (Hall SPI)
* **Digital SPI Handshake**:
  The firmware queries the on-board chip register (`WHO_AM_I` / device ID) to verify channel count.
* **Dynamic Software Configuration**:
  - **USB HID**: Automatically configures the USB descriptor to report 2, 3, 4, 7, or 10 keys.
  - **Display UI**: Automatically formats the LCD layout (e.g. 2 live travel bars vs 10 mania lane meters).
  - **Desktop App**: Automatically syncs calibration sliders per key without requiring user intervention.

---

## ⚡ Milestone: High-Speed Sampling & Latency Analysis

### 1. Scaling to 10 Keys Without Latency Loss
- **Compute Load**:
  The Rapid Trigger calculation (`rt_step`) requires ~15–20 basic CPU instructions per key.
  10 keys sampled at **8,000 Hz** = 80,000 evaluations per second. On a 240 MHz ESP32-S3 or 150 MHz RP2350, this consumes **< 2% of a single CPU core**.
- **Sampling Bandwidth**:
  Reading 10 channels at 8,000 Hz requires 80 kSPS. A high-speed SPI ADC (e.g. TI ADS7953 @ 1 MSPS) reads all 10 keys in **~10 microseconds**.
- **Core Isolation Invariant**:
  - **Core 0**: 100% dedicated to Key Sampling $\to$ Rapid Trigger Engine $\to$ USB HID queue. Zero screen rendering, zero allocations, zero blocking.
  - **Core 1**: Dedicated to Display rendering (LVGL / LCD DMA), telemetry, and PC serial protocol.
  - *Result*: Expanding from 2 keys to 10 keys or driving an ultra-wide bar display adds **0.000 ms** to key latency.

### 2. Internal Scan Rate vs USB Polling Rate
* **Internal Sensor Scan Rate (4,000 – 8,000 Hz)**:
  Critical for Rapid Trigger to detect sub-millimeter finger reversals while traveling at $500\text{ mm/s}$.
* **USB Polling Rate (1,000 Hz)**:
  Optimal host communication. 1,000 Hz USB offers rock-solid 0.5 ms average transfer without the CPU interrupt overhead, DPC latency spikes, or rhythm-game frame drops caused by 8,000 Hz USB flooding on Windows.

---

## 🧠 Milestone: Adaptive Debounce & Velocity-Adaptive Hysteresis

### 1. Mechanical Switches (Adaptive Debounce Learner)
- **Noise Tracking**: Firmware monitors edge chatter inside the lockout window. If contact bounce creeps close to the boundary (e.g. switch wear or hand vibration), the lockout automatically widens from 5 ms $\to$ 8 ms $\to$ 10 ms.
- **Asymmetric Debounce**: Ultra-short press lockout (`2,000 µs`) for immediate re-activation readiness + wider release lockout (`8,000 µs`) to absorb leaf resonance.
- **Eager Zero-Latency Actuation**: First edge always triggers the HID press packet at microsecond zero.

### 2. Hall Effect (Velocity-Adaptive Hysteresis)
- **Hovering / Idle State**: Widened deadzone (e.g. `0.20 mm`) to prevent electrical ADC noise or hand tremors from causing phantom double-hits.
- **Stream / Vibration State**: As velocity ($\Delta x / \Delta t$) spikes during rapid tapping, the threshold tightens down to `0.05 mm` for hyper-responsive re-activation.
- **Auto-Calibrating Noise Floor**: Continuously tracks rest-position ADC variance to guarantee trigger sensitivity remains above the thermal noise floor.
