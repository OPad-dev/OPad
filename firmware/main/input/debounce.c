#include "debounce.h"

void debounce_init(debounce_state_t *state, bool initial_pressed)
{
    if (!state) {
        return;
    }
    state->accepted_pressed = initial_pressed;
    state->last_transition_us = 0;
    state->lockout_end_us = 0;
    state->resampled = true;
}

IRAM_ATTR debounce_action_t debounce_step(
    debounce_state_t *state,
    int64_t now_us,
    bool level,
    debounce_source_t source,
    uint32_t lockout_us
) {
    if (!state) {
        return DEBOUNCE_ACTION_NONE;
    }

    if (source == DEBOUNCE_SOURCE_EDGE) {
        // First edge accepted eagerly if outside lockout
        if (level != state->accepted_pressed) {
            if (now_us >= state->lockout_end_us) {
                state->accepted_pressed = level;
                state->last_transition_us = now_us;
                state->lockout_end_us = now_us + (int64_t)lockout_us;
                state->resampled = false;
                return level ? DEBOUNCE_ACTION_PRESS : DEBOUNCE_ACTION_RELEASE;
            }
            // Inside lockout window: edge suppressed to reject switch bounce
            return DEBOUNCE_ACTION_NONE;
        }
        return DEBOUNCE_ACTION_NONE;
    }

    if (source == DEBOUNCE_SOURCE_RESAMPLE) {
        // Resample only occurs once lockout expires
        if (now_us < state->lockout_end_us) {
            return DEBOUNCE_ACTION_NONE;
        }
        if (state->resampled) {
            return DEBOUNCE_ACTION_NONE;
        }

        state->resampled = true;
        // If pin level differs from accepted state when lockout ends, correct it
        if (level != state->accepted_pressed) {
            state->accepted_pressed = level;
            state->last_transition_us = now_us;
            state->lockout_end_us = now_us + (int64_t)lockout_us;
            state->resampled = false;
            return level ? DEBOUNCE_ACTION_PRESS : DEBOUNCE_ACTION_RELEASE;
        }
        return DEBOUNCE_ACTION_NONE;
    }

    return DEBOUNCE_ACTION_NONE;
}
