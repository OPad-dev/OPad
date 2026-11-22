---
title: Firmware Diagnostics Subsystem
description: Implementation reference for the diagnostic event ring buffer, carrier/module board self-test, and Hall Effect bench streaming mode.
---

The diagnostics subsystem provides zero-allocation ring-buffer event logging, carrier/module PCB electrical testing, and a compile-time Hall Effect sensor bench mode.

## Diagnostic Ring Buffer (`diag.c`)

Implemented in [/firmware/main/diag/diag.c](/firmware/main/diag/diag.c):

- Fixed circular buffer `s_entries[DIAG_RING_SIZE]` of `diag_entry_t` (`timestamp_ms`, `event_id`, `level`, `arg0`, `arg1`), guarded by a FreeRTOS spinlock (`taskENTER_CRITICAL(&s_mux)`).
- `diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1)` performs no heap allocation and no string formatting; when full, it drops the oldest entry and increments `s_dropped`.
- `diag_drain(out_entries, max_entries, out_dropped)` drains queued entries and resets `s_dropped` so the protocol task can ship them to the host in `LogEventBatch`.

## Carrier & Input Module PCB Self-Test (`board_test.c`)

Implemented in [/firmware/main/diag/board_test.c](/firmware/main/diag/board_test.c) and invoked by `BoardTestRequest` (`opadctl board-test` or `opad-gui` **Diagnostics** -> **🔧 PCB Test**, `IDLE` only):

1. **Key line pull test (`keypad_line_test`)**: Temporarily switches each armed key GPIO between internal pull-up and pull-down (`LINE_TEST_SETTLE_US = 300` µs) to verify whether an MX module's external 10 kΩ pull-ups hold the line high against pull-down, or whether a switch is held / shorted to `GND`.
2. **Module ID read (`board_read_module_id`)**: Samples open and loaded voltages on the module ID ADC pin (`GPIO 8`) to classify `MX`, `HE`, `HE_V3`, `MX_V3`, or `NONE` (skipped on V1 HE where ADC1 continuous DMA owns ADC1, using boot module instead).
3. **Reversed-cable probe (`board_probe_reversed_cable`)**: When no module answers on `GPIO 8` and `GPIO 2` (`BOARD_REVERSE_PROBE_GPIO`) is not a key pin, probes for a reversed cable.
4. **V1 spare-line continuity & bridge test (`test_spares`)**: For V1 `MX` / `HE` modules, tests `GPIO 6`, `GPIO 4`, and `GPIO 2` with pull-up/pull-down and drives neighboring pins low one at a time to detect pin-to-pin shorts (`bridged_gpios`).
5. **HE V3 ADS7028 test (`he_adc_test_run`)**: For `BOARD_MODULE_HE_V3`, pauses the streaming SPI sampler (`he_backend_run_paused`) if running, resets and reads ADS7028 status registers, and samples the sensors.

## Hall Effect Bench Mode (`hall_bench.c`)

Compiled only when `CONFIG_OPAD_HALL_BENCH=y` (not a release build; stop `opad-daemon` because it streams plain text over CDC) ([/firmware/main/diag/hall_bench.c](/firmware/main/diag/hall_bench.c)):

- Samples four ADC1 channels at `BENCH_SAMPLE_HZ = 64000` Hz (`BENCH_FRAME_CONVS = 64`, 1 ms frames): Key 1 (`ADC_CHANNEL_9`), Key 2 (`ADC_CHANNEL_6`), module ID (`ADC_CHANNEL_7`), and reference (`ADC_CHANNEL_5`, `GPIO 6`).
- Emits 1 ms averaged lines `H,<ms>,<key1>,<key2>`, 1 s per-channel spread summaries `S,<key>,<n>,<min>,<max>,<mean>,<stddev_x100>,<overflows>`, and pull-cycle phase lines `P,<phase>` for `scripts/hall_bench.py`.
- Also displays a live 2-key voltage/bar view on the LCD (`hall_bench_show_ui`, 33 ms timer, measuring rest during the first `BENCH_REST_MS = 500` ms).
