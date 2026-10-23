/*
 * Input module swapped while powered (input/module_watch.c). Values follow
 * board_module_type_t: 0 none, 1 MX, 2 Hall Effect.
 */
#include <assert.h>
#include <stdio.h>
#include "../../main/input/module_watch.h"

enum { NONE = 0, MX = 1, HE = 2 };

static void test_steady_module_does_nothing(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    for (int i = 0; i < 1000; i++) {
        assert(module_watch_step(&w, MX) == MODULE_WATCH_KEEP);
    }
    assert(!w.keys_off);
    printf("✓ test_steady_module_does_nothing passed\n");
}

// MX out (reads none), HE in: keys off at the first "none", before HE arrives
static void test_mx_to_he_keys_off_at_unplug(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    assert(module_watch_step(&w, NONE) == MODULE_WATCH_KEYS_OFF);
    assert(module_watch_step(&w, NONE) == MODULE_WATCH_KEEP);
    assert(module_watch_step(&w, HE) == MODULE_WATCH_CHANGED);
    for (int i = 0; i < 100; i++) {
        assert(module_watch_step(&w, HE) == MODULE_WATCH_KEEP);
    }
    assert(w.keys_off);
    printf("✓ test_mx_to_he_keys_off_at_unplug passed\n");
}

// HE straight in place of MX within one poll still turns the keys off at once
static void test_direct_swap(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    assert(module_watch_step(&w, HE) == MODULE_WATCH_KEYS_OFF);
    assert(w.keys_off);
    printf("✓ test_direct_swap passed\n");
}

// The same module seated again: keys back after RESTORE_READS readings in a row
static void test_same_module_back(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    assert(module_watch_step(&w, NONE) == MODULE_WATCH_KEYS_OFF);
    assert(module_watch_step(&w, MX) == MODULE_WATCH_CHANGED);
    for (int i = 1; i < MODULE_WATCH_RESTORE_READS - 1; i++) {
        assert(module_watch_step(&w, MX) == MODULE_WATCH_KEEP);
    }
    assert(module_watch_step(&w, MX) == MODULE_WATCH_KEYS_ON);
    assert(!w.keys_off);
    assert(module_watch_step(&w, MX) == MODULE_WATCH_KEEP);
    printf("✓ test_same_module_back passed\n");
}

// A flaky contact while reseating restarts the count
static void test_flaky_reseat_restarts_count(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    module_watch_step(&w, NONE);
    module_watch_step(&w, MX);
    module_watch_step(&w, MX);
    assert(module_watch_step(&w, NONE) == MODULE_WATCH_CHANGED);
    for (int i = 0; i < MODULE_WATCH_RESTORE_READS - 1; i++) {
        assert(module_watch_step(&w, MX) != MODULE_WATCH_KEYS_ON);
    }
    assert(module_watch_step(&w, MX) == MODULE_WATCH_KEYS_ON);
    printf("✓ test_flaky_reseat_restarts_count passed\n");
}

// One noisy fast check of a seated module turns nothing off; a real unplug
// (MODULE_WATCH_FAST_READS low checks in a row) does
static void test_fast_check_needs_reads_in_a_row(void)
{
    module_watch_t w;
    module_watch_init(&w, MX);
    for (int i = 0; i < 100; i++) {
        assert(!module_watch_fast(&w, i % 3 == 0));  // a dip now and then
        assert(!module_watch_fast(&w, i % 5 == 0));
    }
    assert(!module_watch_fast(&w, true));
    assert(!module_watch_fast(&w, true));
    assert(module_watch_fast(&w, true));
    assert(!module_watch_fast(&w, true));  // once, not every poll after
    assert(!module_watch_fast(&w, false));

    // Keys back on after an unplug: the count starts over
    assert(module_watch_step(&w, NONE) == MODULE_WATCH_KEYS_OFF);
    assert(!module_watch_fast(&w, true) && !module_watch_fast(&w, true) && module_watch_fast(&w, true));
    for (int i = 0; i < MODULE_WATCH_RESTORE_READS - 1; i++) {
        module_watch_step(&w, MX);
    }
    assert(module_watch_step(&w, MX) == MODULE_WATCH_KEYS_ON);
    assert(!module_watch_fast(&w, true) && !module_watch_fast(&w, true) && module_watch_fast(&w, true));
    printf("✓ test_fast_check_needs_reads_in_a_row passed\n");
}

int main(void)
{
    test_steady_module_does_nothing();
    test_mx_to_he_keys_off_at_unplug();
    test_direct_swap();
    test_same_module_back();
    test_flaky_reseat_restarts_count();
    test_fast_check_needs_reads_in_a_row();
    printf("All module watch tests passed\n");
    return 0;
}
