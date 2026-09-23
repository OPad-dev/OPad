# Code review: `firmware/main` usb / input / config / counters / runtime / diag / app_main

- Date: 2026-09-23
- Command: `/code-review high firmware/main/usb firmware/main/input firmware/main/config firmware/main/counters firmware/main/runtime firmware/main/diag firmware/main/app_main.c firmware/main/Kconfig.projbuild`
- Base: `main` @ `b0e0a37`
- Verification: none of these findings were re-verified after the review; treat each as *plausible* until checked against the code.
- Note: `firmware/sdkconfig` is untracked, so its stale suspend-callback setting is a local artifact, not a code finding.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. GPIO8 (module-ID ADC pin) is treated as a free key pin — `open`

**File:** `firmware/main/input/keypad.c:433` (also `firmware/main/config/config_validate.c:15`)
**Category:** correctness

GPIO8 is the module-ID ADC pin (100k/10k divider to GND per `board.c:212`) yet it is in `SCAN_PINS` and in `KEY_GPIO_ALLOWED`, so pin detection and validation treat it as a free key pin.

**Failure scenario:** MX or HE module attached → host sends `detect_pin` → `keypad_detect_pressed_pin` enables pull-ups on `SCAN_PINS`; the 10k to GND on GPIO8 overrides the ~45k internal pull-up so `gpio_get_level(8) == 0` immediately → returns 8 before the user touches a switch; the host then persists `key_gpio = 8` (validate accepts it) → that key reads permanently pressed and the pad is stuck until reflash/reconfig.

## 2. Touch-retry submit clears a pending flag set by the keypad task — `open`

**File:** `firmware/main/usb/usb_hid.c:89`
**Category:** correctness

`usb_hid_set_touch_retry` (touch task, core 1) unconditionally clears `s_report_pending` after its own successful submit, wiping a pending flag set concurrently by the keypad task on core 0 whose submit failed.

**Failure scenario:** Core 1: `submit_current_state()` reads key state and claims the EP (success). Core 0 (truly parallel): key1 pressed → `s_key1_pressed = true`, `s_report_pending = true`, submit fails (EP busy). Core 1: `atomic_store(pending, false)`. `tud_hid_report_complete_cb` sees `pending == false` and does not resend → the key1 press (or release) is never delivered until the next unrelated key event: missed hit or stuck key during gameplay.

## 3. Disabled input still reports an HE module's analog level as a held key — `open`

**File:** `firmware/main/input/keypad.c:292`
**Category:** correctness

`keypad_set_input_enabled(false)` only gates the ISR; `keypad_init` still samples the key pins and `keypad_task` reports the sampled level, so a Hall-Effect module's analog outputs can be reported as a held key even though `app_main` says keys are disabled.

**Failure scenario:** HE module detected → `app_main` disables input → `keypad_init` reads `board_key1_read() == true` (Hall output ~1.06 V / analog level below VIH) → `s_key_state[0] = true` → `keypad_task` step 2 sees `current != reported_state` → `s_callback` submits a HID report with key1 down that is never released (ISR ignored) → host sees `z` held forever; same via step 3 re-read after a pin move.

## 4. `device_config_apply` bypasses the keypad's stage-until-released keycode logic — `open`

**File:** `firmware/main/config/device_config.c:128`
**Category:** correctness

`device_config_apply` calls `usb_hid_set_keycodes` directly right after `keypad_set_config`, bypassing the keypad task's stage-until-both-keys-released logic that exists precisely to avoid changing keycodes while a key is held.

**Failure scenario:** Host sends `config_set` changing key1 `z` → `a` while key1 is held: `keypad_set_config` stages it (key pressed) but line 128 changes `s_key1_code` immediately; the next report (key2 press or touch retry) is built with `a` → host sees `z` released and `a` pressed without any physical change; when key1 is finally released the staged config applies again. Also redundant: `keypad_set_config` already calls `usb_hid_set_keycodes` on apply (same redundancy at `app_main.c:101`).

## 5. Stale `dev_cfg` snapshot re-applied after the protocol task may have updated config — `open`

**File:** `firmware/main/app_main.c:194`
**Category:** correctness

`device_config_apply(&dev_cfg)` re-applies a local snapshot taken at line 65, after the CDC/protocol task (line 158) may already have accepted a newer config from the host.

**Failure scenario:** Host daemon connects during `ui_init` (hundreds of ms) and sends `config_set` (new keycodes/debounce/pins) → protocol stores it in `s_current_config` and NVS → line 194 then pushes the stale `dev_cfg` into keypad/usb_hid → pad behaves with old keys while `get_config`/NVS report the new ones; mismatch persists until next `config_set` or reboot.

**Suggested fix:** `device_config_get(&dev_cfg)` here, or apply only brightness/sleep.

## 6. `s_pending_edge_us` armed after the state was already delivered — `open`

**File:** `firmware/main/usb/usb_hid.c:110`
**Category:** correctness

`s_pending_edge_us` is armed after the failed submit without re-checking whether `tud_hid_report_complete_cb` already delivered the state in between, leaking a stale edge timestamp into the next successful submit.

**Failure scenario:** Keypad task: `pending = true`, submit fails. TinyUSB task: `complete_cb` exchanges `pending → false`, submits current state, `record_pending_latency` finds `edge == 0`. Keypad task resumes: CAS sets `s_pending_edge_us = edge_us`. Seconds later the next key event submits successfully and `record_pending_latency` records `now - edge` (e.g. 5,000,000 µs) → `max_us`/p99.9/outlier counters and a `DIAG_EVENT_LATENCY_OUTLIER` are corrupted, and `deferred_reports` is over-counted.

## 7. Staged config committed to `s_config` before `board_keys_set_gpio`; no revert on failure — `fixed`

**File:** `firmware/main/input/keypad.c:223`
**Category:** correctness

The staged config is committed to `s_config` before `board_keys_set_gpio` runs; on failure there is no revert, so the reported config and the pins actually armed diverge (same divergence in `keypad_init:270` fallback vs `device_config`'s `s_current_config`).

**Failure scenario:** `board_keys_set_gpio(applied.key1_gpio, ...)` returns an error (e.g. `gpio_isr_handler_add` fails) → `ESP_LOGE` only; `s_config` now says GPIO10/7 while the ISR/pins remain on 14/9 → `keypad_get_config`, protocol `get_config`, and `keypad_detect_pressed_pin`'s exclude logic all use the wrong pins; the end-of-scan `board_keys_set_gpio(cfg...)` then retries with the failing pins and the pad can end with no key ISR at all.

## 8. Duplicated debounce bounds and key GPIO allow-list have already drifted — `open`

**File:** `firmware/main/config/config_validate.c:8`
**Category:** reuse

`config_validate.c` redefines `DEBOUNCE_MIN_US`/`DEBOUNCE_MAX_US` (already in `input/debounce.h`, which is host-safe) and duplicates the key GPIO allow-list that `keypad.c` keeps separately as `SCAN_PINS`.

**Failure scenario:** Two copies of the pin list and two copies of the debounce bounds must be edited together; they have already drifted in the `CONFIG_OSUPAD_BENCH_DEBUG_GPIO` handling (validate excludes the debug pin, `SCAN_PINS` does not, so a scan reconfigures and `gpio_reset_pin()`s the debug output) and both share the GPIO8 mistake (finding 1).

**Suggested fix:** Include `debounce.h` and export one `device_config_key_gpio_list()` / shared array used by both.

## 9. Dead / duplicated code left behind — `open`

**File:** `firmware/main/usb/usb_cdc.c:140` (and others listed below)
**Category:** simplification

- `usb_cdc.c:140-147`: stale comment block, superseded by 148-156 and wrong about the "oversized length prefix".
- `device_config.c:207`: `device_config_set`'s IDLE check duplicates the one inside `write_to_nvs`.
- `counters.c:148`: mid-file `#include`.
- `latency_stats`: `s_samples` counter is written but never read.

**Failure scenario:** Maintenance cost only: the first comment now contradicts the framing rules documented in the second (a reader following it would think only `BOOTLOADER` is handled and that a length-prefix check is what protects it); `device_config_set` can call `write_to_nvs` unconditionally since it already defers when not IDLE; `s_samples` is redundant with the bucket sum computed in `latency_stats_get`.

## 10. 126-address I2C probe at boot; 100 Hz unconditional CST816 read — `open`

**File:** `firmware/main/input/touch_retry.c:150`
**Category:** efficiency

`touch_retry_init` probes all 126 I2C addresses (each with a 10 ms timeout) purely to log them, serialising up to ~1.3 s of boot before the touch task, brightness/sleep apply, and "ready" log; the task then issues a CST816 register read every 10 ms even when INT is idle.

**Failure scenario:** Startup path: with a NACKing/absent touch controller each probe can hit the 10 ms timeout → up to 1.26 s added to `app_main` before touch retry is live; at runtime the 100 Hz unconditional I2C read against a sleeping CST816 returns errors every 10 ms (and on IDF 5.x the `i2c_master` driver logs a NACK error), burning core-1 time and UART log bandwidth.

**Suggested fix:** Drop the scan (or gate it behind a debug Kconfig) and only read `TOUCH_NUM` when INT is active or a touch was recently down.
