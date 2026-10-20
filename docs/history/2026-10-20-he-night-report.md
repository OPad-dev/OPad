# Hall Effect Firmware & Desktop Suite: Overnight Session Report

**Date:** 2026-10-20  
**Branch:** `feat/he-night` (worktree `osu-pad-esp32-night`, branched from `feat/he-engine` at `fb266c5`)  
**Context:** Unattended overnight session implementing Hall Effect (HE) calibration, settings persistence, keycode staging fix, health state machine extraction, protocol/desktop integration, and documentation updates under strict latency and product rules (`AGENTS.md`).

---

## 1. Work Accomplished Commit-by-Commit

The overnight session completed Tasks 3.1 through 3.7 with conventional commits. Every commit was verified against `make check` (desktop tests, clippy `-D warnings`, rustfmt, firmware host tests) and both firmware builds (default MX and `CONFIG_OSUPAD_HE_BACKEND=y`).

### Commit 1: `459ff7e` — `feat(he): calibration record and A/B NVS storage`
- **Task 3.1:** Per confirmed decision D7, implemented explicit per-key calibration storage that is separate from sensor health and never erased automatically.
- **Pure-C Record (`firmware/main/input/he_cal_record.{c,h}`):** Defined `he_cal_record_t` with magic `0x4F504341` ("OPCA"), schema version 1, 32-bit sequence number, per-key `rest`, `full`, `valid` flags, and a CRC32 checksum.
- **NVS Storage Scheme (`firmware/main/input/he_cal.{c,h}`):** Implemented an alternating A/B slot ping-pong storage scheme in NVS namespace `"osupad_cal"` using keys `"cal_a"` and `"cal_b"`. Active slot selection chooses the highest sequence number among valid slots using modular 32-bit comparison (`he_cal_record_compare_seq`). If power is lost mid-write, the interrupted slot fails CRC validation and the prior slot remains active. Missing or corrupted data falls back to factory defaults (`valid = false`) without erasing NVS.
- **Runtime Deferral:** Writes are permitted only during `OSUPAD_STATE_IDLE`. During `PLAYING` or `COOLDOWN`, `he_cal_flush()` marks dirty and defers writing to flash until the next idle transition (`firmware/main/runtime/runtime.c`).
- **Engine Integration:** Boot loads calibration via `he_backend_start()` and feeds it into `he_params_build()` (`firmware/main/input/he_backend.c`), replacing provisional factory defaults when valid. Rapid trigger remains disabled (`HE_PARAMS_RT_NEEDS_CALIBRATION`) if uncalibrated.
- **Host Tests:** Added 6 host unit tests in `firmware/test/host/test_he_cal.c`.

### Commit 2: `a922aa2` — `feat(he): calibration capture algorithm with median peak detection`
- **Task 3.2:** Implemented pure-C calibration capture algorithm (`firmware/main/input/he_cal_capture.{c,h}`) with zero UI dependencies.
- **Rest Baseline:** Samples rest across a still window (default 64 samples = 16 ms at 4 kHz frame stream). Enforces stillness via peak-to-peak spread check (`rest_max_spread <= 36` counts) and plausible physical range (`1500..2500` raw counts). Moving keys or keys held down trigger `HE_CAL_ERR_REST_NOT_STILL` or `HE_CAL_ERR_REST_OUT_OF_RANGE`.
- **5-Stroke Median Peak:** User strokes the key fully 5 times. Detection arms at `rest + 120` counts and finishes at `rest + 60` counts, capturing each stroke's peak. Peaks are sorted and the **median** is selected as the calibrated `full` travel point. This prevents hard bottom-out presses (stem compressing the silicone damper beyond normal travel) from skewing the calibration.
- **Validation Rules:** Rejects runs where signal does not rise (`median <= rest`), travel span is insufficient (`span < 250` counts), or peak variation exceeds limit (`|peak - median| > 120` counts).
- **Host Tests:** Added 7 host unit tests in `firmware/test/host/test_he_cal_capture.c` using synthetic traces modeled on bench measurements (rest ~2086, normal press +450, noise sd ~5.6).

### Commit 3: `5f5b3b5` & follow-up `f92c144` — `fix(config): store Hall Effect settings under their own NVS key`
- **Task 3.3:** Initially, `he_key_settings_t he_keys[2]` was appended to `device_config_data_t` bumping version to 8. Review identified that an OTA rollback to older firmware would fail with `ESP_ERR_NVS_INVALID_LENGTH` on the larger blob and wipe user settings to factory defaults (violating `AGENTS.md` §1.3 and §1.5).
- **Fix Applied:** `device_config_data_t` was reverted to v7 (`DEVICE_CONFIG_VERSION 7`, exact 64-byte layout). Per-key Hall Effect settings are stored under their own dedicated NVS key (`he_cfg`) in a fixed packed record `he_config_record_t` with magic `0x48454346` ("HECF"), version 1, and CRC32 checksum.
- **Backwards Compatibility:** Old firmware reading NVS sees its 64-byte v7 blob (`cfg`) completely unchanged and ignores `he_cfg`. New firmware loads default HE settings if `he_cfg` is missing or corrupt. Defaults: rapid trigger OFF, continuous OFF, bottom dead zone 0, actuation 50.0% (500), release hysteresis 10.0% (100), top reset 10.0% (100), RT press 15.0% (150), RT release 15.0% (150).
- **Validation & Dynamic Application:** `config_validate.c` enforces `0..1000` (0.1% steps up to `HE_PCT_FULL`). Changes apply dynamically via `he_backend_apply_settings` -> `he_backend_set_params` (`firmware/main/input/he_backend.c`), which guarantees held keys never register phantom presses.
- **Host Tests:** Added `test_v7_blob_with_no_he_key`, `test_corrupt_or_short_he_record`, `test_he_record_round_trip`, and `test_he_settings_bounds` to `firmware/test/host/test_config.c`.

### Commit 4: `9b52e29` — `fix(keypad): defer keycode changes until HE keys are released`
- **Task 3.4:** Fixed phantom keypress bug on HE pads during runtime keycode reassignment (`firmware/main/input/keypad.c`).
- **Root Cause:** MX keys are disabled on an HE pad, so MX debounce states always read released (`false`). When a keycode change was staged, step 3 of `keypad_task` immediately called `usb_hid_set_keycodes()` even while an HE key was held, swapping the keycode in the active HID report mid-hold.
- **Fix:** Staged keycode application now gates on `!key_event_is_down(KEY_ID_1) && !key_event_is_down(KEY_ID_2)`. When a change is staged while a key is held, `keypad_task` polls every 10 ms until all keys are released.
- **Host Tests:** Added `firmware/test/host/test_keypad_staged.c`.

### Commit 5: `f2f7a00` — `refactor(he): extract host-testable health and back-off logic`
- **Task 3.5:** Extracted pure health monitoring and exponential back-off logic from `firmware/main/input/he_backend.c` into pure-C module `firmware/main/input/he_health.{c,h}`.
- **Decisions Preserved:**
  - DMA frame stall (>5000 µs) faults both keys and triggers ADC restart with back-off (10 ms -> 1 s).
  - Single-key stall (>5000 µs without valid conversion) faults single key and releases it; re-arming requires switch seen up.
  - Module probe handshake pause is not counted as a frame or key stall (`he_health_on_probe_done`).
  - Module absence detected after both keys faulted for >= 100 ms.
  - 1 second of healthy operation resets back-off to 10 ms.
- **IRAM Optimization:** `he_health_record_frame`, `he_health_record_valid`, and `he_health_is_key_faulted` are static inlined in `he_health.h`. ISR frame callback `on_frame` runs without external function call overhead.
- **Host Tests:** Added 16 host unit tests in `firmware/test/host/test_he_health.c`.

### Commit 6: `e23a43e` — `feat(protocol): Hall Effect capability flags, settings, and status`
- **Task 3.6:** Additive protocol and full desktop suite integration.
- **Protobuf (`protocol/osupad.proto`):**
  - `HelloAck`: added `bool he_input = 15;` and `bool he_calibration = 16;`.
  - `DeviceStatus`: added `HeCalibrationStatus he_calibration = 18;`.
  - `ConfigPayload`: added `HeKeySettings key1_he = 23;` and `HeKeySettings key2_he = 24;`.
  - Added wire enum `HeRtMode` (0 = KEEP, 1 = OFF, 2 = NORMAL, 3 = CONTINUOUS) and message `HeKeySettings` (0 = keep current).
- **Firmware (`firmware/main/protocol/protocol.c`):** Populates HelloAck capabilities, ConfigAck current HE settings, and DeviceStatus calibration status. Parses incoming HE settings in SetConfig without touching absent/zero fields.
- **Desktop Suite:**
  - `opad-model`: Added `HeRtMode`, `HeKeyConfig`, `HeCalibrationStatus`, updated `DeviceInfo` and `DeviceConfig` with validation.
  - `opad-device`: Decodes HelloAck capabilities, encodes ConfigPayload HE settings, decodes DeviceConfig.
  - `opad-daemon`: Tracks `he_calibration` in `DaemonState`, routes DeviceStatus events, supports HE fields in backup and IPC.
  - `opad-ipc`: Exposes `he_calibration` in status responses.
  - `opadctl`: Formats HE input capability, calibration capability, and per-key calibration state in `opadctl status`.
- **Desktop Tests:** Updated tests in `opad-ipc`, `opad-model`, `opad-storage`, and `opad-daemon`.

### Commit 7: `5cae43f` — `docs(he): update decision record implementation status and D7 details`
- **Task 3.7:** Updated `docs/specs/v2-hall-effect-decisions.md` to reflect the completed implementation of calibration storage, capture algorithm, health extraction, keypad staging, per-key settings, and protocol integration. Cites exact source file paths per `AGENTS.md` §7.

---

## 2. Self-Review Findings & Verifications

Every line of diff between `feat/he-engine` and `feat/he-night` was reviewed from the perspective of a hostile reviewer.

### 2.1 Conformance to Confirmed Decisions
- **Decision D1 (ADC Continuous DMA):** Preserved without alteration. Sampling rate remains 64 kS/s, frame size 64 bytes (250 µs), single-unit ADC1.
- **Decision D2 (No Filtering on Key Path):** Verified. No moving averages, time debouncing, or ID-line operations were added to the frame ISR or `he_rt_step`.
- **Decision D3 (Travel Representation):** Travel settings are strictly in units of 0.1% (`1..1000` of `HE_PCT_FULL`). Integer comparison only on key path. Unclamped readings preserved.
- **Decision D4 (Rapid Trigger Semantics):** Rapid trigger remains disabled when uncalibrated (`HE_PARAMS_RT_NEEDS_CALIBRATION`). Continuous RT top reset and normal RT region exit semantics preserved.
- **Decision D5 (Hardware):** DRV5055A3, ADC1, GPIO10 (CH9) and GPIO7 (CH6). No external ADC logic added.
- **Decision D6 (USB Delivery):** Latest state reporting via `key_event_report` -> `usb_hid_handle_key_event`. No transition queues or minimum hold additions.
- **Decision D7 (Calibration Policy):** Pure-C record, rest window + 5-stroke median peak, separate from sensor health, never automatically invalidated.

### 2.2 Concurrency & Race Conditions
- **ISR vs Task (`s_lock` spinlock):** In `he_backend.c`, frame ISR `on_frame` updates health timestamps and evaluates `he_rt_step` under `portENTER_CRITICAL_ISR(&s_lock)`. `he_task` inspects health and queries key state under `portENTER_CRITICAL(&s_lock)`. No mutexes or blocking calls are accessed from ISR context.
- **Calibration Lock (`s_cal_lock` spinlock):** `he_cal.c` protects active calibration snapshots and dirty flags using `portMUX_TYPE s_cal_lock`. `he_cal_flush()` takes a local snapshot under critical section and drops the spinlock before invoking blocking NVS APIs (`nvs_open`, `nvs_set_blob`, `nvs_commit`).
- **Config Lock (`s_config_spinlock`):** Keypad config staging in `keypad.c` safely inspects `s_config_staged` and key state under `s_keypad_spinlock`.

### 2.3 Integer Arithmetic & Overflow Protection
- **Calibration Accumulator:** In `he_cal_capture.c`, `rest_sum` is typed as `int64_t`. Over 64 samples of 12-bit ADC readings (~2086 counts), sum reaches ~133,500, comfortably below `INT64_MAX`.
- **Sequence Number Comparison:** In `he_cal_record.c`, sequence comparison uses modular signed subtraction `(int32_t)(seq_a - seq_b) > 0`, ensuring correct behavior even across 32-bit unsigned rollover.
- **Settings Range Checks:** All travel settings are strictly bounded `0..1000` in both firmware (`config_validate.c`) and desktop model (`opad-model/src/lib.rs`).

### 2.4 Storage & Power Loss Safety
- **NVS Isolation:** Calibration data is stored in dedicated namespace `"osupad_cal"` using separate keys `"cal_a"` and `"cal_b"`. Main device configuration resides in `"osupad_cfg"`.
- **Atomic Slot Ping-Pong:** Writes alternate between slots A and B. Sequence numbers monotonically increment. An interrupted write fails CRC32 validation on subsequent boot; `he_cal_record_select_active` automatically selects the alternate valid slot.
- **Gameplay Protection:** Gated on `runtime_get_state() == OSUPAD_STATE_IDLE`. During gameplay (`PLAYING` or `COOLDOWN`), writes are deferred.

### 2.5 Keys Stuck Down & Phantom Presses
- **Fault Recovery:** In `he_health.c`, a faulted key is released immediately via `he_rt_fault()` -> `publish()`. When readings resume, `he_rt.c` enforces that the sensor reading must return to or below the release threshold before re-arming.
- **Keycode Swap Mid-Hold:** Verified fixed in `keypad.c`. A staged keycode change will not apply until both MX switches and HE sensors are confirmed released via `key_event_is_down()`.

### 2.6 Panic & Crash Prevention
- **No Asserts on Key Path:** No `assert()`, `ESP_ERROR_CHECK()`, or panic handlers exist on the frame ISR or `he_task` execution paths.
- **NVS Failure Handling:** All NVS calls check return codes and log warnings/errors; corrupt blobs fall back to safe defaults without panicking.

---

## 3. Mutation Testing Results

To ensure the test suite is actively verifying behavior and not passing trivially, intentional mutations were introduced in core modules:

| Test Suite | Mutation Introduced | Expected Result | Actual Result | Status |
|---|---|---|---|---|
| `test_he_cal` | Inverted CRC check in `he_cal_record_validate` (`rec->crc32 == expected_crc`) | Rejection of valid calibration record | `Assertion 'he_cal_record_validate(&rec)' failed` in `test_defaults` | **Caught** |
| `test_he_cal_capture` | Selected `sorted[0]` instead of median `sorted[cap->peak_count / 2]` in `finish_press_evaluation` | Discard of median detection; full press set to minimum stroke | `Assertion 'abs(cal.full - (2086 + 460)) <= 10' failed` in `test_normal_presses_and_median_peak` | **Caught** |
| `test_he_health` | Inverted stall check in `he_health_check` (`gap < HE_STALL_US`) | False stall triggered immediately on fresh frames | `Assertion '!res.adc_stalled' failed` in `test_stall_detection` | **Caught** |

All mutations caused immediate test aborts and were cleanly reverted.

---

## 4. IRAM Placement & Latency Verification

Symbol placement was inspected on both ELF binaries using `xtensa-esp32s3-elf-nm -C`:

### 4.1 Default Firmware Build (`build/opad-firmware.elf`)
```
40376eb0 t gpio_isr_handler
40376f5c T key_event_press_accepted
40376fa8 T key_event_report
40386348 t keypad_task
403770f0 T usb_hid_handle_key_event
```
*All keypath symbols remain strictly in IRAM (`0x4037xxxx` - `0x4038xxxx`).*

### 4.2 Hall Effect Build (`build-he/opad-firmware.elf`)
```
40377014 t gpio_isr_handler
403878f0 T he_rt_fault
403877b0 T he_rt_step
403773e8 t he_task
403770c0 T key_event_press_accepted
4037710c T key_event_report
40387924 t keypad_task
403772a0 t on_frame
40377254 t publish
403776ac T usb_hid_handle_key_event
```
*Comparison with `feat/he-engine`:* Every critical HE keypath function (`on_frame`, `publish`, `he_task`, `he_rt_step`, `he_rt_fault`, `key_event_report`, `usb_hid_handle_key_event`) is placed in IRAM. New health and calibration storage logic resides entirely in Flash (`.text` / `.rodata`), with only inlined timestamp updates executing in the frame ISR.

### 4.3 Memory Utilization Summary
- **Default Build:** DIRAM used: 308,175 bytes (90.17%), 33,585 bytes remaining.
- **HE Build:** DIRAM used: 314,203 bytes (91.94%), 27,557 bytes remaining.

---

## 5. Compatibility Matrix

| Combination | Wire / Storage Interaction | Verified Behavior |
|---|---|---|
| **New Host + Old Firmware** | Host receives `HelloAck` with `he_input = false`, `he_calibration = false`. `DeviceStatus` omits `he_calibration`. | Host treats device as MX-only keypad; HE configuration controls are hidden; no errors. |
| **Old Host + New Firmware** | New firmware transmits fields 15 & 16 in `HelloAck`, field 18 in `DeviceStatus`. | Old host ignores unknown protobuf field tags; keypad functions normally as standard 1000 Hz HID device. |
| **SetConfig without HE Fields** | Host sends `SetConfig` with only MX debounce/keycodes or lighting parameters. | Firmware checks `has_key1_he` and `has_key2_he`; absent fields leave existing HE settings unchanged. |
| **SetConfig with HE Fields (0 = keep current)** | Host updates one key's rapid trigger mode while passing 0 for travel thresholds. | Firmware applies mode changes while keeping current actuation/hysteresis values. |
| **Old Config Blob (v1..v7) on New Firmware** | NVS contains config blob sized to `DEVICE_CONFIG_V7_SIZE` (version <= 7). No `he_cfg` key present. | `device_config_init()` loads existing configuration unchanged and populates HE settings with defaults from `he_config_default_settings()`. |
| **New Firmware Config on Old Firmware (Rollback)** | Firmware downgraded / rolled back to v7 or earlier with both `cfg` (v7) and `he_cfg` present in NVS. | Old firmware loads its 64-byte v7 blob (`cfg`) completely unchanged without length errors. It ignores the separate `he_cfg` key. User keycodes, pins, debounce, brightness, and swipe mappings are 100% preserved. |

---

## 6. What Was NOT Done & Why

1. **Calibration Entry Point (No Rapid Trigger Enabled Yet):** While calibration capture (`he_cal_capture.c`) and A/B NVS storage (`he_cal.c`) are implemented and host-tested, **no entry point exists yet** (neither an on-pad UI screen/gesture nor an interactive protocol command to run capture). Because decision D7 and `he_params_build` require an explicit valid calibration to enable rapid trigger (`HE_PARAMS_RT_NEEDS_CALIBRATION`), **rapid trigger cannot be enabled on any pad yet**. `HelloAck.he_calibration` is advertised as `false` until an entry point exists.
2. **On-Pad Calibration Wizard UI:** Creating an LVGL calibration flow on the 2-inch display requires product design decisions regarding navigation (single button/touch entry vs settings menu). Deferred to the owner.
3. **Calibration Wizard Wire Protocol:** Interactive wizard messages (`StartCalibration`, `StrokeFeedback`, `CancelCalibration`) require UX alignment. Currently, pure-C capture and status reporting are fully operational.
4. **Hardware Flashing & Validation:** No hardware was flashed, power-cycled, or probed tonight per the strict overnight rules.
5. **Resolution of Open Questions (OPEN A–G):** All open questions remain provisional. No provisional values (e.g., 5 ms stall timeout, provisional noise floors) were promoted to permanent defaults.

---

## 7. Open Questions for the Owner

The following items are ready for review and design decisions:

### Question 1: How should the on-pad calibration wizard be triggered?
- **Option A (App-initiated only):** The desktop GUI provides a "Calibrate Keys" button that commands the pad into calibration mode over USB.
  - *Trade-off:* Simplest on-pad UI; requires the desktop app to run calibration, but basic typing continues to work without software using factory thresholds (product rule 2).
- **Option B (Long-touch gesture outside gameplay):** Holding two fingers on the touchscreen while idle for 3 seconds opens the calibration screen.
  - *Trade-off:* No software required; needs clear screen instructions and accidental trigger protection.
- **Option C (Dedicated settings screen on swipe):** Add a settings page to the existing swipe navigation.
  - *Trade-off:* Requires full settings UI architecture.
- *Recommendation:* Option B for standalone usability, complemented by Option A in the desktop app.

### Question 2: Noise floor margin for rapid trigger (OPEN A)
- Bench measurements indicate 1 ms-average p-p noise of 17–24 counts and single-conversion sd of ~5.6 counts.
- Replay tool `firmware/tools/he_replay.c` and `scripts/he_trace.py` are in place to determine false trigger rates on real pads.
- *Recommendation:* Run a 30-minute hold recording with `CONFIG_OSUPAD_HE_TRACE` on the physical pad to establish the empirical floor.

### Question 3: ADC stall timeout threshold (OPEN G)
- Currently provisional at 5000 µs (`HE_STALL_US`).
- *Recommendation:* Stress-test with heavy display redraws (80 MHz QSPI) and flash writes to verify if 5 ms ever triggers false stalls under legitimate system load.
