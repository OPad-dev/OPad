#pragma once

/*
 * One Hall Effect V3 frame: ADS7028 conversions -> a reading per key. Pure C,
 * host-tested (firmware/test/host/test_he_spi_frame.c).
 *
 * Each conversion is the first 16 bits of an SPI frame with
 * DATA_CFG.APPEND_STATUS = channel ID: bits 15-4 the 12-bit reading, bits 3-0
 * the AIN it came from (ADS7028 datasheet SBAS978A, 8.3.9). Keys 1, 2, 3 are
 * AIN5, AIN4, AIN3 (HE V3 netlist). A conversion counts for a key only if it
 * carries that key's AIN and is off both rails: a DRV5055 never drives 0 V or
 * full scale, so a rail is a missing or shorted sensor (as he_frame.h on V1).
 * The newest valid conversion of each key wins.
 */

#include <stdint.h>
#include "input/he_frame.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_SPI_FRAME_KEYS 3

/** The AIN each key's sensor is on; -1 for an AIN no key uses. */
static inline int he_spi_key_of_ain(uint32_t ain)
{
    switch (ain) {
    case 5: return 0;
    case 4: return 1;
    case 3: return 2;
    default: return -1;
    }
}

/** A conversion word as the ADC sends it, for tests. */
static inline uint16_t he_spi_word(uint32_t ain, uint32_t raw)
{
    return (uint16_t)(((raw & 0xFFFu) << 4) | (ain & 0xFu));
}

/**
 * @brief Decode n conversion words. out: every key for he_backend's engine
 * (newest = median = the reading, count 0 or 1). raw: the same readings, -1
 * where a key got no valid conversion.
 */
void he_spi_frame_decode(const uint16_t *words, uint32_t n, he_frame_t *out,
                         int32_t raw[HE_SPI_FRAME_KEYS]);

#ifdef __cplusplus
}
#endif
