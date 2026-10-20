#pragma once

/*
 * Hall Effect key input (CONFIG_OSUPAD_HE_BACKEND): ADC1 continuous DMA on
 * both sensors, the decision engine (he_rt.h) run in the ADC interrupt, and a
 * core-0 task that hands key changes to key_event_report. Started only when
 * the HE module is fitted; with MX nothing here runs.
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "sdkconfig.h"
#include "input/he_rt.h"
#include "input/he_params.h"

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

#else  // MX-only builds carry none of it

static inline esp_err_t he_backend_start(void) { return ESP_ERR_NOT_SUPPORTED; }
static inline bool he_backend_running(void) { return false; }
static inline bool he_backend_keys_absent(void) { return false; }
static inline void he_backend_apply_settings(const he_key_settings_t s[2]) { (void)s; }
static inline void he_backend_reload_params(void) {}
static inline esp_err_t he_backend_probe_module(int *o, int *l) { (void)o; (void)l; return ESP_ERR_NOT_SUPPORTED; }

#endif

#ifdef __cplusplus
}
#endif
