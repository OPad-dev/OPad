#pragma once

#include <stdbool.h>
#include <stdatomic.h>

// One serialized requester; the reporting task acknowledges after publishing.
// Tickets keep a late acknowledgement from satisfying a later start attempt.
typedef struct {
    atomic_uint requested;
    atomic_uint completed;
} he_cal_barrier_t;

static inline unsigned he_cal_barrier_request(he_cal_barrier_t *b)
{
    return atomic_fetch_add_explicit(&b->requested, 1, memory_order_release) + 1;
}

static inline bool he_cal_barrier_done(const he_cal_barrier_t *b, unsigned ticket)
{
    return atomic_load_explicit(&b->completed, memory_order_acquire) == ticket;
}

// Calibration-only path: drain any previously captured key state and publish
// the suppressed state before acknowledging. No lock surrounds HID submission.
static inline void he_cal_barrier_publish(he_cal_barrier_t *b,
                                         void (*publish)(void *), void *ctx)
{
    unsigned ticket = atomic_load_explicit(&b->requested, memory_order_acquire);
    publish(ctx);
    atomic_store_explicit(&b->completed, ticket, memory_order_release);
}
