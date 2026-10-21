#include "he_cal_ui.h"
#include "input/he_cal_button.h"
#include "input/he_cal_runner.h"
#include "input/he_backend.h"
#include "input/he_cal.h"
#include "input/gesture.h"
#include "runtime/runtime.h"
#include "usb/usb_cdc.h"
#include "ui/ui.h"
#include "ui/core/ui_core.h"
#include "protocol/protocol.h"
#include "lvgl.h"
#include "esp_timer.h"
#include "esp_log.h"
#include <stdatomic.h>
#include <string.h>

static const char *TAG = "he_cal_ui";

// send_proto_progress() casts he_cal_error_t straight to osupad_HeCalError
_Static_assert((int)HE_CAL_ERR_NONE == (int)osupad_HeCalError_HE_CAL_ERROR_NONE, "HeCalError");
_Static_assert((int)HE_CAL_ERR_REST_NOT_STILL == (int)osupad_HeCalError_HE_CAL_ERROR_REST_NOT_STILL, "HeCalError");
_Static_assert((int)HE_CAL_ERR_REST_OUT_OF_RANGE == (int)osupad_HeCalError_HE_CAL_ERROR_REST_OUT_OF_RANGE, "HeCalError");
_Static_assert((int)HE_CAL_ERR_REST_PRESSED == (int)osupad_HeCalError_HE_CAL_ERROR_REST_PRESSED, "HeCalError");
_Static_assert((int)HE_CAL_ERR_SIGNAL_NOT_RISING == (int)osupad_HeCalError_HE_CAL_ERROR_SIGNAL_NOT_RISING, "HeCalError");
_Static_assert((int)HE_CAL_ERR_SPAN_TOO_SMALL == (int)osupad_HeCalError_HE_CAL_ERROR_SPAN_TOO_SMALL, "HeCalError");
_Static_assert((int)HE_CAL_ERR_PEAKS_INCONSISTENT == (int)osupad_HeCalError_HE_CAL_ERROR_PEAKS_INCONSISTENT, "HeCalError");
_Static_assert((int)HE_CAL_ERR_TOO_FEW_PEAKS == (int)osupad_HeCalError_HE_CAL_ERROR_TOO_FEW_PEAKS, "HeCalError");
_Static_assert((int)HE_CAL_ERR_TOO_MANY_PEAKS == (int)osupad_HeCalError_HE_CAL_ERROR_TOO_MANY_PEAKS, "HeCalError");

// Button bounds in screen coordinates (320x240)
#define CAL_BTN_X 70
#define CAL_BTN_Y 172
#define CAL_BTN_W 180
#define CAL_BTN_H 44

#define BOOT_APP_WAIT_US 5000000 // 5 seconds grace period for capable app to connect

static lv_obj_t *s_overlay = NULL;
static lv_obj_t *s_title_label = NULL;
static lv_obj_t *s_prompt_label = NULL;
static lv_obj_t *s_info_label = NULL;
static lv_obj_t *s_btn_obj = NULL;
static lv_obj_t *s_btn_label = NULL;
static lv_obj_t *s_boxes[5] = {NULL};

static he_cal_button_t s_btn_tracker;
static he_cal_runner_t s_runner;
static int64_t s_boot_time_us = 0;
static atomic_bool s_app_capable = false;
static atomic_bool s_host_disconnected = false;
static atomic_bool s_btn_active = false;
static bool s_app_started = false;
static bool s_disconnect_failed = false;
static int64_t s_success_until_us = 0;
static int64_t s_fail_until_us = 0;
static const char *s_system_error;
static int64_t s_last_progress_us;
static bool s_touch_down;
static bool s_button_touch_allowed;

static void update_boxes_visibility(bool visible, uint8_t count)
{
    for (int i = 0; i < 5; i++) {
        if (!s_boxes[i]) continue;
        if (!visible) {
            lv_obj_add_flag(s_boxes[i], LV_OBJ_FLAG_HIDDEN);
        } else {
            lv_obj_remove_flag(s_boxes[i], LV_OBJ_FLAG_HIDDEN);
            if (i < count) {
                lv_obj_set_style_bg_color(s_boxes[i], lv_color_hex(0x2E7D32), 0); // Green
                lv_obj_set_style_border_color(s_boxes[i], lv_color_hex(0x4CAF50), 0);
            } else {
                lv_obj_set_style_bg_color(s_boxes[i], lv_color_hex(0x2A2A2A), 0); // Gray
                lv_obj_set_style_border_color(s_boxes[i], lv_color_hex(0x555555), 0);
            }
        }
    }
}

// OPEN (Report §Questions for the owner #3): Exact wording, layout and styling
// of the lock, calibration and error screens are provisional pending owner review.
static void show_lock_screen(int64_t now_us)
{
    if (!s_overlay) return;

    he_lock_screen_mode_t mode = he_cal_lock_screen_mode(
        atomic_load(&s_app_capable),
        atomic_load(&s_host_disconnected),
        now_us - s_boot_time_us,
        BOOT_APP_WAIT_US);

    lv_obj_set_style_bg_color(s_overlay, lv_color_hex(0x5A1010), 0); // Dark Red
    lv_label_set_text(s_title_label, "KEYS NOT CALIBRATED");
    update_boxes_visibility(false, 0);

    switch (mode) {
    case HE_LOCK_SCREEN_USE_APP:
        lv_label_set_text(s_prompt_label, "Calibrate in the OPad app");
        lv_label_set_text(s_info_label, "The desktop app is open.");
        atomic_store(&s_btn_active, false);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        break;
    case HE_LOCK_SCREEN_WAIT_FOR_APP:
        lv_label_set_text(s_prompt_label, "Waiting for OPad app...");
        lv_label_set_text(s_info_label, "Connecting to PC host");
        atomic_store(&s_btn_active, false);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        break;
    case HE_LOCK_SCREEN_NO_APP_BUTTON:
    default:
        lv_label_set_text(s_prompt_label, "Install the OPad app first:");
        lv_label_set_text(s_info_label, OPAD_DOWNLOAD_URL);
        atomic_store(&s_btn_active, true);
        lv_label_set_text(s_btn_label, "Calibrate");
        lv_obj_remove_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        break;
    }
}

static void send_proto_progress(const he_runner_progress_t *prog, const he_calibration_t results[2])
{
    osupad_HeCalibrationProgress p = osupad_HeCalibrationProgress_init_zero;
    switch (prog->stage) {
    case HE_RUNNER_REST:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_REST;
        break;
    case HE_RUNNER_KEY1:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_KEY1;
        break;
    case HE_RUNNER_KEY2:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_KEY2;
        break;
    case HE_RUNNER_SUCCESS:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_SUCCESS;
        if (results) {
            p.key1_rest = results[0].rest >= 0 ? (uint32_t)results[0].rest : 0;
            p.key1_full = results[0].full >= 0 ? (uint32_t)results[0].full : 0;
            p.key2_rest = results[1].rest >= 0 ? (uint32_t)results[1].rest : 0;
            p.key2_full = results[1].full >= 0 ? (uint32_t)results[1].full : 0;
        }
        break;
    case HE_RUNNER_FAILED:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_FAILED;
        break;
    default:
        p.stage = osupad_HeCalStage_HE_CAL_STAGE_NONE;
        break;
    }
    p.key_index = prog->active_key;
    p.presses_done = prog->presses_done;
    p.error = (osupad_HeCalError)prog->error;
    const char *error = s_system_error ? s_system_error : prog->error_desc;
    if (error) {
        strncpy(p.error_message, error, sizeof(p.error_message) - 1);
    }
    // last_raw is -1 until the first sample; 0 means "no reading" to the app
    int32_t raw = (prog->active_key == 2) ? prog->last_raw[1] : prog->last_raw[0];
    p.raw_value = raw >= 0 ? (uint32_t)raw : 0;

    if (atomic_load(&s_app_capable)) protocol_send_he_calibration_progress(&p);
}

static void update_calibration_screen(void)
{
    if (!s_overlay) return;

    if (s_disconnect_failed) {
        lv_obj_set_style_bg_color(s_overlay, lv_color_hex(0x5A1010), 0); // Red
        lv_label_set_text(s_title_label, "CALIBRATION FAILED");
        lv_label_set_text(s_prompt_label, "App disconnected");
        lv_label_set_text(s_info_label, "Please retry");
        update_boxes_visibility(false, 0);
        lv_label_set_text(s_btn_label, "Retry");
        bool button = he_backend_is_locked() && !atomic_load(&s_app_capable);
        if (button) lv_obj_remove_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        else lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        atomic_store(&s_btn_active, button);
        return;
    }

    he_runner_progress_t prog;
    he_cal_runner_get_progress(&s_runner, &prog);

    lv_obj_set_style_bg_color(s_overlay, lv_color_hex(0x1E1E1E), 0); // Dark Gray

    switch (prog.stage) {
    case HE_RUNNER_REST:
        lv_label_set_text(s_title_label, "CALIBRATING: REST");
        lv_label_set_text(s_prompt_label, "Release keys...");
        lv_label_set_text(s_info_label, "Measuring sensor noise floor");
        update_boxes_visibility(false, 0);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        atomic_store(&s_btn_active, false);
        break;

    case HE_RUNNER_KEY1:
        lv_label_set_text(s_title_label, "KEY 1 CALIBRATION");
        lv_label_set_text(s_prompt_label, "Press Key 1 fully 5 times");
        lv_label_set_text(s_info_label, "Release fully between presses");
        update_boxes_visibility(true, prog.presses_done);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        atomic_store(&s_btn_active, false);
        break;

    case HE_RUNNER_KEY2:
        lv_label_set_text(s_title_label, "KEY 2 CALIBRATION");
        lv_label_set_text(s_prompt_label, "Press Key 2 fully 5 times");
        lv_label_set_text(s_info_label, "Release fully between presses");
        update_boxes_visibility(true, prog.presses_done);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        atomic_store(&s_btn_active, false);
        break;

    case HE_RUNNER_SUCCESS:
        lv_obj_set_style_bg_color(s_overlay, lv_color_hex(0x1B381E), 0); // Green Tint
        lv_label_set_text(s_title_label, "SUCCESS");
        lv_label_set_text(s_prompt_label, "Calibrated!");
        lv_label_set_text(s_info_label, "Saved. Pad is ready.");
        update_boxes_visibility(false, 0);
        lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
        atomic_store(&s_btn_active, false);
        break;

    case HE_RUNNER_FAILED: {
        bool app = atomic_load(&s_app_capable) || !he_backend_is_locked();
        lv_obj_set_style_bg_color(s_overlay, lv_color_hex(0x5A1010), 0); // Red
        lv_label_set_text(s_title_label, "CALIBRATION FAILED");
        lv_label_set_text(s_prompt_label, s_system_error ? s_system_error :
                          (prog.error_desc ? prog.error_desc : "Failed"));
        update_boxes_visibility(false, 0);
        if (app) {
            lv_label_set_text(s_info_label, "Retry in the OPad app");
            lv_obj_add_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
            atomic_store(&s_btn_active, false);
        } else {
            lv_label_set_text(s_info_label, "Please retry");
            lv_label_set_text(s_btn_label, "Retry");
            lv_obj_remove_flag(s_btn_obj, LV_OBJ_FLAG_HIDDEN);
            atomic_store(&s_btn_active, true);
        }
        break;
    }

    default:
        break;
    }
}

static void create_overlay_objects(void)
{
    if (s_overlay) return;

    s_overlay = lv_obj_create(lv_layer_top());
    lv_obj_remove_style_all(s_overlay);
    lv_obj_remove_flag(s_overlay, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);
    lv_obj_set_size(s_overlay, UI_SCREEN_W, UI_SCREEN_H);
    lv_obj_set_pos(s_overlay, 0, 0);
    lv_obj_set_style_bg_opa(s_overlay, LV_OPA_COVER, 0);

    // Title
    s_title_label = lv_label_create(s_overlay);
    lv_obj_set_style_text_font(s_title_label, &lv_font_montserrat_16, 0);
    lv_obj_set_style_text_color(s_title_label, lv_color_hex(0xFFB74D), 0); // Gold / Accent
    lv_obj_set_style_text_align(s_title_label, LV_TEXT_ALIGN_CENTER, 0);
    lv_obj_set_size(s_title_label, UI_SCREEN_W, 24);
    lv_obj_set_pos(s_title_label, 0, 20);

    // Prompt
    s_prompt_label = lv_label_create(s_overlay);
    lv_obj_set_style_text_font(s_prompt_label, &lv_font_montserrat_16, 0);
    lv_obj_set_style_text_color(s_prompt_label, lv_color_white(), 0);
    lv_obj_set_style_text_align(s_prompt_label, LV_TEXT_ALIGN_CENTER, 0);
    lv_obj_set_size(s_prompt_label, UI_SCREEN_W, 28);
    lv_obj_set_pos(s_prompt_label, 0, 56);

    // Subtext / URL
    s_info_label = lv_label_create(s_overlay);
    lv_obj_set_style_text_font(s_info_label, &lv_font_montserrat_14, 0);
    lv_obj_set_style_text_color(s_info_label, lv_color_hex(0xCCCCCC), 0);
    lv_obj_set_style_text_align(s_info_label, LV_TEXT_ALIGN_CENTER, 0);
    lv_obj_set_size(s_info_label, UI_SCREEN_W, 24);
    lv_obj_set_pos(s_info_label, 0, 88);

    // 5 Press Indicator Boxes (Key 1 & Key 2)
    int start_x = (UI_SCREEN_W - (5 * 32 + 4 * 12)) / 2;
    for (int i = 0; i < 5; i++) {
        lv_obj_t *box = lv_obj_create(s_overlay);
        s_boxes[i] = box;
        lv_obj_remove_style_all(box);
        lv_obj_remove_flag(box, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);
        lv_obj_set_size(box, 32, 32);
        lv_obj_set_pos(box, start_x + i * 44, 120);
        lv_obj_set_style_radius(box, 6, 0);
        lv_obj_set_style_border_width(box, 2, 0);
        lv_obj_set_style_bg_opa(box, LV_OPA_COVER, 0);

        lv_obj_t *num = lv_label_create(box);
        lv_obj_set_style_text_font(num, &lv_font_montserrat_14, 0);
        lv_obj_set_style_text_color(num, lv_color_white(), 0);
        lv_obj_set_style_text_align(num, LV_TEXT_ALIGN_CENTER, 0);
        lv_obj_set_size(num, 32, 20);
        lv_obj_set_pos(num, 0, 6);
        lv_label_set_text_fmt(num, "%d", i + 1);
    }
    update_boxes_visibility(false, 0);

    // Button container
    s_btn_obj = lv_obj_create(s_overlay);
    lv_obj_remove_style_all(s_btn_obj);
    lv_obj_remove_flag(s_btn_obj, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);
    lv_obj_set_size(s_btn_obj, CAL_BTN_W, CAL_BTN_H);
    lv_obj_set_pos(s_btn_obj, CAL_BTN_X, CAL_BTN_Y);
    lv_obj_set_style_radius(s_btn_obj, 8, 0);
    lv_obj_set_style_bg_color(s_btn_obj, lv_color_hex(0x37474F), 0);
    lv_obj_set_style_bg_opa(s_btn_obj, LV_OPA_COVER, 0);
    lv_obj_set_style_border_color(s_btn_obj, lv_color_hex(0x78909C), 0);
    lv_obj_set_style_border_width(s_btn_obj, 2, 0);

    s_btn_label = lv_label_create(s_btn_obj);
    lv_obj_set_style_text_font(s_btn_label, &lv_font_montserrat_16, 0);
    lv_obj_set_style_text_color(s_btn_label, lv_color_white(), 0);
    lv_obj_set_style_text_align(s_btn_label, LV_TEXT_ALIGN_CENTER, 0);
    lv_obj_set_size(s_btn_label, CAL_BTN_W, 24);
    lv_obj_set_pos(s_btn_label, 0, 10);
    lv_label_set_text(s_btn_label, "Calibrate");
}

static void destroy_overlay_objects(void)
{
    atomic_store(&s_btn_active, false);
    if (s_overlay) {
        lv_obj_delete(s_overlay);
        s_overlay = NULL;
        s_title_label = NULL;
        s_prompt_label = NULL;
        s_info_label = NULL;
        s_btn_obj = NULL;
        s_btn_label = NULL;
        for (int i = 0; i < 5; i++) {
            s_boxes[i] = NULL;
        }
    }
}

static void handle_disconnect_locked(int64_t now_us)
{
    atomic_store(&s_app_capable, false);
    atomic_store(&s_host_disconnected, true);
    if (he_backend_is_calibrating() && s_app_started) {
        he_cal_runner_cancel(&s_runner);
        he_backend_cancel_calibration();
        s_disconnect_failed = true;
        s_fail_until_us = now_us + 2000000;
        s_app_started = false;
    }
}

// Timer runs every 20 ms in LVGL thread (Core 1)
static void he_cal_ui_timer_cb(lv_timer_t *timer)
{
    (void)timer;
    int64_t now_us = esp_timer_get_time();

    if (atomic_load(&s_app_capable) && !usb_cdc_is_connected()) {
        handle_disconnect_locked(now_us);
    }

    bool locked = he_backend_is_locked();
    bool calibrating = he_backend_is_calibrating();

    // Hold the success overlay for 1 s. Keys unlock as soon as saving succeeds.
    if (s_success_until_us > 0) {
        if (now_us >= s_success_until_us) {
            s_success_until_us = 0;
            destroy_overlay_objects();
        }
        return;
    }

    // Handling failure or app-disconnect display when not actively sampling
    // The reason stays up for 2 s, then a locked pad goes back to the lock
    // screen and an unlocked one (failed recalibration) back to normal.
    if (!calibrating && (s_runner.stage == HE_RUNNER_FAILED || s_disconnect_failed)) {
        if (now_us < s_fail_until_us) {
            create_overlay_objects();
            update_calibration_screen();
            return;
        }
        s_disconnect_failed = false;
        s_fail_until_us = 0;
        s_system_error = NULL;
        he_cal_runner_cancel(&s_runner);
    }

    if (!locked && !calibrating) {
        destroy_overlay_objects();
        return;
    }

    create_overlay_objects();

    if (!calibrating) {
        show_lock_screen(now_us);
        return;
    }

    // OPEN (Report §Questions for the owner #1 & #2): On-pad cancel gesture and
    // walk-away timeout during KEY1/KEY2 stages are left open pending owner decision.
    const he_cal_samples_t *samples = he_backend_take_cal_samples();
    if (samples) {
        if (samples->lost) s_system_error = "Calibration samples lost";
        bool changed = he_cal_runner_feed_samples(&s_runner, samples);
        if (changed || now_us - s_last_progress_us >= 100000) {
            he_runner_progress_t prog;

            if (s_runner.stage == HE_RUNNER_SUCCESS) {
                he_calibration_t saved[2];
                esp_err_t err = ESP_FAIL;
                if (he_cal_runner_get_results(&s_runner, saved)) {
                    err = he_backend_finish_calibration(saved);
                }
                if (err == ESP_OK) {
                    ESP_LOGI(TAG, "Calibration successful: K1=%ld..%ld, K2=%ld..%ld",
                             (long)saved[0].rest, (long)saved[0].full,
                             (long)saved[1].rest, (long)saved[1].full);
                    he_cal_runner_get_progress(&s_runner, &prog);
                    send_proto_progress(&prog, saved);
                    s_success_until_us = now_us + 1000000; // Hold for 1 second
                } else {
                    s_runner.stage = HE_RUNNER_FAILED;
                    s_runner.error = HE_CAL_ERR_NONE;
                    s_system_error = "Could not save calibration";
                }
            }
            if (s_runner.stage == HE_RUNNER_FAILED) {
                // Restore previous calibration immediately (or stay locked if none)
                he_backend_cancel_calibration();
                s_fail_until_us = now_us + 2000000;
                he_cal_runner_get_progress(&s_runner, &prog);
                send_proto_progress(&prog, NULL);
            } else if (s_runner.stage != HE_RUNNER_SUCCESS) {
                he_cal_runner_get_progress(&s_runner, &prog);
                send_proto_progress(&prog, NULL);
            }
            s_last_progress_us = now_us;
        }
    }

    update_calibration_screen();
}

esp_err_t he_cal_ui_init(void)
{
    if (!he_backend_running()) return ESP_OK; // no calibration timer on MX
    s_boot_time_us = esp_timer_get_time();
    he_cal_button_init(&s_btn_tracker, CAL_BTN_X, CAL_BTN_Y, CAL_BTN_W, CAL_BTN_H, GESTURE_SWIPE_PX);
    he_cal_runner_init(&s_runner, NULL);

    lv_timer_create(he_cal_ui_timer_cb, 20, NULL);
    ESP_LOGI(TAG, "Hall Effect calibration UI initialized");
    return ESP_OK;
}

void he_cal_ui_set_app_capable(bool capable)
{
    if (!ui_lock()) {
        atomic_store(&s_app_capable, capable);
        return;
    }
    atomic_store(&s_app_capable, capable);
    if (capable) {
        atomic_store(&s_btn_active, false);
        s_button_touch_allowed = false;
        he_cal_button_reset(&s_btn_tracker);
    }
    ui_unlock();
}

void he_cal_ui_on_host_disconnect(void)
{
    if (!ui_lock()) return;
    handle_disconnect_locked(esp_timer_get_time());
    ui_unlock();
}

void he_cal_ui_touch_sample(bool down, int16_t x, int16_t y)
{
    if (!ui_lock()) return;
    if (down && !s_touch_down) {
        s_button_touch_allowed = atomic_load(&s_btn_active);
    }
    s_touch_down = down;
    if (!atomic_load(&s_btn_active)) {
        s_button_touch_allowed = false;
        he_cal_button_reset(&s_btn_tracker);
        ui_unlock();
        return;
    }

    bool click = s_button_touch_allowed && he_cal_button_feed_touch(&s_btn_tracker, down, x, y);
    if (!down) s_button_touch_allowed = false;
    if (click) {
        ESP_LOGI(TAG, "Calibration button touched -> starting calibration");
        he_cal_ui_start_calibration(false);
    }
    ui_unlock();
}

bool he_cal_ui_start_calibration(bool from_app)
{
    if (!ui_lock()) {
        if (from_app) {
            osupad_HeCalibrationProgress p = osupad_HeCalibrationProgress_init_zero;
            p.stage = osupad_HeCalStage_HE_CAL_STAGE_FAILED;
            strncpy(p.error_message, "Calibration UI unavailable", sizeof(p.error_message) - 1);
            protocol_send_he_calibration_progress(&p);
        }
        return false;
    }
    // Recheck under the same recursive LVGL lock as timer/cancel/finish.
    // A click queued before a capable app arrived cannot start the pad flow.
    if (!from_app && (!he_backend_is_locked() || atomic_load(&s_app_capable) ||
                      !atomic_load(&s_btn_active))) {
        ui_unlock();
        return false;
    }
    const char *reason = NULL;
    he_cal_start_check_t chk = he_cal_can_start(
        he_backend_running(),
        runtime_get_state() == OSUPAD_STATE_IDLE,
        he_backend_is_calibrating(),
        &reason);
    if (chk != HE_CAL_START_OK) {
        if (from_app) {
            osupad_HeCalibrationProgress p = osupad_HeCalibrationProgress_init_zero;
            p.stage = osupad_HeCalStage_HE_CAL_STAGE_FAILED;
            p.error = osupad_HeCalError_HE_CAL_ERROR_NONE;
            if (reason) {
                strncpy(p.error_message, reason, sizeof(p.error_message) - 1);
            }
            protocol_send_he_calibration_progress(&p);
        }
        ui_unlock();
        return false;
    }

    he_runner_progress_t prog;
    he_cal_runner_start(&s_runner);
    he_backend_start_calibration();
    s_success_until_us = 0;
    s_fail_until_us = 0;
    s_disconnect_failed = false;
    s_app_started = from_app;
    s_system_error = NULL;
    s_last_progress_us = esp_timer_get_time();
    atomic_store(&s_btn_active, false);
    he_cal_button_reset(&s_btn_tracker);
    he_cal_runner_get_progress(&s_runner, &prog);

    send_proto_progress(&prog, NULL);
    ui_unlock();
    return true;
}

void he_cal_ui_cancel_calibration(void)
{
    if (!ui_lock()) return;
    he_cal_runner_cancel(&s_runner);
    he_backend_cancel_calibration();
    s_success_until_us = 0;
    s_fail_until_us = 0;
    s_disconnect_failed = false;
    s_app_started = false;
    s_system_error = NULL;
    he_runner_progress_t prog;
    he_cal_runner_get_progress(&s_runner, &prog);
    send_proto_progress(&prog, NULL);
    ui_unlock();
}

