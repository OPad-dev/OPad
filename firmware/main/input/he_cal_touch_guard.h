#pragma once

#include <stdbool.h>
#include <stdatomic.h>

// Task-only interlock. A contact owns BUSY through its final release cleanup;
// calibration reserves an idle touch before publishing backend suppression.
enum { HE_CAL_TOUCH_BUSY = 1u, HE_CAL_TOUCH_START = 2u };
typedef struct {
    atomic_uint state;
} he_cal_touch_guard_t;

static inline bool he_cal_touch_begin(he_cal_touch_guard_t *g)
{
    return (atomic_fetch_or(&g->state, HE_CAL_TOUCH_BUSY) & HE_CAL_TOUCH_START) != 0;
}

static inline void he_cal_touch_end(he_cal_touch_guard_t *g)
{
    atomic_fetch_and(&g->state, ~(unsigned)HE_CAL_TOUCH_BUSY);
}

static inline bool he_cal_touch_reserve_start(he_cal_touch_guard_t *g)
{
    unsigned idle = 0;
    return atomic_compare_exchange_strong(&g->state, &idle, HE_CAL_TOUCH_START);
}

// Call only after backend suppression is visible. A concurrent contact keeps
// BUSY and latches the returned start bit until its own physical release.
static inline void he_cal_touch_finish_start(he_cal_touch_guard_t *g)
{
    atomic_fetch_and(&g->state, ~(unsigned)HE_CAL_TOUCH_START);
}
