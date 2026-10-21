#pragma once

/*
 * Hall Effect calibration storage. docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Survives interrupted writes by alternating A/B records in NVS with
 * sequence numbers and CRC32 checks (he_cal_record.h).
 *
 * Missing or corrupted records fall back to factory defaults (valid=false)
 * without erasing NVS. Writes are allowed only in IDLE, and are deferred
 * during PLAYING and COOLDOWN states.
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "input/he_params.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize HE calibration storage at boot.
 * Reads A/B records from NVS, selects the active one, or sets factory defaults.
 * Never erases NVS on corrupted records.
 */
esp_err_t he_cal_init(void);

/**
 * @brief Get the active calibration for a key (0 or 1). Thread-safe.
 */
void he_cal_get(uint8_t key_index, he_calibration_t *out);

/**
 * @brief Get both keys' active calibration. Thread-safe.
 */
void he_cal_get_all(he_calibration_t out[2]);

/**
 * @brief Save new calibration for both keys.
 * If in IDLE state, writes to NVS immediately. Otherwise updates RAM and marks dirty.
 */
esp_err_t he_cal_set(const he_calibration_t in[2]);

/**
 * @brief Commit interactive calibration before publishing it in RAM.
 * IDLE only; a write failure keeps the previous active calibration.
 * Reboot still selects the newest valid NVS record, including a candidate
 * that set_blob persisted before a later error was reported.
 * The caller reloads backend thresholds after checking the result.
 */
esp_err_t he_cal_save(const he_calibration_t in[2]);

/**
 * @brief Flush deferred calibration writes to NVS if state is IDLE.
 */
esp_err_t he_cal_flush(void);

/**
 * @brief Check if there are unpersisted calibration changes.
 */
bool he_cal_is_dirty(void);

#ifdef __cplusplus
}
#endif
