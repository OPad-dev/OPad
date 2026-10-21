#include "he_rt.h"

#include <stddef.h>

static int32_t cap_peak(const he_rt_params_t *p, int32_t raw)
{
    return raw < p->peak_cap ? raw : p->peak_cap;
}

// How far up the key may come before it leaves rapid trigger's region
static int32_t region_exit(const he_rt_params_t *p)
{
    return (p->rapid_trigger && p->continuous) ? p->top_reset : p->release;
}

// The key is up: take waiting thresholds, which put it back in ARMED
static void take_pending(he_rt_key_t *k, int32_t raw)
{
    if (k->has_pending) {
        k->has_pending = false;
        k->p = k->pending;
        k->state = HE_RT_ARMED;
        k->need_release = raw > k->p.release;
    }
}

void he_rt_init(he_rt_key_t *k, const he_rt_params_t *p)
{
    if (k == NULL || p == NULL) {
        return;
    }
    *k = (he_rt_key_t){
        .p = *p,
        .state = HE_RT_ARMED,
        .need_release = true,
    };
}

he_rt_event_t he_rt_step(he_rt_key_t *k, int32_t raw)
{
    const he_rt_params_t *p = &k->p;

    switch (k->state) {
    case HE_RT_ARMED:
        if (k->need_release) {
            // Seen up at last; it cannot also be past `actuation` here
            if (raw <= p->release) {
                k->need_release = false;
            }
            return HE_RT_NONE;
        }
        if (raw >= p->actuation) {
            k->state = HE_RT_PRESSED;
            k->extreme = cap_peak(p, raw);
            return HE_RT_DOWN;
        }
        return HE_RT_NONE;

    case HE_RT_PRESSED:
        if (raw <= region_exit(p)) {
            k->state = HE_RT_ARMED;
            take_pending(k, raw);
            return HE_RT_UP;
        }
        if (!p->rapid_trigger) {
            return HE_RT_NONE;
        }
        if (raw > k->extreme) {
            k->extreme = cap_peak(p, raw);
        }
        if (raw <= k->extreme - p->rt_release) {
            k->state = HE_RT_RT_RELEASED;
            k->extreme = raw;
            take_pending(k, raw);
            return HE_RT_UP;
        }
        return HE_RT_NONE;

    case HE_RT_RT_RELEASED:
        if (raw <= region_exit(p)) {
            // Already up: only rapid trigger's eligibility ends
            k->state = HE_RT_ARMED;
            return HE_RT_NONE;
        }
        if (raw < k->extreme) {
            k->extreme = raw;
        }
        if (raw >= k->extreme + p->rt_press) {
            k->state = HE_RT_PRESSED;
            k->extreme = cap_peak(p, raw);
            return HE_RT_DOWN;
        }
        return HE_RT_NONE;
    }
    return HE_RT_NONE;
}

void he_rt_set_params(he_rt_key_t *k, const he_rt_params_t *p, int32_t raw)
{
    if (k == NULL || p == NULL) {
        return;
    }
    k->pending = *p;
    k->has_pending = true;
    if (k->state != HE_RT_PRESSED) {
        // A settings/calibration reload is not a new sensor observation. Its
        // raw argument may predate a fault (even after health is resynced).
        // Only he_rt_step may satisfy an outstanding release requirement.
        bool need_release = k->need_release;
        take_pending(k, raw);
        k->need_release |= need_release;
    }
}

he_rt_event_t he_rt_fault(he_rt_key_t *k)
{
    if (k == NULL) {
        return HE_RT_NONE;
    }
    bool was_down = k->state == HE_RT_PRESSED;
    if (k->has_pending) {
        k->has_pending = false;
        k->p = k->pending;
    }
    k->state = HE_RT_ARMED;
    k->need_release = true;
    return was_down ? HE_RT_UP : HE_RT_NONE;
}
