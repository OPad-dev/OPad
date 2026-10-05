// Host tests for the touchscreen swipe classifier
#include "input/gesture.h"
#include <assert.h>
#include <stdio.h>

static void test_still_finger_is_no_swipe(void)
{
    gesture_tracker_t g;
    gesture_begin(&g, 160, 120);
    // Jitter of a resting finger
    assert(gesture_feed(&g, 165, 118) == GESTURE_NONE);
    assert(gesture_feed(&g, 150, 128) == GESTURE_NONE);
    assert(gesture_feed(&g, 160 + GESTURE_SWIPE_PX - 1, 120) == GESTURE_NONE);
    assert(!gesture_is_swipe(&g));
    printf("✓ test_still_finger_is_no_swipe passed\n");
}

static void test_swipe_directions(void)
{
    gesture_tracker_t g;

    gesture_begin(&g, 160, 120);
    assert(gesture_feed(&g, 162, 120 - GESTURE_SWIPE_PX) == GESTURE_UP);
    assert(gesture_is_swipe(&g));

    gesture_begin(&g, 160, 120);
    assert(gesture_feed(&g, 158, 120 + GESTURE_SWIPE_PX) == GESTURE_DOWN);

    gesture_begin(&g, 160, 120);
    assert(gesture_feed(&g, 160 - GESTURE_SWIPE_PX, 123) == GESTURE_LEFT);

    gesture_begin(&g, 160, 120);
    assert(gesture_feed(&g, 160 + GESTURE_SWIPE_PX, 117) == GESTURE_RIGHT);
    printf("✓ test_swipe_directions passed\n");
}

static void test_diagonal_waits(void)
{
    gesture_tracker_t g;
    gesture_begin(&g, 100, 100);
    // 45 degrees: neither axis leads
    assert(gesture_feed(&g, 140, 60) == GESTURE_NONE);
    assert(!gesture_is_swipe(&g));
    // Then mostly up: vertical wins, and the whole distance counts
    assert(gesture_feed(&g, 145, 20) == GESTURE_UP);
    printf("✓ test_diagonal_waits passed\n");
}

static void test_vertical_steps_repeat(void)
{
    gesture_tracker_t g;
    gesture_begin(&g, 160, 200);
    int ups = 0;
    // A long drag up, 10 px per poll
    for (int y = 190; y >= 200 - 4 * GESTURE_SWIPE_PX; y -= 10) {
        if (gesture_feed(&g, 160, (int16_t)y) == GESTURE_UP) {
            ups++;
        }
    }
    assert(ups == 4);
    // Turning back steps down without lifting
    assert(gesture_feed(&g, 160, 200 - 3 * GESTURE_SWIPE_PX) == GESTURE_DOWN);
    // Sideways drift after the axis is locked does nothing
    assert(gesture_feed(&g, 60, 200 - 3 * GESTURE_SWIPE_PX) == GESTURE_NONE);
    printf("✓ test_vertical_steps_repeat passed\n");
}

static void test_horizontal_fires_once(void)
{
    gesture_tracker_t g;
    gesture_begin(&g, 40, 120);
    assert(gesture_feed(&g, 40 + GESTURE_SWIPE_PX, 120) == GESTURE_RIGHT);
    assert(gesture_feed(&g, 40 + 3 * GESTURE_SWIPE_PX, 120) == GESTURE_NONE);
    assert(gesture_feed(&g, 0, 120) == GESTURE_NONE);
    // Nor does it turn into a vertical swipe
    assert(gesture_feed(&g, 0, 0) == GESTURE_NONE);
    printf("✓ test_horizontal_fires_once passed\n");
}

int main(void)
{
    test_still_finger_is_no_swipe();
    test_swipe_directions();
    test_diagonal_waits();
    test_vertical_steps_repeat();
    test_horizontal_fires_once();
    printf("All gesture tests passed\n");
    return 0;
}
