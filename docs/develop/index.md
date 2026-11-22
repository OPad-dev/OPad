---
title: Developer Guide
description: Contributor workflow, product invariants, git branch lifecycle, commit conventions, and documentation rules for OPad.
---

Welcome to the OPad developer documentation. This section covers contributor rules, building from source, running automated tests, firmware and desktop internals, protocol evolution, hardware files, packaging, releases, lab testing, and latency benchmarking.

## Core Product Invariants

Every change in this repository must uphold the six product rules in [/AGENTS.md](/AGENTS.md) §1:

1. **Latency always wins**: If a feature measurably worsens key latency or latency jitter, reduce, defer, freeze during gameplay, or remove it. Nothing joins the keypress path: screen, storage, tray, and `tosu` stay off it.
2. **The pad is a keyboard with no software installed**: 1000 Hz USB HID keyboard requiring no driver or desktop software on Linux or Windows 10/11.
3. **The app and the pad update independently**: Protocol and wire changes must work both ways (new app with old firmware, old app with new firmware) via detection and negotiation.
4. **The touchscreen is the third button**: Clean press/hold/release; Quick Retry (`` ` ``) during a map (never from swipes); configurable swipes outside a map; never inserts modifiers or extra keys into keyboard reports mid-map; exception: uncalibrated Hall Effect pads lock to their calibration screen (D7).
5. **Never the BOOT or RESET buttons**: Flashing, updating, and recovering happen over USB/serial with A/B OTA rollback and `opadctl recover`.
6. **Every key comes from a finger**: No macros, auto-tap, scripted/timed input, or host-to-pad key injection commands.

## Git Workflow & Commit Rules

Per [/AGENTS.md](/AGENTS.md) §2–§3:

- **Linear history**: Rebase onto `main`; no merge commits.
- **Short-lived branches**: Maintainers work in a worktree on a branch, fast-forward merge to `main`, then delete the branch and worktree. External contributors fork, branch, and open a PR against `main`.
- **Commit authorship & AI disclosure**: Always sign commits with your own name and email (never an AI author). End AI-assisted commits with one trailer (`AI: assisted` or `AI: generated`); omit it when no AI was used. Replace tool-specific `Co-Authored-By` lines with this trailer.
- **Conventional commits**: `type(scope): plain statement of behaviour (#issue)`. Add a `Compatibility:` trailer whenever protocol or IPC behavior changes.
- **Licensing (REUSE)**: New files need license headers or an entry in [/REUSE.toml](/REUSE.toml).

## Code as Single Source of Truth

Per [/AGENTS.md](/AGENTS.md) §7:

- When documentation and code disagree, the code wins—fix or remove the stale doc.
- Developer pages (`docs/explanation/*`, `docs/develop/*`) link to source files using repo-root paths (for example, `[/firmware/main/app_main.c](/firmware/main/app_main.c)`).
- Never invent CLI flags, config fields, or GPIO pins that do not exist in the codebase.

## Documentation Structure (Diátaxis)

- **Start (`docs/start/*`)**: Onboarding tutorials (hardware, wiring, first connection, installing the app).
- **Guides (`docs/guides/*`, `docs/recovery.md`)**: Task-oriented how-to procedures.
- **Reference (`docs/reference/*`, `docs/troubleshooting.md`, `docs/develop/*`)**: Authoritative catalogs of CLI, GUI, settings, pinout, serial/IPC protocols, paths, and internals.
- **Explanation (`docs/explanation/*`)**: System architecture and design rationale.
