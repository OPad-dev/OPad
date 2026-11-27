---
title: Firmware Diagnostics Subsystem
description: Implementation reference for the diagnostic event ring buffer, carrier/module board self-test, and Hall Effect bench streaming mode.
sidebar:
  order: 8
---

The diagnostics subsystem provides zero-allocation ring-buffer event logging, carrier/module PCB electrical testing, and a compile-time Hall Effect sensor bench mode.

## Diagnostic Ring Buffer (`diag.c`)

Implemented in [/firmware/main/diag/diag.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/diag.c):

- Fixed circular buffer `s_entries[DIAG_RING_SIZE]` (`DIAG_RING_SIZE = 64`) of `diag_entry_t` (`timestamp_ms`, `event_id`, `level`, `arg0`, `arg1`), guarded by a FreeRTOS spinlock (`taskENTER_CRITICAL(&s_mux)`).
- `diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1)` performs no heap allocation and no string formatting; when full, it drops the oldest entry and increments `s_dropped`.
- `diag_drain(out_entries, max_entries, out_dropped)` drains queued entries and resets `s_dropped` so the protocol task can ship them to the host in `LogEventBatch`.
- Event IDs are wire format (append only), defined in [/firmware/main/diag/diag.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/diag.h) and formatted for `opadctl logs` in [/desktop/crates/opad-model/src/diag.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-model/src/diag.rs):

| ID | Event | ID | Event |
|---|---|---|---|
| 1 | `BOOT` (arg0 = reset reason) | 13 | `COUNTER_SYNC_REJECTED` |
| 2 | `HID_MOUNTED` | 14 | `LAYOUT_REJECTED` (arg0 = screen) |
| 3 | `HID_UNMOUNTED` | 15 | `DEFERRED_WRITE_FLUSHED` |
| 4 | `HID_SUSPENDED` | 16 | `DISPLAY_SLEEP` |
| 5 | `CDC_OPENED` | 17 | `DISPLAY_WAKE` |
| 6 | `CDC_CLOSED` | 18 | `LATENCY_OUTLIER` (arg0 = µs) |
| 7 | `NVS_INIT_FAILED` | 19 | `CDC_WRITE_DROPPED` |
| 8 | `NVS_ERASED` | 20 | `BUFFER_OVERFLOW` |
| 9 | `LCD_INIT_FAILED` | 21 | `UNKNOWN_HOST_MSG` (arg0 = field tag) |
| 10 | `DECODE_FAILED` | 22 | `DECODE_BYTES` |
| 11 | `FRAME_TOO_LARGE` (CDC bytes dropped; arg1 = `DIAG_FRAME_DROP_*` reason, including stale partial and bad CRC) | 23 | `MEMORY` |
| 12 | `CONFIG_REJECTED` | 24 | `UI_PERF` |

Two of them need more detail:
  - `DIAG_EVENT_MEMORY = 23`: Recorded once after boot in [/firmware/main/app_main.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/app_main.c) with `arg0 =` free internal RAM (bytes) and `arg1 =` largest free internal DMA-capable block (bytes), visible in `opadctl logs`.
  - `DIAG_EVENT_UI_PERF = 24`: Recorded in `CONFIG_OPAD_UI_PERF_TEST` builds by [/firmware/main/diag/ui_perf_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/ui_perf_test.c) at the end of each phase (`DIAG_UI_PERF_IDLE = 0`, `DIAG_UI_PERF_SONG = 1`, `DIAG_UI_PERF_RESULT = 2`) with frame count, average frame time, and longest frame time (in 0.1 ms units).

## Carrier & Input Module PCB Self-Test (`board_test.c`)

Implemented in [/firmware/main/diag/board_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/board_test.c) and invoked by the `run_board_test` host message (`opadctl pad test`, or the GUI's **Troubleshooting** → **Board test**). The protocol task refuses it outside `IDLE` (`"not while a map is played"`) and while pin detection runs (`"pin detection is running"`), because the test reconfigures key pins ([/firmware/main/protocol/protocol.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/protocol/protocol.c)). Steps:

1. **Key line pull test (`keypad_line_test`)**: Temporarily switches each armed key GPIO between internal pull-up and pull-down (`LINE_TEST_SETTLE_US = 300` µs) to verify whether an MX module's external 10 kΩ pull-ups hold the line high against pull-down, or whether a switch is held / shorted to `GND`.
2. **Module ID read (`board_read_module_id`)**: Samples open and loaded voltages on the module ID ADC pin (`GPIO 8`) to classify `MX`, `HE`, `HE_V3`, `MX_V3`, or `NONE` (skipped on V1 HE where ADC1 continuous DMA owns ADC1, using boot module instead).
3. **Reversed-cable probe (`board_probe_reversed_cable`)**: When no module answers on `GPIO 8` and `GPIO 2` (`BOARD_REVERSE_PROBE_GPIO`) is not a key pin, probes for a reversed cable.
4. **V1 spare-line continuity & bridge test (`test_spares`)**: For V1 `MX` / `HE` modules, tests `GPIO 6`, `GPIO 4`, and `GPIO 2` with pull-up/pull-down and drives neighboring pins low one at a time to detect pin-to-pin shorts (`bridged_gpios`).
5. **HE V3 ADS7028 test (`he_adc_test_run`, [/firmware/main/diag/he_adc_test.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/he_adc_test.c))**: For `BOARD_MODULE_HE_V3`, resets the ADS7028, reads its status registers and samples the sensors. If the SPI sampler is running it is paused for the test (`he_backend_run_paused`, keys released first); if instead the MX key pins are armed on that module, the test is skipped with `"key input is on: restart the pad"`, since selecting the ADC (CS is GPIO10) would press a key.

## Hall Effect Bench Mode (`hall_bench.c`)

Compiled only when `CONFIG_OPAD_HALL_BENCH=y` (not a release build; stop `opad-daemon` because it streams plain text over CDC) ([/firmware/main/diag/hall_bench.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/diag/hall_bench.c)):

- Samples four ADC1 channels at `BENCH_SAMPLE_HZ = 64000` Hz (`BENCH_FRAME_CONVS = 64`, 1 ms frames): Key 1 (`ADC_CHANNEL_9`), Key 2 (`ADC_CHANNEL_6`), module ID (`ADC_CHANNEL_7`), and reference (`ADC_CHANNEL_5`, `GPIO 6`).
- Runs its own `hall_bench` task (core 0, `configMAX_PRIORITIES - 3`) and emits 1 ms averaged lines `H,<ms>,<key1>,<key2>`, 1 s per-channel spread summaries `S,<key>,<n>,<min>,<max>,<mean>,<stddev_x100>,<overflows>`, pull-cycle phase lines `P,<phase>`, and `E,start|restart|read,...` error lines for `scripts/hall_bench.py`.
- Also displays a live 2-key voltage/bar view on the LCD (`hall_bench_show_ui`, 33 ms timer, measuring rest during the first `BENCH_REST_MS = 500` ms).
