/*
 * Which input module an ID reading names (input/module_id.c). Voltages in mV:
 * open = pull-down off, loaded = pull-down on.
 */
#include <assert.h>
#include <stdio.h>
#include "../../main/input/module_id.h"

static void test_nominal_dividers(void)
{
    assert(module_id_classify(300, 200) == MODULE_ID_MX);       // MX V1 100k/10k
    assert(module_id_classify(1060, 500) == MODULE_ID_HE);      // HE V1 100k/47k
    assert(module_id_classify(1650, 440) == MODULE_ID_HE_V3);   // HE V3 100k/100k
    assert(module_id_classify(2240, 1300) == MODULE_ID_MX_V3);  // MX V3 47k/100k
    printf("✓ test_nominal_dividers passed\n");
}

// The HE V3 board measured on 2026-11-20: 1655 mV open, 440 mV loaded
static void test_measured_he_v3(void)
{
    assert(module_id_classify(1655, 440) == MODULE_ID_HE_V3);
    printf("✓ test_measured_he_v3 passed\n");
}

static void test_cut_offs(void)
{
    assert(module_id_classify(679, 300) == MODULE_ID_MX);
    assert(module_id_classify(680, 300) == MODULE_ID_HE);
    assert(module_id_classify(1349, 300) == MODULE_ID_HE);
    assert(module_id_classify(1350, 300) == MODULE_ID_HE_V3);
    assert(module_id_classify(1949, 300) == MODULE_ID_HE_V3);
    assert(module_id_classify(1950, 300) == MODULE_ID_MX_V3);
    assert(module_id_classify(2599, 300) == MODULE_ID_MX_V3);
    assert(module_id_classify(2600, 300) == MODULE_ID_NONE);    // shorted to 3V3
    printf("✓ test_cut_offs passed\n");
}

// A floating pin reads ~0 V with the pull-down, whatever it reads open
static void test_floating_and_failed_reads(void)
{
    assert(module_id_classify(1650, 20) == MODULE_ID_NONE);
    assert(module_id_classify(1650, 59) == MODULE_ID_NONE);
    assert(module_id_classify(-1, 400) == MODULE_ID_NONE);
    assert(module_id_classify(1650, -1) == MODULE_ID_NONE);
    printf("✓ test_floating_and_failed_reads passed\n");
}

static void test_v3_classes(void)
{
    assert(module_id_is_v3(MODULE_ID_HE_V3));
    assert(module_id_is_v3(MODULE_ID_MX_V3));
    assert(!module_id_is_v3(MODULE_ID_MX));
    assert(!module_id_is_v3(MODULE_ID_HE));
    assert(!module_id_is_v3(MODULE_ID_NONE));
    printf("✓ test_v3_classes passed\n");
}

int main(void)
{
    test_nominal_dividers();
    test_measured_he_v3();
    test_cut_offs();
    test_floating_and_failed_reads();
    test_v3_classes();
    return 0;
}
