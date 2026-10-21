# Hall Effect Calibration Plan: Lock Screen, On-Pad & App Flow

Date: 2026-10-20
Author: GFerreiroS
Branch: `feat/he-night`
Worktree: `~/Documents/projects/esp32/osu-pad-esp32-night`

---

## 1. Overview & Requirements Citations

This plan implements the Hall Effect calibration lock screen, on-pad calibration flow, app-driven calibration, daemon auto-launch, and protocol messaging per `docs/specs/v2-hall-effect-decisions.md` §D4 and §D7, subject to the core repository invariants in `AGENTS.md` (§1 product rules, §3 commit conventions, §5 protobuf/sdkconfig constraints, §8 behavior).

### Key Invariant Rules (AGENTS.md §1)
- **Product Rule 1 (Latency always wins):** Nothing joins the keypress path. The frame ISR `on_frame` in `firmware/main/input/he_backend.c` incurs only a single predictable branch (`__builtin_expect(s_calibrating, 0)`) when not calibrating. When locked or calibrating, keypresses to the PC are suppressed by configuring thresholds to `HE_PARAMS_NEVER` (`actuation = INT32_MAX, release = -1`), adding zero branches to `he_rt_step()` or `key_event_report()`.
- **Product Rule 2 (Keyboard with no software):** The pad can be fully calibrated on-pad via the touchscreen without any desktop software installed.
- **Product Rule 3 (Independent updates):** Compatibility both ways (new app + old firmware, old app + new firmware, MX pads unaffected).
- **Product Rule 4 (Touchscreen as third button):** Touchscreen Quick Retry and swipes are gated while locked; lock screen button requires landing inside, moving < `GESTURE_SWIPE_PX` (30 px from `firmware/main/input/gesture.h:12`), and releasing inside.
- **Product Rule 5 (Never BOOT/RESET):** No hardware buttons involved.
- **Product Rule 6 (Every key from a finger):** No macros or automated host key generation.

---

## 2. Doc Discrepancy Correction

`docs/specs/v2-hall-effect-decisions.md` D7 states:
> "Captures rest over a still window (default 100 samples / 25 ms, noise spread ≤ 32 counts... travel span exceeds min_span (≥ 100 counts), and peak-to-peak consistency is within max_peak_spread (≤ 80 counts)."

However, `firmware/main/input/he_cal_capture.c:10-19` (`he_cal_capture_default_cfg()`) defines the authoritative values based on V1.1 bench measurements:
- `rest_window_samples = 64` (64 samples at 1 kHz = 64 ms)
- `rest_max_spread = 36` counts
- `rest_min_raw = 1500`, `rest_max_raw = 2500`
- `target_peaks = 5`
- `min_span = 250` counts
- `press_arm_span = 120` counts
- `release_arm_span = 60` counts
- `max_peak_diff = 120` counts

`docs/specs/v2-hall-effect-decisions.md` will be updated to reflect these actual values from `he_cal_capture.c`.

---

## 3. Protocol Definitions & Exact Tags

All protocol updates are in `protocol/osupad.proto` and compiled with `./scripts/gen_proto.sh`. Hand editing of generated files is strictly forbidden (`AGENTS.md` §5).

### 3.1 New & Modified Fields

1. `message Hello` (`protocol/osupad.proto:32`):
   - `bool he_calibration = 3;` (host indicates it supports HE calibration flow)

2. `message HelloAck` (`protocol/osupad.proto:39`):
   - `bool he_calibration = 16;` (set to `he_backend_running()`, i.e. `true` only when the HE backend is active at runtime and interactive calibration is available)

3. `message HeCalibrationStatus` (`protocol/osupad.proto:104`):
   - Existing fields 1..6: `key1_calibrated`, `key2_calibrated`, `key1_rest`, `key1_full`, `key2_rest`, `key2_full`
   - `bool is_locked = 7;` (pad is currently locked due to missing calibration)
   - `bool is_calibrating = 8;` (pad is currently executing calibration)
   - Sent on `DeviceStatus` (`has_he_calibration = true`) only when `he_backend_running()` is true.

4. Stage & Error Enums:
   ```protobuf
   enum HeCalStage {
       HE_CAL_STAGE_NONE = 0;
       HE_CAL_STAGE_REST = 1;
       HE_CAL_STAGE_KEY1 = 2;
       HE_CAL_STAGE_KEY2 = 3;
       HE_CAL_STAGE_SUCCESS = 4;
       HE_CAL_STAGE_FAILED = 5;
   }

   enum HeCalError {
       HE_CAL_ERROR_NONE = 0;
       HE_CAL_ERROR_REST_NOT_STILL = 1;
       HE_CAL_ERROR_REST_OUT_OF_RANGE = 2;
       HE_CAL_ERROR_REST_PRESSED = 3;
       HE_CAL_ERROR_SIGNAL_NOT_RISING = 4;
       HE_CAL_ERROR_SPAN_TOO_SMALL = 5;
       HE_CAL_ERROR_PEAKS_INCONSISTENT = 6;
       HE_CAL_ERROR_TOO_FEW_PEAKS = 7;
       HE_CAL_ERROR_TOO_MANY_PEAKS = 8;
       HE_CAL_ERROR_TIMEOUT = 9;
   }
   ```

5. Progress Message:
   ```protobuf
   message HeCalibrationProgress {
       HeCalStage stage = 1;
       uint32 key_index = 2;         // 1 = Key 1, 2 = Key 2 (0 during rest/idle/complete)
       uint32 presses_done = 3;      // 0..5
       HeCalError error = 4;
       string error_message = 5;     // Fixed size in osupad.options: max_size 48
       uint32 raw_value = 6;         // Current live raw sensor count
       uint32 key1_rest = 7;         // Populated on HE_CAL_STAGE_SUCCESS
       uint32 key1_full = 8;         // Populated on HE_CAL_STAGE_SUCCESS
       uint32 key2_rest = 9;         // Populated on HE_CAL_STAGE_SUCCESS
       uint32 key2_full = 10;        // Populated on HE_CAL_STAGE_SUCCESS
   }
   ```

6. `HostToDevice` Envelope (`protocol/osupad.proto:413`):
   - `bool start_he_calibration = 19;`
   - `bool cancel_he_calibration = 20;`

7. `DeviceToHost` Envelope (`protocol/osupad.proto:439`):
   - `HeCalibrationProgress he_calibration_progress = 11;`

8. `protocol/osupad.options`:
   - `osupad.HeCalibrationProgress.error_message max_size:48`

---

## 4. Compatibility Matrix (AGENTS.md §1.3)

| Scenario | Old Firmware (pre-calibration UI) | New Firmware (this change) |
|---|---|---|
| **Old App (pre-calibration UI)** | Pad behaves as before. Uncalibrated keys use provisional factory defaults. | Pad is locked if HE uncalibrated. Red lock screen displayed on pad with download URL `https://opad.dev/download` and on-pad Calibrate button. Old app connects, receives `DeviceStatus`, ignores unknown fields, does not crash. Pad can be calibrated on-pad without the app. |
| **New App (with calibration UI)** | App receives `HelloAck.he_calibration == false`. Settings page shows calibration disabled with tooltip "Firmware update required for calibration". No crash. | App receives `HelloAck.he_calibration == true`. If uncalibrated, daemon auto-launches GUI on `Page::Calibration`. Full bidirectional live calibration supported. Settings page shows "Recalibrate" button. |
| **MX Module / MX Firmware** | Normal operation. | `he_backend_running() == false`. Lock is inactive, `he_calibration` is false, no calibration UI or lock screen shown. MX keypath is completely identical. |

---

## 5. File Changes & Architecture

### 5.1 Pure-C Logic & Host Tests
- `firmware/main/input/he_cal_button.h` & `firmware/main/input/he_cal_button.c`:
  Pure-C button hit detector. Parameters: button bounding box (`x, y, w, h`), touch start `(x0, y0)`, max excursion `max_dist` (< `GESTURE_SWIPE_PX`), touch release `(x1, y1)`. Host tested in `firmware/test/host/test_he_cal_button.c`.
- `firmware/main/input/he_cal_runner.h` & `firmware/main/input/he_cal_runner.c`:
  Pure-C multi-stage calibration controller. Coordinates Rest capture on both channels, Key 1 (5 peaks), Key 2 (5 peaks), validation, and calibration packaging. Host tested in `firmware/test/host/test_he_cal_runner.c`.

### 5.2 Firmware Backend & Input Integration
- `firmware/main/input/he_backend.h` & `firmware/main/input/he_backend.c`:
  - `he_backend_is_locked()`: returns `true` if `he_backend_running()` and `(!cal[0].valid || !cal[1].valid)`.
  - When locked or calibrating: thresholds set to `HE_PARAMS_NEVER` (`actuation = INT32_MAX, release = -1`).
  - Sample tapping in `on_frame`: `if (__builtin_expect(s_calibrating, 0))` copies `f.newest[0]` and `f.newest[1]` to calibration runner buffer.
  - `he_backend_start_calibration()` and `he_backend_cancel_calibration()`.
- `firmware/main/input/touch_retry.c`:
  - Gated during lock or calibration: suppresses Quick Retry (`usb_hid_set_touch_retry(true)`) and swipes (`handle_swipe`).
  - Passes touch coordinates to `he_cal_ui_touch_event()`.

### 5.3 Firmware UI
- `firmware/main/ui/he_cal_ui.h` & `firmware/main/ui/he_cal_ui.c`:
  - URL constant: `#define OPAD_DOWNLOAD_URL "https://opad.dev/download"`
  - Red lock screen (dark red `0x5A1010` / `0x7A1F1F` background).
  - 5-second timer from boot: Calibrate button appears after 5s if no capable app connected. If capable app connects (`s_app_capable`), button is hidden and "Calibrate in the OPad app" is displayed.
  - Interactive calibration screen: step prompts ("Release keys...", "Press Key 1 fully 5 times", "Press Key 2 fully 5 times"), 5-box visual progress indicator per key, 1-second "Calibrated!" success message, or error message with "Retry" button.
- `firmware/main/ui/ui_port.c`:
  - Interlocks with `pad_timer_cb()`: when `he_backend_is_locked()` is true, prevents loading `UI_SCREEN_PLAYING`.

### 5.4 Firmware Protocol Handler
- `firmware/main/protocol/protocol.c`:
  - Sets `hello_ack.he_calibration = true` when `CONFIG_OSUPAD_HE_BACKEND` is compiled.
  - Records `he_calibration` capability from incoming `Hello` message (`s_app_capable = msg->payload.hello.he_calibration`).
  - Populates `status.he_calibration.is_locked` and `status.he_calibration.is_calibrating`.
  - Dispatches `start_he_calibration` and `cancel_he_calibration`.
  - Sends `he_calibration_progress` updates.

### 5.5 Desktop App, Daemon & IPC
- `desktop/crates/opad-device/src/lib.rs`:
  - Sets `he_calibration: true` in `hello_message()`.
  - Dispatches `start_he_calibration` and `cancel_he_calibration` to pad over serial.
  - Emits calibration progress events from `DeviceToHost::HeCalibrationProgress`.
- `desktop/crates/opad-model/src/lib.rs`:
  - Exposes `HeCalibrationProgress`, `HeCalStage`, `HeCalError` types.
- `desktop/crates/opad-ipc/src/lib.rs`:
  - Adds `IpcRequest::StartHeCalibration`, `IpcRequest::CancelHeCalibration`, `IpcRequest::GetCalibrationProgress`.
  - Adds `IpcResponse::HeCalibrationProgress`.
- `desktop/daemon/src/runtime.rs` & `desktop/daemon/src/ipc_handlers.rs`:
  - Tracks HE calibration status. When uncalibrated HE pad connects, launches `opad-gui --page calibrate`.
  - Handles calibration IPC requests and forwards them to `opad-device`.
- `desktop/gui/src/main.rs` & `desktop/gui/src/pages.rs`:
  - Registers `Page::Calibration` with token `"calibrate"`.
  - Renders live calibration page: 0..5 press visual boxes, stage transitions, cancel button, retry on error, and calibrated rest/full counts on success.
  - Settings page: adds "Recalibrate Keys" button when connected to HE pad.
  - Walk-away timeout and on-pad cancel gesture are left open with `// OPEN:` comments and documented in the report's "Questions for the owner" section.
  - Disconnect handling: cancels and shows warning if pad is unplugged or app disconnects during calibration.

### 5.6 Test Checklist & Verification
- `docs/testing-checklist.md`:
  - Adds manual verification rows for HE lock screen, button hit test, app launch, calibration flow, recalibration, cancel, and timeout.
- Host tests in `firmware/test/host/run_tests.sh` and `cargo test` across all desktop crates.
