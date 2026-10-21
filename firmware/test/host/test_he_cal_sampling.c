#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include "input/he_cal_runner.h"

static uint32_t rng = 42;

// Rounded sum of 12 uniform nibbles: sd = sqrt(255) * .35 ~= 5.6 counts.
static int32_t noise(void)
{
    int sum = 0;
    for (int i = 0; i < 12; i++) {
        rng = rng * 1664525u + 1013904223u;
        sum += (int)(rng >> 28);
    }
    int n = (sum - 90) * 35;
    return (n + (n >= 0 ? 50 : -50)) / 100;
}

typedef struct {
    he_cal_runner_t runner;
    he_cal_runner_t old;
    he_cal_samples_t batch;
    unsigned frames;
} stream_t;

static void frame(stream_t *s, int32_t raw0, int32_t raw1)
{
    he_cal_samples_push(&s->batch, raw0, raw1);
    if (++s->frames % 80 == 0) { // LVGL consumes every 20 ms
        he_cal_runner_feed_samples(&s->runner, &s->batch);
        he_cal_samples_reset(&s->batch);
        he_cal_runner_feed(&s->old, raw0, raw1); // previous firmware
    }
}

static void rest(stream_t *s, unsigned n)
{
    for (unsigned i = 0; i < n; i++) frame(s, 2086 + noise(), 2086 + noise());
}

static int32_t median(int32_t values[5])
{
    for (int i = 0; i < 5; i++) {
        for (int j = i + 1; j < 5; j++) {
            if (values[i] > values[j]) {
                int32_t tmp = values[i];
                values[i] = values[j];
                values[j] = tmp;
            }
        }
    }
    return values[2];
}

static void test_frame_batches_preserve_real_peaks(void)
{
    stream_t s = {0};
    he_cal_runner_init(&s.runner, NULL);
    he_cal_runner_init(&s.old, NULL);
    he_cal_runner_start(&s.runner);
    he_cal_runner_start(&s.old);
    he_cal_samples_reset(&s.batch);

    // Allow even the old 50 Hz rest/settle counters to complete, so its
    // failure below measures lost peaks rather than its much slower rest.
    rest(&s, 128 * 80);
    assert(s.runner.stage == HE_RUNNER_KEY1);
    assert(s.old.stage == HE_RUNNER_KEY1);
    int32_t truth[2];
    const int spans[5] = {450, 480, 510, 470, 490};
    for (int key = 0; key < 2; key++) {
        int32_t peaks[5];
        for (int p = 0; p < 5; p++) {
            unsigned len = 80 + 40 * (unsigned)p; // 20, 30, 40, 50, 60 ms
            rest(&s, 117); // peaks fall between the 20 ms reads
            peaks[p] = 0;
            for (unsigned t = 0; t <= len; t++) {
                unsigned triangle = t <= len / 2 ? t : len - t;
                int32_t raw = 2086 + (int32_t)(triangle * (unsigned)spans[p] / (len / 2)) + noise();
                if (raw > peaks[p]) peaks[p] = raw;
                int32_t other = 2086 + noise();
                frame(&s, key == 0 ? raw : other, key == 1 ? raw : other);
            }
            rest(&s, 320);
        }
        truth[key] = median(peaks);
    }
    rest(&s, 80);
    assert(s.runner.stage == HE_RUNNER_SUCCESS);
    for (int k = 0; k < 2; k++) {
        printf("key %d: true median %ld, batched full %ld, snapshot full %ld (stage %d)\n",
               k + 1, (long)truth[k], (long)s.runner.results[k].full,
               (long)s.old.results[k].full, s.old.stage);
        assert(abs(s.runner.results[k].full - truth[k]) <= 3);
        assert(abs(s.runner.results[k].rest - 2086) <= 3);
    }
    // The same accuracy assertion would fail with the previous snapshot feed.
    assert(s.old.stage != HE_RUNNER_SUCCESS ||
           abs(s.old.results[0].full - truth[0]) > 3 ||
           abs(s.old.results[1].full - truth[1]) > 3);
}

static void test_rest_extrema_and_lost_data_fail_closed(void)
{
    he_cal_runner_t r;
    he_cal_runner_init(&r, NULL);
    he_cal_runner_start(&r);
    he_cal_samples_t s;
    he_cal_samples_reset(&s);
    for (int i = 0; i < 128; i++) {
        // One moving rest sample between timer snapshots must not be missed.
        he_cal_samples_push(&s, i == 100 ? 2160 : 2086, 2086);
    }
    assert(s.min[0] == 2086 && s.max[0] == 2160);
    assert(he_cal_runner_feed_samples(&r, &s));
    assert(r.stage == HE_RUNNER_FAILED && r.error == HE_CAL_ERR_REST_NOT_STILL);

    he_cal_runner_start(&r);
    he_cal_samples_reset(&s);
    for (int i = 0; i <= HE_CAL_SAMPLE_CAPACITY; i++) he_cal_samples_push(&s, 2086, 2086);
    assert(s.lost && s.count == HE_CAL_SAMPLE_CAPACITY);
    assert(he_cal_runner_feed_samples(&r, &s));
    assert(r.stage == HE_RUNNER_FAILED);

    he_cal_runner_start(&r);
    he_cal_samples_reset(&s);
    he_cal_samples_push(&s, -1, 2086);
    assert(s.lost);
    assert(he_cal_runner_feed_samples(&r, &s));
    assert(r.stage == HE_RUNNER_FAILED);
}

int main(void)
{
    test_frame_batches_preserve_real_peaks();
    test_rest_extrema_and_lost_data_fail_closed();
    puts("All calibration sampling tests passed.");
    return 0;
}
