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
    r->last_raw[0] = -1;
    r->last_raw[1] = -1;
}

void he_cal_runner_start(he_cal_runner_t *r)
{
    if (!r) {
        return;
    }
    he_cal_capture_init(&r->cap[0], &r->cfg);
    he_cal_capture_init(&r->cap[1], &r->cfg);
    r->stage = HE_RUNNER_REST;
    r->error = HE_CAL_ERR_NONE;
    r->error_key = 255;
    r->rest_settle_count = 0;
    r->last_raw[0] = -1;
    r->last_raw[1] = -1;
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

// OPEN: exact friendly error wording for on-pad and app display (see report §7 Q3)
const char *he_cal_runner_error_string(he_cal_error_t err)
{
    switch (err) {
    case HE_CAL_ERR_NONE:
        return "None";
    case HE_CAL_ERR_REST_NOT_STILL:
        return "Noisy rest";
    case HE_CAL_ERR_REST_OUT_OF_RANGE:
        return "Rest out of range";
    case HE_CAL_ERR_REST_PRESSED:
        return "Key held during rest";
    case HE_CAL_ERR_SIGNAL_NOT_RISING:
        return "Signal not rising";
    case HE_CAL_ERR_SPAN_TOO_SMALL:
        return "Not pressed enough";
    case HE_CAL_ERR_PEAKS_INCONSISTENT:
        return "Presses uneven";
    case HE_CAL_ERR_TOO_FEW_PEAKS:
        return "Too few presses";
    case HE_CAL_ERR_TOO_MANY_PEAKS:
        return "Too many presses";
    default:
        return "Calibration failed";
    }
}

bool he_cal_runner_feed(he_cal_runner_t *r, int32_t raw0, int32_t raw1)
{
    if (!r) {
        return false;
    }
    r->last_raw[0] = raw0;
    r->last_raw[1] = raw1;

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

        bool done0 = he_cal_capture_feed_rest(&r->cap[0], raw0);
        bool done1 = he_cal_capture_feed_rest(&r->cap[1], raw1);

        if (r->cap[0].stage == HE_CAL_STAGE_FAILED) {
            r->stage = HE_RUNNER_FAILED;
            r->error = r->cap[0].error;
            r->error_key = 0;
            return true;
        }
        if (r->cap[1].stage == HE_CAL_STAGE_FAILED) {
            r->stage = HE_RUNNER_FAILED;
            r->error = r->cap[1].error;
            r->error_key = 1;
            return true;
        }

        if (done0 && done1) {
            r->stage = HE_RUNNER_KEY1;
            return true;
        }
        return false;
    }

    case HE_RUNNER_KEY1: {
        uint8_t prev_peaks = r->cap[0].peak_count;
        bool done = he_cal_capture_feed_press(&r->cap[0], raw0);
        if (r->cap[0].stage == HE_CAL_STAGE_FAILED) {
            r->stage = HE_RUNNER_FAILED;
            r->error = r->cap[0].error;
            r->error_key = 0;
            return true;
        }
        if (done || r->cap[0].stage == HE_CAL_STAGE_COMPLETE) {
            r->results[0] = r->cap[0].result;
            r->stage = HE_RUNNER_KEY2;
            return true;
        }
        return r->cap[0].peak_count != prev_peaks;
    }

    case HE_RUNNER_KEY2: {
        uint8_t prev_peaks = r->cap[1].peak_count;
        bool done = he_cal_capture_feed_press(&r->cap[1], raw1);
        if (r->cap[1].stage == HE_CAL_STAGE_FAILED) {
            r->stage = HE_RUNNER_FAILED;
            r->error = r->cap[1].error;
            r->error_key = 1;
            return true;
        }
        if (done || r->cap[1].stage == HE_CAL_STAGE_COMPLETE) {
            r->results[1] = r->cap[1].result;
            r->stage = HE_RUNNER_SUCCESS;
            return true;
        }
        return r->cap[1].peak_count != prev_peaks;
    }
    }

    return false;
}

bool he_cal_runner_feed_samples(he_cal_runner_t *r, const he_cal_samples_t *samples)
{
    if (!r || !samples || r->stage < HE_RUNNER_REST || r->stage > HE_RUNNER_KEY2) {
        return false;
    }
    if (samples->lost) {
        r->stage = HE_RUNNER_FAILED;
        r->error = HE_CAL_ERR_NONE; // transport failure, not a capture decision
        return true;
    }
    bool changed = false;
    for (uint16_t i = 0; i < samples->count; i++) {
        changed |= he_cal_runner_feed(r, samples->raw[i][0], samples->raw[i][1]);
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
    out->last_raw[0] = r->last_raw[0];
    out->last_raw[1] = r->last_raw[1];

    switch (r->stage) {
    case HE_RUNNER_REST:
        out->active_key = 0;
        out->presses_done = 0;
        break;
    case HE_RUNNER_KEY1:
        out->active_key = 1;
        out->presses_done = r->cap[0].peak_count;
        break;
    case HE_RUNNER_KEY2:
        out->active_key = 2;
        out->presses_done = r->cap[1].peak_count;
        break;
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

bool he_cal_runner_get_results(const he_cal_runner_t *r, he_calibration_t out[2])
{
    if (!r || r->stage != HE_RUNNER_SUCCESS || !out) {
        return false;
    }
    out[0] = r->results[0];
    out[1] = r->results[1];
    return true;
}

bool he_cal_is_locked_decision(bool he_running, bool key1_valid, bool key2_valid)
{
    if (!he_running) {
        return false;
    }
    return !key1_valid || !key2_valid;
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

bool he_cal_runner_apply_to_saved(const he_cal_runner_t *r, he_calibration_t saved[2])
{
    if (!r || !saved || r->stage != HE_RUNNER_SUCCESS) {
        return false;
    }
    saved[0] = r->results[0];
    saved[1] = r->results[1];
    return true;
}
