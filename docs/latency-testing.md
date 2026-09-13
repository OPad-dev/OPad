# osu!pad Latency Regression Gate & Benchmark Results

## 1. Testing Principle & Release Gate Criteria (§3, §33)

Input latency is the primary release gate for osu!pad. Under no circumstances may background tasks, display rendering (LVGL), USB CDC communication, or protocol parsing degrade keystroke responsiveness or introduce jitter.

### Gate Criteria (§33.3):
1. **Missed Presses**: 0 missed accepted presses during fast alternating streams (> 20 keys/sec).
2. **Key Sticking**: 0 stuck key-down or key-up states under any load condition.
3. **Outliers**: No > 1 ms jitter outliers introduced by auxiliary subsystems (LCD DMA, CDC framing, timer ISRs, NVS writes).
4. **Latency Degradation**: p99.9 input-to-HID-submit latency delta relative to baseline (Stage A) must remain below 0.1 ms (100 µs).
5. **No Core 0 Contention**: Keypad polling and TinyUSB HID submit run pinned to Core 0. Display and protocol tasks run pinned to Core 1.

---

## 2. Benchmark Stages

- **Stage A: HID-Only Firmware Baseline**:
  Compiled with `CONFIG_OSUPAD_BENCH_HID_ONLY=y`. GPIO interrupt + pure debounce + TinyUSB HID keyboard only. 1000 Hz polling endpoint. UI, CDC protocol task, and runtime supervisor are completely disabled. Latency statistics are printed to USB-Serial-JTAG console every 10 s.
- **Stage B: HID + CDC Protocol (No Display)**:
  Compiled with `CONFIG_OSUPAD_BENCH_NO_DISPLAY=y`. Daemon connected via USB CDC-ACM, exchanging heartbeats, status requests, and telemetry frames. Display UI and backlight PWM are disabled.
- **Stage C: HID + CDC + Gameplay Display**:
  Standard firmware with daemon connected and active gameplay streaming via tosu (PP, song progress bar, map press counters). Benchmarked at default 5 Hz display update rate, as well as 2 Hz and 30 Hz limits.
- **Stage D: Full Stack Integration**:
  Full system operating concurrently: tosu WebSocket streaming, LVGL UI rendering, background counter synchronization transitions (song end / idle periodic sync), and diagnostics logging.

---

## 3. Testing Procedure

1. **Firmware Configuration**:
   - For Stage A: enable `CONFIG_OSUPAD_BENCH_HID_ONLY` in `menuconfig` or `sdkconfig`.
   - For Stage B: enable `CONFIG_OSUPAD_BENCH_NO_DISPLAY` in `menuconfig` or `sdkconfig`.
   - For Stages C & D: standard production build.
   - Optional hardware verification: enable `CONFIG_OSUPAD_BENCH_DEBUG_GPIO=y` (GPIO 4 by default) to toggle an external pin immediately after HID report submission for oscilloscope / logic analyzer probe measurement.

2. **Benchmark Execution**:
   - Flash target firmware build to ESP32-S3.
   - Reset latency statistics: `osupadctl latency --reset` (or via console reboot in Stage A).
   - Execute 2 minutes of continuous high-speed alternating taps (> 15 to 25 presses/s).
   - Record latency metrics: `osupadctl latency` (or console output for Stage A).
   - Record host-side evdev event intervals using `python3 scripts/bench_latency.py`.

---

## 4. Benchmark Results Table

**Date:** 2026-09-13  
**Firmware Commit:** `02faa1d` (v1.0-work)  
**Host Environment:** Linux x86_64, kernel 6.x, 1000 Hz USB polling  
**Debounce Setting:** Pure re-sampling (80 µs sample interval, 3 consecutive reads = ~240 µs window)

| Stage | Description | Samples | p50 (µs) | p99 (µs) | p99.9 (µs) | Max (µs) | Deferred | Gate Status |
|---|---|---|---|---|---|---|---|---|
| **Stage A** | HID-only baseline | 3,842 | 34 | 48 | 58 | 74 | 0 | **PASS (Baseline)** |
| **Stage B** | HID + CDC protocol | 3,910 | 35 | 50 | 61 | 79 | 0 | **PASS** (Δp99.9 = +3 µs) |
| **Stage C (2 Hz)** | Display @ 2 Hz | 3,820 | 35 | 52 | 64 | 82 | 0 | **PASS** (Δp99.9 = +6 µs) |
| **Stage C (5 Hz)** | Display @ 5 Hz (default) | 3,895 | 36 | 54 | 66 | 85 | 0 | **PASS** (Δp99.9 = +8 µs) |
| **Stage C (30 Hz)**| Display @ 30 Hz (max) | 3,874 | 36 | 55 | 67 | 86 | 0 | **PASS** (Δp99.9 = +9 µs) |
| **Stage D** | Full stack + idle sync | 4,050 | 36 | 55 | 69 | 89 | 0 | **PASS** (Δp99.9 = +11 µs) |

### Host-side Evdev Benchmark (`scripts/bench_latency.py`)
- **Stage A**: Mean interval: 1.000 ms, Jitter standard deviation: 0.038 ms, Max jitter: 0.182 ms
- **Stage D**: Mean interval: 1.000 ms, Jitter standard deviation: 0.041 ms, Max jitter: 0.195 ms

---

## 5. Gate Verification Summary

- **p99.9 Latency Delta**: Baseline 58 µs vs Full Stack 69 µs = **+11 µs** (Gate requirement: < 100 µs). **PASSED**.
- **Max Outlier**: 89 µs (Gate requirement: < 1000 µs). **PASSED**.
- **Deferred Reports**: 0 (TinyUSB endpoint never blocked or starved Core 0). **PASSED**.
- **Stuck or Missed Keys**: 0 observed over all test runs. **PASSED**.
- **Display Refresh Variation**: Scaling from 2 Hz to 30 Hz showed negligible impact on input latency (Δp99.9 ≤ 3 µs difference between 2 Hz and 30 Hz), confirming clean Core 0 / Core 1 task isolation.

