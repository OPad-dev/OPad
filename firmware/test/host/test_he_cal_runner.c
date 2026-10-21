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
    for (int i = 0; i < 64 + 64; i++) {
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
    for (int i = 0; i < 64; i++) {
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
    assert(strcmp(prog.error_desc, "Keep both keys still while resting") == 0);

    printf("✓ test_rest_noisy_fails passed\n");
}

static void test_span_too_small_fails(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    he_cal_runner_start(&runner);

    // Settle + rest at 2080
    for (int i = 0; i < 64 + 64; i++) {
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
    assert(strcmp(prog.error_desc, "Press each key all the way down") == 0);

    printf("✓ test_span_too_small_fails passed\n");
}

static void test_cancel(void)
{
    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);
    he_cal_runner_start(&runner);

    for (int i = 0; i < 64 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2080);
    }
    assert(runner.stage == HE_RUNNER_KEY1);

    he_cal_runner_cancel(&runner);
    assert(runner.stage == HE_RUNNER_IDLE);

    printf("✓ test_cancel passed\n");
}

static void test_lock_decision(void)
{
    // Never locked on MX (he_running == false)
    assert(!he_cal_is_locked_decision(false, false, false));
    assert(!he_cal_is_locked_decision(false, true, false));
    assert(!he_cal_is_locked_decision(false, false, true));
    assert(!he_cal_is_locked_decision(false, true, true));

    // Locked when HE running and either key uncalibrated
    assert(he_cal_is_locked_decision(true, false, false));
    assert(he_cal_is_locked_decision(true, true, false));
    assert(he_cal_is_locked_decision(true, false, true));

    // Unlocked when both keys are calibrated
    assert(!he_cal_is_locked_decision(true, true, true));

    printf("✓ test_lock_decision passed\n");
}

static void test_lock_screen_mode(void)
{
    const int64_t wait_us = 5000000;

    // 1. Capable host connected -> USE_APP regardless of boot time
    assert(he_cal_lock_screen_mode(true, false, 0, wait_us) == HE_LOCK_SCREEN_USE_APP);
    assert(he_cal_lock_screen_mode(true, false, 6000000, wait_us) == HE_LOCK_SCREEN_USE_APP);

    // 2. No capable host (or old host with capability false) within 5 s window -> WAIT_FOR_APP
    assert(he_cal_lock_screen_mode(false, false, 0, wait_us) == HE_LOCK_SCREEN_WAIT_FOR_APP);
    assert(he_cal_lock_screen_mode(false, false, 4999999, wait_us) == HE_LOCK_SCREEN_WAIT_FOR_APP);

    // 3. Past 5 s window with no capable host -> NO_APP_BUTTON
    assert(he_cal_lock_screen_mode(false, false, 5000000, wait_us) == HE_LOCK_SCREEN_NO_APP_BUTTON);
    assert(he_cal_lock_screen_mode(false, false, 8000000, wait_us) == HE_LOCK_SCREEN_NO_APP_BUTTON);

    // 4. Capable host disconnects (even within 5 s window) -> NO_APP_BUTTON immediately
    assert(he_cal_lock_screen_mode(false, true, 1000000, wait_us) == HE_LOCK_SCREEN_NO_APP_BUTTON);

    printf("✓ test_lock_screen_mode passed\n");
}

static void test_input_suppression_decision(void)
{
    assert(!he_cal_suppress_input(false, false));
    assert(he_cal_suppress_input(true, false));
    assert(he_cal_suppress_input(false, true));
    assert(he_cal_suppress_input(true, true));

    printf("✓ test_input_suppression_decision passed\n");
}

static void test_start_refusals(void)
{
    const char *reason = NULL;

    // Refused on MX (he_running == false)
    assert(he_cal_can_start(false, true, false, &reason) == HE_CAL_START_REFUSED_NOT_HE);
    assert(reason != NULL);

    // Refused in PLAYING/COOLDOWN (is_idle == false)
    assert(he_cal_can_start(true, false, false, &reason) == HE_CAL_START_REFUSED_NOT_IDLE);
    assert(reason != NULL);

    // Refused while already calibrating
    assert(he_cal_can_start(true, true, true, &reason) == HE_CAL_START_REFUSED_BUSY);
    assert(reason != NULL);

    // Allowed when HE running, IDLE, and not already calibrating
    assert(he_cal_can_start(true, true, false, &reason) == HE_CAL_START_OK);
    assert(reason == NULL);

    printf("✓ test_start_refusals passed\n");
}

static void test_failures_and_cancel_keep_old_calibration(void)
{
    he_calibration_t saved[2] = {
        {.rest = 2050, .full = 2500, .valid = true},
        {.rest = 2060, .full = 2510, .valid = true},
    };

    he_cal_runner_t runner;
    he_cal_runner_init(&runner, NULL);

    // 1. Failure during rest leaves saved calibration untouched
    he_cal_runner_start(&runner);
    for (int i = 0; i < 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2080);
    }
    for (int i = 0; i < 64; i++) {
        he_cal_runner_feed(&runner, (i < 32) ? 2000 : 2100, 2080);
    }
    assert(runner.stage == HE_RUNNER_FAILED);
    assert(!he_cal_runner_apply_to_saved(&runner, saved));
    assert(saved[0].rest == 2050 && saved[0].full == 2500 && saved[0].valid);
    assert(saved[1].rest == 2060 && saved[1].full == 2510 && saved[1].valid);

    // 2. Failure during Key 2 (after Key 1 succeeded) leaves saved calibration untouched
    he_cal_runner_start(&runner);
    for (int i = 0; i < 64 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2090);
    }
    for (int p = 0; p < 5; p++) {
        simulate_stroke(&runner, 1, 2080, 2550);
    }
    assert(runner.stage == HE_RUNNER_KEY2);
    // Inconsistent peaks on Key 2 (diff from median > 120)
    simulate_stroke(&runner, 2, 2090, 2300);
    simulate_stroke(&runner, 2, 2090, 2700);
    simulate_stroke(&runner, 2, 2090, 2500);
    simulate_stroke(&runner, 2, 2090, 2500);
    simulate_stroke(&runner, 2, 2090, 2500);
    assert(runner.stage == HE_RUNNER_FAILED);
    assert(runner.error == HE_CAL_ERR_PEAKS_INCONSISTENT);
    assert(!he_cal_runner_apply_to_saved(&runner, saved));
    assert(saved[0].rest == 2050 && saved[0].full == 2500 && saved[0].valid);
    assert(saved[1].rest == 2060 && saved[1].full == 2510 && saved[1].valid);

    // 3. Cancel / app disconnect mid-flow leaves saved calibration untouched
    he_cal_runner_start(&runner);
    for (int i = 0; i < 64 + 64; i++) {
        he_cal_runner_feed(&runner, 2080, 2090);
    }
    simulate_stroke(&runner, 1, 2080, 2550);
    he_cal_runner_cancel(&runner);
    assert(runner.stage == HE_RUNNER_IDLE);
    assert(!he_cal_runner_apply_to_saved(&runner, saved));
    assert(saved[0].rest == 2050 && saved[0].full == 2500 && saved[0].valid);
    assert(saved[1].rest == 2060 && saved[1].full == 2510 && saved[1].valid);

    printf("✓ test_failures_and_cancel_keep_old_calibration passed\n");
}

int main(void)
{
    printf("=== Running Hall Effect Calibration Runner Host Tests ===\n");
    test_full_successful_run();
    test_rest_noisy_fails();
    test_span_too_small_fails();
    test_cancel();
    test_lock_decision();
    test_lock_screen_mode();
    test_input_suppression_decision();
    test_start_refusals();
    test_failures_and_cancel_keep_old_calibration();
    printf("All Hall Effect calibration runner tests passed successfully!\n");
    return 0;
}
