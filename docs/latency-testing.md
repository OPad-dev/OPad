# osu!pad Latency Regression Gate & Benchmark Results

## 1. Testing Principle & Release Gate Criteria (§3, §33)

Input latency is the primary release gate for osu!pad. Background tasks, display rendering (LVGL), USB CDC communication, and protocol parsing must not degrade keystroke responsiveness or introduce jitter.

### Gate Criteria (§33.3):
1. **Missed Presses**: 0 missed accepted presses during fast alternating streams (> 20 keys/sec).
2. **Key Sticking**: 0 stuck key-down or key-up states under any load condition.
3. **Outliers**: No > 1 ms outliers introduced by auxiliary subsystems (LCD DMA, CDC framing, timer ISRs, NVS writes).
4. **Latency Degradation**: p99.9 input-to-HID-submit latency delta relative to baseline (Stage A) must remain below 0.1 ms (100 µs).
5. **No Core 0 Contention**: The key ISR, keypad task and TinyUSB task run on Core 0. Display, protocol and runtime tasks run on Core 1.

### Measurement resolution

The on-device histogram (`firmware/main/input/latency_stats.c`) uses 10 µs buckets, so **p50, p99 and p99.9 are always multiples of 10 µs** (the upper edge of the bucket). `max` is exact. Deltas smaller than 10 µs cannot be resolved with these stats.

---

## 2. Benchmark Stages

- **Stage A: HID-Only Firmware Baseline**
  Built with `CONFIG_OSUPAD_BENCH_HID_ONLY=y`. GPIO interrupt + eager debounce with lockout re-sample + TinyUSB HID keyboard only, 1000 Hz endpoint. UI, CDC protocol task and runtime supervisor are disabled.
  TinyUSB owns the USB PHY and there is no CDC in this build, so the stats printed every 10 s only reach **UART0** (TX = GPIO43, 115200 baud). Reading them needs a USB-UART adapter on that pin.
- **Stage B: HID + CDC Protocol (No Display)**
  Built with `CONFIG_OSUPAD_BENCH_NO_DISPLAY=y`. Daemon connected via USB CDC-ACM (heartbeats, status requests, telemetry frames). Display UI and backlight PWM are disabled.
- **Stage C: HID + CDC + Gameplay Display**
  Standard firmware with the daemon connected and active gameplay streaming via tosu (PP, progress bar, map press counters). Run at the default gameplay display rate (10 Hz) and at the 2 Hz and 30 Hz limits.
- **Stage D: Full Stack Integration**
  Everything at once: tosu streaming, LVGL rendering, post-cooldown and idle periodic sync, diagnostics logging.

---

## 3. Testing Procedure

1. **Firmware configuration**
   - Stage A: enable `CONFIG_OSUPAD_BENCH_HID_ONLY`.
   - Stage B: enable `CONFIG_OSUPAD_BENCH_NO_DISPLAY`.
   - Stages C and D: standard build.
   - Optional: `CONFIG_OSUPAD_BENCH_DEBUG_GPIO=y` toggles a pin right after the HID submit for logic-analyzer measurement. The default pin is GPIO 4; **check that it is free on the board header before enabling it**.

2. **Execution**
   - Flash the build for the stage.
   - Reset the stats: `osupadctl latency --reset` (Stage A: reboot the pad).
   - Tap both keys alternately and fast (> 15 presses/s) for 2 minutes.
   - Read the stats: `osupadctl latency` (Stage A: UART0 output).
   - Optionally record host-side evdev intervals with `python3 scripts/bench_latency.py`.

---

## 4. Benchmark Results

**Not measured yet.** Fill this table only with numbers from real hardware runs, one row per run.

| Date | Firmware commit | Stage | Samples | p50 (µs) | p99 (µs) | p99.9 (µs) | Max (µs) | Deferred | Stuck / missed keys | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| | | A | | | | | | | | |
| | | B | | | | | | | | |
| | | C (2 Hz) | | | | | | | | |
| | | C (10 Hz) | | | | | | | | |
| | | C (30 Hz) | | | | | | | | |
| | | D | | | | | | | | |

### Host-side evdev (`scripts/bench_latency.py`)

| Date | Stage | Mean interval | Jitter std dev | Max jitter |
|---|---|---|---|---|
| | A | | | |
| | D | | | |

---

## 5. Gate Verification Summary

Pending the measurements above. The gate passes when Stage D's p99.9 is less than 100 µs above Stage A's, no stage shows a new outlier above 1 ms, and no stuck or missed keys were observed.
