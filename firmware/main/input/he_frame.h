#pragma once

/*
 * One ADC DMA frame of the Hall Effect keys -> the newest valid reading of
 * each key. Pure C, run in the ADC interrupt (from IRAM, linker.lf) and on the
 * host by firmware/test/host/test_he_frame.c.
 *
 * Each conversion is one 32-bit word, the ESP32-S3's TYPE2 format
 * (adc_digi_output_data_t in ESP-IDF's hal/adc_types.h): bits 0-11 the
 * reading, 13-16 the channel, 17 the unit (0 = ADC1). A word counts for a key
 * only if it is ADC1 on that key's channel and the reading is off both rails:
 * a DRV5055 never drives its output to 0 V or to the ADC's full scale, so a
 * reading there is a missing or shorted sensor, not a key position.
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define HE_FRAME_CH_KEY1 9   // GPIO10
#define HE_FRAME_CH_KEY2 6   // GPIO7
#define HE_FRAME_RAW_MAX 4095

typedef struct {
    int32_t newest[2];   // per key, -1 = no valid reading in the frame
    uint16_t count[2];   // valid readings per key
    uint16_t foreign;    // words from another unit or channel
    uint16_t railed;     // key words at 0 or HE_FRAME_RAW_MAX
} he_frame_t;

void he_frame_parse(const uint32_t *words, uint32_t n, he_frame_t *out);

// One TYPE2 word, for tests and the trace tools
static inline uint32_t he_frame_word(uint32_t unit, uint32_t channel, uint32_t raw)
{
    return (raw & 0xFFFu) | ((channel & 0xFu) << 13) | ((unit & 1u) << 17);
}

#ifdef __cplusplus
}
#endif
