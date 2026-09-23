# Code review: `firmware/main/ui`

- Date: 2026-09-23
- Command: `/code-review high firmware/main/ui`
- Base: `main` @ `b0e0a37`
- Scope: no diff touched `firmware/main/ui`, so the review covers the directory as-is plus its call sites in `protocol.c`, `runtime.c`, `app_main.c`, `device_config.c`, `usb_cdc.c`, `keypad.c`, `counters.c`. Assumptions were checked against the vendored LVGL 9.5.0 / esp_lvgl_port 2.9.0 sources.
- Verification: findings were not re-verified after the review; treat each as *plausible* until checked against the code.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. `data_update` source 67 calls `easter_egg_trigger()` directly, bypassing the `s_ui_ok` guard — `open`

**File:** `firmware/main/protocol/protocol.c:522`
**Category:** correctness

`data_update` with source 67 calls `easter_egg_trigger()` directly under `ui_lock()`, bypassing the `s_ui_ok` guard that `ui_trigger_easter_egg()` (`ui_port.c:338`) provides; `ui_lock()`/`ui_unlock()` (`ui.h:30`) silently become no-ops when the UI is disabled, so the caller runs LVGL with neither lock nor `lv_init()`.

**Failure scenario:** LCD init fails (supported "headless mode", `app_main.c:188`) → `ui_init` returns before `lvgl_port_init`, `lv_init` never runs. Host app sends a `DataUpdate` containing source 67 (the desktop app's easter-egg trigger) → `lv_layer_top()` returns NULL, `lv_gif_create(NULL)` → `lv_malloc_zeroed` on an uninitialised TLSF state / `lv_obj_class_init_obj(NULL)` → null deref → panic and reboot, repeatedly while the host keeps sending.

**Suggested fix:** Call `ui_trigger_easter_egg()` (the mutex is recursive so nesting under `ui_lock` is fine) or make `ui_lock()` return false when `!s_ui_ok` and have callers skip the batch.

## 2. Brightness change received while asleep is lost on wake — `open`

**File:** `firmware/main/ui/ui_port.c:117`
**Category:** correctness

`ui_set_brightness()` skips `board_display_set_brightness()` when `s_asleep`, and `update_sleep()`'s wake path calls `board_display_wake()`, which restores the board's stale `s_display_brightness` instead of ui_port's `s_brightness`.

**Failure scenario:** Display asleep at 80%. Host sets brightness 30 → `ui_set_brightness` stores `s_brightness = 30`, does not touch the board. User taps a key → `update_sleep` wakes → `board_display_wake` → `board_backlight_set(80)`. Display comes back at 80% and stays there until the next config apply. Compounding: `device_config_apply()` (`device_config.c:131`) also calls `board_backlight_set()` directly while asleep, lighting the backlight over a panel that is `disp_off`, and `ui_set_brightness` reads the volatile `s_asleep` from the protocol task without the LVGL lock, so it can race the LVGL task's sleep transition and turn the backlight on right after `board_display_sleep()`.

**Suggested fix:** On wake call `board_display_set_brightness(s_brightness)`, and take the LVGL lock (or set a flag for `pad_timer_cb`) in `ui_set_brightness`. See also the `firmware/boards` review, finding 1.

## 3. KPS ring uses unsigned subtraction; lifetime counters can decrease — `open`

**File:** `firmware/main/ui/ui_port.c:87`
**Category:** correctness

KPS ring uses unsigned subtraction (`total - oldest`) and treats 0 as an "unfilled" sentinel; lifetime counters can legitimately decrease (`counters.c` `rebase_lifetime_presses`: "works for decreases (forced sync, reset) too"), producing a wrapped uint64 that is clamped to `INT32_MAX`.

**Failure scenario:** Ring holds `total = 1500` from the last second. Host force-restores counters to 200 (or resets to 0) → `total(200) - oldest(1500)` wraps to ~1.8e19 → `(double)` → `ui_data_set_number` clamps to `INT32_MAX` → the KPS widget reads `2147483647` for up to 50 ticks (1 s). Separately, on a brand-new device or after a reset to 0, `oldest == 0` for the first second of tapping so KPS shows 0 while the user is pressing.

**Suggested fix:** Signed diff clamped at 0 (or reset the ring when `total < oldest`) and track fill count instead of using 0 as sentinel.

## 4. `ui_store_flush_dirty()` clears the dirty bit before the save; failure drops the layout — `open`

**File:** `firmware/main/ui/ui_store.c:143`
**Category:** correctness

`ui_store_flush_dirty()` clears the dirty bit before calling `ui_store_save()`/`ui_store_erase()`; if the NVS write fails the pending layout is dropped for good, and the only caller (`runtime.c:49,54`) ignores the returned error.

**Failure scenario:** Layout pushed during gameplay → deferred (pending + bit set). COOLDOWN → IDLE → flush clears bit, `ui_store_save` returns `ESP_ERR_NVS_NOT_ENOUGH_SPACE` (or `nvs_open` fails) → `last_err` set but bit is gone and runtime discards `last_err` → the host was already told "applied, will be saved after gameplay" (`protocol.c:487`); after the next reboot the old layout comes back with no diagnostic.

**Suggested fix:** Clear the bit only on success (or re-arm on failure) and `diag_record` the failure.

## 5. `s_pending_layouts[]` / dirty masks shared across cores with no lock — `open`

**File:** `firmware/main/ui/ui_store.c:58`
**Category:** correctness

`s_pending_layouts[]`, `s_dirty_save_mask` and `s_dirty_erase_mask` are written by the protocol task (core 0) and read-modify-written by the runtime supervisor task (core 1) with no lock or atomics.

**Failure scenario:** Runtime task in flush: reads mask, is about to do `s_dirty_save_mask &= ~(1<<i)`. Protocol task on the other core: `host_status(playing)` flips state to PLAYING, then `set_layout` arrives → `ui_store_save` deferred → `s_dirty_save_mask |= (1<<i)` and a 2 KB struct copy into `s_pending_layouts[i]`. The runtime's stale RMW clears the newly set bit (layout never saved), or `ui_store_save`'s `blob->layout = *layout` (line 78) memcpy's a half-written pending layout to flash. Neither the header's "only ever written while not PLAYING" note nor the state check inside `ui_store_save` closes the window because the state flip is itself done by the protocol task.

**Suggested fix:** Guard with a small mutex or make the masks atomic and copy under it.

## 6. `ui_init()` calls `lv_screen_load()` on a possibly-NULL screen — `open`

**File:** `firmware/main/ui/ui_port.c:229`
**Category:** correctness

`ui_init()` calls `lv_screen_load(s_screens[UI_SCREEN_IDLE])` without checking for NULL, even though `rebuild_screen()` can leave the slot NULL when `ui_screen_create()` fails (`lv_malloc` of the 2 KB layout copy or any widget allocation).

**Failure scenario:** LVGL pool is 128 KB in the current generated `firmware/sdkconfig` (defaults say 160) and `easter_egg_init()`'s `lv_mem_add_pool` only helps if PSRAM is present; on a board without PSRAM and a fragmented pool, `ui_screen_create` returns NULL → `rebuild_screen` logs and returns → `s_screens[0] == NULL` → `lv_screen_load(NULL)` derefs NULL inside `lv_obj_get_display` → boot panic instead of the intended headless fallback. `pad_timer_cb` (line 149) already guards `s_screens[want]`.

**Suggested fix:** Guard in `ui_init` too (return `ESP_ERR_NO_MEM`).

## 7. Per-screen layout copy freed in `LV_EVENT_DELETE` before children are torn down — `open` (latent)

**File:** `firmware/main/ui/core/ui_screen.c:341`
**Category:** correctness

The per-screen `ui_layout_t` copy is freed in the screen's `LV_EVENT_DELETE` handler, but LVGL 9.5 `obj_delete_core()` sends `LV_EVENT_DELETE` to the parent before recursively deleting its children (`lv_obj_tree.c:676` vs `:688`), so every observer's `user_data` (pointer into `copy`) dangles while the children are torn down.

**Failure scenario:** Today no observer callback fires during child deletion, so this is latent. Any code path that notifies a subject while a screen is being deleted (e.g. a future LVGL version notifying on unsubscribe, or an `LV_EVENT_CHILD_DELETED`/`SIZE_CHANGED` handler that reads `w`) becomes a use-after-free with the LVGL lock held.

**Suggested fix:** Free `copy` after the children are gone — e.g. in `rebuild_screen` after `lv_obj_delete(old)`, or via a screen-owned struct torn down after children.

## 8. `ui_store_erase()` commits and counts an NVS write even when nothing was stored — `open`

**File:** `firmware/main/ui/ui_store.c:126`
**Category:** correctness

`ui_store_erase()` commits and calls `counters_record_nvs_write()` even when `nvs_erase_key` returned `ESP_ERR_NVS_NOT_FOUND` (nothing stored), unlike `ui_store_save()` which skips identical writes.

**Failure scenario:** User presses "Reset to default" on a pad that never had a custom layout → `nvs_commit` on an unchanged namespace plus an inflated NVS-write counter in the counters/diag telemetry every time.

**Suggested fix:** Return `ESP_OK` before commit when the key was not found.

## 9. `update_pad_sources()` runs `localtime_r`/`strftime` every 20 ms — `open`

**File:** `firmware/main/ui/ui_port.c:92`
**Category:** efficiency

`update_pad_sources()` runs `localtime_r()` and two `strftime()` calls every 20 ms on the LVGL task even though the clock string changes once a minute and the date once a day.

**Failure scenario:** 50 localtime/strftime pairs per second on core 1 for values that are almost always identical (`lv_subject_copy_string` then compares and drops them).

**Suggested fix:** Keep the last formatted `time_t`/minute and only reformat when `(t / 60)` changes, or run the clock update from a 1 s `lv_timer`.

## 10. Dead / redundant code — `fixed`

**File:** `firmware/main/ui/easter_egg.c:29`
**Category:** simplification

- `easter_egg_is_active()` has no callers anywhere in `firmware/`.
- `build_keycard()` (`ui_screen.c:243`) sets `user_data` on `count` that `count_observer_cb` never reads.
- `easter_egg` sets both `style_opa` and `style_image_opa` on the same object (lines 73-74, 119-120) where `opa` alone already fades the image.

**Failure scenario:** Public API surface and per-frame style writes that do nothing, and a misleading `user_data` link that suggests `count_observer_cb` hides the card like `text_observer_cb` does.

**Suggested fix:** Remove `easter_egg_is_active()`, drop `lv_obj_set_user_data(count, card)`, and keep a single opa setter.

---

## Notes from the reviewer (not findings)

- `firmware/sdkconfig` (untracked, dated Sep 17) disagrees with `sdkconfig.defaults`: it has `CONFIG_LV_USE_GIF` unset and `CONFIG_LV_MEM_SIZE_KILOBYTES=128` vs `=y`/`160` in defaults. Delete and regenerate it after editing defaults; a build from the stale file would fail on `lv_gif_create` in `easter_egg.c`.
- Verified against vendored sources and *not* flagged: `lvgl_port_stop()` only disables LVGL timers and the tick esp_timer (the LVGL task keeps running); `lvgl_port_lock` is a recursive mutex; `lv_subject_set_int`/`lv_subject_copy_string` use `notify_if_changed` (so the 20 ms pad refresh does not redraw unchanged labels); observers are notified once on subscribe; `lv_obj_delete` cancels bound animations.
