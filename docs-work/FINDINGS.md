# Documentation Findings

Discrepancies, code bugs, documentation conflicts, and notable architectural observations identified during documentation drafting and code audits.

## F1: Roadmap describes modular PCBs and hardware detection as future milestones
- **Page**: `docs/roadmap.md`
- **Problem**: `docs/roadmap.md` describes modular swappable switch PCBs (universal 8-wire SPI bus) and analog ID voltage detection as future roadmap targets. However, the V3 hardware redesign (`hardware/pcb/V3/`) and firmware code (`firmware/main/input/module_id.c`, `firmware/main/input/module_monitor.c`) already implement and test swappable MX/HE modules over FFC and hardware module detection.
- **Files involved**: `docs/roadmap.md`, `hardware/pcb/V3/README.md`, `firmware/main/input/module_id.c`, `firmware/main/input/module_monitor.c`.

## F2: File path drift in existing architecture document
- **Page**: `docs/architecture.md`
- **Problem**: `docs/architecture.md` refers to `diag/counters.c` and `main.c`, but the repository codebase implements counters in `firmware/main/counters/counters.c` and the entry point in `firmware/main/app_main.c`.
- **Files involved**: `docs/architecture.md`, `firmware/main/counters/counters.c`, `firmware/main/app_main.c`.

## F3: Host-driven firmware updater blocks pads running from `ota_1`
- **Page**: `docs/guides/update-firmware.md`, `docs/explanation/updates-and-rollback.md`
- **Problem**: `desktop/daemon/src/firmware_update.rs` (`offer()` and `install()`) uses `opad_update::firmware::is_ota_0()` to check `running_from_ota_0`, raising `Blocker::NeedsFullReflash` ("the pad's partition layout does not match this update (it is not running from ota_0)") whenever `running_partition != Some("ota_0")`. However, `flash_and_verify()` in the same file calls `flash::plan_app_update()`, which writes an update into the opposite OTA slot (`ota_1` when running `ota_0`). After one successful A/B update onto `ota_1`, subsequent host-driven updates are blocked until a full reflash back to `ota_0`.
- **Files involved**: `desktop/daemon/src/firmware_update.rs`, `desktop/crates/opad-update/src/firmware.rs`, `desktop/crates/opad-device/src/flash.rs`.

## F4: Stale recovery details in `docs/recovery.md`
- **Page**: `docs/recovery.md`
- **Problem**: The pre-rewrite `docs/recovery.md` stated that `opadctl bootloader` tries three triggers (code has two: `BootTrigger::Command` and `BootTrigger::BaudTouch`; the esptool RTS/DTR trigger was removed in `firmware/main/usb/usb_cdc.c` because ModemManager/serial probes triggered accidental reboots), named a non-existent IPC command `InheritDeviceCounters` (code uses `ResolveReplacement`), quoted an outdated replacement banner text, and stated replacement triggers only when `gen == 0, k1 == 0, k2 == 0` with exactly one previous pad (code triggers when `counter_generation <= 1 && lifetime_key1 < 1000 && lifetime_key2 < 1000 && pc_is_ahead_of_pad()`).
- **Files involved**: `docs/recovery.md`, `desktop/crates/opad-device/src/flash.rs`, `firmware/main/usb/usb_cdc.c`, `desktop/daemon/src/runtime.rs`, `desktop/daemon/src/ipc_handlers.rs`, `desktop/gui/src/main.rs`.
