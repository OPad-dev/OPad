#pragma once

/*
 * Hall Effect V3 readings: the module's ADS7028 over SPI (input/ads7028.h),
 * one frame every HE_SPI_FRAME_US (50 us, 20 kHz) for he_backend.c, five times
 * V1's ADC DMA frame rate; he_backend.c hands calibration one frame in five,
 * so it still counts samples at 4 kHz.
 *
 * A GPTimer alarm wakes the sampler task (core 0, the key path's priority)
 * every frame. The ADC runs its auto-sequence over AIN3/AIN4/AIN5 with each
 * result tagged with its channel ID (DATA_CFG.APPEND_STATUS), so a frame
 * that lost or gained a bit lands on no key instead of the wrong one. Each
 * frame reads four conversions: the first was started at the end of the
 * previous frame, a frame period earlier, so it is dropped; the other three were
 * started within this frame and give one fresh reading per key.
 *
 * The link runs with the ADC's CRC on (GENERAL_CFG.CRC_EN, input/ads7028.h):
 * a conversion whose CRC does not match is dropped, so a bit flipped on the
 * cable never becomes a reading. Reads run at 40 MHz and, after the first
 * since start, straight on SPI3's registers (ads7028_read16_again): a frame
 * reads in about 14 us (docs/hardware/components/ads7028.md).
 *
 * he_backend.c owns start and stop (from he_task only).
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "input/he_frame.h"

#ifdef __cplusplus
extern "C" {
#endif

// 20 kHz: the fastest period that divides V1's 250 us (calibration's 4 kHz)
// and leaves core 0 room for USB. Measured in docs/hardware/components/ads7028.md.
#define HE_SPI_FRAME_US 50
#define HE_SPI_KEYS     3

/** Called from the sampler task with each frame (keys 1 and 2 in f). */
typedef void (*he_spi_frame_cb_t)(const he_frame_t *f, const int32_t raw[HE_SPI_KEYS], int64_t now_us);

typedef struct {
    uint32_t frames;
    uint32_t spi_errors;      // transfers the driver failed
    uint32_t crc_errors;      // conversions dropped for a CRC mismatch
    uint32_t foreign;         // conversions tagged with a channel no key uses
    uint32_t max_frame_us;    // longest frame read, wake to done
} he_spi_stats_t;

/**
 * @brief Claim SPI3 on the V3 carrier's pins (cable either way round),
 * configure the ADC and start the frames. The sampler task and timer are
 * created on the first call, on the calling core (core 0).
 */
esp_err_t he_spi_source_start(bool cable_reversed, he_spi_frame_cb_t cb);

/** @brief Stop the frames, wait for the one in flight, and free SPI3 and the pins. */
void he_spi_source_stop(void);

void he_spi_source_get_stats(he_spi_stats_t *out);

#ifdef __cplusplus
}
#endif
