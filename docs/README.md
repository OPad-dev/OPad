# OPad Documentation Hub & Wiki

Welcome to the official documentation and technical wiki for the **OPad** project.

This directory serves as the single source of truth for hardware specifications, firmware implementation, desktop daemon architecture, testing protocols, and future milestones.

---

## 📚 Documentation Index

### 🏗️ Architecture & Core System
* **[System Architecture](architecture.md)** — Hardware layout, ESP32-S3 dual-core isolation, IPC design, desktop daemon, and state machine.
* **[USB Framing & Serial Protocol](protocol.md)** — Binary CDC packet framing, command opcodes, sequence numbers, and telemetry streams.
* **[OPad on Windows](windows-portability.md)** — Windows-specific design: Named Pipes IPC, Windows Service daemon, auto-start, and driverless HID operation.
* **[Recovery, Reconciliation & Unbinding](recovery.md)** — Safe counter synchronization, flashing procedures, bootloader entry, and pairing models.

### ⏱️ Latency & Hardware Testing
* **[Latency Testing Methodology](latency-testing.md)** — Hardware test methodology, Stages A–D, electronic key-to-HID gate requirements, and latency benchmarks.
* **[Testing & Verification Checklist](testing-checklist.md)** — Comprehensive quality assurance checklist covering firmware, daemon, GUI, and hardware reliability.

### 🚀 Future Milestones & Catalog Roadmap
* **[Future Roadmap & Hardware Variants](roadmap.md)**:
  * **ESP-NOW Ultra-Low-Latency Wireless Dongle** (Dual-mode 1.0 ms wireless).
  * **Heavy Ballast & Anti-Slip Deskmat Enclosure** (High-BPM anti-slip case V2).
  * **Multi-Key Catalog Expansion** (3-Key Standard with Quick Retry, 4k/5k/6k/7k/10k Mania).
  * **Modular Swappable Switch PCBs** (On-board ADC chip architecture, universal 8-wire SPI bus).
  * **Hardware Auto-Detection** (Analog ID voltage net & digital SPI handshake).
  * **Adaptive Debounce & Rapid Trigger Velocity Hysteresis**.

### 📋 Specifications & Development Plans
* **[Technical Specification v1](specs/technical-spec-v1.md)** — Complete normative specification for OPad V1 hardware, firmware, and desktop integration.
* **[Remaining Work & Gap Analysis](specs/remaining-work-v1.md)** — As-built gap analysis and audit tracking for V1 release.
* **[Packaging & Distribution Plan](specs/packaging-distribution.md)** — Build system, CI/CD, Windows/Linux packaging, and release delivery.
* **[Rapid Trigger (Hall-Effect Keys) v2 Plan](specs/v2-rapid-trigger.md)** — Implementation blueprint for Hall-effect magnetic switches, ADC sampling pipeline, and continuous Rapid Trigger engine.

---

## 🌐 Community & Repository

* **Primary Repository**: [https://github.com/OPad-dev/OPad](https://github.com/OPad-dev/OPad)
* **Organization**: [OPad-dev](https://github.com/OPad-dev)

---

## ⚖️ Trademark Disclaimer
OPad is an independent open-source hardware and software project. OPad is not affiliated with, endorsed by, or sponsored by ppy Pty Ltd or osu!. "osu!" is a registered trademark of ppy Pty Ltd.
