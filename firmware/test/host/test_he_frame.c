/*
 * Hall Effect DMA frame parser (input/he_frame.c): newest valid reading per
 * key, everything else counted and ignored.
 */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include "../../main/input/he_frame.h"

#define K1 HE_FRAME_CH_KEY1
#define K2 HE_FRAME_CH_KEY2

// The word layout matches ESP-IDF's adc_digi_output_data_t.type2 on the S3
static void test_word_layout(void)
{
    uint32_t w = he_frame_word(0, K1, 2084);
    assert((w & 0xFFF) == 2084);
    assert(((w >> 13) & 0xF) == 9);
    assert(((w >> 17) & 1) == 0);
    assert((w & (1u << 12)) == 0);
    assert(he_frame_word(1, 0, 0) == (1u << 17));
    printf("✓ test_word_layout passed\n");
}

// A normal frame: 8 alternating conversions per key, the newest wins
static void test_newest_per_key(void)
{
    uint32_t f[16];
    for (int i = 0; i < 8; i++) {
        f[2 * i] = he_frame_word(0, K1, 2000 + i);
        f[2 * i + 1] = he_frame_word(0, K2, 3000 + i);
    }
    he_frame_t r;
    he_frame_parse(f, 16, &r);
    assert(r.newest[0] == 2007 && r.newest[1] == 3007);
    assert(r.count[0] == 8 && r.count[1] == 8);
    assert(r.foreign == 0 && r.railed == 0);
    printf("✓ test_newest_per_key passed\n");
}

// Other units and channels, and readings at a rail, never become a key reading
static void test_rejects(void)
{
    uint32_t f[] = {
        he_frame_word(0, K1, 2100),
        he_frame_word(1, K1, 1234),          // ADC2, same channel number
        he_frame_word(0, 7, 1200),           // the ID line
        he_frame_word(0, 15, 2200),          // invalid channel marker
        he_frame_word(0, K2, 0),             // rail
        he_frame_word(0, K2, HE_FRAME_RAW_MAX),
        0xFFFFFFFFu,                         // garbage: unit bit set
    };
    he_frame_t r;
    he_frame_parse(f, sizeof(f) / sizeof(f[0]), &r);
    assert(r.newest[0] == 2100 && r.count[0] == 1);
    assert(r.newest[1] == -1 && r.count[1] == 0);
    assert(r.foreign == 4 && r.railed == 2);
    printf("✓ test_rejects passed\n");
}

// A valid reading before a railed one is still the newest valid one
static void test_newest_valid_survives_rail(void)
{
    uint32_t f[] = {he_frame_word(0, K2, 2500), he_frame_word(0, K2, HE_FRAME_RAW_MAX)};
    he_frame_t r;
    he_frame_parse(f, 2, &r);
    assert(r.newest[1] == 2500 && r.railed == 1);
    printf("✓ test_newest_valid_survives_rail passed\n");
}

static void test_empty(void)
{
    he_frame_t r = {.foreign = 99};
    he_frame_parse(NULL, 16, &r);
    assert(r.newest[0] == -1 && r.newest[1] == -1 && r.foreign == 0);
    uint32_t f[1] = {0};
    he_frame_parse(f, 0, &r);
    assert(r.newest[0] == -1 && r.count[0] == 0);
    printf("✓ test_empty passed\n");
}

int main(void)
{
    test_word_layout();
    test_newest_per_key();
    test_rejects();
    test_newest_valid_survives_rail();
    test_empty();
    printf("All HE frame tests passed\n");
    return 0;
}
