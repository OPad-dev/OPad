#pragma once

/*
 * Hall Effect calibration runner / coordinator.
 * docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Coordinates the full calibration workflow:
 * 1. Rest: collects 64 samples at rest for both channels simultaneously.
 * 2. Key 1: detects 5 presses on Key 1, evaluates median peak and consistency.
 * 3. Key 2: detects 5 presses on Key 2, evaluates median peak and consistency.
 * 4. Completion: yields two validated he_calibration_t structs ready for he_cal_save().
 *
 * Pure C, no ESP-IDF/FreeRTOS dependencies, host-testable.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "he_cal_capture.h"
#include "he_params.h"
#include "he_cal_samples.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    HE_RUNNER_IDLE = 0,
    HE_RUNNER_REST,
    HE_RUNNER_KEY1,
    HE_RUNNER_KEY2,
    HE_RUNNER_KEY3,     // HE V3 only (he_cal_runner_set_keys)
    HE_RUNNER_SUCCESS,
    HE_RUNNER_FAILED,
} he_runner_stage_t;

typedef struct {
    he_runner_stage_t stage;
    uint8_t active_key;       // 1, 2 or 3 for the key being pressed, 0 otherwise
    uint8_t presses_done;     // 0..5
    he_cal_error_t error;
    uint8_t error_key;        // Key index that failed (0-2), or 255
    const char *error_desc;
    int32_t last_raw[HE_KEYS_MAX];
    uint8_t keys;             // keys this calibration covers (2 or 3)
} he_runner_progress_t;

typedef struct {
    he_cal_capture_cfg_t cfg;
    he_runner_stage_t stage;
    uint8_t keys;               // 2 (V1, the default) or 3 (V3)
    he_cal_capture_t cap[HE_KEYS_MAX];
    he_cal_error_t error;
    uint8_t error_key;
    he_calibration_t results[HE_KEYS_MAX];
    int32_t last_raw[HE_KEYS_MAX];
    uint16_t rest_settle_count; // Brief settle delay before collecting rest
} he_cal_runner_t;

/**
 * @brief Initialize runner with configuration (or default if cfg is NULL).
 */
void he_cal_runner_init(he_cal_runner_t *r, const he_cal_capture_cfg_t *cfg);

/**
 * @brief Start calibration process (transitions from IDLE/FAILED/SUCCESS to REST).
 */
/** Calibrate this many keys (2 or 3). Before he_cal_runner_start. */
void he_cal_runner_set_keys(he_cal_runner_t *r, uint8_t keys);
void he_cal_runner_start(he_cal_runner_t *r);

/**
 * @brief Cancel calibration and return to IDLE.
 */
void he_cal_runner_cancel(he_cal_runner_t *r);

/**
 * @brief Feed one sample: raw[] holds a reading per key, r->keys of them.
 * @return True if stage changed or press count incremented.
 */
bool he_cal_runner_feed(he_cal_runner_t *r, const int32_t *raw);

// Drain an ordered ISR batch. Missing/overflowed data fails the attempt;
// extrema alone cannot preserve stroke arming and release order.
bool he_cal_runner_feed_samples(he_cal_runner_t *r, const he_cal_samples_t *samples);

/**
 * @brief Get current calibration progress snapshot.
 */
void he_cal_runner_get_progress(const he_cal_runner_t *r, he_runner_progress_t *out);

/**
 * @brief Get calibration results when stage is HE_RUNNER_SUCCESS.
 */
/** out[] gets r->keys results; later entries are left alone. */
bool he_cal_runner_get_results(const he_cal_runner_t *r, he_calibration_t out[HE_KEYS_MAX]);

/**
 * @brief Friendly short error description for display on screen.
 */
const char *he_cal_runner_error_string(he_cal_error_t err);

typedef enum {
    HE_LOCK_SCREEN_WAIT_FOR_APP = 0, // Within boot grace window and no capable host yet
    HE_LOCK_SCREEN_USE_APP,          // Calibration-capable host connected ("Calibrate in the OPad app", no button)
    HE_LOCK_SCREEN_NO_APP_BUTTON,    // No capable host after grace window (or host disconnected): red screen + URL + Calibrate button
} he_lock_screen_mode_t;

typedef enum {
    HE_CAL_START_OK = 0,
    HE_CAL_START_REFUSED_NOT_HE,
    HE_CAL_START_REFUSED_NOT_IDLE,
    HE_CAL_START_REFUSED_BUSY,
} he_cal_start_check_t;

/**
 * @brief Pure decision: is the pad locked waiting for calibration?
 * Locked iff the HE backend is running and either key lacks a valid calibration.
 * Never locked on MX (he_running == false).
 */
/** Locked while any of the module's keys (keys of cal[]) has no calibration. */
bool he_cal_is_locked_decision(bool he_running, const he_calibration_t *cal, uint8_t keys);

/**
 * @brief Pure decision: should key and touch/swipe input be suppressed from reaching the PC?
 * True whenever the pad is locked or actively calibrating.
 */
bool he_cal_suppress_input(bool locked, bool calibrating);

/**
 * @brief Pure decision: which lock-screen variant to display.
 */
he_lock_screen_mode_t he_cal_lock_screen_mode(bool capable_host_connected,
                                              bool host_ever_disconnected,
                                              int64_t elapsed_since_boot_us,
                                              int64_t boot_wait_us);

/**
 * @brief Pure decision: can calibration be started right now?
 */
he_cal_start_check_t he_cal_can_start(bool he_running, bool is_idle, bool already_calibrating, const char **reason_out);

/**
 * @brief Apply runner results to saved calibration only if stage == HE_RUNNER_SUCCESS.
 * Leaves saved[0..1] untouched on failure or cancel.
 */
bool he_cal_runner_apply_to_saved(const he_cal_runner_t *r, he_calibration_t saved[HE_KEYS_MAX]);

#ifdef __cplusplus
}
#endif
