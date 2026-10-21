/*
 * Hall Effect decision engine (input/he_rt.c) and settings builder
 * (input/he_params.c). Exact DOWN/UP sequences on synthetic traces.
 *
 * Rest/full come from the V1.1 bench (key 1: 2084 -> 2530 counts,
 * docs/specs/v2-hall-effect-design.md §10b) and are test data only. TEST_FLOOR
 * is NOT a validated noise floor: it stands in for whatever raw-conversion
 * traces decide (OPEN A). Tests that need that number are marked PENDING and
 * skipped.
 */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include "../../main/input/he_rt.h"
#include "../../main/input/he_params.h"

#define REST 2084
#define FULL 2530
#define TEST_FLOOR 32       // test-only stand-in, see above
#define TEST_TOP_FLOOR 32   // test-only stand-in (OPEN D)

static const he_limits_t LIM = {.min_movement = TEST_FLOOR, .min_top_reset = TEST_TOP_FLOOR};
static const he_calibration_t CAL = {.rest = REST, .full = FULL, .valid = true};

// Deterministic noise, uniform in [-amp, amp]
static uint32_t s_lcg = 12345;
static int32_t noise(int32_t amp)
{
    s_lcg = s_lcg * 1103515245u + 12345u;
    if (amp == 0) {
        return 0;
    }
    return (int32_t)((s_lcg >> 8) % (uint32_t)(2 * amp + 1)) - amp;
}

typedef struct {
    int downs;
    int ups;
    int first_down_at;  // sample index, -1 = none
    int first_up_at;
} tally_t;

static void tally(tally_t *t, he_rt_event_t e, int i)
{
    if (e == HE_RT_DOWN) {
        if (t->downs++ == 0) {
            t->first_down_at = i;
        }
    } else if (e == HE_RT_UP) {
        if (t->ups++ == 0) {
            t->first_up_at = i;
        }
    }
}

// Steps from `from` to `to` (exclusive of from), `n` samples, linear
static void ramp(he_rt_key_t *k, tally_t *t, int32_t from, int32_t to, int n, int32_t amp)
{
    for (int i = 1; i <= n; i++) {
        int32_t v = from + (to - from) * i / n + noise(amp);
        tally(t, he_rt_step(k, v), i);
    }
}

static void hold(he_rt_key_t *k, tally_t *t, int32_t at, int n, int32_t amp)
{
    for (int i = 0; i < n; i++) {
        tally(t, he_rt_step(k, at + noise(amp)), i);
    }
}

static tally_t fresh(void)
{
    return (tally_t){.first_down_at = -1, .first_up_at = -1};
}

static he_rt_params_t build(he_key_settings_t s)
{
    he_rt_params_t p;
    uint32_t r = he_params_build(&s, &CAL, &LIM, &p);
    assert(!HE_PARAMS_FAILED(r));
    return p;
}

static he_key_settings_t plain_settings(void)
{
    return (he_key_settings_t){.actuation = 500, .release_hyst = 100, .top_reset = 100};
}

static he_key_settings_t rt_settings(bool continuous)
{
    return (he_key_settings_t){
        .actuation = 400, .release_hyst = 100, .top_reset = 150,
        .rt_press = 150, .rt_release = 150,  // 67 counts each at this span
        .rapid_trigger = true, .continuous = continuous,
    };
}

// A key seen at rest once, so it is armed
static void start_at_rest(he_rt_key_t *k, he_rt_params_t p)
{
    he_rt_init(k, &p);
    assert(he_rt_step(k, REST) == HE_RT_NONE);
}

// 1. An untouched key never presses
static void test_untouched_never_presses(void)
{
    he_rt_key_t k;
    start_at_rest(&k, build(plain_settings()));
    tally_t t = fresh();
    hold(&k, &t, REST, 200000, 12);  // ~24 count p-p, the worst 1 ms-average spread seen
    assert(t.downs == 0 && t.ups == 0);

    start_at_rest(&k, build(rt_settings(true)));
    t = fresh();
    hold(&k, &t, REST, 200000, 12);
    assert(t.downs == 0 && t.ups == 0);
    printf("✓ test_untouched_never_presses passed\n");
}

// 2, 3. A full press is one DOWN, its release one UP, at the thresholds
static void test_full_press_and_release(void)
{
    he_rt_params_t p = build(plain_settings());
    assert(p.actuation == REST + 223 && p.release == REST + 178);
    he_rt_key_t k;
    start_at_rest(&k, p);
    tally_t t = fresh();
    ramp(&k, &t, REST, FULL, 446, 0);  // one count per sample
    assert(t.downs == 1 && t.ups == 0);
    assert(t.first_down_at == 223);    // the first sample at actuation
    t = fresh();
    ramp(&k, &t, FULL, REST, 446, 0);
    assert(t.downs == 0 && t.ups == 1);
    assert(t.first_up_at == 446 - 178);
    printf("✓ test_full_press_and_release passed\n");
}

// 4, 5. A still key keeps its state, in every mode
static void test_stationary_keeps_state(void)
{
    // Held at mid travel, noise below every threshold distance
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, build(rt_settings(false)));
    ramp(&k, &t, REST, REST + 300, 300, 0);
    assert(t.downs == 1 && he_rt_is_down(&k));
    t = fresh();
    hold(&k, &t, REST + 300, 100000, 15);
    assert(t.downs == 0 && t.ups == 0 && he_rt_is_down(&k));

    // Rapid-trigger released and left there
    t = fresh();
    ramp(&k, &t, REST + 300, REST + 220, 80, 0);
    assert(t.ups == 1 && k.state == HE_RT_RT_RELEASED);
    t = fresh();
    hold(&k, &t, REST + 220, 100000, 15);
    assert(t.downs == 0 && t.ups == 0 && k.state == HE_RT_RT_RELEASED);
    printf("✓ test_stationary_keeps_state passed\n");
}

// 6, 7. Movement below sensitivity does nothing; a reversal by it does, on time
static void test_reversals(void)
{
    he_rt_params_t p = build(rt_settings(false));
    assert(p.rt_press == 67 && p.rt_release == 67);
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, p);
    ramp(&k, &t, REST, REST + 400, 400, 0);
    assert(t.downs == 1);

    t = fresh();
    ramp(&k, &t, REST + 400, REST + 334, 66, 0);   // 66 up: one short
    ramp(&k, &t, REST + 334, REST + 400, 66, 0);
    assert(t.downs == 0 && t.ups == 0);

    t = fresh();
    ramp(&k, &t, REST + 400, REST + 300, 100, 0);
    assert(t.ups == 1 && t.first_up_at == 67);      // peak 400, released at 333

    t = fresh();
    ramp(&k, &t, REST + 300, REST + 366, 66, 0);    // trough 300, 66 down: one short
    assert(t.downs == 0);
    ramp(&k, &t, REST + 366, REST + 367, 1, 0);
    assert(t.downs == 1);
    printf("✓ test_reversals passed\n");
}

// 9. Continuous: re-presses near the top, resets at top_reset, no phantom there
static void test_continuous_top_reset(void)
{
    he_rt_params_t p = build(rt_settings(true));
    assert(p.top_reset == REST + 67 && p.actuation == REST + 178);
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, p);
    ramp(&k, &t, REST, REST + 300, 300, 0);
    ramp(&k, &t, REST + 300, REST + 100, 200, 0);   // above actuation's release, still eligible
    assert(t.downs == 1 && t.ups == 1 && k.state == HE_RT_RT_RELEASED);
    t = fresh();
    ramp(&k, &t, REST + 100, REST + 170, 70, 0);    // re-press below actuation: continuous
    assert(t.downs == 1);
    t = fresh();
    ramp(&k, &t, REST + 170, REST, 170, 0);         // back to the top
    assert(t.ups == 1 && t.downs == 0 && k.state == HE_RT_ARMED);
    t = fresh();
    hold(&k, &t, REST + 40, 50000, 20);             // wander around the top
    assert(t.downs == 0 && t.ups == 0);
    t = fresh();
    ramp(&k, &t, REST, REST + 170, 170, 0);         // needs actuation again
    assert(t.downs == 0);
    ramp(&k, &t, REST + 170, REST + 178, 8, 0);
    assert(t.downs == 1);
    printf("✓ test_continuous_top_reset passed\n");
}

// 10. Normal rapid trigger ends where the actuation region ends
static void test_normal_rt_region(void)
{
    he_rt_params_t p = build(rt_settings(false));
    assert(p.actuation == REST + 178 && p.release == REST + 133);
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, p);
    ramp(&k, &t, REST, REST + 300, 300, 0);
    ramp(&k, &t, REST + 300, REST + 140, 160, 0);
    assert(t.downs == 1 && t.ups == 1 && k.state == HE_RT_RT_RELEASED);
    ramp(&k, &t, REST + 140, REST + 133, 7, 0);     // out of the region
    assert(k.state == HE_RT_ARMED && t.ups == 1);
    t = fresh();
    ramp(&k, &t, REST + 133, REST + 177, 44, 0);    // > rt_press of movement, short of actuation
    assert(t.downs == 0);
    ramp(&k, &t, REST + 177, REST + 178, 1, 0);
    assert(t.downs == 1);
    printf("✓ test_normal_rt_region passed\n");
}

// 11. Two keys, interleaved, do not affect each other
static void test_keys_independent(void)
{
    he_rt_key_t a, b;
    start_at_rest(&a, build(rt_settings(false)));
    start_at_rest(&b, build(plain_settings()));
    tally_t ta = fresh(), tb = fresh();
    for (int i = 1; i <= 2000; i++) {
        int32_t pa = REST + (i % 400 < 200 ? (i % 200) * 2 : 400 - (i % 200) * 2);  // fast taps
        int32_t pb = i < 1000 ? REST + 400 : REST;                                    // one long hold
        tally(&ta, he_rt_step(&a, pa), i);
        tally(&tb, he_rt_step(&b, pb), i);
    }
    assert(tb.downs == 1 && tb.ups == 1);
    assert(ta.downs == 5 && ta.ups == 5);
    printf("✓ test_keys_independent passed\n");
}

// 12. A press far past the calibrated full press still releases by rt_release
static void test_hard_press_no_bottom_lock(void)
{
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, build(rt_settings(false)));
    ramp(&k, &t, REST, FULL + 200, 646, 0);         // ~+568 mV hard press
    t = fresh();
    ramp(&k, &t, FULL + 200, FULL + 100, 100, 0);
    assert(t.ups == 1 && t.first_up_at == 67);
    printf("✓ test_hard_press_no_bottom_lock passed\n");
}

// Bottom dead zone: noise past its cap never releases, coming out of it does
static void test_bottom_dead_zone(void)
{
    he_key_settings_t s = rt_settings(false);
    s.bottom_dz = 100;
    he_rt_params_t p = build(s);
    assert(p.peak_cap == FULL - 45);
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, p);
    ramp(&k, &t, REST, FULL + 150, 596, 0);
    t = fresh();
    ramp(&k, &t, FULL + 150, FULL - 45, 195, 0);    // 195 counts up, all inside the zone
    assert(t.ups == 0);
    ramp(&k, &t, FULL - 45, FULL - 45 - 67, 67, 0);
    assert(t.ups == 1);
    printf("✓ test_bottom_dead_zone passed\n");
}

// 13. Threshold changes never press a key; a held key keeps its press
static void test_param_change_no_phantom(void)
{
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, build(plain_settings()));

    // Up but resting halfway (below actuation 223); actuation moves below it
    ramp(&k, &t, REST, REST + 200, 200, 0);
    assert(t.downs == 0);
    he_key_settings_t low = plain_settings();
    low.actuation = 300;  // 134 counts
    he_rt_params_t lp = build(low);
    he_rt_set_params(&k, &lp, REST + 200);
    t = fresh();
    hold(&k, &t, REST + 200, 1000, 0);
    assert(t.downs == 0);                          // needs to come up first
    ramp(&k, &t, REST + 200, REST, 200, 0);
    ramp(&k, &t, REST, REST + 134, 134, 0);
    assert(t.downs == 1);

    // Held down: the change waits for the release
    he_key_settings_t high = plain_settings();
    high.actuation = 900;
    he_rt_params_t hp = build(high);
    he_rt_set_params(&k, &hp, REST + 134);
    assert(he_rt_is_down(&k) && k.has_pending);
    t = fresh();
    hold(&k, &t, REST + 134, 1000, 0);
    assert(t.ups == 0);                            // old release (89) still applies
    ramp(&k, &t, REST + 134, REST, 134, 0);
    assert(t.ups == 1 && !k.has_pending && k.p.actuation == hp.actuation);

    // Rapid trigger turned on while held: no event at all until the key moves
    he_rt_params_t rp = build(rt_settings(false));
    t = fresh();
    ramp(&k, &t, REST, REST + 450, 450, 0);
    assert(t.downs == 1);
    he_rt_set_params(&k, &rp, REST + 450);
    t = fresh();
    hold(&k, &t, REST + 450, 1000, 10);
    assert(t.downs == 0 && t.ups == 0);
    printf("✓ test_param_change_no_phantom passed\n");
}

static void test_param_reload_preserves_release_requirement(void)
{
    for (int mode = 0; mode < 3; mode++) {
        he_rt_params_t p = build(mode == 0 ? plain_settings() : rt_settings(mode == 2));
        for (int was_down = 0; was_down < 2; was_down++) {
            he_rt_key_t k;
            he_rt_init(&k, &p);
            // Boot/reload must also wait for an actual post-reload sample.
            he_rt_set_params(&k, &p, REST);
            assert(k.need_release);
            assert(he_rt_step(&k, REST + 450) == HE_RT_NONE);
            assert(he_rt_step(&k, REST) == HE_RT_NONE);
            if (was_down) assert(he_rt_step(&k, REST + 450) == HE_RT_DOWN);
            assert(he_rt_fault(&k) == (was_down ? HE_RT_UP : HE_RT_NONE));
            assert(k.need_release);
            // The backend can retain a pre-fault rest reading across ADC
            // restart/resync. Repeated reloads cannot use it to re-arm.
            for (int reload = 0; reload < 3; reload++) {
                he_rt_set_params(&k, &p, REST);
                assert(k.need_release);
            }
            for (int frame = 0; frame < 20; frame++) {
                assert(he_rt_step(&k, REST + 450) == HE_RT_NONE);
            }
            assert(he_rt_step(&k, REST) == HE_RT_NONE);
            assert(!k.need_release);
            assert(he_rt_step(&k, REST + 450) == HE_RT_DOWN);
        }
    }
    printf("✓ test_param_reload_preserves_release_requirement passed\n");
}

// 14. A fault releases a held key; recovery does not press it again
static void test_fault_releases_no_phantom(void)
{
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, build(rt_settings(false)));
    ramp(&k, &t, REST, REST + 400, 400, 0);
    assert(he_rt_is_down(&k));
    assert(he_rt_fault(&k) == HE_RT_UP);
    assert(he_rt_fault(&k) == HE_RT_NONE);
    t = fresh();
    hold(&k, &t, REST + 400, 1000, 10);            // still held after recovery
    assert(t.downs == 0);
    ramp(&k, &t, REST + 400, REST, 400, 0);
    ramp(&k, &t, REST, REST + 400, 400, 0);
    assert(t.downs == 1 && t.ups == 0);
    printf("✓ test_fault_releases_no_phantom passed\n");
}

// 16 (engine side). Booting with a key held is no press until it comes up
static void test_boot_held(void)
{
    he_rt_key_t k;
    he_rt_params_t p = build(plain_settings());
    he_rt_init(&k, &p);
    tally_t t = fresh();
    hold(&k, &t, REST + 400, 1000, 10);
    assert(t.downs == 0 && t.ups == 0);
    ramp(&k, &t, REST + 400, REST, 400, 0);
    ramp(&k, &t, REST, REST + 400, 400, 0);
    assert(t.downs == 1);
    printf("✓ test_boot_held passed\n");
}

// Plain mode: a slow press parked on the actuation point does not chatter
static void test_slow_press_no_chatter(void)
{
    he_rt_key_t k;
    tally_t t = fresh();
    start_at_rest(&k, build(plain_settings()));
    ramp(&k, &t, REST, REST + 223, 2230, 0);
    hold(&k, &t, REST + 223, 100000, 20);          // ±20 < 45 counts of hysteresis
    assert(t.downs == 1 && t.ups == 0);
    printf("✓ test_slow_press_no_chatter passed\n");
}

// 18. Settings and calibration that cannot work are refused or clamped
static void test_params_validation(void)
{
    he_rt_params_t p = {.actuation = -1};
    he_key_settings_t s = rt_settings(false);
    he_calibration_t flat = {.rest = 2000, .full = 2000, .valid = true};
    assert(HE_PARAMS_FAILED(he_params_build(&s, &flat, &LIM, &p)) && p.actuation == -1);
    he_calibration_t inverted = {.rest = 2500, .full = 2000, .valid = true};
    assert(HE_PARAMS_FAILED(he_params_build(&s, &inverted, &LIM, &p)));
    he_calibration_t over = {.rest = 2000, .full = 5000, .valid = true};
    assert(HE_PARAMS_FAILED(he_params_build(&s, &over, &LIM, &p)));
    he_limits_t zero = {0};
    assert(HE_PARAMS_FAILED(he_params_build(&s, &CAL, &zero, &p)));

    // Rapid trigger without calibration: plain mode, and it says so
    he_calibration_t guess = CAL;
    guess.valid = false;
    uint32_t r = he_params_build(&s, &guess, &LIM, &p);
    assert(!HE_PARAMS_FAILED(r) && (r & HE_PARAMS_RT_NEEDS_CALIBRATION));
    assert(!p.rapid_trigger && !p.continuous);

    // Continuous needs rapid trigger
    s = plain_settings();
    s.continuous = true;
    assert(!build(s).continuous);

    // Sensitivities below the floor are raised, and reported
    s = rt_settings(false);
    s.rt_press = 1;
    s.rt_release = 0;
    s.release_hyst = 0;
    r = he_params_build(&s, &CAL, &LIM, &p);
    assert(r & HE_PARAMS_RT_PRESS_RAISED && r & HE_PARAMS_RT_RELEASE_RAISED && r & HE_PARAMS_HYST_RAISED);
    assert(p.rt_press == TEST_FLOOR && p.rt_release == TEST_FLOOR && p.actuation - p.release == TEST_FLOOR);

    // Out-of-range percentages, actuation at the very top
    s = (he_key_settings_t){.actuation = 0, .release_hyst = 60000, .top_reset = 0, .bottom_dz = 65535};
    r = he_params_build(&s, &CAL, &LIM, &p);
    assert(r & HE_PARAMS_SETTING_CLAMPED && r & HE_PARAMS_ACTUATION_RAISED && r & HE_PARAMS_TOP_RESET_RAISED);

    // Property: every setting combination keeps the engine's ordering
    for (uint32_t i = 0; i < 200000; i++) {
        s = (he_key_settings_t){
            .actuation = (uint16_t)noise(700) + 700, .release_hyst = (uint16_t)(noise(700) + 700),
            .rt_press = (uint16_t)(noise(700) + 700), .rt_release = (uint16_t)(noise(700) + 700),
            .top_reset = (uint16_t)(noise(700) + 700), .bottom_dz = (uint16_t)(noise(700) + 700),
            .rapid_trigger = noise(1) > 0, .continuous = noise(1) > 0,
        };
        he_calibration_t c = {.rest = 1500 + noise(500), .valid = noise(1) >= 0};
        c.full = c.rest + 1 + (noise(600) + 600);
        if (c.full > 4095) {
            c.full = 4095;
        }
        if (HE_PARAMS_FAILED(he_params_build(&s, &c, &LIM, &p))) {
            assert(c.full <= c.rest);
            continue;
        }
        assert(p.top_reset >= c.rest + LIM.min_top_reset);
        assert(p.top_reset <= p.release && p.release < p.actuation);
        assert(p.actuation - p.release >= LIM.min_movement);
        assert(p.actuation <= p.peak_cap);
        assert(p.rt_press >= LIM.min_movement && p.rt_release >= LIM.min_movement);
        assert(!p.continuous || p.rapid_trigger);
        assert(!p.rapid_trigger || c.valid);
    }
    printf("✓ test_params_validation passed\n");
}

static void pending(const char *name, const char *why)
{
    printf("- %s PENDING: %s\n", name, why);
}

int main(void)
{
    test_param_reload_preserves_release_requirement();
    test_untouched_never_presses();
    test_full_press_and_release();
    test_stationary_keeps_state();
    test_reversals();
    test_continuous_top_reset();
    test_normal_rt_region();
    test_keys_independent();
    test_hard_press_no_bottom_lock();
    test_bottom_dead_zone();
    test_param_change_no_phantom();
    test_fault_releases_no_phantom();
    test_boot_held();
    test_slow_press_no_chatter();
    test_params_validation();
    pending("continuous_off_by_default", "product defaults are set with the calibration manager (Phase 5)");
    pending("noise_spikes_at_validated_settings", "needs the raw-conversion noise floor (OPEN A)");
    pending("recorded_trace_replay", "needs raw 250 us frame traces from the pad");
    printf("All HE engine tests passed\n");
    return 0;
}
