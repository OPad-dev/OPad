#include <stdio.h>
#include <assert.h>
#include "diag/diag.h"

static void test_diag_basic(void)
{
    diag_init();
    assert(diag_available() == 0);

    diag_record(DIAG_EVENT_BOOT, 1, 0, 0);
    diag_record(DIAG_EVENT_HID_MOUNTED, 1, 0, 0);
    assert(diag_available() == 2);

    diag_entry_t entries[4];
    uint32_t dropped = 999;
    size_t count = diag_drain(entries, 4, &dropped);
    assert(count == 2);
    assert(dropped == 0);
    assert(entries[0].event_id == DIAG_EVENT_BOOT);
    assert(entries[1].event_id == DIAG_EVENT_HID_MOUNTED);
    assert(diag_available() == 0);

    printf("✓ test_diag_basic passed\n");
}

static void test_diag_ring_buffer_overflow(void)
{
    diag_init();

    // Push 70 events into 64-entry ring buffer
    for (uint16_t i = 1; i <= 70; i++) {
        diag_record(i, 1, i * 10, 0);
    }

    assert(diag_available() == DIAG_RING_SIZE); // Capped at 64

    diag_entry_t entries[DIAG_RING_SIZE];
    uint32_t dropped = 0;
    size_t count = diag_drain(entries, DIAG_RING_SIZE, &dropped);

    assert(count == 64);
    assert(dropped == 6); // 70 - 64 = 6 dropped

    // The first 6 entries (1..6) were dropped; remaining start at 7
    assert(entries[0].event_id == 7);
    assert(entries[63].event_id == 70);

    // Further drain should be empty with 0 dropped
    count = diag_drain(entries, DIAG_RING_SIZE, &dropped);
    assert(count == 0);
    assert(dropped == 0);

    printf("✓ test_diag_ring_buffer_overflow passed\n");
}

int main(void)
{
    test_diag_basic();
    test_diag_ring_buffer_overflow();
    printf("All diag buffer unit tests passed successfully!\n");
    return 0;
}
