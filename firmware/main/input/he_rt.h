#pragma once

/*
 * Hall Effect key decision engine: one raw ADC reading in, at most one key
 * change out. Pure C, no ESP-IDF, run per DMA frame on the key path and on the
 * host by firmware/test/host/test_he_rt.c. Integer only, no division, no
 * allocation. Thresholds are raw ADC counts, worked out off the key path by
 * he_params_build (he_params.h).
 *
 * The reading rises as the key goes down (the switch magnet's N pole faces
 * the sensor). It is never clamped to the calibrated full press: a hard press
 * on the silent damper reads past it, and rapid trigger must still see the key
 * come back up from there.
 *
 * States (docs/specs/v2-hall-effect-design.md, "Rapid trigger"):
 *   ARMED        up; the next press must cross `actuation`
 *   PRESSED      down; tracks the deepest reading (peak)
 *   RT_RELEASED  up after a rapid-trigger release; tracks the shallowest
 *                reading (trough) and presses again `rt_press` counts deeper
 *
 * Plain mode (rapid_trigger off): press at `actuation`, release at `release`,
 * like a mechanical switch with hysteresis. Rapid trigger: after the first
 * press, release on `rt_release` counts of upward movement from the peak and
 * press again on `rt_press` counts of downward movement from the trough, for
 * as long as the key stays deeper than `release` (normal) or `top_reset`
 * (continuous). Leaving that region returns the key to ARMED.
 *
 * Nothing here runs on a timer: a key that does not move never changes state.
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    HE_RT_NONE = 0,
    HE_RT_DOWN = 1,
    HE_RT_UP = 2,
} he_rt_event_t;

typedef enum {
    HE_RT_ARMED = 0,
    HE_RT_PRESSED = 1,
    HE_RT_RT_RELEASED = 2,
} he_rt_state_t;

// One key's thresholds, in raw ADC counts. he_params_build guarantees
// top_reset <= release < actuation <= peak_cap and rt_press, rt_release >= 1.
typedef struct {
    int32_t actuation;   // ARMED -> DOWN at or above this
    int32_t release;     // at or below: out of the actuation region (plain release)
    int32_t top_reset;   // at or below: continuous rapid trigger ends
    int32_t rt_press;    // movement down from the trough that presses again
    int32_t rt_release;  // movement up from the peak that releases
    int32_t peak_cap;    // the peak stops following past here (bottom dead zone); INT32_MAX = off
    bool rapid_trigger;
    bool continuous;     // only with rapid_trigger
} he_rt_params_t;

typedef struct {
    he_rt_params_t p;
    he_rt_params_t pending;  // waits for the key to be up (has_pending)
    bool has_pending;
    he_rt_state_t state;
    int32_t extreme;         // peak while PRESSED, trough while RT_RELEASED
    bool need_release;       // ARMED: wait for a reading at or below `release` before a press
} he_rt_key_t;

/**
 * Start a key in ARMED. It must read at or below `release` once before it can
 * press, so a key held down at boot (or a reading not yet seen) never becomes
 * a press on its own.
 */
void he_rt_init(he_rt_key_t *k, const he_rt_params_t *p);

/**
 * Step the key with its newest valid reading. Key path: no side effects
 * beyond *k.
 */
he_rt_event_t he_rt_step(he_rt_key_t *k, int32_t raw);

/**
 * New thresholds (settings or calibration changed). Applied at once while the
 * key is up, otherwise on its next release, so a held key keeps its press. A
 * key it is applied to goes back to ARMED and, if it reads deeper than the new
 * `release` (`raw`), must come up past it before it can press: a threshold
 * change never presses a key by itself. An outstanding boot/fault/reload
 * release requirement survives this call, even if `raw` looks released; only
 * a subsequent he_rt_step observation can satisfy it.
 */
void he_rt_set_params(he_rt_key_t *k, const he_rt_params_t *p, int32_t raw);

/**
 * The readings stopped being trustworthy (ADC fault). Returns HE_RT_UP if the
 * key was down, so the caller releases it. The key then waits for a reading at
 * or below `release`, so recovery never presses a key that is still held.
 */
he_rt_event_t he_rt_fault(he_rt_key_t *k);

static inline bool he_rt_is_down(const he_rt_key_t *k)
{
    return k->state == HE_RT_PRESSED;
}

#ifdef __cplusplus
}
#endif
