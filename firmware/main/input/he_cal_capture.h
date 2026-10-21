#pragma once

/*
 * Pure-C Hall Effect calibration capture algorithm.
 * docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Captures explicit per-key calibration:
 * 1. Rest: averaged over a still window where peak-to-peak spread is within
 *    the noise floor. Never taken while pressed or railed.
 * 2. Full press: user presses fully 5 times. Detects the 5 peaks, selects the
 *    median peak (softer than bottoming out hard into the damper), and validates
 *    span, signal direction, and peak consistency.
 *
 * Pure C, no ESP-IDF/FreeRTOS, host-tested with synthetic and bench-shaped traces.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "input/he_params.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_CAL_MAX_PEAKS 16

typedef enum {
    HE_CAL_ERR_NONE = 0,
    HE_CAL_ERR_REST_NOT_STILL,
    HE_CAL_ERR_REST_OUT_OF_RANGE,
    HE_CAL_ERR_REST_PRESSED,
    HE_CAL_ERR_SIGNAL_NOT_RISING,
    HE_CAL_ERR_SPAN_TOO_SMALL,
    HE_CAL_ERR_PEAKS_INCONSISTENT,
    HE_CAL_ERR_TOO_FEW_PEAKS,
    HE_CAL_ERR_TOO_MANY_PEAKS,
} he_cal_error_t;

typedef enum {
    HE_CAL_STAGE_IDLE = 0,
    HE_CAL_STAGE_REST,
    HE_CAL_STAGE_PRESSES,
    HE_CAL_STAGE_COMPLETE,
    HE_CAL_STAGE_FAILED,
} he_cal_stage_t;

typedef struct {
    uint16_t rest_window_samples; // 64 frame samples = 16 ms at 4 kHz in the UI flow
    int32_t  rest_max_spread;     // Max allowable p-p spread at rest (e.g. 36)
    int32_t  rest_min_raw;        // Min acceptable rest count (e.g. 1500)
    int32_t  rest_max_raw;        // Max acceptable rest count (e.g. 2500)
    uint8_t  target_peaks;        // Target peak count (5)
    int32_t  min_span;            // Minimum allowable span (full - rest, e.g. 250)
    int32_t  press_arm_span;      // Movement above rest to arm peak detector (e.g. 120)
    int32_t  release_arm_span;    // Return towards rest to confirm peak (e.g. 60)
    int32_t  max_peak_diff;       // Max allowable deviation from median peak (e.g. 120)
} he_cal_capture_cfg_t;

typedef struct {
    he_cal_capture_cfg_t cfg;
    he_cal_stage_t stage;
    he_cal_error_t error;

    // Rest capture accumulator
    int64_t rest_sum;
    int32_t rest_min;
    int32_t rest_max;
    uint16_t rest_count;
    int32_t calculated_rest;

    // Press stroke peak tracking
    bool in_stroke;
    int32_t current_stroke_peak;
    uint8_t peak_count;
    int32_t peaks[HE_CAL_MAX_PEAKS];

    he_calibration_t result;
} he_cal_capture_t;

/**
 * Default calibration capture parameters matching V1.1 bench measurements.
 */
void he_cal_capture_default_cfg(he_cal_capture_cfg_t *cfg);

/**
 * Initialize capture state machine.
 */
void he_cal_capture_init(he_cal_capture_t *cap, const he_cal_capture_cfg_t *cfg);

/**
 * Feed sample during rest capture.
 * Returns true when rest window completes successfully, false if in progress or failed.
 */
bool he_cal_capture_feed_rest(he_cal_capture_t *cap, int32_t raw);

/**
 * Feed sample during full-press capture.
 * Returns true when all peaks are collected and validated, false if in progress or failed.
 */
bool he_cal_capture_feed_press(he_cal_capture_t *cap, int32_t raw);

/**
 * Batch processing helper: feeds rest samples, then press samples, and evaluates.
 */
he_cal_error_t he_cal_capture_process(const he_cal_capture_cfg_t *cfg,
                                      const int32_t *rest_samples, size_t rest_len,
                                      const int32_t *press_samples, size_t press_len,
                                      he_calibration_t *out_cal);

const char *he_cal_error_to_name(he_cal_error_t err);

#ifdef __cplusplus
}
#endif
