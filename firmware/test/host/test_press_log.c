// Host tests for the key press log (issue #2)
#include "input/press_log.h"
#include <assert.h>
#include <stdio.h>

int main(void)
{
    press_log_entry_t out[200];
    uint32_t dropped = 0;

    // Order and contents survive the trip
    press_log_push(1, 1000);
    press_log_push(2, 1037);
    press_log_push(1, 1075);
    assert(press_log_pending() == 3);
    size_t n = press_log_drain(out, 2, &dropped);
    assert(n == 2 && out[0].key == 1 && out[0].t_us == 1000 && out[1].key == 2 && out[1].t_us == 1037);
    n = press_log_drain(out, 10, &dropped);
    assert(n == 1 && out[0].t_us == 1075 && dropped == 0);

    // A full log keeps the oldest and counts the rest, then works again
    for (int i = 0; i < PRESS_LOG_CAPACITY + 5; i++) {
        press_log_push(1, i);
    }
    assert(press_log_pending() == PRESS_LOG_CAPACITY);
    n = press_log_drain(out, 200, &dropped);
    assert(n == PRESS_LOG_CAPACITY && out[0].t_us == 0 && dropped == 5);
    press_log_push(2, 42);
    n = press_log_drain(out, 200, &dropped);
    assert(n == 1 && out[0].key == 2 && dropped == 5);

    // Clearing forgets what waits, wrapping the counters many times over
    for (int round = 0; round < 1000; round++) {
        press_log_push(1, round);
        press_log_push(2, round);
        press_log_clear();
    }
    assert(press_log_pending() == 0);
    press_log_push(1, 7);
    assert(press_log_drain(out, 200, NULL) == 1 && out[0].t_us == 7);

    printf("test_press_log: all passed\n");
    return 0;
}
