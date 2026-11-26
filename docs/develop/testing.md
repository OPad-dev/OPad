---
title: Testing
description: The checks a change has to pass - make check, the 28 firmware host tests, the CI jobs and where they run (the lab before every push of main, GitHub on pull requests and releases), and the on-pad checklist.
sidebar:
  order: 3
---

A change is tested at four levels: `make check` on your machine, the full CI jobs, the release-time lab installs, and, for anything touching input, USB, touch or the screen, a real pad ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §4). If something could not be tested (no pad connected, no Windows machine), say so in the commit or pull request.

## `make check`

```bash
make check
```

Run it before calling any change done. In the [Makefile](https://github.com/OPad-dev/OPad/blob/main/Makefile) it runs, in order:

1. `cargo fmt --all -- --check` on the `desktop/` workspace;
2. `cargo clippy --all-targets -- -D warnings` (warnings are errors);
3. `cargo test` on the whole workspace (unit and integration tests of every crate, the daemon, the GUI and the CLI);
4. [`firmware/test/host/run_tests.sh`](https://github.com/OPad-dev/OPad/blob/main/firmware/test/host/run_tests.sh), the firmware host tests;
5. [`scripts/release/check_versions.py`](https://github.com/OPad-dev/OPad/blob/main/scripts/release/check_versions.py), which checks that the versions in `desktop/Cargo.toml`, `firmware/CMakeLists.txt`, `.release-please-manifest.json` and `Cargo.lock` agree.

It does not build the firmware, regenerate protobuf or run the licence checks; CI does (below).

## Firmware host tests

The firmware's hardware-independent logic is plain C, compiled with the host's `gcc -Wall -Wextra -Werror` and run directly, so it can be tested without a pad or ESP-IDF. `run_tests.sh` builds and runs **28** test programs (`test_config` twice, once with `-DCONFIG_OPAD_BENCH_DEBUG_GPIO=1`):

| Area | Tests |
|---|---|
| Keys and modules | `test_debounce`, `test_press_log`, `test_keypad_staged`, `test_game_keys`, `test_module_id`, `test_module_watch` |
| Hall Effect and ADS7028 | `test_he_rt`, `test_he_frame`, `test_he_health`, `test_he_cal`, `test_he_cal_capture`, `test_he_cal_button`, `test_he_cal_runner`, `test_he_cal_sampling`, `test_he_cal_storage`, `test_he_cal_touch_guard`, `test_he_cal_barrier`, `test_ads7028`, `test_he_spi_frame` |
| Touch and USB | `test_gesture`, `test_swipe_labels`, `test_usb_report_barrier` |
| Storage, config, protocol, diagnostics | `test_counters`, `test_config`, `test_owner`, `test_nvs_ns`, `test_frame_parser`, `test_diag` |

A few tests link small stubs for ESP-IDF APIs (`cal_stubs/`, `barrier_stubs/`, `nvs_stubs/` in [firmware/test/host](https://github.com/OPad-dev/OPad/tree/main/firmware/test/host)). New firmware logic that can be written without hardware calls belongs in a pure-C file with a test here; a new test is added to `run_tests.sh` and its `[n/28]` counters.

## CI jobs

The checks are written once, as shell functions in [scripts/ci/jobs.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.sh) (Linux) and [scripts/ci/jobs.ps1](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.ps1) (Windows). Both the lab and GitHub run these scripts, so a check passes or fails the same way in both places. A change to a check goes in the scripts, never only in `ci.yml`.

| Job | Runs |
|---|---|
| `versions` | `check_versions.py` |
| `rust` | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` |
| `firmware-host-tests` | `run_tests.sh` |
| `proto-check` | `scripts/gen_proto.sh` with nanopb 0.4.9.1, then `git diff --exit-code firmware/main/protocol/` |
| `firmware-build` | `idf.py set-target esp32s3 build` inside `espressif/idf:v5.5.2` |
| `licences` | `scripts/check_reuse.py` (REUSE), `cargo deny check`, and generating the third-party notices (`make notices`) |
| `rust-windows` | fmt, clippy, build and test on MSVC, every crate except `opad-gui` |
| `rust-windows-gui` | clippy, build and test of `opad-gui` on MSVC, kept separate so one crate's Windows port does not hide the others |

You can run any of them locally: `scripts/ci/jobs.sh list`, `scripts/ci/jobs.sh proto-check`, or `scripts/ci/jobs.sh all`. On a machine without `/opt/esp/idf`, `firmware-build` runs itself inside the `espressif/idf:v5.5.2` docker image.

### Where CI runs

- **The lab, before every push of `main`.** [scripts/ci/lab-ci.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/lab-ci.sh) bundles exactly one commit, copies it to the lab's mini PC and runs the Linux jobs in its `ci-linux` VM and the Windows jobs on the mini PC itself (with Git for Windows' CRLF checkout, as on GitHub's runners), in parallel. When every job passes it records the commit. `make lab-ci` runs it for `HEAD`; `make push` runs it for `main` and then pushes. The pre-push hook ([scripts/git-hooks/pre-push](https://github.com/OPad-dev/OPad/blob/main/scripts/git-hooks/pre-push), installed with `make hooks`) refuses to push a `main` the lab has not passed. Only `main` is checked; other branches push freely. A commit that already passed is not run again unless `--force` is given. The lab itself is described in [Testing Lab](lab.md).
- **GitHub Actions** ([ci.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/ci.yml)), which does **not** run on pushes: it runs on pull requests, by hand (`gh workflow run ci.yml --ref <branch>`), and before every release, when [release-please.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/release-please.yml) calls it after a release PR is merged; the release is built only if it passes.

## On a pad

Host tests cannot see timing, USB enumeration or a real touch panel. Changes to `firmware/main/input/`, `firmware/main/usb/`, touch or the screen are also tested on a real pad, following the rows of [the testing checklist](../testing-checklist.md). A change to the keypress path also states its effect on latency; [Latency Measurement](latency-testing.md) describes how to measure it.

Installers and upgrades are tested in the VM lab as part of every app release ([Testing Lab](lab.md)).
