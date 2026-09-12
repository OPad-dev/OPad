#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef ESP_PLATFORM
#include "esp_attr.h"
#else
#ifndef IRAM_ATTR
#define IRAM_ATTR
#endif
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define DEBOUNCE_MIN_US     500
#define DEBOUNCE_MAX_US     20000
#define DEBOUNCE_DEFAULT_US 3000

typedef enum {
    DEBOUNCE_SOURCE_EDGE = 0,
    DEBOUNCE_SOURCE_RESAMPLE = 1,
} debounce_source_t;

typedef enum {
    DEBOUNCE_ACTION_NONE = 0,
    DEBOUNCE_ACTION_PRESS = 1,   // Transition from released -> pressed
    DEBOUNCE_ACTION_RELEASE = 2, // Transition from pressed -> released
} debounce_action_t;

typedef struct {
    bool accepted_pressed;      // Currently accepted key state (true = pressed, false = released)
    int64_t last_transition_us; // Timestamp of the last accepted transition
    int64_t lockout_end_us;     // End of active lockout window (0 if none)
    bool resampled;             // True if resampled since lockout started
} debounce_state_t;

/**
 * @brief Initialize debounce state with the current physical level.
 */
void debounce_init(debounce_state_t *state, bool initial_pressed);

/**
 * @brief Step the eager debounce state machine.
 *
 * Pure function with no ESP-IDF dependencies, safe to call from ISR or host test.
 *
 * @param state Pointer to key's debounce state.
 * @param now_us Current monotonic timestamp in microseconds.
 * @param level Current physical GPIO level (true = pressed / active low switch grounded).
 * @param source Trigger source: DEBOUNCE_SOURCE_EDGE (GPIO interrupt) or DEBOUNCE_SOURCE_RESAMPLE (timeout).
 * @param lockout_us Configured debounce lockout window in microseconds.
 * @return debounce_action_t The action to take (NONE, PRESS, or RELEASE).
 */
debounce_action_t debounce_step(
    debounce_state_t *state,
    int64_t now_us,
    bool level,
    debounce_source_t source,
    uint32_t lockout_us
);

#ifdef __cplusplus
}
#endif
