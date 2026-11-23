---
title: Testing and Quality Assurance
description: Running make check, desktop Cargo tests, the 28 firmware host unit tests, CI checks, and the manual hardware testing checklist.
sidebar:
  order: 3
---

OPad verifies changes across four layers: local `make check`, 28 host-compiled C unit tests for firmware logic, GitHub Actions CI, and manual on-pad verification ([/AGENTS.md](/AGENTS.md) §4, [/Makefile](/Makefile)).

## Primary Local Gate (`make check`)

Run before finishing any change:
```bash
make check
```
In [/Makefile](/Makefile), `make check` executes:
1. `cargo fmt --check` (formatting across the `desktop/` workspace)
2. `cargo clippy --all-targets -- -D warnings` (warnings denied)
3. `cargo test --workspace` (unit and integration tests across all desktop crates, daemon, GUI, and CLI)
4. `./firmware/test/host/run_tests.sh` (all 28 firmware host unit tests)

## Firmware Host Unit Tests (`run_tests.sh`)

Hardware-independent C modules are compiled with `gcc -Wall -Wextra -Werror` and executed directly on the host via [/firmware/test/host/run_tests.sh](/firmware/test/host/run_tests.sh):

```bash
./firmware/test/host/run_tests.sh
```

The script runs **28** test suites covering:
- **Input & mechanical**: `test_debounce`, `test_press_log`, `test_keypad_staged`, `test_game_keys`, `test_module_id`, `test_module_watch`
- **Hall Effect & ADS7028**: `test_he_rt`, `test_he_frame`, `test_he_cal`, `test_he_cal_capture`, `test_he_health`, `test_he_cal_button`, `test_he_cal_runner`, `test_he_cal_sampling`, `test_he_cal_storage`, `test_he_cal_touch_guard`, `test_he_cal_barrier`, `test_ads7028`, `test_he_spi_frame`
- **Touch & USB**: `test_gesture`, `test_swipe_labels`, `test_usb_report_barrier`
- **Storage, config, protocol & diagnostics**: `test_counters`, `test_config` (plus `-DCONFIG_OPAD_BENCH_DEBUG_GPIO=1`), `test_diag`, `test_frame_parser`, `test_owner`, `test_nvs_ns`

## Continuous Integration (CI)

Every check lives in [/scripts/ci/jobs.sh](/scripts/ci/jobs.sh) (Linux jobs) and [/scripts/ci/jobs.ps1](/scripts/ci/jobs.ps1) (Windows jobs). Two places run them:

- **The lab, before every push of `main`**: [/scripts/ci/lab-ci.sh](/scripts/ci/lab-ci.sh) (`make push`, or `make lab-ci` without pushing) copies the commit to the mini PC and runs the Linux jobs in its `ci-linux` VM ([/scripts/ci/provision-linux.sh](/scripts/ci/provision-linux.sh)) and the Windows jobs on the mini PC itself, with Git for Windows' CRLF checkout. When all of them pass it records the commit, and the pre-push hook ([/scripts/git-hooks/pre-push](/scripts/git-hooks/pre-push), installed by `make hooks`) only lets a recorded commit be pushed to `main`.
- **GitHub Actions** ([/.github/workflows/ci.yml](/.github/workflows/ci.yml)), which does not run on pushes: on pull requests, by hand (`gh workflow run ci.yml --ref <branch>`), and before every release, since merging a release PR runs it from [/.github/workflows/release-please.yml](/.github/workflows/release-please.yml) and the release is built only if it passes.

Besides `make check`, the jobs run:
- Windows MSVC build and tests (with GUI in a separate job).
- Protobuf regeneration via [/scripts/gen_proto.sh](/scripts/gen_proto.sh) (fails if generated `opad.pb.c/.h` differ).
- ESP32-S3 firmware build in the `espressif/idf:v5.5.2` container.
- License & dependency checks: REUSE compliance ([/scripts/check_reuse.py](/scripts/check_reuse.py)), `cargo-deny`, and third-party notice verification.

## Manual Hardware Checklist

Changes touching `firmware/main/input/`, `firmware/main/usb/`, touchscreen handling, or display rendering must be tested on a real pad using [/docs/testing-checklist.md](/docs/testing-checklist.md) ([/AGENTS.md](/AGENTS.md) §2, §4). Always report plainly if hardware was not connected and tests were skipped.
