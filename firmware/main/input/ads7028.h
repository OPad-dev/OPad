#pragma once

/*
 * SPI access to the ADS7028 on a Hall Effect V3 module (input/ads7028_regs.h).
 * For now only the board test uses it: it opens the bus, runs its checks and
 * closes it again, so the pins are free between tests. SPI3 (the LCD has
 * SPI2), no DMA, polling transfers: nothing here is on the keypress path.
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    int miso;
    int mosi;
    int sclk;
    int cs;
} ads7028_pins_t;

/** @brief The module's SPI pins on a V3 carrier, for the cable either way round. */
void ads7028_carrier_pins(bool cable_reversed, ads7028_pins_t *out);

/** @brief Claim SPI3 on these pins at this SCLK. */
esp_err_t ads7028_open(const ads7028_pins_t *pins, uint32_t sclk_hz);

/** @brief Change SCLK. ESP_ERR_INVALID_ARG when the pins cannot run that fast. */
esp_err_t ads7028_set_clock(uint32_t sclk_hz);

/** @brief Let go of SPI3 and reset the pins. Safe when not open. */
void ads7028_close(void);

/** @brief One 24-bit frame, full duplex. */
esp_err_t ads7028_transfer(const uint8_t tx[3], uint8_t rx[3]);

esp_err_t ads7028_write_reg(uint8_t addr, uint8_t value);

/** @brief Two frames: the read command, then a no-op that returns the register. */
esp_err_t ads7028_read_reg(uint8_t addr, uint8_t *value);

/**
 * @brief Keep SPI3 for this task until ads7028_release_bus: transfers then
 * skip the bus lock (the streaming path, input/he_spi_source.c).
 */
esp_err_t ads7028_acquire_bus(void);
void ads7028_release_bus(void);

/** @brief One 16-bit no-op frame: a conversion word (12 bits + 4-bit tag). */
esp_err_t ads7028_read16(uint16_t *out);

#ifdef __cplusplus
}
#endif
