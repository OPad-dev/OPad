#include "he_params.h"

#include <stddef.h>

#define ADC_RAW_MAX 4095  // 12-bit

static uint16_t clamp_pct(uint16_t v, uint32_t *result)
{
    if (v > HE_PCT_FULL) {
        *result |= HE_PARAMS_SETTING_CLAMPED;
        return HE_PCT_FULL;
    }
    return v;
}

// A share of the span in counts, rounded to nearest
static int32_t pct_to_counts(int32_t span, uint16_t pct)
{
    return (span * (int32_t)pct + HE_PCT_FULL / 2) / HE_PCT_FULL;
}

// A movement in counts, at least `floor`; `flag` set if it had to be raised
static int32_t movement(int32_t span, uint16_t pct, int32_t floor, uint32_t flag, uint32_t *result)
{
    int32_t c = pct_to_counts(span, pct);
    if (c < floor) {
        *result |= flag;
        return floor;
    }
    return c;
}

uint32_t he_params_build(const he_key_settings_t *s, const he_calibration_t *cal,
                         const he_limits_t *lim, he_rt_params_t *out)
{
    if (s == NULL || cal == NULL || lim == NULL || out == NULL) {
        return HE_PARAMS_BAD_LIMITS;
    }
    if (lim->min_movement < 1 || lim->min_top_reset < 1) {
        return HE_PARAMS_BAD_LIMITS;
    }
    if (cal->rest < 0 || cal->full > ADC_RAW_MAX || cal->full <= cal->rest) {
        return HE_PARAMS_BAD_CALIBRATION;
    }

    uint32_t result = HE_PARAMS_OK;
    int32_t span = cal->full - cal->rest;

    he_rt_params_t p = {
        .rapid_trigger = s->rapid_trigger,
        .continuous = s->rapid_trigger && s->continuous,
    };
    if (p.rapid_trigger && !cal->valid) {
        result |= HE_PARAMS_RT_NEEDS_CALIBRATION;
        p.rapid_trigger = false;
        p.continuous = false;
    }

    int32_t top = pct_to_counts(span, clamp_pct(s->top_reset, &result));
    if (top < lim->min_top_reset) {
        result |= HE_PARAMS_TOP_RESET_RAISED;
        top = lim->min_top_reset;
    }
    p.top_reset = cal->rest + top;

    int32_t hyst = movement(span, clamp_pct(s->release_hyst, &result), lim->min_movement,
                            HE_PARAMS_HYST_RAISED, &result);
    p.actuation = cal->rest + pct_to_counts(span, clamp_pct(s->actuation, &result));
    if (p.actuation - hyst < p.top_reset) {
        // The release point must clear the noise at rest, or a key could stay down
        result |= HE_PARAMS_ACTUATION_RAISED;
        p.actuation = p.top_reset + hyst;
    }
    p.release = p.actuation - hyst;

    p.rt_press = movement(span, clamp_pct(s->rt_press, &result), lim->min_movement,
                          HE_PARAMS_RT_PRESS_RAISED, &result);
    p.rt_release = movement(span, clamp_pct(s->rt_release, &result), lim->min_movement,
                            HE_PARAMS_RT_RELEASE_RAISED, &result);
    if (!p.rapid_trigger) {
        // Unused in plain mode: report only what applies
        result &= ~(HE_PARAMS_RT_PRESS_RAISED | HE_PARAMS_RT_RELEASE_RAISED);
    }

    uint16_t bdz = clamp_pct(s->bottom_dz, &result);
    p.peak_cap = INT32_MAX;
    if (bdz > 0) {
        p.peak_cap = cal->full - pct_to_counts(span, bdz);
        if (p.peak_cap < p.actuation) {
            result |= HE_PARAMS_BOTTOM_DZ_REDUCED;
            p.peak_cap = p.actuation;
        }
    }

    *out = p;
    return result;
}
