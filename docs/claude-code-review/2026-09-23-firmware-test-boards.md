# Code review: `firmware/test`, `firmware/boards`

- Date: 2026-09-23
- Command: `/code-review medium firmware/test firmware/boards`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched these paths, so the review covers the files as they stand (11 files, 1537 lines). The host test suite (`firmware/test/host/run_tests.sh`) builds and passes all 6 binaries with `-Wall -Wextra -Werror`.
- Verification: verdicts as given by the reviewer.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

---

## 1. `board_display_wake` restores a stale private brightness — `open` (confirmed)

**File:** `firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c:42`
**Category:** correctness

`board_display_wake` restores its private `s_display_brightness`, but `device_config.c:131` applies brightness via `board_backlight_set` directly while `ui_set_brightness` skips an asleep display. A brightness change received while asleep turns the backlight on over a powered-off panel, and on wake the old brightness is restored, discarding the new setting.

Same defect as `firmware/main/ui` finding 2, seen from the board side.

## 2. `board_keys_set_gpio` is called from two tasks despite "call from one task only" — `fixed`

**File:** `firmware/boards/waveshare_esp32s3_touch_lcd_2/board.c:19`
**Category:** correctness

`board_keys_set_gpio` is documented "call from one task only" and has an unlocked detach/reconfigure window, yet it is called from both `keypad_task` (staged pin move, `keypad.c:232`) and the `cdc_proto` task via `keypad_detect_pressed_pin` (`keypad.c:504`). Detect-then-apply-config from the host can interleave the two, leaving a handler on a reset pad or the keys stuck at `GPIO_NUM_NC` until reboot.

---

## Checked and not reported

- Module-ID thresholds vs. the divider math (MX 0.25 V loaded / 0.30 V open, HE 0.62 V / 1.06 V, all clear of the 120/650/1600 mV cuts).
- ISR IRAM safety (`gpio_isr_handler` and `debounce_step` are `IRAM_ATTR`).
- The same-pins re-arm path.
- `board_backlight_get` returning 0 while asleep (status also carries `display_asleep`, so it reads as intentional).
- Error-path handle leaks in `board_display_init` (called once; headless fallback follows).
