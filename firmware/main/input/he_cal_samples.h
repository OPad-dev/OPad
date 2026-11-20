#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "input/he_params.h"

// One newest conversion per key per 250 us DMA frame. Keep their order:
// a min/max pair alone cannot distinguish rest -> press -> rest in one tick
// from the return of one stroke followed by the start of the next.
#define HE_CAL_SAMPLE_HZ 4000
#define HE_CAL_SAMPLE_CAPACITY 512 // 128 ms; overflow invalidates the attempt

typedef struct {
    int16_t raw[HE_CAL_SAMPLE_CAPACITY][HE_KEYS_MAX];
    int32_t min[HE_KEYS_MAX];
    int32_t max[HE_KEYS_MAX];
    uint16_t count;
    uint8_t keys;    // columns pushed: 2 (V1) or 3 (V3); unused columns hold 0
    bool lost;
} he_cal_samples_t;

// Task context, before handing the empty buffer to the ISR.
static inline void he_cal_samples_reset(he_cal_samples_t *s)
{
    s->count = 0;
    s->keys = 0;
    s->lost = false;
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        s->min[k] = INT32_MAX;
        s->max[k] = INT32_MIN;
    }
}

// Inlined in on_frame (IRAM), under its existing s_lock. No allocation,
// logging, division or additional lock. Invalid frames also fail closed.
// keys: how many of raw[] the module has (2 or 3); every one must be valid.
static inline __attribute__((always_inline)) void he_cal_samples_push(
    he_cal_samples_t *s, const int32_t *raw, uint8_t keys)
{
    if (s->count == HE_CAL_SAMPLE_CAPACITY || keys > HE_KEYS_MAX) {
        s->lost = true;
        return;
    }
    for (int k = 0; k < keys; k++) {
        if (raw[k] < 0) {
            s->lost = true;
            return;
        }
    }
    s->keys = keys;
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        int32_t v = k < keys ? raw[k] : 0;
        s->raw[s->count][k] = (int16_t)v;
        if (k < keys) {
            if (v < s->min[k]) s->min[k] = v;
            if (v > s->max[k]) s->max[k] = v;
        }
    }
    s->count++;
}
