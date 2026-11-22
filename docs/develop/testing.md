---
title: Testing and Quality Assurance
description: Running test suites, make check, Rust clippy and formatting, firmware host unit tests, and the manual hardware testing checklist.
---

OPad employs a multi-tiered testing strategy encompassing automated desktop tests, firmware host mock unit tests, CI validation pipelines, and manual hardware verification checklists. <!-- src: /AGENTS.md -->

## The Primary Verification Command (`make check`)

`make check` is the local gatekeeper executed by developers before committing changes: <!-- src: /Makefile -->

```bash
make check
```

Under the hood, `make check` executes: <!-- src: /Makefile -->
1. **Rust Formatting**: `cargo fmt --check` across the entire workspace.
2. **Clippy Linter**: `cargo clippy --all-targets -D warnings` enforcing zero compiler warnings.
3. **Workspace Tests**: `cargo test --workspace` executing unit tests for all desktop crates (`opad-model`, `opad-protocol`, `opad-storage`, `opad-ipc`, `opad-tosu`, `opad-device`, `opad-update`, `daemon`, `gui`, `cli`).
4. **Firmware Host Unit Tests**: `firmware/test/host/run_tests.sh` executing C unit tests on the host machine.

## Firmware Host Unit Tests (`run_tests.sh`)

Because embedded hardware is not always connected during development, critical firmware logic is compiled and executed directly on the host machine using standard `gcc`: <!-- src: /firmware/test/host/run_tests.sh -->

```bash
./firmware/test/host/run_tests.sh
```

This suite executes 28 standalone test suites covering:
- **`test_debounce`**: Eager lockout timing, bounce suppression, and resampling state transitions.
- **`test_counters`**: Monotonic keypress tracking, generation bumping, and delta preservation rules.
- **`test_config`**: Range boundary validation for debounce windows, GPIO pins, and backlight levels.
- **`test_frame_parser`**: Marked framing magic detection (`0xAA, 0x55`), checked framing CRC validation (`0xAA, 0x5A`), and recovery from corrupted byte streams.
- **`test_he_rt`**: Rapid trigger threshold crossings, dynamic reset sensitivity, and fault state transitions.

## Continuous Integration (CI) Checks

In addition to `make check`, GitHub Actions CI validates: <!-- src: /AGENTS.md -->
- **Cross-Platform Builds**: Windows MSVC builds for the desktop daemon, CLI, and GUI.
- **Protobuf Generation Check**: Runs `scripts/gen_proto.sh` and fails on any uncommitted diff in `firmware/main/protocol/opad.pb.c/.h`.
- **Firmware Compilation**: Compiles the ESP32-S3 firmware inside Docker container `espressif/idf:v5.5.2`.
- **Licensing Compliance**: Runs REUSE license checking (`scripts/check_reuse.py`), `cargo-deny`, and third-party notice audits.

## Manual Hardware Verification Checklist

Changes that touch `firmware/main/input/`, `firmware/main/usb/`, touch controller drivers, or screen rendering require testing on physical hardware: <!-- src: /AGENTS.md -->

- Refer to `docs/testing-checklist.md` for the normative hardware QA matrix. <!-- src: /docs/testing-checklist.md -->
- Test rows cover 1000 Hz USB HID enumeration, mechanical switch actuation, Rapid Trigger sensitivity on magnetic switches, touchscreen Quick Retry stillness detection, and OTA trial boot recovery.
- Never claim a hardware test passed without running it on a real pad (AGENTS.md §2).
