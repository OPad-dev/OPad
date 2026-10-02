#include "press_log.h"
#include <stdatomic.h>

_Static_assert((PRESS_LOG_CAPACITY & (PRESS_LOG_CAPACITY - 1)) == 0, "capacity must be a power of two");

static press_log_entry_t s_entries[PRESS_LOG_CAPACITY];
// Free-running counters: head is written by the producer only, tail by the consumer
static atomic_uint s_head;
static atomic_uint s_tail;
static atomic_uint s_dropped;

void IRAM_ATTR press_log_push(uint8_t key, int64_t t_us)
{
    unsigned head = atomic_load_explicit(&s_head, memory_order_relaxed);
    unsigned tail = atomic_load_explicit(&s_tail, memory_order_acquire);
    if (head - tail >= PRESS_LOG_CAPACITY) {
        atomic_fetch_add_explicit(&s_dropped, 1, memory_order_relaxed);
        return;
    }
    s_entries[head & (PRESS_LOG_CAPACITY - 1)] = (press_log_entry_t){ .t_us = t_us, .key = key };
    // Publish the entry before the consumer can see the new head
    atomic_store_explicit(&s_head, head + 1, memory_order_release);
}

size_t press_log_drain(press_log_entry_t *out, size_t max, uint32_t *dropped)
{
    unsigned tail = atomic_load_explicit(&s_tail, memory_order_relaxed);
    unsigned head = atomic_load_explicit(&s_head, memory_order_acquire);
    size_t n = 0;
    while (tail != head && n < max) {
        out[n++] = s_entries[tail & (PRESS_LOG_CAPACITY - 1)];
        tail++;
    }
    atomic_store_explicit(&s_tail, tail, memory_order_release);
    if (dropped) {
        *dropped += atomic_exchange_explicit(&s_dropped, 0, memory_order_relaxed);
    }
    return n;
}

size_t press_log_pending(void)
{
    return atomic_load_explicit(&s_head, memory_order_acquire) -
           atomic_load_explicit(&s_tail, memory_order_relaxed);
}

void press_log_clear(void)
{
    atomic_store_explicit(&s_tail, atomic_load_explicit(&s_head, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&s_dropped, 0, memory_order_relaxed);
}
