#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    OSUPAD_STATE_UNKNOWN = 0,
    OSUPAD_STATE_IDLE = 1,
    OSUPAD_STATE_PLAYING = 2,
    OSUPAD_STATE_COOLDOWN = 3,
    OSUPAD_STATE_SYNC = 4,
} osupad_state_t;

/**
 * @brief Initialize runtime supervisor state machine and background manager task.
 */
esp_err_t runtime_init(void);

/**
 * @brief Get current operational state.
 */
osupad_state_t runtime_get_state(void);

/**
 * @brief Notify runtime of gameplay activity from host telemetry.
 * Automatically handles IDLE -> PLAYING -> COOLDOWN -> IDLE transitions.
 */
void runtime_notify_gameplay(bool is_playing);

/**
 * @brief Restart the pad (the host asked, e.g. after an input module swap).
 * Done by the runtime task once IDLE and the running image is confirmed:
 * counters, config and layouts are saved first. Safe from any task.
 */
void runtime_request_restart(void);

/**
 * @brief Get total system uptime in seconds.
 */
uint32_t runtime_get_uptime_seconds(void);

#ifdef __cplusplus
}
#endif
