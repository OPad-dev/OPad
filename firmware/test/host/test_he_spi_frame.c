/*
 * Hall Effect V3 frame decode (input/he_spi_frame.c): conversions tagged with
 * their AIN -> one reading per key.
 */
#include <assert.h>
#include <stdio.h>
#include "../../main/input/he_spi_frame.h"

static void test_one_reading_per_key(void)
{
    // The first word is the stale one from the last frame (AIN3); the rest are fresh
    const uint16_t w[4] = {he_spi_word(3, 2000), he_spi_word(4, 2201), he_spi_word(5, 2198),
                           he_spi_word(3, 2190)};
    he_frame_t f;
    int32_t raw[3];
    he_spi_frame_decode(w, 4, &f, raw);
    assert(raw[0] == 2198 && raw[1] == 2201 && raw[2] == 2190);  // newest AIN3 wins
    assert(f.median[0] == 2198 && f.median[1] == 2201);
    assert(f.newest[0] == 2198 && f.count[0] == 1 && f.count[1] == 1);
    assert(f.foreign == 0 && f.railed == 0);
    printf("✓ test_one_reading_per_key passed\n");
}

// A shifted frame carries a channel no key uses: it presses nothing
static void test_foreign_channel_is_dropped(void)
{
    const uint16_t w[3] = {he_spi_word(7, 3000), he_spi_word(0, 3000), 0xFFFF};
    he_frame_t f;
    int32_t raw[3];
    he_spi_frame_decode(w, 3, &f, raw);
    assert(raw[0] == -1 && raw[1] == -1 && raw[2] == -1);
    assert(f.median[0] == -1 && f.median[1] == -1 && f.count[0] == 0);
    assert(f.foreign == 3);
    printf("✓ test_foreign_channel_is_dropped passed\n");
}

static void test_rails_are_not_readings(void)
{
    const uint16_t w[3] = {he_spi_word(5, 0), he_spi_word(4, 4095), he_spi_word(3, 1)};
    he_frame_t f;
    int32_t raw[3];
    he_spi_frame_decode(w, 3, &f, raw);
    assert(raw[0] == -1 && raw[1] == -1 && raw[2] == 1);
    assert(f.railed == 2);
    printf("✓ test_rails_are_not_readings passed\n");
}

// MISO stuck low reads as AIN0 at 0: foreign, never a key
static void test_stuck_miso(void)
{
    const uint16_t w[4] = {0, 0, 0, 0};
    he_frame_t f;
    int32_t raw[3];
    he_spi_frame_decode(w, 4, &f, raw);
    assert(f.median[0] == -1 && f.median[1] == -1 && raw[2] == -1);
    printf("✓ test_stuck_miso passed\n");
}

int main(void)
{
    test_one_reading_per_key();
    test_foreign_channel_is_dropped();
    test_rails_are_not_readings();
    test_stuck_miso();
    return 0;
}
