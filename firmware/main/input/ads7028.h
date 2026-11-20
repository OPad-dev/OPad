#pragma once

/*
 * SPI access to the ADS7028 on a Hall Effect V3 module (input/ads7028_regs.h).
 * The board test opens the bus, runs its checks and closes it again, so the
 * pins are free between tests; key input (input/he_spi_source.c) keeps it
 * open and streams conversions. SPI3 (the LCD has SPI2), no DMA, polling
 * transfers.
 *
 * The driver follows GENERAL_CFG.CRC_EN: a write to GENERAL_CFG that sets it
 * switches every later frame to 32 bits with CRC, and one that clears it or
 * resets the ADC switches back. With CRC on, a register or conversion whose
 * CRC does not match reads as ESP_ERR_INVALID_CRC. ads7028_open starts with
 * CRC off: the ADC's power-up and reset state.
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

/** @brief One 24-bit frame, full duplex (CRC off). */
esp_err_t ads7028_transfer(const uint8_t tx[3], uint8_t rx[3]);

/** @brief Whether frames carry a CRC (GENERAL_CFG.CRC_EN as last written). */
bool ads7028_crc_on(void);

/** @brief A write to GENERAL_CFG sets or clears CRC for the frames after it. */
esp_err_t ads7028_write_reg(uint8_t addr, uint8_t value);

/**
 * @brief Software reset (GENERAL_CFG.RST), whatever CRC state the ADC is in:
 * the reset as a 32-bit frame with its CRC (taken only with CRC on), then as
 * a 24-bit one (taken only with CRC off). CRC is off afterwards. Wait
 * ADS7028_RESET_MS before the next frame.
 */
esp_err_t ads7028_reset(void);

/** @brief Two frames: the read command, then a no-op that returns the register. */
esp_err_t ads7028_read_reg(uint8_t addr, uint8_t *value);

/**
 * @brief Keep SPI3 for this task until ads7028_release_bus: transfers then
 * skip the bus lock (the streaming path, input/he_spi_source.c).
 */
esp_err_t ads7028_acquire_bus(void);
void ads7028_release_bus(void);

/**
 * @brief One no-op frame: a conversion word (12 bits + 4-bit tag). 16 bits
 * with CRC off; with CRC on 32, and ESP_ERR_INVALID_CRC when the word's CRC
 * does not match (out is then not to be used).
 */
esp_err_t ads7028_read16(uint16_t *out);

/**
 * @brief The last ads7028_read16 again, straight on the SPI registers: no
 * driver, a few register writes and a wait for the bits. Only with the bus
 * acquired (ads7028_acquire_bus) and after an ads7028_read16 since, which
 * leaves SPI3 set up for it. Same result and errors as ads7028_read16.
 */
esp_err_t ads7028_read16_again(uint16_t *out);

#ifdef __cplusplus
}
#endif
