#pragma once

/*
 * Hall Effect calibration runner / coordinator.
 * docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Coordinates the full calibration workflow:
 * 1. Rest: collects 64 samples at rest for both channels simultaneously.
 * 2. Key 1: detects 5 presses on Key 1, evaluates median peak and consistency.
 * 3. Key 2: detects 5 presses on Key 2, evaluates median peak and consistency.
 * 4. Completion: yields two validated he_calibration_t structs ready for he_cal_set().
 *
 * Pure C, no ESP-IDF/FreeRTOS dependencies, host-testable.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "he_cal_capture.h"
#include "he_params.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    HE_RUNNER_IDLE = 0,
    HE_RUNNER_REST,
    HE_RUNNER_KEY1,
    HE_RUNNER_KEY2,
    HE_RUNNER_SUCCESS,
    HE_RUNNER_FAILED,
} he_runner_stage_t;

typedef struct {
    he_runner_stage_t stage;
    uint8_t active_key;       // 1 for Key 1, 2 for Key 2, 0 otherwise
    uint8_t presses_done;     // 0..5
    he_cal_error_t error;
    uint8_t error_key;        // Key index that failed (0 or 1), or 255
    const char *error_desc;
    int32_t last_raw[2];
} he_runner_progress_t;

typedef struct {
    he_cal_capture_cfg_t cfg;
    he_runner_stage_t stage;
    he_cal_capture_t cap[2];
    he_cal_error_t error;
    uint8_t error_key;
    he_calibration_t results[2];
    int32_t last_raw[2];
    uint16_t rest_settle_count; // Brief settle delay before collecting rest
} he_cal_runner_t;

/**
 * @brief Initialize runner with configuration (or default if cfg is NULL).
 */
void he_cal_runner_init(he_cal_runner_t *r, const he_cal_capture_cfg_t *cfg);

/**
 * @brief Start calibration process (transitions from IDLE/FAILED/SUCCESS to REST).
 */
void he_cal_runner_start(he_cal_runner_t *r);

/**
 * @brief Cancel calibration and return to IDLE.
 */
void he_cal_runner_cancel(he_cal_runner_t *r);

/**
 * @brief Feed raw ADC readings for both keys.
 * @param r Pointer to runner.
 * @param raw0 Reading for Key 1.
 * @param raw1 Reading for Key 2.
 * @return True if stage changed or press count incremented.
 */
bool he_cal_runner_feed(he_cal_runner_t *r, int32_t raw0, int32_t raw1);

/**
 * @brief Get current calibration progress snapshot.
 */
void he_cal_runner_get_progress(const he_cal_runner_t *r, he_runner_progress_t *out);

/**
 * @brief Get calibration results when stage is HE_RUNNER_SUCCESS.
 */
bool he_cal_runner_get_results(const he_cal_runner_t *r, he_calibration_t out[2]);

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
bool he_cal_is_locked_decision(bool he_running, bool key1_valid, bool key2_valid);

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
bool he_cal_runner_apply_to_saved(const he_cal_runner_t *r, he_calibration_t saved[2]);

#ifdef __cplusplus
}
#endif
