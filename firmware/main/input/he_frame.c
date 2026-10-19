#include "he_frame.h"

#include <stddef.h>

void he_frame_parse(const uint32_t *words, uint32_t n, he_frame_t *out)
{
    *out = (he_frame_t){.newest = {-1, -1}};
    if (words == NULL) {
        return;
    }
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
            out->count[key]++;
        }
    }
}
