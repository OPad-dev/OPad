#pragma once

/*
 * Board test of a Hall Effect V3 module's ADC (ADS7028 over SPI, three
 * DRV5055 sensors on AIN5/AIN4/AIN3). Measurements only, like the rest of the
 * board test: the host decides pass or fail (HeAdcTest in protocol/osupad.proto).
 *
 * In order: software reset and SYSTEM_STATUS (the chip answers, and BOR shows
 * it reset), clearing BOR, a write/read walk over the 8 bits of
 * AUTO_SEQ_CH_SEL (every MOSI and MISO bit both ways), offset calibration, the
 * fixed 0xA5A pattern at rising SCLK (signal margin of the cable), the three
 * sensors (manual mode, no averaging) and SYSTEM_STATUS again (a BOR set
 * during the test is a supply dip). About 100 ms.
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_ADC_TEST_SPEEDS   6
#define HE_ADC_TEST_KEYS     3
#define HE_ADC_TEST_SAMPLES  256

typedef struct {
    uint32_t sclk_hz;
    uint32_t frames;     // 0: the pins could not run this fast (not tried)
    uint32_t errors;     // frames whose 12 bits were not 0xA5A
} he_adc_speed_t;

typedef struct {
    uint8_t ain;         // ADC input the key's sensor is on
    uint16_t samples;
    uint16_t min;        // 12-bit codes, AVDD full scale
    uint16_t max;
    uint32_t mean_x16;   // mean × 16
    uint32_t noise_x100; // standard deviation × 100, in codes
} he_adc_channel_t;

typedef struct {
    bool ran;
    char message[48];             // why it did not run, or the step that failed
    bool cable_reversed;          // the pins used
    uint32_t sclk_hz;             // for everything but the speed sweep
    int status_after_reset;       // SYSTEM_STATUS after RST, 0x81 expected; -1 = not read
    int status_after_clear;       // after clearing BOR, 0x80 expected
    uint32_t walk_writes;
    uint32_t walk_mismatches;
    uint8_t walk_wrong_bits;      // bits that ever read back wrong
    bool cal_done;                // CAL cleared itself
    int status_end;               // SYSTEM_STATUS when done
    uint8_t speed_count;
    he_adc_speed_t speeds[HE_ADC_TEST_SPEEDS];
    uint8_t channel_count;
    he_adc_channel_t channels[HE_ADC_TEST_KEYS];  // key 1, 2, 3
} he_adc_test_t;

/**
 * @brief Run it on the V3 carrier's module pins, for the cable as given.
 * The key input must be off (it is with an HE V3 module): CS is GPIO10, a
 * carrier key pin. Protocol task only.
 */
void he_adc_test_run(bool cable_reversed, he_adc_test_t *out);

#ifdef __cplusplus
}
#endif
