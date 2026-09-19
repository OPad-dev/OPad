# osu!pad Documentation Hub & Wiki

Welcome to the official documentation and technical wiki for the **osu!pad** project.

This directory serves as the single source of truth for hardware specifications, firmware implementation, desktop daemon architecture, testing protocols, and future milestones.

---

## 📚 Documentation Index

### 🏗️ Architecture & Core System
* **[System Architecture](architecture.md)** — Hardware layout, ESP32-S3 dual-core isolation, IPC design, desktop daemon, and state machine.
* **[USB Framing & Serial Protocol](protocol.md)** — Binary CDC packet framing, command opcodes, sequence numbers, and telemetry streams.
* **[osu!pad on Windows](windows-portability.md)** — Windows-specific design: Named Pipes IPC, Windows Service daemon, auto-start, and driverless HID operation.
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
* **[Technical Specification v1](specs/technical-spec-v1.md)** — Complete normative specification for osu!pad V1 hardware, firmware, and desktop integration.
* **[Remaining Work & Gap Analysis](specs/remaining-work-v1.md)** — As-built gap analysis and audit tracking for V1 release.
* **[Packaging & Distribution Plan](specs/packaging-distribution.md)** — Build system, CI/CD, Windows/Linux packaging, and release delivery.
* **[Rapid Trigger (Hall-Effect Keys) v2 Plan](specs/v2-rapid-trigger.md)** — Implementation blueprint for Hall-effect magnetic switches, ADC sampling pipeline, and continuous Rapid Trigger engine.

---

## 🌐 Using Gitea Wiki

Gitea includes a built-in **Wiki** system that operates almost identically to GitHub:

### 1. Enabling the Wiki in Gitea
1. Navigate to your repository on your Gitea server: `https://git.gferreiro.com/GFerreiroS/osu-pad`.
2. Go to **Settings** $\to$ **Repository Settings**.
3. Under **Navigation / Features**, make sure **Enable Wiki** is checked.
4. A **Wiki** tab will appear directly in the top navigation bar of the repository.

### 2. The Wiki is a Standalone Git Repository
Just like GitHub, Gitea wikis are standard Git repositories ending in `.wiki.git`:
```bash
git clone https://git.gferreiro.com/GFerreiroS/osu-pad.wiki.git
```
You can edit markdown files locally, organize pages, and push changes directly with standard `git commit` and `git push`.

### 3. Automated Sync via Gitea Actions (Optional)
If you prefer maintaining your documentation inside this `docs/` folder in the main repository, you can set up a simple Gitea Action to automatically mirror `docs/` into the Gitea Wiki whenever commits merge to `main`:

```yaml
# .gitea/workflows/sync-wiki.yml
name: Sync Docs to Wiki
on:
  push:
    branches: [ main ]
    paths:
      - 'docs/**'

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Push docs to Gitea Wiki
        run: |
          git clone https://${{ secrets.GITEA_TOKEN }}@git.gferreiro.com/GFerreiroS/osu-pad.wiki.git wiki
          cp -r docs/* wiki/
          cd wiki
          git config user.name "GFerreiroS"
          git config user.email "info@gferreiro.com"
          git add -A
          git diff-index --quiet HEAD || (git commit -m "docs: sync wiki from main repo" && git push origin master)
```
