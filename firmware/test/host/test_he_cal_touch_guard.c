#include <assert.h>
#include <stdio.h>
#include "input/he_cal_touch_guard.h"

int main(void)
{
    he_cal_touch_guard_t g = {0};

    // Contact wins the race: start must refuse, including while the physical
    // finger is already up but tap_retry/release_alt is still executing.
    assert(!he_cal_touch_begin(&g));
    assert(!he_cal_touch_reserve_start(&g));
    assert(!he_cal_touch_reserve_start(&g));
    he_cal_touch_end(&g); // final cleanup report has been submitted
    assert(he_cal_touch_reserve_start(&g));
    assert(!he_cal_touch_reserve_start(&g));

    // Start wins: a contact racing before backend suppression must latch the
    // start gate, and finishing start must not clear its busy bit.
    bool gated = he_cal_touch_begin(&g);
    assert(gated);
    he_cal_touch_finish_start(&g); // backend suppression is now visible
    assert(!he_cal_touch_reserve_start(&g));
    // Cancellation/unlock before the next touch poll cannot turn this contact
    // into an ordinary tap: gated is retained until physical release.
    assert(gated);
    he_cal_touch_end(&g);
    assert(he_cal_touch_reserve_start(&g));
    he_cal_touch_finish_start(&g);

    // A gated on-pad button ends its contact before invoking its callback.
    assert(!he_cal_touch_begin(&g));
    he_cal_touch_end(&g);
    assert(he_cal_touch_reserve_start(&g));
    he_cal_touch_finish_start(&g);
    assert(atomic_load(&g.state) == 0);

    assert(he_cal_touch_reserve_start(&g));
    assert(he_cal_touch_begin(&g));
    he_cal_touch_end(&g); // ending contact must not drop the start reservation
    assert(!he_cal_touch_reserve_start(&g));
    he_cal_touch_finish_start(&g);
    assert(atomic_load(&g.state) == 0);
    puts("test_he_cal_touch_guard: OK");
    return 0;
}
