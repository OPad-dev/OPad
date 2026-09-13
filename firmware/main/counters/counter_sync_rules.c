#include "counter_sync_rules.h"
#include <stdio.h>

bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len)
{
    if (!force) {
        if (host_gen < current_gen) {
            if (err_msg && err_msg_len > 0) {
                snprintf(err_msg, err_msg_len, "stale generation");
            }
            return false;
        }
        if (host_gen == current_gen && (host_k1 < current_k1 || host_k2 < current_k2)) {
            if (err_msg && err_msg_len > 0) {
                snprintf(err_msg, err_msg_len, "non-monotonic");
            }
            return false;
        }
    }
    return true;
}
