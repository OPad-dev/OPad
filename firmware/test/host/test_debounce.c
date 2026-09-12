#include <stdio.h>
#include <assert.h>
#include "../../main/input/debounce.h"

static void test_clean_press_release(void)
{
    debounce_state_t state;
    debounce_init(&state, false);

    // Press at t = 1000 us
    debounce_action_t act = debounce_step(&state, 1000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_PRESS);
    assert(state.accepted_pressed == true);
    assert(state.lockout_end_us == 4000);

    // Resample at t = 4000 us (pin still pressed)
    act = debounce_step(&state, 4000, true, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    assert(state.accepted_pressed == true);

    // Release at t = 50000 us
    act = debounce_step(&state, 50000, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_RELEASE);
    assert(state.accepted_pressed == false);
    assert(state.lockout_end_us == 53000);

    // Resample at t = 53000 us (pin still released)
    act = debounce_step(&state, 53000, false, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    assert(state.accepted_pressed == false);

    printf("✓ test_clean_press_release passed\n");
}

static void test_bounce_burst_on_press(void)
{
    debounce_state_t state;
    debounce_init(&state, false);

    // Initial edge: press at t = 1000 us
    debounce_action_t act = debounce_step(&state, 1000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_PRESS);
    assert(state.lockout_end_us == 4000);

    // Bounce bursts during lockout: 1100, 1200, 1500 us
    act = debounce_step(&state, 1100, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    act = debounce_step(&state, 1200, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    act = debounce_step(&state, 1500, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    act = debounce_step(&state, 1700, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);

    // Settle at true; resample at t = 4000 us
    act = debounce_step(&state, 4000, true, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    assert(state.accepted_pressed == true);

    printf("✓ test_bounce_burst_on_press passed\n");
}

static void test_bounce_burst_on_release(void)
{
    debounce_state_t state;
    debounce_init(&state, true);

    // Initial release edge at t = 10000 us
    debounce_action_t act = debounce_step(&state, 10000, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_RELEASE);
    assert(state.lockout_end_us == 13000);

    // Bounces during lockout: 10100, 10300 us
    act = debounce_step(&state, 10100, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    act = debounce_step(&state, 10300, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);

    // Resample at t = 13000 us (settled on released)
    act = debounce_step(&state, 13000, false, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    assert(state.accepted_pressed == false);

    printf("✓ test_bounce_burst_on_release passed\n");
}

static void test_glitch_shorter_than_lockout(void)
{
    debounce_state_t state;
    debounce_init(&state, false);

    // Noise glitch triggers press at t = 1000 us
    debounce_action_t act = debounce_step(&state, 1000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_PRESS);
    assert(state.accepted_pressed == true);
    assert(state.lockout_end_us == 4000);

    // Glitch ends at t = 1050 us (pin returns to false inside lockout)
    act = debounce_step(&state, 1050, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    // Key is still marked accepted_pressed = true inside lockout
    assert(state.accepted_pressed == true);

    // Resample at end of lockout t = 4000 us: pin is false!
    act = debounce_step(&state, 4000, false, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_RELEASE);
    assert(state.accepted_pressed == false); // Corrected to released!

    printf("✓ test_glitch_shorter_than_lockout passed\n");
}

static void test_tap_shorter_than_lockout(void)
{
    debounce_state_t state;
    debounce_init(&state, false);

    // Fast finger tap: press at t = 1000 us
    debounce_action_t act = debounce_step(&state, 1000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_PRESS);
    assert(state.lockout_end_us == 4000);

    // Release at t = 2500 us (shorter than 3000 us lockout)
    act = debounce_step(&state, 2500, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(act == DEBOUNCE_ACTION_NONE);
    assert(state.accepted_pressed == true);

    // Resample at t = 4000 us: pin is released!
    act = debounce_step(&state, 4000, false, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(act == DEBOUNCE_ACTION_RELEASE);
    assert(state.accepted_pressed == false); // Key is NOT stuck down!

    printf("✓ test_tap_shorter_than_lockout passed\n");
}

static void test_interleaved_keys(void)
{
    debounce_state_t k1, k2;
    debounce_init(&k1, false);
    debounce_init(&k2, false);

    // Key 1 pressed at t = 1000
    debounce_action_t a1 = debounce_step(&k1, 1000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(a1 == DEBOUNCE_ACTION_PRESS);

    // Key 2 pressed at t = 2000 (while Key 1 is in lockout)
    debounce_action_t a2 = debounce_step(&k2, 2000, true, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(a2 == DEBOUNCE_ACTION_PRESS);
    assert(k1.accepted_pressed == true);
    assert(k2.accepted_pressed == true);

    // Resample K1 at t = 4000
    a1 = debounce_step(&k1, 4000, true, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(a1 == DEBOUNCE_ACTION_NONE);

    // Release K1 at t = 4500
    a1 = debounce_step(&k1, 4500, false, DEBOUNCE_SOURCE_EDGE, 3000);
    assert(a1 == DEBOUNCE_ACTION_RELEASE);

    // Resample K2 at t = 5000
    a2 = debounce_step(&k2, 5000, true, DEBOUNCE_SOURCE_RESAMPLE, 3000);
    assert(a2 == DEBOUNCE_ACTION_NONE);

    assert(k1.accepted_pressed == false);
    assert(k2.accepted_pressed == true);

    printf("✓ test_interleaved_keys passed\n");
}

int main(void)
{
    test_clean_press_release();
    test_bounce_burst_on_press();
    test_bounce_burst_on_release();
    test_glitch_shorter_than_lockout();
    test_tap_shorter_than_lockout();
    test_interleaved_keys();
    printf("All debounce unit tests passed successfully!\n");
    return 0;
}
