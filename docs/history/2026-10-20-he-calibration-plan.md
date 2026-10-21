# Hall Effect Calibration Plan: Lock Screen, On-Pad & App Flow

Date: 2026-10-20
Author: GFerreiroS
Branch: `feat/he-night`
Worktree: `~/Documents/projects/esp32/osu-pad-esp32-night`

Implementation corrections on 2026-10-21 are recorded in
`docs/history/2026-10-21-he-calibration-report.md`. This plan describes intended
and implemented code paths; it is not evidence of operation on hardware.

---

## 1. Overview & Requirements Citations

This plan implements the Hall Effect calibration lock screen, on-pad calibration flow, app-driven calibration, daemon auto-launch, and protocol messaging per `docs/specs/v2-hall-effect-decisions.md` §D4 and §D7, subject to the core repository invariants in `AGENTS.md` (§1 product rules, §3 commit conventions, §5 protobuf/sdkconfig constraints, §8 behavior).

### Key Invariant Rules (AGENTS.md §1)
- **Product Rule 1 (Latency always wins):** Nothing joins the keypress path. The frame ISR `on_frame` in `firmware/main/input/he_backend.c` incurs only a single predictable branch (`__builtin_expect(s_calibrating, 0)`) when not calibrating. When locked or calibrating, keypresses to the PC are suppressed by configuring thresholds to `HE_PARAMS_NEVER` (`actuation = INT32_MAX, release = -1`), adding zero branches to `he_rt_step()` or `key_event_report()`.
- **Product Rule 2 (Keyboard with no software):** The pad can be fully calibrated on-pad via the touchscreen without any desktop software installed.
- **Product Rule 3 (Independent updates):** Compatibility both ways (new app + old firmware, old app + new firmware, MX pads unaffected).
- **Product Rule 4 (Touchscreen as third button):** Touchscreen Quick Retry and swipes are gated while locked; lock screen button requires landing inside, moving < `GESTURE_SWIPE_PX` (30 px from `firmware/main/input/gesture.h`), and releasing inside.
- **Product Rule 5 (Never BOOT/RESET):** No hardware buttons involved.
- **Product Rule 6 (Every key from a finger):** No macros or automated host key generation.

---

## 2. Doc Discrepancy Correction

At base commit `0fbcb6d`, `docs/specs/v2-hall-effect-decisions.md` D7 stated:
> "Captures rest over a still window (default 100 samples / 25 ms, noise spread ≤ 32 counts... travel span exceeds min_span (≥ 100 counts), and peak-to-peak consistency is within max_peak_spread (≤ 80 counts)."

However, `firmware/main/input/he_cal_capture.c` (`he_cal_capture_default_cfg()`) defines the configured values:
- `rest_window_samples = 64` (64 frame samples at 4 kHz = 16 ms;
  `firmware/main/input/he_cal_capture.c`, `he_backend.c`)
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

1. `message Hello` (`protocol/osupad.proto`):
   - `bool he_calibration = 3;` (host indicates it supports HE calibration flow)

2. `message HelloAck` (`protocol/osupad.proto`):
   - `bool he_calibration = 16;` (set to `he_backend_running()`, i.e. `true` only when the HE backend is active at runtime and interactive calibration is available)

3. `message HeCalibrationStatus` (`protocol/osupad.proto`):
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

6. `HostToDevice` Envelope (`protocol/osupad.proto`):
   - `bool start_he_calibration = 19;`
   - `bool cancel_he_calibration = 20;`

7. `DeviceToHost` Envelope (`protocol/osupad.proto`):
   - `HeCalibrationProgress he_calibration_progress = 11;`

8. `protocol/osupad.options`:
   - `osupad.HeCalibrationProgress.error_message max_size:48`

---

## 4. Compatibility Matrix (AGENTS.md §1.3)

| Scenario | Old Firmware (pre-calibration UI) | New Firmware (this change) |
|---|---|---|
| **Old App (pre-calibration UI)** | Pad behaves as before. Uncalibrated keys use provisional factory defaults. | Pad is locked if HE uncalibrated. Red lock screen displayed on pad with download URL `https://opad.dev/download` and on-pad Calibrate button. Old app connects, receives `DeviceStatus`, ignores unknown fields, does not crash. Pad can be calibrated on-pad without the app. |
| **New App (with calibration UI)** | App receives `HelloAck.he_calibration == false` and shows no calibration entry (`desktop/gui/src/pages.rs`, Keypad settings); the daemon ignores any `HeCalibrationStatus` from that firmware (`desktop/daemon/src/runtime.rs`, `DeviceStatus` handler). No crash. | App receives `HelloAck.he_calibration == true`. If uncalibrated, daemon auto-launches GUI on `Page::Calibration`. Full bidirectional live calibration supported. Settings page shows "Recalibrate" button. |
| **MX Module / MX Firmware** | Normal operation. | `he_backend_running() == false`. Lock is inactive, `he_calibration` is false, no calibration UI or timer is created (`firmware/main/ui/he_cal_ui.c`, `firmware/main/protocol/protocol.c`). MX key reporting is not edited by this batch (`firmware/main/input/keypad.c`, `key_event.c`); latency still needs hardware measurement. |

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
  - Sample tapping in `on_frame`: `if (__builtin_expect(s_calibrating, 0))`
    accumulates the ordered newest conversion pair from every 250 µs frame,
    plus each key's min/max. Two bounded 512-frame buffers are swapped under
    `s_lock`; the LVGL timer drains the detached buffer every 20 ms. Overflow
    or an invalid frame fails the attempt. A min/max pair per timer tick alone
    cannot preserve press/release order (`firmware/main/input/he_cal_samples.h`,
    `he_backend.c`, `he_cal_runner.c`, `firmware/main/ui/he_cal_ui.c`).
  - The runner discards 64 frame samples (16 ms) to settle, then averages 64
    frame samples (16 ms) at rest. The timer delays presentation, not sampling
    (`firmware/main/input/he_cal_runner.c`, `he_cal_capture.c`).
  - `he_backend_start_calibration()` and `he_backend_cancel_calibration()`
    serialize with settings/finish through a task-only mutex; the ISR uses
    only its existing `s_lock` (`firmware/main/input/he_backend.c`).
  - Start suppresses the engines, waits for HE publication and a TinyUSB FIFO
    fence, then enables capture. A failed wait restores thresholds without
    capture; the UI reports FAILED/NONE. Previously submitted USB transfers
    may finish (`he_backend.c`, `he_cal_barrier.h`,
    `firmware/main/usb/usb_report_barrier.c`, `firmware/main/ui/he_cal_ui.c`).
- `firmware/main/input/touch_retry.c`:
  - Gated during lock or calibration: suppresses Quick Retry (`usb_hid_set_touch_retry(true)`) and swipes (`handle_swipe`).
  - App start refuses a current contact through release cleanup. An atomic
    start reservation gates a racing new touch before backend suppression
    becomes visible (`firmware/main/input/he_cal_touch_guard.h`, `touch_retry.c`,
    `firmware/main/ui/he_cal_ui.c`; report §8 Q4).
  - Passes touch coordinates to `he_cal_ui_touch_sample()` (`firmware/main/ui/he_cal_ui.h`).

### 5.3 Firmware UI
- `firmware/main/ui/he_cal_ui.h` & `firmware/main/ui/he_cal_ui.c`:
  - URL constant: `#define OPAD_DOWNLOAD_URL "https://opad.dev/download"`
  - Red lock screen (dark red `0x5A1010` background, `firmware/main/ui/he_cal_ui.c`).
  - 5-second timer from boot: Calibrate button appears after 5s if no capable app connected. If capable app connects (`s_app_capable`), button is hidden and "Calibrate in the OPad app" is displayed.
  - Interactive calibration screen: step prompts ("Release keys...", "Press Key 1 fully 5 times", "Press Key 2 fully 5 times"), 5-box visual progress indicator per key, 1-second "Calibrated!" success message, or 2-second failure message. An on-pad Retry is offered only while locked without a capable app (`firmware/main/ui/he_cal_ui.c`).
- `firmware/main/ui/ui_port.c`:
  - Interlocks with `pad_timer_cb()`: when `he_backend_is_locked()` is true, prevents loading `UI_SCREEN_PLAYING`.

### 5.4 Firmware Protocol Handler
- `firmware/main/protocol/protocol.c`:
  - Sets `hello_ack.he_calibration = he_backend_running()` in builds with `CONFIG_OSUPAD_HE_BACKEND`, `false` otherwise (`firmware/main/protocol/protocol.c`, `protocol_send_hello_ack`).
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
  - Adds `IpcRequest::StartHeCalibration`, `IpcRequest::CancelHeCalibration`, `IpcRequest::GetHeCalibrationProgress` (`desktop/crates/opad-ipc/src/lib.rs`).
  - Adds `IpcResponse::HeCalibrationProgress`.
- `desktop/daemon/src/runtime.rs` & `desktop/daemon/src/ipc_handlers.rs`:
  - Tracks HE calibration status. When uncalibrated HE pad connects, launches `opad-gui --page calibrate`.
  - Handles calibration IPC requests and forwards them to `opad-device`.
- `desktop/gui/src/main.rs` & `desktop/gui/src/pages.rs`:
  - Registers `Page::Calibration` with token `"calibrate"`.
  - Renders live calibration page: 0..5 press visual boxes, stage transitions, cancel button, retry on error, and calibrated rest/full counts on success.
  - Settings page: adds "Recalibrate Keys" button when connected to HE pad.
  - Walk-away timeout and on-pad cancel gesture are left open with `// OPEN:` comments and documented in the report's "Questions for the owner" section.
  - A CDC host disconnect cancels an app-started flow; a flow begun on-pad
    continues. The GUI reports device disconnection. Closing only the GUI
    leaves the daemon's CDC connection alive and does not cancel firmware
    calibration (`firmware/main/ui/he_cal_ui.c`, `desktop/gui/src/main.rs`).

### 5.6 Test Checklist & Verification
- `docs/testing-checklist.md`:
  - Adds manual verification rows HE-CAL-01..HE-CAL-12 for the lock screen,
    button, app launch, calibration, cancellation, module compatibility,
    touch/start interleaving, stale parameter-reload release arming and the
    reporting start barrier.
    There is no timeout test or implementation; that policy is undecided
    (`docs/testing-checklist.md`, `firmware/main/ui/he_cal_ui.c`).
- Host tests in `firmware/test/host/run_tests.sh` and `cargo test` across all desktop crates.
