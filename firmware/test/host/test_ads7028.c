/*
 * ADS7028 frame layout (input/ads7028_regs.h), against the datasheet
 * SBAS978A figures 25, 32 and 33.
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

int main(void)
{
    test_command_frames();
    test_data12();
    test_data16();
    return 0;
}
