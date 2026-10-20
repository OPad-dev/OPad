#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "input/he_cal_runner.h"

static void simulate_stroke(he_cal_runner_t *r, int key, int32_t rest, int32_t peak)
{
    // Rising stroke
    for (int32_t v = rest; v <= peak; v += 30) {
        if (key == 1) {
            he_cal_runner_feed(r, v, rest);
        } else {
            he_cal_runner_feed(r, rest, v);
        }
    }
    // Falling stroke back below release arm threshold
    for (int32_t v = peak; v >= rest; v -= 30) {
        if (key == 1) {
            he_cal_runner_feed(r, v, rest);
        } else {
            he_cal_runner_feed(r, rest, v);
        }
    }
}

static void test_full_successful_run(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    assert(runner.stage == HE_RUNNER_IDLE);

    he_cal_runner_start(&runner);
    assert(runner.stage == HE_RUNNER_REST);

    he_runner_progress_t prog;
    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_REST);

    // Feed settle + 64 rest samples for both keys at rest = 2080
    for (int i = 0; i < 16 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2090);
    }
    assert(runner.stage == HE_RUNNER_KEY1);

    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_KEY1);
    assert(prog.active_key == 1);
    assert(prog.presses_done == 0);

    // 5 strokes on Key 1 with peaks around 2550 (span ~470 counts)
    const int32_t peaks1[5] = {2540, 2550, 2560, 2550, 2545};
    for (int p = 0; p < 4; p++) {
        simulate_stroke(&runner, 1, 2080, peaks1[p]);
        he_cal_runner_get_progress(&runner, &prog);
        assert(prog.stage == HE_RUNNER_KEY1);
        assert(prog.presses_done == (uint8_t)(p + 1));
    }
    // 5th stroke completes Key 1 and transitions to Key 2
    simulate_stroke(&runner, 1, 2080, peaks1[4]);
    assert(runner.stage == HE_RUNNER_KEY2);
    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_KEY2);
    assert(prog.active_key == 2);
    assert(prog.presses_done == 0);

    // 5 strokes on Key 2 with peaks around 2580
    const int32_t peaks2[5] = {2570, 2580, 2590, 2580, 2575};
    for (int p = 0; p < 4; p++) {
        simulate_stroke(&runner, 2, 2090, peaks2[p]);
        he_cal_runner_get_progress(&runner, &prog);
        assert(prog.stage == HE_RUNNER_KEY2);
        assert(prog.presses_done == (uint8_t)(p + 1));
    }
    // 5th stroke completes Key 2 and transitions to SUCCESS
    simulate_stroke(&runner, 2, 2090, peaks2[4]);
    assert(runner.stage == HE_RUNNER_SUCCESS);
    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_SUCCESS);
    assert(prog.presses_done == 5);

    he_calibration_t results[2];
    bool ok = he_cal_runner_get_results(&runner, results);
    assert(ok);
    assert(results[0].valid);
    assert(results[0].rest == 2080);
    assert(results[0].full == 2550); // median of 2540, 2545, 2550, 2550, 2560
    assert(results[1].valid);
    assert(results[1].rest == 2090);
    assert(results[1].full == 2580); // median of 2570, 2575, 2580, 2580, 2590

    printf("✓ test_full_successful_run passed\n");
}

static void test_rest_noisy_fails(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    he_cal_runner_start(&runner);

    // Settle
    for (int i = 0; i < 16; i++) {
        he_cal_runner_feed(&runner, 2080, 2080);
    }

    // Feed noisy samples for Key 1 (spread = 50 > max_spread 36)
    for (int i = 0; i < 32; i++) {
        he_cal_runner_feed(&runner, 2050, 2080);
    }
    for (int i = 0; i < 32; i++) {
        he_cal_runner_feed(&runner, 2100, 2080);
    }

    assert(runner.stage == HE_RUNNER_FAILED);
    assert(runner.error == HE_CAL_ERR_REST_NOT_STILL);
    assert(runner.error_key == 0);

    he_runner_progress_t prog;
    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_FAILED);
    assert(strcmp(prog.error_desc, "Noisy rest") == 0);

    printf("✓ test_rest_noisy_fails passed\n");
}

static void test_span_too_small_fails(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    he_cal_runner_start(&runner);

    // Settle + rest at 2080
    for (int i = 0; i < 16 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2080);
    }
    assert(runner.stage == HE_RUNNER_KEY1);

    // Weak presses on Key 1: peak 2220 (span 140 < min_span 250)
    for (int p = 0; p < 5; p++) {
        simulate_stroke(&runner, 1, 2080, 2220);
    }

    assert(runner.stage == HE_RUNNER_FAILED);
    assert(runner.error == HE_CAL_ERR_SPAN_TOO_SMALL);
    assert(runner.error_key == 0);

    he_runner_progress_t prog;
    he_cal_runner_get_progress(&runner, &prog);
    assert(prog.stage == HE_RUNNER_FAILED);
    assert(strcmp(prog.error_desc, "Not pressed enough") == 0);

    printf("✓ test_span_too_small_fails passed\n");
}

static void test_cancel(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    he_cal_runner_start(&runner);

    for (int i = 0; i < 16 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2080);
    }
    assert(runner.stage == HE_RUNNER_KEY1);

    he_cal_runner_cancel(&runner);
    assert(runner.stage == HE_RUNNER_IDLE);

    printf("✓ test_cancel passed\n");
}

int main(void)
{
    printf("=== Running Hall Effect Calibration Runner Host Tests ===\n");
    test_full_successful_run();
    test_rest_noisy_fails();
    test_span_too_small_fails();
    test_cancel();
    printf("All Hall Effect calibration runner tests passed successfully!\n");
    return 0;
}
