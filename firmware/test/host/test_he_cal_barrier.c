#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include "input/he_cal_barrier.h"

typedef struct {
    he_cal_barrier_t barrier;
    unsigned ticket;
    bool old_snapshot_down;
    bool reported_down;
    unsigned reports;
    bool retry_during_publish;
} fixture_t;

static void finish_old_report_and_release(void *ctx)
{
    fixture_t *f = ctx;
    // Model the reporting task paused between snapshot and HID submission.
    // Completion must not become visible until this callback returns.
    assert(!he_cal_barrier_done(&f->barrier, f->ticket));
    if (f->old_snapshot_down) {
        f->reported_down = true;
        f->reports++;
        f->old_snapshot_down = false;
    }
    f->reported_down = false; // publish the now-suppressed engine state
    f->reports++;
    if (f->retry_during_publish) {
        // Previous caller timed out; the retry gets its own completion.
        f->ticket = he_cal_barrier_request(&f->barrier);
    }
}

int main(void)
{
    fixture_t f = {.old_snapshot_down = true};
    f.ticket = he_cal_barrier_request(&f.barrier);
    assert(!he_cal_barrier_done(&f.barrier, f.ticket));
    he_cal_barrier_publish(&f.barrier, finish_old_report_and_release, &f);
    assert(he_cal_barrier_done(&f.barrier, f.ticket));
    assert(!f.reported_down && f.reports == 2);

    // Idle/already-released keys still need an acknowledgement, not a new
    // key event to wake the task. Simulate explicit calibration notification.
    f.ticket = he_cal_barrier_request(&f.barrier);
    assert(!he_cal_barrier_done(&f.barrier, f.ticket));
    he_cal_barrier_publish(&f.barrier, finish_old_report_and_release, &f);
    assert(he_cal_barrier_done(&f.barrier, f.ticket));
    assert(!f.reported_down);

    f.ticket = he_cal_barrier_request(&f.barrier);
    f.retry_during_publish = true;
    he_cal_barrier_publish(&f.barrier, finish_old_report_and_release, &f);
    assert(!he_cal_barrier_done(&f.barrier, f.ticket));
    f.retry_during_publish = false;
    he_cal_barrier_publish(&f.barrier, finish_old_report_and_release, &f);
    assert(he_cal_barrier_done(&f.barrier, f.ticket));

    atomic_store(&f.barrier.requested, UINT_MAX);
    atomic_store(&f.barrier.completed, UINT_MAX);
    f.ticket = he_cal_barrier_request(&f.barrier);
    assert(f.ticket == 0 && !he_cal_barrier_done(&f.barrier, f.ticket));
    he_cal_barrier_publish(&f.barrier, finish_old_report_and_release, &f);
    assert(he_cal_barrier_done(&f.barrier, f.ticket));
    puts("HE calibration barrier orders publication before acknowledgement.");
    return 0;
}
