#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Pure function to validate counter synchronization acceptance rules (§13, P1-1).
 * Safe to call on host.
 */
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len);

/** Quiet time after the last keypress before dirty counters are written */
#define COUNTERS_CHECKPOINT_QUIET_US   (30LL * 1000000)
/** Upper bound between writes while keys keep being pressed outside gameplay */
#define COUNTERS_CHECKPOINT_MAX_AGE_US (300LL * 1000000)

/**
 * @brief Pure debounced-checkpoint policy. Safe to call on host.
 * Write when the counters changed and either no key was pressed for
 * COUNTERS_CHECKPOINT_QUIET_US or the last write is COUNTERS_CHECKPOINT_MAX_AGE_US old.
 */
bool counters_checkpoint_due(bool dirty, int64_t now_us, int64_t last_press_us,
                             int64_t last_checkpoint_us);

#ifdef __cplusplus
}
#endif
