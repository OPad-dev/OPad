#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t generation;
    uint64_t lifetime_key1;
    uint64_t lifetime_key2;
} counters_snapshot_t;

/**
 * @brief Initialize persistent counters from NVS storage.
 */
esp_err_t counters_init(void);

/**
 * @brief Get current consolidated lifetime counters (baseline + RAM increments).
 */
void counters_get(counters_snapshot_t *snapshot);

/**
 * @brief Check whether NVS storage is available for counters.
 */
bool counters_is_nvs_ok(void);

/**
 * @brief Total number of successful NVS checkpoint writes since boot.
 */
uint32_t counters_get_nvs_writes(void);

/**
 * @brief Record an NVS write commit from any subsystem (counters, config, ui).
 */
void counters_record_nvs_write(void);

/**
 * @brief Update baseline counters from host synchronization.
 * Only permitted when system is in IDLE state.
 * @param err_msg Optional buffer to receive reason string on rejection.
 * @param err_msg_len Size of err_msg buffer.
 */
esp_err_t counters_sync_from_host(uint32_t generation, uint64_t k1, uint64_t k2, bool force, char *err_msg, size_t err_msg_len);

/**
 * @brief Checkpoint dirty RAM counters to NVS flash storage.
 * GUARANTEED NO-OP if current state is PLAYING or COOLDOWN.
 * @param force If true, forces write even if periodic interval hasn't elapsed (still subject to IDLE check).
 */
esp_err_t counters_checkpoint(bool force);

/**
 * @brief Reset counters to initial state (generation incremented, counts zeroed).
 */
esp_err_t counters_reset(void);

#ifdef __cplusplus
}
#endif
