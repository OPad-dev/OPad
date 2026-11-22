---
title: Firmware Diagnostics Subsystem
description: Implementation reference for firmware telemetry, circular ring buffer logging, board electrical self-tests, and Hall Effect bench analysis.
---

The diagnostics subsystem provides self-test routines, electrical continuity verification, circular ring buffer event recording, and magnetic sensor benchmarking tools. <!-- src: /firmware/main/diag/diag.c -->

## Diagnostic Ring Buffer Logging (`diag.c`)

Firmware events (USB connect/disconnect, framing drops, CRC mismatches, NVS flushes) are recorded in an in-memory circular ring buffer: <!-- src: /firmware/main/diag/diag.c -->

- **Data Structure (`diag_entry_t`)**: Stored in a fixed-size circular array `s_entries[DIAG_RING_SIZE]`. <!-- src: /firmware/main/diag/diag.c -->
- **Zero-Allocation Critical Sections**: Events are appended via `diag_record(event_id, severity, arg1, arg2)`. The function acquires a FreeRTOS port spinlock (`taskENTER_CRITICAL`), writes the entry, and exits immediately. No memory allocation or formatted string generation occurs on the target. <!-- src: /firmware/main/diag/diag.c#diag_init -->
- **Drain Interface**: The host drains logged entries periodically over serial via `LogEventBatch` protobuf envelopes.

## Board Electrical Self-Test (`board_test.c`)

When triggered via `run_board_test` over serial or `opadctl board-test`, the firmware runs hardware continuity and peripheral checks: <!-- src: /firmware/main/diag/board_test.c -->

1. **Spare GPIO Pull Test**: Exercises spare module header lines (`BOARD_MODULE_IO6_GPIO`, `IO4`, `IO2`). Toggles pull-up and pull-down states (`read_with_pull`), verifying lines follow internal pulls without short circuits to ground or 3.3V. <!-- src: /firmware/main/diag/board_test.c#read_with_pull -->
2. **I2C Bus Scan**: Scans address `0x15` (CST816S touch controller) and checks responsiveness.
3. **Display Communication**: Reads back the ST7789 display controller ID register.
4. **Input Module Identification**: Samples the analog divider on GPIO 8 (`module_id`) to verify whether an MX mechanical module, Hall Effect V1 module, or V3 carrier is installed.
5. **Report Generation**: Populates `BoardTestResult` with per-subsystem pass/fail status flags and diagnostic messages.

## Hall Effect Bench Test Mode (`hall_bench.c`)

For characterization of magnetic switch travel curves during hardware validation (`CONFIG_OPAD_HALL_BENCH`): <!-- src: /firmware/main/diag/hall_bench.c -->

- **Continuous Streaming**: Streams raw 12-bit ADC conversions directly over USB CDC at millisecond resolution:
  `H,<ms>,<key1_raw>,<key2_raw>`
- **Statistical Aggregation**: Every second, computes spread statistics across thousands of conversions:
  `S,<key>,<sample_count>,<min>,<max>,<mean>,<stddev_x100>,<overflows>`
- **Python Harness**: Companion host script `scripts/hall_bench.py` records streams to compute standard deviation, resting noise floors, and magnetic travel deltas.
