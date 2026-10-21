#pragma once

/*
 * Hall Effect key settings -> raw thresholds for the decision engine
 * (he_rt.h). Runs off the key path, whenever settings or calibration change.
 *
 * Settings are in normalized magnetic units: 0.1 % steps of the key's
 * calibrated span (full - rest), so 1000 is a normal full press. That is a
 * share of the magnetic signal, not of physical travel: the field is not
 * linear with stem position, and V1 has no displacement calibration to map
 * one onto the other.
 *
 * Every movement threshold is held at or above a noise floor in raw counts
 * (he_limits_t). Where a setting maps below it, the floor is applied and the
 * result says so, never silently. The floor's value is not decided yet: it
 * comes from raw-conversion noise traces (docs/specs/v2-hall-effect-design.md,
 * decision record, OPEN A and D), so callers pass it in.
 */

#include <stdbool.h>
#include <stdint.h>
#include "he_rt.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_PCT_FULL 1000  // settings unit: 0.1 % of the calibrated span

typedef struct {
    uint16_t actuation;     // depth of the first press
    uint16_t release_hyst;  // plain release this far above `actuation`
    uint16_t rt_press;      // rapid trigger: movement down that presses again
    uint16_t rt_release;    // rapid trigger: movement up that releases
    uint16_t top_reset;     // continuous rapid trigger ends above this depth
    uint16_t bottom_dz;     // the peak stops following this close to a full press; 0 = off
    bool rapid_trigger;
    bool continuous;        // only with rapid_trigger
    // Owner's choice per key (app, Advanced): drop the noise floors to 1 count,
    // so any setting is used as is. The sensor's noise can then press the key.
    bool no_floor;
} he_key_settings_t;

typedef struct {
    int32_t rest;  // released reading
    int32_t full;  // normal full press reading (not the hardest)
    bool valid;    // from an explicit calibration; false = factory defaults
} he_calibration_t;

typedef struct {
    int32_t min_movement;   // smallest rt_press / rt_release / hysteresis, counts
    int32_t min_top_reset;  // smallest top_reset / release above rest, counts
} he_limits_t;

// he_params_build result bits. HE_PARAMS_OK (0) is a clean build.
#define HE_PARAMS_OK                   0u
#define HE_PARAMS_BAD_CALIBRATION      (1u << 0)  // nothing built, *out untouched
#define HE_PARAMS_BAD_LIMITS           (1u << 1)  // nothing built, *out untouched
#define HE_PARAMS_RT_NEEDS_CALIBRATION (1u << 2)  // rapid trigger off until calibrated
#define HE_PARAMS_SETTING_CLAMPED      (1u << 3)  // a setting above HE_PCT_FULL
#define HE_PARAMS_RT_PRESS_RAISED      (1u << 4)  // to min_movement
#define HE_PARAMS_RT_RELEASE_RAISED    (1u << 5)  // to min_movement
#define HE_PARAMS_HYST_RAISED          (1u << 6)  // to min_movement
#define HE_PARAMS_TOP_RESET_RAISED     (1u << 7)  // to min_top_reset
#define HE_PARAMS_ACTUATION_RAISED     (1u << 8)  // release would sit below its floor
#define HE_PARAMS_BOTTOM_DZ_REDUCED    (1u << 9)  // its cap would sit below actuation

#define HE_PARAMS_FAILED(r) (((r) & (HE_PARAMS_BAD_CALIBRATION | HE_PARAMS_BAD_LIMITS)) != 0)

/**
 * Work out one key's thresholds. Rapid trigger stays off unless the
 * calibration is valid: an uncalibrated span is a guess, and a sensitivity
 * taken from a guess can sit inside the noise. Returns HE_PARAMS_* bits;
 * unless HE_PARAMS_FAILED, *out holds thresholds that keep he_rt.h's
 * ordering guarantees.
 */
uint32_t he_params_build(const he_key_settings_t *s, const he_calibration_t *cal,
                         const he_limits_t *lim, he_rt_params_t *out);

#ifdef __cplusplus
}
#endif
