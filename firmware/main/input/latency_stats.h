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
    uint32_t dropped_reports;  // HID reports that could not be submitted (endpoint busy)
} latency_stats_t;

/**
 * @brief Record one key-edge-to-HID-submit latency sample.
 * Single writer: only call from the keypad task. Lock-free, no allocation.
 */
void latency_stats_record(uint32_t latency_us);

/**
 * @brief Count a HID report that could not be submitted immediately.
 */
void latency_stats_record_drop(void);

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
