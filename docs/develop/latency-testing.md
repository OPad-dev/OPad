---
title: Latency Measurement
description: How to measure OPad's key latency - the on-device histogram, benchmark stages A to D that isolate each subsystem's cost, the release gate, and the scripts that record a run.
sidebar:
  order: 9
---

Latency always wins ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1), so a change that might slow the keys has to be measured, not argued. The method is to measure the same thing with more and more of the firmware switched on: if the full stack is no slower than the key path alone, nothing else is getting in the way. Why the key path is built the way it is is explained in [Latency Engineering](../explanation/latency.md).

## What is measured

[latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c) records, for every key change, the time from the switch edge (`edge_us`, taken in the interrupt) to TinyUSB accepting the HID report:

- 500 buckets of 10 µs (0–5 ms). `p50`, `p99` and `p99.9` are the upper edge of a bucket, so always multiples of 10 µs: **differences under 10 µs cannot be resolved**. `max` is exact.
- `deferred_reports`: changes that had to wait for a transfer in flight on their endpoint.
- Samples above 1 ms are counted as outliers and reported to the host as `DIAG_EVENT_LATENCY_OUTLIER` diagnostics.

The host's own 1 ms polling and everything after it are not in these numbers; `bench_latency.py` (below) looks at that side on Linux.

## Release gate

1. No missed presses in fast alternating streams (above 15–20 presses/s).
2. No stuck keys under any load.
3. No outliers above 1 ms caused by the display, CDC traffic, timers or flash writes.
4. Stage D's `p99.9` less than 100 µs above Stage A's, on the same platform.
5. The key ISR, input tasks and TinyUSB on Core 0; display, protocol, touch and runtime on Core 1.

## Benchmark stages

```mermaid
flowchart LR
    A["A: keys + HID only"] --> B["B: + CDC protocol, daemon connected"] --> C["C: + display at 2 / 10 / 30 Hz"] --> D["D: everything"]
```

| Stage | Build | What runs | Reading the stats |
|---|---|---|---|
| **A** | `CONFIG_OPAD_BENCH_HID_ONLY=y` | Keypad task and TinyUSB HID only; no CDC, runtime or display | Printed every 10 s on **UART0** only (TX = GPIO43, 115200 baud): there is no CDC in this build, so it needs a USB-UART adapter. Reboot to reset. |
| **B** | `CONFIG_OPAD_BENCH_NO_DISPLAY=y` | Adds the CDC protocol with the daemon connected; display and backlight off | `opadctl latency` |
| **C** | Standard build | Adds the gameplay screen fed by tosu, at the gameplay refresh cap (`gameplay_display_hz`: 2, 10 (default) and 30 Hz) | `opadctl latency` |
| **D** | Standard build | Everything: tosu streaming, LVGL, post-cooldown and idle sync, diagnostics | `opadctl latency` |

The Kconfig options are in [Kconfig.projbuild](https://github.com/OPad-dev/OPad/blob/main/firmware/main/Kconfig.projbuild); see [Building](building.md#firmware) for how to set them. `CONFIG_OPAD_BENCH_DEBUG_GPIO` additionally toggles a pin (default GPIO 4, `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM`) right after each HID submit, for a logic analyser; check that the pin is free on your wiring first.

## Recording a run

[scripts/latency_row.py](https://github.com/OPad-dev/OPad/blob/main/scripts/latency_row.py) wraps `opadctl latency` and turns the result into a table row:

```bash
python3 scripts/latency_row.py --reset                 # right before you start tapping
# tap both keys alternately, above 15 presses/s, for 2 minutes
python3 scripts/latency_row.py --stage "C (10 Hz)"     # prints the row
```

`--stage` takes one of `A`, `B`, `C (2 Hz)`, `C (10 Hz)`, `C (30 Hz)`, `D` (`KNOWN_STAGES`). The commit column comes from git and is marked `-dirty` when `firmware/` or `protocol/` has uncommitted changes, because a row naming a commit that was not what was flashed is worse than none. Rows go into the table below; the platform goes in the notes. The gate compares stages on the same platform only: a Windows Stage D against a Linux Stage A measures the wrong difference.

| Date | Firmware commit | Stage | Samples | p50 (µs) | p99 (µs) | p99.9 (µs) | Max (µs) | Deferred | Stuck / missed keys | Notes |
|---|---|---|---|---|---|---|---|---|---|---|

No staged A–D run has been recorded in this table yet. Single measurements on the standard firmware are in [the testing checklist](../testing-checklist.md) (rows HW-04, HW-14, PPM-04).

## Host side (Linux)

[scripts/bench_latency.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/bench_latency.sh) runs [bench_latency.py](https://github.com/OPad-dev/OPad/blob/main/scripts/bench_latency.py). It finds the pad's input device (`/dev/input/by-id/*OPad*`, or by name under `/sys/class/input/event*`), collects 100 intervals between `EV_KEY` events shorter than 50 ms, and prints their mean, minimum, maximum and standard deviation, against targets of a standard deviation under 0.25 ms and a maximum of 2.5 ms. It needs read access to the event device. There is no Windows equivalent; on Windows, stages A–D are measured on the pad, which is the part the gate is about.
