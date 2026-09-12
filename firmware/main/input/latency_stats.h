#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint32_t samples;
    uint32_t p50_us;
    uint32_t p99_us;
    uint32_t p999_us;
    uint32_t max_us;
    uint32_t deferred_reports; // Key changes that waited for the next USB poll (endpoint busy), then resent
} latency_stats_t;

/**
 * @brief Record one key-edge-to-HID-submit latency sample.
 * Called from the keypad task and the TinyUSB task (both core 0). Lock-free, no allocation.
 */
void latency_stats_record(uint32_t latency_us);

/**
 * @brief Count a key change whose report could not be submitted immediately.
 * Its latency is recorded when the report is resent.
 */
void latency_stats_record_deferred(void);

/**
 * @brief Compute a snapshot (percentiles from the histogram). Safe from any task.
 */
void latency_stats_get(latency_stats_t *out);

/**
 * @brief Clear all samples, e.g. before measuring one map.
 */
void latency_stats_reset(void);

#ifdef __cplusplus
}
#endif
