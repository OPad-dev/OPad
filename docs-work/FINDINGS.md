# Documentation Findings

Discrepancies, code bugs, documentation conflicts, and notable architectural observations identified during documentation drafting and code audits.

## F1: Documentation Hub specifies MkDocs Material instead of Astro Starlight
- **Page**: `docs/README.md`
- **Problem**: `docs/README.md` lines 17-21 state that `docs/` is planned to become an MkDocs Material site. However, the authoritative documentation prompt (`docs-work/prompts/1-draft.md` §4) specifies that the docs are published by an Astro Starlight site.
- **Files involved**: `docs/README.md`, `docs-work/prompts/1-draft.md`.

## F2: Roadmap describes modular PCBs and hardware detection as future milestones
- **Page**: `docs/roadmap.md`
- **Problem**: `docs/roadmap.md` describes modular swappable switch PCBs (universal 8-wire SPI bus) and analog ID voltage detection as future roadmap targets. However, the V3 hardware redesign (`hardware/pcb/V3/`) and firmware code (`firmware/main/input/module_id.c`, `firmware/main/input/module_monitor.c`) already implement and test swappable MX/HE modules over FFC and hardware module detection.
- **Files involved**: `docs/roadmap.md`, `hardware/pcb/V3/README.md`, `firmware/main/input/module_id.c`, `firmware/main/input/module_monitor.c`.

## F3: File path drift in existing architecture document
- **Page**: `docs/architecture.md`
- **Problem**: `docs/architecture.md` refers to `diag/counters.c` and `main.c`, but the repository codebase implements counters in `firmware/main/counters/counters.c` and the entry point in `firmware/main/app_main.c`.
- **Files involved**: `docs/architecture.md`, `firmware/main/counters/counters.c`, `firmware/main/app_main.c`.
