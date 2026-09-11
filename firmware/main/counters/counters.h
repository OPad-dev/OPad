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
 * @brief Update baseline counters from host synchronization.
 * Only permitted when system is in IDLE state.
 */
esp_err_t counters_sync_from_host(uint32_t generation, uint64_t k1, uint64_t k2, bool force);

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
