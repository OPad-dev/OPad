#include "he_frame.h"

#include <stddef.h>

// Insertion sort: at most HE_FRAME_MEDIAN_MAX values, in the frame interrupt
static int32_t median_of(uint16_t *v, uint32_t n)
{
    for (uint32_t i = 1; i < n; i++) {
        uint16_t x = v[i];
        uint32_t j = i;
        for (; j > 0 && v[j - 1] > x; j--) {
            v[j] = v[j - 1];
        }
        v[j] = x;
    }
    return ((int32_t)v[(n - 1) / 2] + v[n / 2]) / 2;
}

void he_frame_parse(const uint32_t *words, uint32_t n, he_frame_t *out)
{
    *out = (he_frame_t){.newest = {-1, -1}, .median = {-1, -1}};
    if (words == NULL) {
        return;
    }
    uint16_t keep[2][HE_FRAME_MEDIAN_MAX];
    for (uint32_t i = 0; i < n; i++) {
        uint32_t w = words[i];
        uint32_t raw = w & 0xFFFu;
        uint32_t channel = (w >> 13) & 0xFu;
        int key = -1;
        if (((w >> 17) & 1u) == 0) {
            key = channel == HE_FRAME_CH_KEY1 ? 0 : channel == HE_FRAME_CH_KEY2 ? 1 : -1;
        }
        if (key < 0) {
            out->foreign++;
        } else if (raw == 0 || raw == HE_FRAME_RAW_MAX) {
            out->railed++;
        } else {
            out->newest[key] = (int32_t)raw;
            keep[key][out->count[key] % HE_FRAME_MEDIAN_MAX] = (uint16_t)raw;
            out->count[key]++;
        }
    }
    for (int k = 0; k < 2; k++) {
        if (out->count[k] > 0) {
            uint32_t m = out->count[k] < HE_FRAME_MEDIAN_MAX ? out->count[k] : HE_FRAME_MEDIAN_MAX;
            out->median[k] = median_of(keep[k], m);
        }
    }
}
