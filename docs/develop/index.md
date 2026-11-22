---
title: Developer Guide
description: Contributor workflow, git branch lifecycle, commit signing, AI attribution trailers, and repository architecture guidelines.
category: overview
source_files:
  - /AGENTS.md
  - /README.md
  - /.github/workflows/ci.yml
  - /Makefile
---

Welcome to the OPad developer documentation. This section covers development workflows, repository invariants, build instructions, test harnesses, and architecture references for contributing to the firmware, desktop suite, and hardware designs. <!-- src: /AGENTS.md -->

## Core Product Invariants

All development in this repository must adhere to the immutable product rules defined in [AGENTS.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/AGENTS.md): <!-- src: /AGENTS.md -->

1. **Latency Always Wins**: If a feature measurably worsens key latency or latency jitter, it is reduced, deferred, frozen during gameplay, or removed. Nothing joins the keypress path: screen, storage, tray, and tosu remain completely off it.
2. **The Pad is a Keyboard with No Software Installed**: It operates as a 1000 Hz USB HID composite keyboard requiring no driver or daemon on Linux or Windows.
3. **App and Pad Update Independently**: Wire protocols must remain bidirectional (new app with old firmware, old app with new firmware). Never require updating both together.
4. **The Touchscreen is the Third Button**: In-game resting finger is Quick Retry (`` ` ``) and swipes never trigger it. Out-of-game swipes execute media actions. Mid-map swipes never insert Alt into the keyboard report.
5. **Never the BOOT or RESET Buttons**: Flashing, updating, and recovering happen strictly over USB or serial. An update flashes the alternate OTA slot and rolls back if USB enumeration fails.
6. **Every Key Comes from a Finger**: No macros, auto-tap, scripted or timed inputs, or host-to-pad key injection commands.

## Git Workflow & Repository Rules

Contributors must adhere to the following git workflow rules: <!-- src: /AGENTS.md -->

- **Linear History**: Rebase on `main`. No merge commits.
- **Short-Lived Branches**: Branches exist only while active work is underway. Delete branches and worktrees immediately upon merging.
- **Author Identity & AI Disclosure**:
  - Sign commits with your own name and email. Never set an AI tool as commit author.
  - Include an AI disclosure trailer at the end of the commit message:
    - `AI: assisted`: A human directed or wrote the change with AI assistance.
    - `AI: generated`: An AI wrote the change, reviewed and committed by a person.
    - No trailer indicates no AI involvement.
- **Conventional Commits**: Format commit subjects as `type(scope): subject (#issue)`.

## Code as the Single Source of Truth

Documentation in `docs/` is written against the codebase: <!-- src: /AGENTS.md -->
- When code and documentation disagree, **the code is authoritative**. Fix or update the documentation to match the code.
- Every factual claim in developer documentation includes source citations (invisible HTML source comments or direct file links). <!-- src: /AGENTS.md -->
- Never invent parameters, CLI flags, or hardware pins not present in the codebase.

## Documentation Structure (Diátaxis Framework)

OPad documentation follows the four Diátaxis quadrants:
- **Tutorials (`docs/start/*`)**: Step-by-step onboarding for newcomers (What You Need, First Connection, Wiring the Keys).
- **How-To Guides (`docs/guides/*`)**: Task-oriented instructions (Update Firmware, Calibrate Hall Effect, Backup & Restore).
- **Explanation (`docs/explanation/*`)**: Conceptual background and architecture rationale (System Architecture, Latency Engineering, Touchscreen as Third Button).
- **Reference (`docs/reference/*` and `docs/develop/*`)**: Verbatim catalogs of APIs, CLI commands, device settings, pinouts, and subsystem internals.
