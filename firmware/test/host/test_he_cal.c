#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "input/he_cal_record.h"

static void test_crc32_basic(void)
{
    const char *test_str = "123456789";
    // Standard CRC-32 for "123456789" is 0xCBF43926
    uint32_t crc = he_cal_crc32(test_str, strlen(test_str));
    assert(crc == 0xCBF43926u);

    // Bit flip changes CRC
    char flipped[10];
    memcpy(flipped, test_str, 10);
    flipped[0] ^= 1;
    uint32_t crc_flipped = he_cal_crc32(flipped, strlen(flipped));
    assert(crc != crc_flipped);

    printf("✓ test_crc32_basic passed\n");
}

static void test_defaults(void)
{
    he_cal_record_t rec;
    he_cal_record_init_defaults(&rec);

    assert(rec.magic == HE_CAL_MAGIC);
    assert(rec.version == HE_CAL_VERSION);
    assert(rec.seq == 0);
    assert(rec.keys[0].rest == HE_CAL_FACTORY_REST);
    assert(rec.keys[0].full == HE_CAL_FACTORY_FULL);
    assert(rec.keys[0].valid == 0);
    assert(rec.keys[1].rest == HE_CAL_FACTORY_REST);
    assert(rec.keys[1].full == HE_CAL_FACTORY_FULL);
    assert(rec.keys[1].valid == 0);

    assert(he_cal_record_validate(&rec));

    he_calibration_t unpacked[2];
    he_cal_record_unpack(&rec, unpacked);
    assert(!unpacked[0].valid);
    assert(unpacked[0].rest == HE_CAL_FACTORY_REST);
    assert(unpacked[0].full == HE_CAL_FACTORY_FULL);
    assert(!unpacked[1].valid);

    printf("✓ test_defaults passed\n");
}

static void test_pack_and_validate(void)
{
    he_calibration_t cal[2] = {
        {.rest = 2084, .full = 2084 + 447, .valid = true},
        {.rest = 2088, .full = 2088 + 508, .valid = true},
    };
    he_cal_record_t rec;
    he_cal_record_pack(&rec, 42, cal);

    assert(rec.magic == HE_CAL_MAGIC);
    assert(rec.version == HE_CAL_VERSION);
    assert(rec.seq == 42);
    assert(rec.keys[0].valid == 1);
    assert(rec.keys[0].rest == 2084);
    assert(rec.keys[0].full == 2531);
    assert(rec.keys[1].valid == 1);
    assert(rec.keys[1].rest == 2088);
    assert(rec.keys[1].full == 2596);

    assert(he_cal_record_validate(&rec));

    he_calibration_t out[2];
    he_cal_record_unpack(&rec, out);
    assert(out[0].valid);
    assert(out[0].rest == 2084);
    assert(out[0].full == 2531);
    assert(out[1].valid);
    assert(out[1].rest == 2088);
    assert(out[1].full == 2596);

    printf("✓ test_pack_and_validate passed\n");
}

static void test_corruptions(void)
{
    he_calibration_t cal[2] = {
        {.rest = 2084, .full = 2531, .valid = true},
        {.rest = 2088, .full = 2596, .valid = true},
    };
    he_cal_record_t rec;
    he_cal_record_pack(&rec, 10, cal);
    assert(he_cal_record_validate(&rec));

    // 1. Corrupt CRC
    he_cal_record_t bad = rec;
    bad.crc32 ^= 0x12345678;
    assert(!he_cal_record_validate(&bad));

    // 2. Corrupt magic
    bad = rec;
    bad.magic = 0xDEADBEEF;
    assert(!he_cal_record_validate(&bad));

    // 3. Corrupt version
    bad = rec;
    bad.version = 2;
    assert(!he_cal_record_validate(&bad));

    // 4. Inverted span (full <= rest)
    bad = rec;
    bad.keys[0].full = bad.keys[0].rest - 10;
    bad.crc32 = he_cal_crc32(&bad, offsetof(he_cal_record_t, crc32));
    assert(!he_cal_record_validate(&bad));

    // 5. Out of bounds (rest < 0)
    bad = rec;
    bad.keys[0].rest = -1;
    bad.crc32 = he_cal_crc32(&bad, offsetof(he_cal_record_t, crc32));
    assert(!he_cal_record_validate(&bad));

    // 6. Out of bounds (full > 4095)
    bad = rec;
    bad.keys[0].full = 4096;
    bad.crc32 = he_cal_crc32(&bad, offsetof(he_cal_record_t, crc32));
    assert(!he_cal_record_validate(&bad));

    // 7. Valid flag > 1
    bad = rec;
    bad.keys[0].valid = 2;
    bad.crc32 = he_cal_crc32(&bad, offsetof(he_cal_record_t, crc32));
    assert(!he_cal_record_validate(&bad));

    printf("✓ test_corruptions passed\n");
}

static void test_slot_selection(void)
{
    he_calibration_t cal[2] = {
        {.rest = 2084, .full = 2531, .valid = true},
        {.rest = 2088, .full = 2596, .valid = true},
    };
    he_cal_record_t slot_a = {0}, slot_b = {0};

    // Both missing/invalid
    assert(he_cal_record_select_active(&slot_a, false, &slot_b, false) == -1);
    assert(he_cal_record_select_active(NULL, false, NULL, false) == -1);

    // Only A valid
    he_cal_record_pack(&slot_a, 1, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, false) == 0);

    // Only B valid
    he_cal_record_pack(&slot_b, 1, cal);
    assert(he_cal_record_select_active(&slot_a, false, &slot_b, true) == 1);

    // Both valid: seq A (5) > seq B (4) -> A wins
    he_cal_record_pack(&slot_a, 5, cal);
    he_cal_record_pack(&slot_b, 4, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, true) == 0);

    // Both valid: seq B (5) > seq A (4) -> B wins
    he_cal_record_pack(&slot_a, 4, cal);
    he_cal_record_pack(&slot_b, 5, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, true) == 1);

    // Both valid: seq equal -> A wins
    he_cal_record_pack(&slot_a, 5, cal);
    he_cal_record_pack(&slot_b, 5, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, true) == 0);

    // Both valid: seq wrap-around (seq 2 vs seq 0xFFFFFFFE)
    // 2 - 0xFFFFFFFE = 4 > 0, so A (2) is newer
    he_cal_record_pack(&slot_a, 2, cal);
    he_cal_record_pack(&slot_b, 0xFFFFFFFEu, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, true) == 0);

    // Reverse wrap-around: B (2) is newer than A (0xFFFFFFFE)
    he_cal_record_pack(&slot_a, 0xFFFFFFFEu, cal);
    he_cal_record_pack(&slot_b, 2, cal);
    assert(he_cal_record_select_active(&slot_a, true, &slot_b, true) == 1);

    printf("✓ test_slot_selection passed\n");
}

static void test_next_slot_and_seq(void)
{
    // If no slot active yet (-1)
    assert(he_cal_record_next_slot(-1) == 0);
    assert(he_cal_record_next_seq(-1, 0) == 1);

    // If slot 0 (A) active, next slot is 1 (B)
    assert(he_cal_record_next_slot(0) == 1);
    assert(he_cal_record_next_seq(0, 10) == 11);

    // If slot 1 (B) active, next slot is 0 (A)
    assert(he_cal_record_next_slot(1) == 0);
    assert(he_cal_record_next_seq(1, 11) == 12);

    // Wrap around 0xFFFFFFFF
    assert(he_cal_record_next_seq(1, 0xFFFFFFFFu) == 1);

    printf("✓ test_next_slot_and_seq passed\n");
}

int main(void)
{
    printf("=== Running Hall Effect Calibration Record Host Tests ===\n");
    test_crc32_basic();
    test_defaults();
    test_pack_and_validate();
    test_corruptions();
    test_slot_selection();
    test_next_slot_and_seq();
    printf("All Hall Effect calibration record tests passed successfully!\n");
    return 0;
}
