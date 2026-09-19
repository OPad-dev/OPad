# OPad Latency Regression Gate & Benchmark Results

## 1. Testing Principle & Release Gate Criteria (§3, §33)

Input latency is the primary release gate for OPad. Background tasks, display rendering (LVGL), USB CDC communication, and protocol parsing must not degrade keystroke responsiveness or introduce jitter.

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

Linux only — it reads `/dev/input/event*`.

| Date | Stage | Mean interval | Jitter std dev | Max jitter |
|---|---|---|---|---|
| | A | | | |
| | D | | | |

---

## 4b. Windows (§W4-3)

**Not measured.** Nothing below has been run: there is no Windows machine on
the development host, and §A.5 gives the pad to one agent at a time.

**Fill the Linux table above first.** Windows numbers on their own say nothing —
the gate in §1 is a *delta* against Stage A, and a Windows Stage D compared
against a Linux Stage A would be measuring the wrong difference. Run both stages
on each platform and compare within the platform.

### What carries over unchanged

Stages A–D are firmware builds, so they are identical. The on-device histogram
is measured on the pad, between the GPIO edge and the HID submit, and never
touches the host — so **for stages A–C the host operating system is irrelevant
to the number**, and `osupadctl latency` reads the same statistics on Windows
that it does on Linux.

That is the cheap and honest Windows run: same stages, same
`osupadctl latency --reset` / tap / `osupadctl latency` procedure as §3.

### What does not carry over

`scripts/bench_latency.py` uses evdev and has no Windows equivalent, so the
host-side interval and jitter measurement is missing. Three options, in order of
what they cost:

1. **Device-side only.** Report stages A–D from `osupadctl latency` and leave
   the host-side table Linux-only. This measures the firmware, which is what the
   §1 gate is actually written about, and is enough for a release decision.
2. **Raw Input.** A small host tool registering for `WM_INPUT` keyboard events
   and timestamping them. Measures what Linux's evdev script measures, including
   the USB stack and the scheduler, and is the direct equivalent.
3. **ETW.** Trace `Microsoft-Windows-USB-USBPORT` / `-Kernel-Input` with
   `xperf`. The most accurate, and by far the most work to set up and read.

Option 1 is what §W4-3 asks for as a minimum. Option 2 is worth building only if
the Windows and Linux device-side numbers agree but Windows still *feels*
different, because that gap would be in the host stack, which is the only part
option 1 cannot see.

### Windows-specific things to watch

These do not exist on Linux and would not show up in a Linux run:

- **The daemon holding the COM port.** A Windows serial handle is exclusive.
  This costs no latency on the HID path — CDC is a separate endpoint (§1 of the
  packaging plan) — but run Stage D with the daemon connected, not with the port
  free, or the test is not the real configuration.
- **Windows power management.** Disable USB selective suspend on the pad's hub
  before measuring, or the first press after an idle period includes a wake.
  HW-05's HID-first invariant is about the pad's display, and this is the host
  version of the same trap.
- **Timer resolution.** Anything host-side measured with `QueryPerformanceCounter`
  is fine; anything using the default 15.6 ms scheduler tick is not.

### Results

| Date | Firmware commit | Stage | Samples | p50 (µs) | p99 (µs) | p99.9 (µs) | Max (µs) | Deferred | Stuck / missed keys | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| | | A | | | | | | | | |
| | | B | | | | | | | | |
| | | C (10 Hz) | | | | | | | | |
| | | D | | | | | | | | |

---

## 5. Gate Verification Summary

**Pending on both platforms.** The gate passes, per platform, when that
platform's Stage D p99.9 is less than 100 µs above its own Stage A, no stage
shows a new outlier above 1 ms, and no stuck or missed keys were observed.

- **Linux:** not measured. This is P3-1, the last open v1 item (A.8).
- **Windows:** not measured, and blocked on Linux for a baseline (§4b).

`scripts/latency_row.py` turns an `osupadctl latency` run into the markdown row
for either table, so filling these in is flash, tap, paste.
