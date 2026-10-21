#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "input/he_cal_capture.h"

// Deterministic LCG for reproducible noise
static uint32_t s_prng = 123456789;
static int32_t prng_noise_sd5(void)
{
    // Sum of 6 uniform random numbers in [0, 7] - 21 -> mean 0, sd ~ 5.6 counts
    int32_t sum = 0;
    for (int i = 0; i < 6; i++) {
        s_prng = s_prng * 1664525u + 1013904223u;
        sum += (int32_t)((s_prng >> 24) & 0x07);
    }
    return sum - 21;
}

// Generate rest stream
static void make_rest(int32_t *buf, size_t n, int32_t rest_center)
{
    for (size_t i = 0; i < n; i++) {
        buf[i] = rest_center + prng_noise_sd5();
    }
}

// Generate a smooth stroke from rest up to peak and back down to rest
static size_t make_stroke(int32_t *buf, size_t max_len, int32_t rest, int32_t peak)
{
    size_t idx = 0;
    // Rise: 10 steps
    for (int step = 1; step <= 10 && idx < max_len; step++) {
        int32_t target = rest + (peak - rest) * step / 10;
        buf[idx++] = target + prng_noise_sd5();
    }
    // Peak hold: 3 steps
    for (int step = 0; step < 3 && idx < max_len; step++) {
        buf[idx++] = peak + prng_noise_sd5();
    }
    // Fall: 10 steps
    for (int step = 9; step >= 0 && idx < max_len; step--) {
        int32_t target = rest + (peak - rest) * step / 10;
        buf[idx++] = target + prng_noise_sd5();
    }
    // Settle at rest: 5 steps
    for (int step = 0; step < 5 && idx < max_len; step++) {
        buf[idx++] = rest + prng_noise_sd5();
    }
    return idx;
}

static void test_normal_presses_and_median_peak(void)
{
    // Bench numbers: rest ~2086, normal press +450, hard presses up to +540
    int32_t rest_samples[100];
    make_rest(rest_samples, 100, 2086);

    // 5 strokes: 3 normal (+440, +450, +460) and 2 hard bottoming-out (+520, +540)
    int32_t peak_deltas[5] = {440, 450, 460, 520, 540};
    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 5; i++) {
        press_len += make_stroke(press_samples + press_len, sizeof(press_samples)/sizeof(press_samples[0]) - press_len,
                                 2086, 2086 + peak_deltas[i]);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);
    cfg.max_peak_diff = 120; // 540 - 460 = 80 <= 120, acceptable variation

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);

    assert(err == HE_CAL_ERR_NONE);
    assert(cal.valid);
    // Rest should be within a few counts of 2086
    assert(abs(cal.rest - 2086) <= 5);
    // Median peak delta is 460 (sorted: 440, 450, 460, 520, 540 -> index 2 is 460)
    // The hardest press was 2086 + 540 = 2626, but the median is ~2546!
    assert(abs(cal.full - (2086 + 460)) <= 10);
    assert(cal.full < 2086 + 520); // NOT the hardest press!

    printf("✓ test_normal_presses_and_median_peak passed (rest=%ld, full=%ld, median selected over hardest)\n",
           (long)cal.rest, (long)cal.full);
}

static void test_reject_inconsistent_peaks(void)
{
    int32_t rest_samples[100];
    make_rest(rest_samples, 100, 2086);

    // Wildly inconsistent peaks: +200, +250, +450, +650, +800
    int32_t peak_deltas[5] = {200, 250, 450, 650, 800};
    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 5; i++) {
        press_len += make_stroke(press_samples + press_len, sizeof(press_samples)/sizeof(press_samples[0]) - press_len,
                                 2086, 2086 + peak_deltas[i]);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);
    cfg.max_peak_diff = 100;

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);
    assert(err == HE_CAL_ERR_PEAKS_INCONSISTENT);

    printf("✓ test_reject_inconsistent_peaks passed (%s)\n", he_cal_error_to_name(err));
}

static void test_reject_too_few_peaks(void)
{
    int32_t rest_samples[100];
    make_rest(rest_samples, 100, 2086);

    // Only 3 strokes instead of 5
    int32_t peak_deltas[3] = {450, 450, 450};
    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 3; i++) {
        press_len += make_stroke(press_samples + press_len, sizeof(press_samples)/sizeof(press_samples[0]) - press_len,
                                 2086, 2086 + peak_deltas[i]);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);
    assert(err == HE_CAL_ERR_TOO_FEW_PEAKS);

    printf("✓ test_reject_too_few_peaks passed (%s)\n", he_cal_error_to_name(err));
}

static void test_reject_span_too_small(void)
{
    int32_t rest_samples[100];
    make_rest(rest_samples, 100, 2086);

    // 5 very shallow taps (+130 counts; arms at 120, but span 130 < min_span 250)
    int32_t peak_deltas[5] = {130, 135, 130, 132, 130};
    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 5; i++) {
        press_len += make_stroke(press_samples + press_len, sizeof(press_samples)/sizeof(press_samples[0]) - press_len,
                                 2086, 2086 + peak_deltas[i]);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);
    cfg.min_span = 250;

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);
    assert(err == HE_CAL_ERR_SPAN_TOO_SMALL);

    printf("✓ test_reject_span_too_small passed (%s)\n", he_cal_error_to_name(err));
}

static void test_reject_signal_not_rising(void)
{
    he_cal_capture_t cap;
    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);
    he_cal_capture_init(&cap, &cfg);

    // Feed rest
    for (int i = 0; i < 64; i++) {
        he_cal_capture_feed_rest(&cap, 2086);
    }
    assert(cap.stage == HE_CAL_STAGE_PRESSES);

    // Fake 5 negative peaks (e.g. inverted magnet)
    cap.peak_count = 5;
    for (int i = 0; i < 5; i++) {
        cap.peaks[i] = 2000; // < 2086
    }
    // Simulate finishing
    cap.current_stroke_peak = 2000;
    // Feed one sample to trigger evaluation or use process
    int32_t rest_samples[64];
    for (int i = 0; i < 64; i++) rest_samples[i] = 2086;

    // Use a downward stroke
    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 5; i++) {
        press_len += make_stroke(press_samples + press_len, sizeof(press_samples)/sizeof(press_samples[0]) - press_len,
                                 2086, 1800);
    }
    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 64, press_samples, press_len, &cal);
    // Since downward stroke doesn't cross press_arm_span (+120), no strokes detected -> too few peaks
    assert(err == HE_CAL_ERR_TOO_FEW_PEAKS);

    printf("✓ test_reject_signal_not_rising passed\n");
}

static void test_reject_rest_not_still(void)
{
    int32_t rest_samples[100];
    // User moves finger while taking rest: spread is 80 counts (> rest_max_spread 36)
    for (int i = 0; i < 100; i++) {
        rest_samples[i] = 2086 + (i % 2 == 0 ? 40 : -40);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, NULL, 0, &cal);
    assert(err == HE_CAL_ERR_REST_NOT_STILL);

    printf("✓ test_reject_rest_not_still passed (%s)\n", he_cal_error_to_name(err));
}

static void test_reject_rest_out_of_range(void)
{
    int32_t rest_samples[100];
    // User started calibration with key already pressed down (reads 2800 > rest_max_raw 2500)
    make_rest(rest_samples, 100, 2800);

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);

    he_calibration_t cal;
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, NULL, 0, &cal);
    assert(err == HE_CAL_ERR_REST_OUT_OF_RANGE);

    printf("✓ test_reject_rest_out_of_range passed (%s)\n", he_cal_error_to_name(err));
}

// Rest taken with the key held ~200 counts down: rest itself passes, but the
// key then comes fully up between presses, far below that "rest"
static void test_reject_rest_taken_while_held(void)
{
    int32_t rest_samples[100];
    make_rest(rest_samples, 100, 2086 + 200);

    int32_t press_samples[500];
    size_t press_len = 0;
    for (int i = 0; i < 5; i++) {
        press_len += make_stroke(press_samples + press_len,
                                 sizeof(press_samples) / sizeof(press_samples[0]) - press_len,
                                 2086, 2086 + 760);
    }

    he_cal_capture_cfg_t cfg;
    he_cal_capture_default_cfg(&cfg);
    he_calibration_t cal = {0};
    he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);
    assert(err == HE_CAL_ERR_REST_PRESSED);
    assert(!cal.valid);
    printf("✓ test_reject_rest_taken_while_held passed\n");
}

// A released key's noise over a long session (4 kHz, ~1 s between presses)
// never reaches the margin, and a key held less than the margin is accepted
static void test_rest_pressed_no_false_alarm(void)
{
    static int32_t press_samples[5 * (4000 + 40)];
    int32_t rest_samples[100];
    const int32_t offsets[2] = {0, 40};
    for (int o = 0; o < 2; o++) {
        make_rest(rest_samples, 100, 2086 + offsets[o]);
        size_t press_len = 0;
        for (int i = 0; i < 5; i++) {
            make_rest(press_samples + press_len, 4000, 2086);
            press_len += 4000;
            press_len += make_stroke(press_samples + press_len,
                                     sizeof(press_samples) / sizeof(press_samples[0]) - press_len,
                                     2086, 2086 + 450);
        }
        he_cal_capture_cfg_t cfg;
        he_cal_capture_default_cfg(&cfg);
        he_calibration_t cal = {0};
        he_cal_error_t err = he_cal_capture_process(&cfg, rest_samples, 100, press_samples, press_len, &cal);
        assert(err == HE_CAL_ERR_NONE);
        assert(cal.valid);
    }
    printf("✓ test_rest_pressed_no_false_alarm passed\n");
}

int main(void)
{
    printf("=== Running Hall Effect Calibration Capture Host Tests ===\n");
    test_normal_presses_and_median_peak();
    test_reject_inconsistent_peaks();
    test_reject_too_few_peaks();
    test_reject_span_too_small();
    test_reject_signal_not_rising();
    test_reject_rest_not_still();
    test_reject_rest_out_of_range();
    test_reject_rest_taken_while_held();
    test_rest_pressed_no_false_alarm();
    printf("All Hall Effect calibration capture tests passed successfully!\n");
    return 0;
}
