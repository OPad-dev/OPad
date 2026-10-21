#pragma once

/*
 * Hall Effect key input (CONFIG_OSUPAD_HE_BACKEND): ADC1 continuous DMA on
 * both sensors, the decision engine (he_rt.h) run in the ADC interrupt, and a
 * core-0 task that hands key changes to key_event_report. Started only when
 * the HE module is fitted; with MX nothing here runs.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "esp_err.h"
#include "sdkconfig.h"
#include "input/he_rt.h"
#include "input/he_params.h"
#include "input/he_cal_samples.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t frames;          // DMA frames handled
    uint32_t late_frames;     // a frame more than two frame periods after the previous
    uint32_t max_gap_us;      // longest time between two frames
    uint32_t foreign_words;   // conversions from a channel that is not a key's
    uint32_t railed_samples;  // key conversions at 0 or full scale
    uint32_t key_faults;      // a key without a valid reading for the stall time
    uint32_t stalls;          // no frames for the stall time (ADC restarted)
    uint32_t restarts;        // ADC restarts that succeeded
    uint32_t isr_max_cycles;  // longest frame interrupt (CPU cycles, 240 per us)
    int32_t raw[2];           // newest reading per key, -1 = none yet
} he_backend_stats_t;

#ifdef CONFIG_OSUPAD_HE_BACKEND

/**
 * @brief Start HE input: creates the core-0 task, which sets up and starts
 * the ADC. Call once, with the key pins left analog (board_keys_set_analog).
 */
esp_err_t he_backend_start(void);

/**
 * @brief True once he_backend_start has run: ADC1 then belongs to it, and
 * nothing else may open it (the board test's ID reading).
 */
bool he_backend_running(void);

/**
 * @brief New thresholds for one key (0 or 1). Safe from any task; applied as
 * he_rt_set_params describes, so it never presses a key.
 */
void he_backend_set_params(uint8_t key_index, const he_rt_params_t *p);

/**
 * @brief Apply new per-key HE settings (from device_config).
 * Safe from any task; applies thresholds as he_rt_set_params describes.
 */
void he_backend_apply_settings(const he_key_settings_t s[2]);

/**
 * @brief Rebuild thresholds using active calibration and current settings.
 * Safe from any task; applies thresholds as he_rt_set_params describes.
 */
void he_backend_reload_params(void);

void he_backend_get_stats(he_backend_stats_t *out);

/**
 * @brief True when neither key has had a valid reading for a while: no HE
 * module on the connector any more (nothing, or an MX module).
 */
bool he_backend_keys_absent(void);

/**
 * @brief Read the module ID (board_read_module_id) with the HE ADC paused for
 * the ~8 ms it takes. For the runtime's module watch, only while
 * he_backend_keys_absent(): the keys get no readings meanwhile.
 */
esp_err_t he_backend_probe_module(int *out_open_mv, int *out_loaded_mv);

/**
 * @brief True if the backend is running and either key has no valid saved calibration.
 * When locked, keypresses to the PC are suppressed (thresholds set to HE_PARAMS_NEVER).
 */
bool he_backend_is_locked(void);

/**
 * @brief True if interactive calibration is currently running.
 */
bool he_backend_is_calibrating(void);

/**
 * @brief The noise floors thresholds are built with (he_limits_t), in raw
 * counts, for the app to show where a key really presses.
 */
he_limits_t he_backend_noise_floors(void);

/**
 * @brief Start interactive calibration. Temporarily suppresses PC keypresses
 * and enables sample tapping in the frame ISR.
 * Waits for the reporting task and USB callbacks before capture starts.
 * Failure restores thresholds without starting capture. Caller serializes the
 * UI lifecycle and reserves touch; never call from the HE or TinyUSB task.
 */
esp_err_t he_backend_start_calibration(void);

/**
 * @brief Cancel interactive calibration, restoring normal thresholds.
 */
void he_backend_cancel_calibration(void);

/**
 * @brief Finish calibration: saves the new calibration in NVS and updates thresholds.
 */
esp_err_t he_backend_finish_calibration(const he_calibration_t results[2]);

/**
 * @brief Swap out the ordered samples and extrema accumulated since the last read.
 * Single reader (LVGL). The returned buffer belongs to it until its next call.
 * The empty replacement is prepared outside s_lock; the swap holds it briefly.
 */
const he_cal_samples_t *he_backend_take_cal_samples(void);

#else  // MX-only builds carry none of it

static inline esp_err_t he_backend_start(void) { return ESP_ERR_NOT_SUPPORTED; }
static inline bool he_backend_running(void) { return false; }
static inline bool he_backend_keys_absent(void) { return false; }
static inline void he_backend_apply_settings(const he_key_settings_t s[2]) { (void)s; }
static inline void he_backend_reload_params(void) {}
static inline esp_err_t he_backend_probe_module(int *o, int *l) { (void)o; (void)l; return ESP_ERR_NOT_SUPPORTED; }
static inline bool he_backend_is_locked(void) { return false; }
static inline bool he_backend_is_calibrating(void) { return false; }
static inline he_limits_t he_backend_noise_floors(void) { return (he_limits_t){0}; }
static inline esp_err_t he_backend_start_calibration(void) { return ESP_ERR_NOT_SUPPORTED; }
static inline void he_backend_cancel_calibration(void) {}
static inline esp_err_t he_backend_finish_calibration(const he_calibration_t r[2]) { (void)r; return ESP_ERR_NOT_SUPPORTED; }
static inline const he_cal_samples_t *he_backend_take_cal_samples(void) { return NULL; }

#endif

#ifdef __cplusplus
}
#endif
