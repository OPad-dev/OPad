/*
 * ADS7028 frame layout (input/ads7028_regs.h), against the datasheet
 * SBAS978A figures 25, 32 and 33, and the CRC frames (ADS7038 SBAS979C
 * 7.3.8, Figure 7-12) against bytes read from the owner's pad.
 */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include "../../main/input/ads7028_regs.h"

static void test_command_frames(void)
{
    uint8_t f[3];
    ads7028_frame(ADS7028_OP_WRITE, ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_RST, f);
    assert(f[0] == 0x08 && f[1] == 0x01 && f[2] == 0x01);
    ads7028_frame(ADS7028_OP_READ, ADS7028_REG_SYSTEM_STATUS, 0, f);
    assert(f[0] == 0x10 && f[1] == 0x00 && f[2] == 0x00);
    printf("✓ test_command_frames passed\n");
}

// 12 bits MSB first, then 4 optional bits (Figure 25)
static void test_data12(void)
{
    const uint8_t pattern[3] = {0xA5, 0xA0, 0x00};
    assert(ads7028_data12(pattern) == ADS7028_FIXED_PATTERN);
    const uint8_t with_channel_id[3] = {0xA5, 0xA5, 0xFF};  // AIN5 appended
    assert(ads7028_data12(with_channel_id) == 0xA5A);
    const uint8_t full[3] = {0xFF, 0xF0, 0x00};
    assert(ads7028_data12(full) == 4095);
    const uint8_t mid[3] = {0x80, 0x00, 0x00};
    assert(ads7028_data12(mid) == 2048);
    printf("✓ test_data12 passed\n");
}

static void test_data16(void)
{
    const uint8_t v[3] = {0x80, 0x01, 0x00};
    assert(ads7028_data16(v) == 0x8001);
    printf("✓ test_data16 passed\n");
}

// Bytes from the ADS7028 on the owner's pad with CRC_EN set (2026-11-20)
static void test_crc_matches_the_chip(void)
{
    const uint8_t status[4] = {0x81, 0x8E, 0x00, 0x00};  // SYSTEM_STATUS after reset
    assert(ads7028_rx_crc_ok(status, 1));
    const uint8_t conv[][4] = {
        {0x7D, 0x20, 0xAB, 0x00},
        {0x7C, 0xE0, 0xF0, 0x00},
        {0x81, 0x10, 0xD3, 0x00},
    };
    for (unsigned i = 0; i < sizeof(conv) / sizeof(conv[0]); i++) {
        assert(ads7028_rx_crc_ok(conv[i], 2));
    }
    // One flipped bit anywhere fails it
    for (int bit = 0; bit < 24; bit++) {
        uint8_t bad[4] = {0x7D, 0x20, 0xAB, 0x00};
        bad[bit / 8] ^= (uint8_t)(0x80u >> (bit % 8));
        assert(!ads7028_rx_crc_ok(bad, 2));
    }
    // A board without the ADC (MISO stuck) never passes as data
    const uint8_t low[4] = {0x00, 0x00, 0x00, 0x00};
    const uint8_t high[4] = {0xFF, 0xFF, 0xFF, 0xFF};
    assert(ads7028_rx_crc_ok(low, 2));  // all zeros is a valid frame: the caller rejects rails
    assert(!ads7028_rx_crc_ok(high, 2));
    printf("✓ test_crc_matches_the_chip passed\n");
}

static void test_crc_command_frames(void)
{
    uint8_t f[4];
    ads7028_frame_crc(ADS7028_OP_NOP, 0, 0, f);
    assert(f[0] == 0 && f[1] == 0 && f[2] == 0 && f[3] == 0);  // a no-op stays all zeros
    // The frames the pad accepted: the read that returned 0x81 0x8E above,
    // and the write that turned CRC off again
    ads7028_frame_crc(ADS7028_OP_READ, ADS7028_REG_SYSTEM_STATUS, 0, f);
    assert(f[3] == 0xA2);
    ads7028_frame_crc(ADS7028_OP_WRITE, ADS7028_REG_GENERAL_CFG, 0, f);
    assert(f[3] == 0x44);
    printf("✓ test_crc_command_frames passed\n");
}

int main(void)
{
    test_command_frames();
    test_data12();
    test_data16();
    test_crc_matches_the_chip();
    test_crc_command_frames();
    return 0;
}
