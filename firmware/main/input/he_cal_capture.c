#include "he_cal_capture.h"
#include <stdlib.h>
#include <string.h>

void he_cal_capture_default_cfg(he_cal_capture_cfg_t *cfg)
{
    if (!cfg) {
        return;
    }
    cfg->rest_window_samples = 64;
    cfg->rest_max_spread = 36;
    cfg->rest_min_raw = 1500;
    cfg->rest_max_raw = 2500;
    cfg->target_peaks = 5;
    cfg->min_span = 250;
    cfg->press_arm_span = 120;
    cfg->release_arm_span = 60;
    cfg->max_peak_diff = 120;
}

void he_cal_capture_init(he_cal_capture_t *cap, const he_cal_capture_cfg_t *cfg)
{
    if (!cap) {
        return;
    }
    memset(cap, 0, sizeof(*cap));
    if (cfg) {
        cap->cfg = *cfg;
    } else {
        he_cal_capture_default_cfg(&cap->cfg);
    }
    cap->stage = HE_CAL_STAGE_REST;
    cap->rest_min = INT32_MAX;
    cap->rest_max = INT32_MIN;
}

bool he_cal_capture_feed_rest(he_cal_capture_t *cap, int32_t raw)
{
    if (!cap || cap->stage != HE_CAL_STAGE_REST) {
        return false;
    }

    if (raw < cap->rest_min) {
        cap->rest_min = raw;
    }
    if (raw > cap->rest_max) {
        cap->rest_max = raw;
    }
    cap->rest_sum += raw;
    cap->rest_count++;

    if (cap->rest_count < cap->cfg.rest_window_samples) {
        return false;
    }

    int32_t mean = (int32_t)((cap->rest_sum + cap->rest_count / 2) / cap->rest_count);
    int32_t spread = cap->rest_max - cap->rest_min;

    if (mean < cap->cfg.rest_min_raw || mean > cap->cfg.rest_max_raw) {
        cap->stage = HE_CAL_STAGE_FAILED;
        cap->error = HE_CAL_ERR_REST_OUT_OF_RANGE;
        return false;
    }

    if (spread > cap->cfg.rest_max_spread) {
        cap->stage = HE_CAL_STAGE_FAILED;
        cap->error = HE_CAL_ERR_REST_NOT_STILL;
        return false;
    }

    cap->calculated_rest = mean;
    cap->stage = HE_CAL_STAGE_PRESSES;
    return true;
}

static void sort_peaks(int32_t *p, uint8_t n)
{
    for (uint8_t i = 1; i < n; i++) {
        int32_t key = p[i];
        int j = (int)i - 1;
        while (j >= 0 && p[j] > key) {
            p[j + 1] = p[j];
            j--;
        }
        p[j + 1] = key;
    }
}

static bool finish_press_evaluation(he_cal_capture_t *cap)
{
    if (cap->peak_count < cap->cfg.target_peaks) {
        cap->stage = HE_CAL_STAGE_FAILED;
        cap->error = HE_CAL_ERR_TOO_FEW_PEAKS;
        return false;
    }

    int32_t sorted[HE_CAL_MAX_PEAKS];
    for (uint8_t i = 0; i < cap->peak_count; i++) {
        sorted[i] = cap->peaks[i];
    }
    sort_peaks(sorted, cap->peak_count);

    int32_t median = sorted[cap->peak_count / 2];

    if (median <= cap->calculated_rest || sorted[0] <= cap->calculated_rest) {
        cap->stage = HE_CAL_STAGE_FAILED;
        cap->error = HE_CAL_ERR_SIGNAL_NOT_RISING;
        return false;
    }

    int32_t span = median - cap->calculated_rest;
    if (span < cap->cfg.min_span) {
        cap->stage = HE_CAL_STAGE_FAILED;
        cap->error = HE_CAL_ERR_SPAN_TOO_SMALL;
        return false;
    }

    for (uint8_t i = 0; i < cap->peak_count; i++) {
        int32_t diff = abs(cap->peaks[i] - median);
        if (diff > cap->cfg.max_peak_diff) {
            cap->stage = HE_CAL_STAGE_FAILED;
            cap->error = HE_CAL_ERR_PEAKS_INCONSISTENT;
            return false;
        }
    }

    cap->result.rest = cap->calculated_rest;
    cap->result.full = median;
    cap->result.valid = true;
    cap->stage = HE_CAL_STAGE_COMPLETE;
    cap->error = HE_CAL_ERR_NONE;
    return true;
}

bool he_cal_capture_feed_press(he_cal_capture_t *cap, int32_t raw)
{
    if (!cap || cap->stage != HE_CAL_STAGE_PRESSES) {
        return false;
    }

    int32_t press_threshold = cap->calculated_rest + cap->cfg.press_arm_span;
    int32_t release_threshold = cap->calculated_rest + cap->cfg.release_arm_span;

    if (!cap->in_stroke) {
        if (raw >= press_threshold) {
            cap->in_stroke = true;
            cap->current_stroke_peak = raw;
        }
    } else {
        if (raw > cap->current_stroke_peak) {
            cap->current_stroke_peak = raw;
        }
        if (raw <= release_threshold) {
            // Stroke finished
            cap->in_stroke = false;
            if (cap->peak_count < HE_CAL_MAX_PEAKS) {
                cap->peaks[cap->peak_count++] = cap->current_stroke_peak;
            }
            if (cap->peak_count == cap->cfg.target_peaks) {
                return finish_press_evaluation(cap);
            }
        }
    }

    return false;
}

he_cal_error_t he_cal_capture_process(const he_cal_capture_cfg_t *cfg,
                                      const int32_t *rest_samples, size_t rest_len,
                                      const int32_t *press_samples, size_t press_len,
                                      he_calibration_t *out_cal)
{
    he_cal_capture_t cap;
    he_cal_capture_init(&cap, cfg);

    for (size_t i = 0; i < rest_len; i++) {
        if (he_cal_capture_feed_rest(&cap, rest_samples[i])) {
            break;
        }
        if (cap.stage == HE_CAL_STAGE_FAILED) {
            return cap.error;
        }
    }

    if (cap.stage != HE_CAL_STAGE_PRESSES) {
        return cap.error ? cap.error : HE_CAL_ERR_REST_NOT_STILL;
    }

    for (size_t i = 0; i < press_len; i++) {
        if (he_cal_capture_feed_press(&cap, press_samples[i])) {
            break;
        }
        if (cap.stage == HE_CAL_STAGE_FAILED) {
            return cap.error;
        }
    }

    if (cap.stage == HE_CAL_STAGE_PRESSES && cap.peak_count < cap.cfg.target_peaks) {
        return HE_CAL_ERR_TOO_FEW_PEAKS;
    }

    if (cap.stage == HE_CAL_STAGE_COMPLETE) {
        if (out_cal) {
            *out_cal = cap.result;
        }
        return HE_CAL_ERR_NONE;
    }

    return cap.error;
}

const char *he_cal_error_to_name(he_cal_error_t err)
{
    switch (err) {
    case HE_CAL_ERR_NONE:
        return "None";
    case HE_CAL_ERR_REST_NOT_STILL:
        return "Rest not still";
    case HE_CAL_ERR_REST_OUT_OF_RANGE:
        return "Rest out of range";
    case HE_CAL_ERR_REST_PRESSED:
        return "Rest taken while pressed";
    case HE_CAL_ERR_SIGNAL_NOT_RISING:
        return "Signal not rising on press";
    case HE_CAL_ERR_SPAN_TOO_SMALL:
        return "Travel span too small";
    case HE_CAL_ERR_PEAKS_INCONSISTENT:
        return "Press peaks inconsistent";
    case HE_CAL_ERR_TOO_FEW_PEAKS:
        return "Too few presses detected";
    case HE_CAL_ERR_TOO_MANY_PEAKS:
        return "Too many presses detected";
    default:
        return "Unknown error";
    }
}
