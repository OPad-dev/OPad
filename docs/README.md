# OPad Documentation Hub & Wiki

Welcome to the official documentation and technical wiki for the **OPad** project.

This directory serves as the single source of truth for hardware specifications, firmware implementation, desktop daemon architecture, testing protocols, and future milestones.

---

## ⚠️ Planned: full documentation reorganization and wiki (noted 2026-09-24)

This folder has grown by accretion: design docs, four readiness/audit reviews,
an evening TODO, a code-review folder and a fix plan sit side by side, several
of them describing the code as it was before the 2026-09-24 review fixes
(commits `0e3286b`…`d56211d`, 93 findings closed). It will be rebuilt as a
coherent wiki. The plan:

1. **Wiki lives here, not in the GitHub Wiki tab.** `docs/` becomes an
   [MkDocs Material](https://squidfunk.github.io/mkdocs-material/) site
   published with GitHub Pages: same repo, same commits, reviewed in PRs, links
   to source files stay valid, `mkdocs serve` to preview.
2. **Structure first, then pages.** One session designs the navigation and
   writes the cross-cutting pages (Overview, System architecture, Serial
   protocol incl. framing negotiation, Counter sync & pad ownership, Update &
   release pipeline). It ends with a checklist of remaining pages, each naming
   the source files to read — same format as
   [claude-code-review/FIX-PLAN.md](claude-code-review/FIX-PLAN.md).
3. **Per-component pages** from that checklist: firmware modules (keypad &
   debounce, USB HID/CDC, UI & LVGL layouts, `ui_store`, diag & counters,
   touch as the third key), one page per desktop crate, the daemon state
   machine, GUI pages, `opadctl` reference (generated from `--help`), packaging
   per distro and Windows, hardware & wiring (Waveshare ESP32-S3 Touch LCD 2
   pinout), troubleshooting, FAQ.
4. **Rules for whoever writes it:** every factual claim comes from a source
   file read in that session and cites its path; when code and an existing doc
   disagree, the code wins and the doc is fixed or deleted; no placeholder
   pages.
5. **Historical material** is kept but moved under `docs/history/` and marked
   as such: the readiness/audit reviews of 2026-09-22, `todo-2026-09-21-evening.md`,
   and the `claude-code-review/` findings (a record of what was found and fixed,
   not current guidance). `specs/remaining-work-v1.md` is folded into a changelog.

Until that lands, treat the documents below as **possibly stale**; the code and
`git log` are authoritative.

---

## 📚 Documentation Index

### 🏗️ Architecture & Core System
* **[System Architecture](architecture.md)** — Hardware layout, ESP32-S3 dual-core isolation, IPC design, desktop daemon, and state machine.
* **[USB Framing & Serial Protocol](protocol.md)** — Binary CDC packet framing, command opcodes, sequence numbers, and telemetry streams.
* **[OPad on Windows](windows-portability.md)** — Windows-specific design: Named Pipes IPC, Windows Service daemon, auto-start, and driverless HID operation.
* **[Recovery, Reconciliation & Unbinding](recovery.md)** — Safe counter synchronization, flashing procedures, bootloader entry, and pairing models.

* **[Troubleshooting](troubleshooting.md)** — Common failures (no live data, port busy, tosu memory access) and their fixes.

### ⏱️ Latency & Hardware Testing
* **[Latency Testing Methodology](latency-testing.md)** — Hardware test methodology, Stages A–D, electronic key-to-HID gate requirements, and latency benchmarks.
* **[Testing & Verification Checklist](testing-checklist.md)** — Comprehensive quality assurance checklist covering firmware, daemon, GUI, and hardware reliability.

### 🔍 Code review record (2026-09-23/24)
* **[Code review findings & fix plan](claude-code-review/README.md)** — 93 findings across firmware, desktop and packaging, all closed (92 fixed, 1 invalid); [FIX-PLAN.md](claude-code-review/FIX-PLAN.md) holds the per-cluster progress log and the hardware validation record.

### 🗂️ Historical readiness reviews (pre-fix snapshots, to move under `history/`)
* [Software readiness review (2026-09-22)](software-readiness-review-2026-09-22.md), [Software audit & readiness review](software-audit-and-readiness-review.md), [Release readiness guide (2026-09-22)](release-readiness-guide-2026-09-22.md), [Full-repo architecture & readiness guide](full-repo-architecture-and-readiness-guide.md), [TODO 2026-09-21 evening](todo-2026-09-21-evening.md) — written before the review fixes; many items they list are now done.

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
