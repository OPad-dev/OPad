#include "he_cal_runner.h"
#include <string.h>

#define REST_SETTLE_SAMPLES 64 // 16 ms at one sample per 250 us frame

void he_cal_runner_init(he_cal_runner_t *r, const he_cal_capture_cfg_t *cfg)
{
    if (!r) {
        return;
    }
    memset(r, 0, sizeof(*r));
    if (cfg) {
        r->cfg = *cfg;
    } else {
        he_cal_capture_default_cfg(&r->cfg);
    }
    r->stage = HE_RUNNER_IDLE;
    r->error = HE_CAL_ERR_NONE;
    r->error_key = 255;
    r->keys = 2;
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        r->last_raw[k] = -1;
    }
}

void he_cal_runner_set_keys(he_cal_runner_t *r, uint8_t keys)
{
    if (r && keys >= 2 && keys <= HE_KEYS_MAX) {
        r->keys = keys;
    }
}

void he_cal_runner_start(he_cal_runner_t *r)
{
    if (!r) {
        return;
    }
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        he_cal_capture_init(&r->cap[k], &r->cfg);
        r->last_raw[k] = -1;
    }
    r->stage = HE_RUNNER_REST;
    r->error = HE_CAL_ERR_NONE;
    r->error_key = 255;
    r->rest_settle_count = 0;
}

void he_cal_runner_cancel(he_cal_runner_t *r)
{
    if (!r) {
        return;
    }
    r->stage = HE_RUNNER_IDLE;
    r->error = HE_CAL_ERR_NONE;
    r->error_key = 255;
}

// Shown on the pad (owner-approved wording, 2026-10-21; Q3 in
// docs/history/2026-10-21-he-calibration-report.md). One 16 px line each.
const char *he_cal_runner_error_string(he_cal_error_t err)
{
    switch (err) {
    case HE_CAL_ERR_NONE:
        return "None";
    case HE_CAL_ERR_REST_NOT_STILL:
        return "Keep both keys still while resting";
    case HE_CAL_ERR_REST_OUT_OF_RANGE:
        return "Sensor reading looks wrong";
    case HE_CAL_ERR_REST_PRESSED:
        return "A key was pressed while resting";
    case HE_CAL_ERR_SIGNAL_NOT_RISING:
        return "The key didn't register";
    case HE_CAL_ERR_SPAN_TOO_SMALL:
        return "Press each key all the way down";
    case HE_CAL_ERR_PEAKS_INCONSISTENT:
        return "Press the same way every time";
    case HE_CAL_ERR_TOO_FEW_PEAKS:
        return "Press exactly 5 times";
    case HE_CAL_ERR_TOO_MANY_PEAKS:
        return "Press exactly 5 times";
    default:
        return "Calibration failed";
    }
}

static he_runner_stage_t key_stage(int k)
{
    return (he_runner_stage_t)(HE_RUNNER_KEY1 + k);
}

static bool fail_key(he_cal_runner_t *r, int k)
{
    r->stage = HE_RUNNER_FAILED;
    r->error = r->cap[k].error;
    r->error_key = (uint8_t)k;
    return true;
}

bool he_cal_runner_feed(he_cal_runner_t *r, const int32_t *raw)
{
    if (!r || !raw) {
        return false;
    }
    for (int k = 0; k < r->keys; k++) {
        r->last_raw[k] = raw[k];
    }

    switch (r->stage) {
    case HE_RUNNER_IDLE:
    case HE_RUNNER_SUCCESS:
    case HE_RUNNER_FAILED:
        return false;

    case HE_RUNNER_REST: {
        if (r->rest_settle_count < REST_SETTLE_SAMPLES) {
            r->rest_settle_count++;
            return false;
        }
        bool all_done = true;
        for (int k = 0; k < r->keys; k++) {
            all_done &= he_cal_capture_feed_rest(&r->cap[k], raw[k]);
        }
        for (int k = 0; k < r->keys; k++) {
            if (r->cap[k].stage == HE_CAL_STAGE_FAILED) {
                return fail_key(r, k);
            }
        }
        if (all_done) {
            r->stage = HE_RUNNER_KEY1;
            return true;
        }
        return false;
    }

    case HE_RUNNER_KEY1:
    case HE_RUNNER_KEY2:
    case HE_RUNNER_KEY3: {
        int k = (int)(r->stage - HE_RUNNER_KEY1);
        if (k >= r->keys) {
            r->stage = HE_RUNNER_SUCCESS;  // never reached: the key before moves on
            return true;
        }
        uint8_t prev_peaks = r->cap[k].peak_count;
        bool done = he_cal_capture_feed_press(&r->cap[k], raw[k]);
        if (r->cap[k].stage == HE_CAL_STAGE_FAILED) {
            return fail_key(r, k);
        }
        if (done || r->cap[k].stage == HE_CAL_STAGE_COMPLETE) {
            r->results[k] = r->cap[k].result;
            r->stage = k + 1 < r->keys ? key_stage(k + 1) : HE_RUNNER_SUCCESS;
            return true;
        }
        return r->cap[k].peak_count != prev_peaks;
    }
    }

    return false;
}

bool he_cal_runner_feed_samples(he_cal_runner_t *r, const he_cal_samples_t *samples)
{
    if (!r || !samples || r->stage < HE_RUNNER_REST || r->stage > HE_RUNNER_KEY3) {
        return false;
    }
    if (samples->lost || (samples->count > 0 && samples->keys < r->keys)) {
        r->stage = HE_RUNNER_FAILED;
        r->error = HE_CAL_ERR_NONE; // transport failure, not a capture decision
        return true;
    }
    bool changed = false;
    for (uint16_t i = 0; i < samples->count; i++) {
        int32_t raw[HE_KEYS_MAX];
        for (int k = 0; k < HE_KEYS_MAX; k++) {
            raw[k] = samples->raw[i][k];
        }
        changed |= he_cal_runner_feed(r, raw);
    }
    return changed;
}

void he_cal_runner_get_progress(const he_cal_runner_t *r, he_runner_progress_t *out)
{
    if (!r || !out) {
        return;
    }
    out->stage = r->stage;
    out->error = r->error;
    out->error_key = r->error_key;
    out->error_desc = he_cal_runner_error_string(r->error);
    out->keys = r->keys;
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        out->last_raw[k] = r->last_raw[k];
    }

    switch (r->stage) {
    case HE_RUNNER_REST:
        out->active_key = 0;
        out->presses_done = 0;
        break;
    case HE_RUNNER_KEY1:
    case HE_RUNNER_KEY2:
    case HE_RUNNER_KEY3: {
        int k = (int)(r->stage - HE_RUNNER_KEY1);
        out->active_key = (uint8_t)(k + 1);
        out->presses_done = r->cap[k].peak_count;
        break;
    }
    case HE_RUNNER_SUCCESS:
        out->active_key = 0;
        out->presses_done = 5;
        break;
    case HE_RUNNER_IDLE:
    case HE_RUNNER_FAILED:
    default:
        out->active_key = 0;
        out->presses_done = 0;
        break;
    }
}

bool he_cal_runner_get_results(const he_cal_runner_t *r, he_calibration_t out[HE_KEYS_MAX])
{
    if (!r || r->stage != HE_RUNNER_SUCCESS || !out) {
        return false;
    }
    for (int k = 0; k < r->keys; k++) {
        out[k] = r->results[k];
    }
    return true;
}

bool he_cal_is_locked_decision(bool he_running, const he_calibration_t *cal, uint8_t keys)
{
    if (!he_running || !cal) {
        return false;
    }
    for (int k = 0; k < keys && k < HE_KEYS_MAX; k++) {
        if (!cal[k].valid) {
            return true;
        }
    }
    return false;
}

bool he_cal_suppress_input(bool locked, bool calibrating)
{
    return locked || calibrating;
}

he_lock_screen_mode_t he_cal_lock_screen_mode(bool capable_host_connected,
                                              bool host_ever_disconnected,
                                              int64_t elapsed_since_boot_us,
                                              int64_t boot_wait_us)
{
    if (capable_host_connected) {
        return HE_LOCK_SCREEN_USE_APP;
    }
    if (host_ever_disconnected || elapsed_since_boot_us >= boot_wait_us) {
        return HE_LOCK_SCREEN_NO_APP_BUTTON;
    }
    return HE_LOCK_SCREEN_WAIT_FOR_APP;
}

he_cal_start_check_t he_cal_can_start(bool he_running, bool is_idle, bool already_calibrating, const char **reason_out)
{
    if (!he_running) {
        if (reason_out) {
            *reason_out = "Hall Effect backend is not running";
        }
        return HE_CAL_START_REFUSED_NOT_HE;
    }
    if (!is_idle) {
        if (reason_out) {
            *reason_out = "Calibration is only allowed in IDLE";
        }
        return HE_CAL_START_REFUSED_NOT_IDLE;
    }
    if (already_calibrating) {
        if (reason_out) {
            *reason_out = "Calibration is already in progress";
        }
        return HE_CAL_START_REFUSED_BUSY;
    }
    if (reason_out) {
        *reason_out = NULL;
    }
    return HE_CAL_START_OK;
}

bool he_cal_runner_apply_to_saved(const he_cal_runner_t *r, he_calibration_t saved[HE_KEYS_MAX])
{
    return he_cal_runner_get_results(r, saved);
}
