#pragma once

/*
 * TI ADS7028 (and its sibling ADS7038): 8-channel 12-bit SAR ADC with SPI.
 * Register addresses, bits and frame layout from the ADS7028 datasheet
 * SBAS978A (May 2020); notes and section references in
 * docs/hardware/components/ads7028.md. Pure C, host-tested
 * (firmware/test/host/test_ads7028.c).
 *
 * Every command is one 24-bit frame on SDI: opcode, address, data (8.3.14.2).
 * A register read takes two frames: the second returns the register in its
 * first SDO byte. Conversion data comes MSB first: 12 bits, or 16 with
 * averaging (8.3.9). SPI-00 (CPOL 0, CPHA 0) after power-up and reset.
 *
 * With GENERAL_CFG.CRC_EN set, every frame is exactly 32 bits: the 24-bit
 * command and its CRC on SDI; on SDO the data (8 bits for a register, 16 for
 * a conversion) and its CRC. The ADS7028 datasheet does not show this; the
 * ADS7038 one does (SBAS979C, 7.3.8.1, 7.3.8.2, Figure 7-12). The CRC is
 * CRC-8 x^8 + x^2 + x + 1, MSB first, seed 0x00, measured on the owner's
 * pad (docs/hardware/components/ads7028.md). A no-op frame is then all zeros.
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opcodes (Table 7)
#define ADS7028_OP_NOP          0x00
#define ADS7028_OP_READ         0x10
#define ADS7028_OP_WRITE        0x08
#define ADS7028_OP_SET_BITS     0x18
#define ADS7028_OP_CLEAR_BITS   0x20

// Registers (Table 9)
#define ADS7028_REG_SYSTEM_STATUS   0x00   // reset 0x81
#define ADS7028_REG_GENERAL_CFG     0x01
#define ADS7028_REG_DATA_CFG        0x02
#define ADS7028_REG_OSR_CFG         0x03
#define ADS7028_REG_OPMODE_CFG      0x04
#define ADS7028_REG_PIN_CFG         0x05
#define ADS7028_REG_SEQUENCE_CFG    0x10
#define ADS7028_REG_CHANNEL_SEL     0x11
#define ADS7028_REG_AUTO_SEQ_CH_SEL 0x12   // 8 R/W bits: the register walk test uses it

// SYSTEM_STATUS (8.5.1)
#define ADS7028_STATUS_RSVD         0x80   // always reads 1
#define ADS7028_STATUS_SEQ          0x40
#define ADS7028_STATUS_OSR_DONE     0x08
#define ADS7028_STATUS_CRC_ERR_FUSE 0x04   // power-up configuration not loaded
#define ADS7028_STATUS_CRC_ERR_IN   0x02   // a frame failed CRC: writes blocked
#define ADS7028_STATUS_BOR          0x01   // power cycled, brown-out or reset
#define ADS7028_STATUS_AFTER_RESET  0x81

// GENERAL_CFG (8.5.2)
#define ADS7028_GENERAL_CRC_EN      0x40
#define ADS7028_GENERAL_CAL         0x02   // offset calibration, clears itself when done
#define ADS7028_GENERAL_RST         0x01   // software reset, at most 5 ms (tRST)

// DATA_CFG (8.5.3)
#define ADS7028_DATA_FIX_PAT        0x80   // conversion data reads 0xA5A
#define ADS7028_FIXED_PATTERN       0xA5A

#define ADS7028_RESET_MS            5      // tRST and tPU (7.7)
#define ADS7028_MAX_SCLK_HZ         60000000

#define ADS7028_CRC_POLY            0x07   // x^8 + x^2 + x + 1

/** @brief CRC-8 of n bytes, MSB first, seed 0x00, as the ADC computes it. */
static inline uint8_t ads7028_crc8(const uint8_t *d, unsigned n)
{
    uint8_t c = 0;
    for (unsigned i = 0; i < n; i++) {
        c ^= d[i];
        for (int b = 0; b < 8; b++) {
            c = (c & 0x80) ? (uint8_t)((c << 1) ^ ADS7028_CRC_POLY) : (uint8_t)(c << 1);
        }
    }
    return c;
}

/** @brief The 32-bit frame for one command with CRC on: the 24-bit frame and its CRC. */
static inline void ads7028_frame_crc(uint8_t op, uint8_t addr, uint8_t data, uint8_t out[4])
{
    out[0] = op;
    out[1] = addr;
    out[2] = data;
    out[3] = ads7028_crc8(out, 3);
}

/**
 * @brief With CRC on, whether a frame's SDO bytes carry n data bytes (1 for
 * a register, 2 for a conversion) followed by their correct CRC.
 */
static inline bool ads7028_rx_crc_ok(const uint8_t rx[4], unsigned n)
{
    return ads7028_crc8(rx, n) == rx[n];
}

/** @brief The 24-bit frame for one command. */
static inline void ads7028_frame(uint8_t op, uint8_t addr, uint8_t data, uint8_t out[3])
{
    out[0] = op;
    out[1] = addr;
    out[2] = data;
}

/** @brief The 12-bit conversion result (averaging off) from a frame's SDO bytes. */
static inline uint16_t ads7028_data12(const uint8_t rx[3])
{
    return (uint16_t)(((uint16_t)rx[0] << 4) | (rx[1] >> 4));
}

/** @brief The 16-bit averaged result (averaging on) from a frame's SDO bytes. */
static inline uint16_t ads7028_data16(const uint8_t rx[3])
{
    return (uint16_t)(((uint16_t)rx[0] << 8) | rx[1]);
}

#ifdef __cplusplus
}
#endif
