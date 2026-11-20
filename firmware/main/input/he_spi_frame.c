#include "he_spi_frame.h"

void he_spi_frame_decode(const uint16_t *words, uint32_t n, he_frame_t *out,
                         int32_t raw[HE_SPI_FRAME_KEYS])
{
    *out = (he_frame_t){.newest = {-1, -1, -1}, .median = {-1, -1, -1}};
    for (int k = 0; k < HE_SPI_FRAME_KEYS; k++) {
        raw[k] = -1;
    }
    for (uint32_t i = 0; i < n; i++) {
        int key = he_spi_key_of_ain(words[i] & 0xFu);
        int32_t v = words[i] >> 4;
        if (key < 0) {
            out->foreign++;
            continue;
        }
        if (v <= 0 || v >= HE_FRAME_RAW_MAX) {
            out->railed++;
            continue;
        }
        raw[key] = v;
    }
    for (int k = 0; k < HE_SPI_FRAME_KEYS; k++) {
        if (raw[k] >= 0) {
            out->newest[k] = raw[k];
            out->median[k] = raw[k];
            out->count[k] = 1;
        }
    }
}
