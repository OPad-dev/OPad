#pragma once

#include <stdbool.h>
#include <stdint.h>

// One newest conversion per key per 250 us DMA frame. Keep their order:
// a min/max pair alone cannot distinguish rest -> press -> rest in one tick
// from the return of one stroke followed by the start of the next.
#define HE_CAL_SAMPLE_HZ 4000
#define HE_CAL_SAMPLE_CAPACITY 512 // 128 ms; overflow invalidates the attempt

typedef struct {
    int16_t raw[HE_CAL_SAMPLE_CAPACITY][2];
    int32_t min[2];
    int32_t max[2];
    uint16_t count;
    bool lost;
} he_cal_samples_t;

// Task context, before handing the empty buffer to the ISR.
static inline void he_cal_samples_reset(he_cal_samples_t *s)
{
    s->count = 0;
    s->lost = false;
    for (int k = 0; k < 2; k++) {
        s->min[k] = INT32_MAX;
        s->max[k] = INT32_MIN;
    }
}

// Inlined in on_frame (IRAM), under its existing s_lock. No allocation,
// logging, division or additional lock. Invalid frames also fail closed.
static inline __attribute__((always_inline)) void he_cal_samples_push(
    he_cal_samples_t *s, int32_t raw0, int32_t raw1)
{
    if (s->count == HE_CAL_SAMPLE_CAPACITY || raw0 < 0 || raw1 < 0) {
        s->lost = true;
        return;
    }
    const int32_t raw[2] = {raw0, raw1};
    for (int k = 0; k < 2; k++) {
        s->raw[s->count][k] = (int16_t)raw[k];
        if (raw[k] < s->min[k]) s->min[k] = raw[k];
        if (raw[k] > s->max[k]) s->max[k] = raw[k];
    }
    s->count++;
}
