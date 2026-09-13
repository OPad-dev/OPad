#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>

// Pure function extracted from firmware counters.c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len);

static void test_stale_generation_rejected(void)
{
    char err[64] = {0};
    bool ok = counters_validate_sync_acceptance(5, 100, 100, 4, 200, 200, false, err, sizeof(err));
    assert(!ok);
    assert(strcmp(err, "stale generation") == 0);
    printf("✓ test_stale_generation_rejected passed\n");
}

static void test_non_monotonic_rejected(void)
{
    char err[64] = {0};
    // K1 decreased
    bool ok = counters_validate_sync_acceptance(5, 100, 100, 5, 99, 100, false, err, sizeof(err));
    assert(!ok);
    assert(strcmp(err, "non-monotonic") == 0);

    // K2 decreased
    memset(err, 0, sizeof(err));
    ok = counters_validate_sync_acceptance(5, 100, 100, 5, 100, 99, false, err, sizeof(err));
    assert(!ok);
    assert(strcmp(err, "non-monotonic") == 0);

    printf("✓ test_non_monotonic_rejected passed\n");
}

static void test_monotonic_same_generation_accepted(void)
{
    char err[64] = {0};
    bool ok = counters_validate_sync_acceptance(5, 100, 100, 5, 100, 100, false, err, sizeof(err));
    assert(ok);

    ok = counters_validate_sync_acceptance(5, 100, 100, 5, 150, 200, false, err, sizeof(err));
    assert(ok);
    printf("✓ test_monotonic_same_generation_accepted passed\n");
}

static void test_generation_bump_accepted(void)
{
    char err[64] = {0};
    // New generation can have lower counts (reset)
    bool ok = counters_validate_sync_acceptance(5, 100, 100, 6, 0, 0, false, err, sizeof(err));
    assert(ok);
    printf("✓ test_generation_bump_accepted passed\n");
}

static void test_force_bypasses_validation(void)
{
    char err[64] = {0};
    // Force allows lower generation
    bool ok = counters_validate_sync_acceptance(5, 100, 100, 2, 0, 0, true, err, sizeof(err));
    assert(ok);

    // Force allows lower counters on same generation
    ok = counters_validate_sync_acceptance(5, 100, 100, 5, 10, 20, true, err, sizeof(err));
    assert(ok);
    printf("✓ test_force_bypasses_validation passed\n");
}

int main(void)
{
    test_stale_generation_rejected();
    test_non_monotonic_rejected();
    test_monotonic_same_generation_accepted();
    test_generation_bump_accepted();
    test_force_bypasses_validation();
    printf("All counter sync unit tests passed successfully!\n");
    return 0;
}
