# OPad Future Roadmap

This document outlines planned future hardware and firmware extensions for the OPad ecosystem.

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
