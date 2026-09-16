#include <stdio.h>
#include <assert.h>
#include <string.h>
#include "config/owner.h"

static const uint8_t UNCLAIMED[OWNER_ID_LEN] = {0};
static const uint8_t HOST_A[OWNER_ID_LEN] = {
    0x9c, 0x1e, 0x44, 0x02, 0x7a, 0x31, 0x4f, 0x88,
    0xb6, 0x05, 0xd3, 0x77, 0x21, 0xae, 0x60, 0x19,
};
static const uint8_t HOST_B[OWNER_ID_LEN] = {
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
    0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x01,
};

static void test_unclaimed_detection(void)
{
    assert(owner_is_unclaimed(UNCLAIMED));
    assert(owner_is_unclaimed(NULL));
    assert(!owner_is_unclaimed(HOST_A));

    // A pad flashed before §W3-2 reads as unclaimed, not as someone else's
    uint8_t blank[OWNER_ID_LEN];
    memset(blank, 0, sizeof(blank));
    assert(owner_is_unclaimed(blank));

    // Every byte counts: one non-zero byte anywhere means claimed
    for (int i = 0; i < OWNER_ID_LEN; i++) {
        uint8_t almost[OWNER_ID_LEN];
        memset(almost, 0, sizeof(almost));
        almost[i] = 1;
        assert(!owner_is_unclaimed(almost));
    }
    printf("✓ test_unclaimed_detection passed\n");
}

static void test_first_claim_is_applied(void)
{
    // The common case: an unclaimed pad is claimed silently on first connect
    assert(owner_claim_decide(UNCLAIMED, HOST_A, OWNER_STATE_IDLE) == OWNER_CLAIM_APPLY);
    printf("✓ test_first_claim_is_applied passed\n");
}

static void test_reclaim_by_the_same_host_writes_nothing(void)
{
    // Otherwise every reconnect would burn a flash erase cycle for no change
    assert(owner_claim_decide(HOST_A, HOST_A, OWNER_STATE_IDLE) == OWNER_CLAIM_ALREADY_OWNED);
    // and it is still a no-op mid-map, because a no-op need not wait
    assert(owner_claim_decide(HOST_A, HOST_A, OWNER_STATE_ACTIVE) == OWNER_CLAIM_ALREADY_OWNED);
    printf("✓ test_reclaim_by_the_same_host_writes_nothing passed\n");
}

static void test_takeover_by_another_host_is_applied(void)
{
    // The firmware does not arbitrate: the host prompts the user (§W3-3) and
    // only sends a claim once they have chosen.
    assert(owner_claim_decide(HOST_A, HOST_B, OWNER_STATE_IDLE) == OWNER_CLAIM_APPLY);
    printf("✓ test_takeover_by_another_host_is_applied passed\n");
}

static void test_no_claim_while_a_map_is_running(void)
{
    // P1-3: claiming writes NVS, and nothing writes NVS during PLAYING or
    // COOLDOWN. Refused rather than deferred — the host claims at connect
    // time, which is already IDLE, and a deferred claim would be a silent one.
    assert(owner_claim_decide(UNCLAIMED, HOST_A, OWNER_STATE_ACTIVE) == OWNER_CLAIM_REJECT_ACTIVE);
    assert(owner_claim_decide(HOST_A, HOST_B, OWNER_STATE_ACTIVE) == OWNER_CLAIM_REJECT_ACTIVE);
    printf("✓ test_no_claim_while_a_map_is_running passed\n");
}

static void test_unclaiming_over_the_wire_is_refused(void)
{
    // §W3-4 makes a full reflash the only way to unbind a pad, so there is
    // deliberately no wire path to it: an all-zero claim is not an unpair.
    assert(owner_claim_decide(HOST_A, UNCLAIMED, OWNER_STATE_IDLE) == OWNER_CLAIM_REJECT_INVALID);
    assert(owner_claim_decide(HOST_A, NULL, OWNER_STATE_IDLE) == OWNER_CLAIM_REJECT_INVALID);
    assert(owner_claim_decide(UNCLAIMED, UNCLAIMED, OWNER_STATE_IDLE) == OWNER_CLAIM_REJECT_INVALID);
    printf("✓ test_unclaiming_over_the_wire_is_refused passed\n");
}

static void test_owner_comparison_is_exact(void)
{
    assert(owner_equals(HOST_A, HOST_A));
    assert(!owner_equals(HOST_A, HOST_B));
    assert(!owner_equals(HOST_A, NULL));
    assert(!owner_equals(NULL, NULL));

    // A single differing byte, including the last one, is a different owner
    uint8_t nearly[OWNER_ID_LEN];
    memcpy(nearly, HOST_A, sizeof(nearly));
    nearly[OWNER_ID_LEN - 1] ^= 0x01;
    assert(!owner_equals(HOST_A, nearly));
    assert(owner_claim_decide(HOST_A, nearly, OWNER_STATE_IDLE) == OWNER_CLAIM_APPLY);
    printf("✓ test_owner_comparison_is_exact passed\n");
}

int main(void)
{
    test_unclaimed_detection();
    test_first_claim_is_applied();
    test_reclaim_by_the_same_host_writes_nothing();
    test_takeover_by_another_host_is_applied();
    test_no_claim_while_a_map_is_running();
    test_unclaiming_over_the_wire_is_refused();
    test_owner_comparison_is_exact();
    printf("All pad ownership unit tests passed successfully!\n");
    return 0;
}
