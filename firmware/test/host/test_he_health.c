#include <assert.h>
#include <stdio.h>
#include <string.h>

#include "input/he_health.h"
#include "input/he_rt.h"
#include "input/he_params.h"

static void test_stall_detection(void)
{
    he_health_t h;
    he_health_init(&h, 1000);
    h.adc_ok = true;

    // Receive frames every 250 us up to 2000 us
    for (int64_t t = 1000; t <= 2000; t += 250) {
        he_health_record_frame(&h, t);
        he_health_record_valid(&h, 0, t);
        he_health_record_valid(&h, 1, t);
    }

    he_health_check_result_t res = he_health_check(&h, 2000);
    assert(!res.adc_stalled);
    assert(!res.key_fault_started[0] && !res.key_fault_started[1]);
    assert(!h.key_faulted[0] && !h.key_faulted[1]);

    // Advance to 2000 + 5000 = 7000 us (exactly HE_STALL_US: not yet stalled)
    res = he_health_check(&h, 7000);
    assert(!res.adc_stalled);

    // Advance 1 us further: 7001 us > HE_STALL_US -> stall triggered
    res = he_health_check(&h, 7001);
    assert(res.adc_stalled);
    assert(res.key_fault_started[0] && res.key_fault_started[1]);
    assert(h.key_faulted[0] && h.key_faulted[1]);

    // Handle ADC stall
    he_health_on_adc_stalled(&h, 7001);
    assert(!h.adc_ok);
    assert(h.retry_at_us == 7001 + HE_RESTART_BACKOFF_MIN_US);
    assert(!he_health_should_retry_adc(&h, 7001));
    assert(!he_health_should_retry_adc(&h, 7001 + HE_RESTART_BACKOFF_MIN_US - 1));
    assert(he_health_should_retry_adc(&h, 7001 + HE_RESTART_BACKOFF_MIN_US));
}

static void test_single_key_fault_and_recovery(void)
{
    he_health_t h;
    he_health_init(&h, 1000);
    h.adc_ok = true;

    // Both keys healthy at t=1000
    he_health_record_frame(&h, 1000);
    he_health_record_valid(&h, 0, 1000);
    he_health_record_valid(&h, 1, 1000);

    // DMA frames continue, but key 1 stops getting valid conversions
    for (int64_t t = 1250; t <= 6000; t += 250) {
        he_health_record_frame(&h, t);
        he_health_record_valid(&h, 0, t);
        // key 1 has no valid readings
    }

    // At t=6000, 5000 us elapsed since key 1 last valid
    he_health_check_result_t res = he_health_check(&h, 6000);
    assert(!res.adc_stalled);
    assert(!res.key_fault_started[0] && !res.key_fault_started[1]);
    assert(!h.key_faulted[0] && !h.key_faulted[1]);

    // At t=6001, key 1 exceeds HE_STALL_US
    he_health_record_frame(&h, 6001);
    he_health_record_valid(&h, 0, 6001);
    res = he_health_check(&h, 6001);
    assert(!res.adc_stalled);
    assert(!res.key_fault_started[0]);
    assert(res.key_fault_started[1]);
    assert(!h.key_faulted[0]);
    assert(h.key_faulted[1]);

    // Subsequent check while still faulted should not re-trigger key_fault_started
    res = he_health_check(&h, 7000);
    assert(!res.adc_stalled);
    assert(!res.key_fault_started[0] && !res.key_fault_started[1]);
    assert(h.key_faulted[1]);

    // Readings for key 1 resume at t=8000
    he_health_record_frame(&h, 8000);
    he_health_record_valid(&h, 0, 8000);
    he_health_record_valid(&h, 1, 8000);
    res = he_health_check(&h, 8000);
    assert(!res.adc_stalled);
    assert(res.key_fault_cleared[1]);
    assert(!h.key_faulted[1]);
}

static void test_recovery_requires_key_seen_up(void)
{
    // Build parameters: rest 2088, full 2596 (span 508)
    // actuation 500 -> 2088 + 254 = 2342
    // top_reset 100 -> 2088 + 50 = 2138
    he_key_settings_t settings = {
        .actuation = 500,
        .release_hyst = 100,
        .top_reset = 100,
        .rt_press = 150,
        .rt_release = 150,
    };
    he_calibration_t cal = {.rest = 2088, .full = 2596, .valid = true};
    he_limits_t limits = {.min_movement = 32, .min_top_reset = 32};
    he_rt_params_t params;
    uint32_t err = he_params_build(&settings, &cal, &limits, &params);
    assert(err == 0);

    he_rt_key_t key;
    he_rt_init(&key, &params);

    // Initial state: at rest 2088 -> armed
    he_rt_event_t ev = he_rt_step(&key, 2088);
    assert(ev == HE_RT_NONE);

    // Press down past actuation (2342) -> DOWN
    ev = he_rt_step(&key, 2400);
    assert(ev == HE_RT_DOWN);

    // Key faults while held down!
    ev = he_rt_fault(&key);
    assert(ev == HE_RT_UP); // Released immediately!

    // Readings resume, but finger is still holding switch down (2400)
    ev = he_rt_step(&key, 2400);
    assert(ev == HE_RT_NONE); // Must NOT press down!

    // Finger moves up partially to 2250 (still above top reset 2138)
    ev = he_rt_step(&key, 2250);
    assert(ev == HE_RT_NONE); // Still faulted, cannot press!

    // Finger fully releases switch, moving past top reset to 2100 (< 2138)
    ev = he_rt_step(&key, 2100);
    assert(ev == HE_RT_NONE); // Re-arms!

    // Now a new press down succeeds cleanly
    ev = he_rt_step(&key, 2400);
    assert(ev == HE_RT_DOWN);
}

static void test_backoff_growth_and_reset(void)
{
    he_health_t h;
    he_health_init(&h, 1000);
    assert(h.backoff_us == HE_RESTART_BACKOFF_MIN_US);

    // Simulate repeated restart failures
    int64_t now = 10000;
    he_health_on_adc_stalled(&h, now);
    assert(!h.adc_ok);
    assert(h.retry_at_us == now + 10000);

    // Attempt 1: fails
    now = h.retry_at_us;
    he_health_on_adc_restart_result(&h, now, false);
    assert(!h.adc_ok);
    assert(h.backoff_us == 20000);

    // Attempt 2: fails
    now = h.retry_at_us;
    he_health_on_adc_restart_result(&h, now, false);
    assert(h.backoff_us == 40000);

    // Continue until backoff hits maximum (1 s = 1,000,000 us)
    while (h.backoff_us < HE_RESTART_BACKOFF_MAX_US) {
        now = h.retry_at_us;
        he_health_on_adc_restart_result(&h, now, false);
    }
    assert(h.backoff_us == HE_RESTART_BACKOFF_MAX_US);

    // Failing again clamps at maximum and does not overflow
    now = h.retry_at_us;
    he_health_on_adc_restart_result(&h, now, false);
    assert(h.backoff_us == HE_RESTART_BACKOFF_MAX_US);

    // Restart succeeds
    now = h.retry_at_us;
    he_health_on_adc_restart_result(&h, now, true);
    assert(h.adc_ok);
    assert(h.healthy_since_us == now);
    // Backoff is still high immediately after restart
    assert(h.backoff_us == HE_RESTART_BACKOFF_MAX_US);

    // Healthy for 500 ms (< 1 s): backoff remains high
    he_health_step_backoff(&h, now + 500000);
    assert(h.backoff_us == HE_RESTART_BACKOFF_MAX_US);

    // Healthy for 1000001 us (> 1 s): backoff resets to minimum!
    he_health_step_backoff(&h, now + HE_HEALTHY_RESET_US + 1);
    assert(h.backoff_us == HE_RESTART_BACKOFF_MIN_US);
}

static void test_probe_pause_not_counted_as_stall(void)
{
    he_health_t h;
    he_health_init(&h, 1000);
    h.adc_ok = true;

    // DMA frame at t=1000
    he_health_record_frame(&h, 1000);
    he_health_record_valid(&h, 0, 1000);
    he_health_record_valid(&h, 1, 1000);

    // Probe pauses ADC for 20 ms (20,000 us >> HE_STALL_US)
    // During pause, check_health is skipped (serve_probe returns true)
    int64_t pause_end = 1000 + 20000;
    he_health_on_probe_done(&h, pause_end);

    // When check_health runs immediately after probe done, no stall occurs!
    he_health_check_result_t res = he_health_check(&h, pause_end + 100);
    assert(!res.adc_stalled);
    assert(!res.key_fault_started[0] && !res.key_fault_started[1]);
    assert(!h.key_faulted[0] && !h.key_faulted[1]);
}

static void test_keys_absent(void)
{
    he_health_t h;
    he_health_init(&h, 1000);
    h.adc_ok = true;

    // Neither key faulted -> not absent
    assert(!he_health_keys_absent(&h, 1000));

    // Fault key 0 at t=1000
    h.key_faulted[0] = true;
    h.fault_since_us[0] = 1000;
    assert(!he_health_keys_absent(&h, 200000)); // Only one key faulted -> not absent

    // Fault key 1 at t=2000
    h.key_faulted[1] = true;
    h.fault_since_us[1] = 2000;

    // Both faulted, but elapsed since most recent fault is 50 ms < 100 ms (HE_ABSENT_US)
    assert(!he_health_keys_absent(&h, 52000));

    // At 102000 us (100 ms after key 1 fault) -> absent!
    assert(he_health_keys_absent(&h, 102000));

    // Key 0 recovers -> no longer absent
    h.key_faulted[0] = false;
    assert(!he_health_keys_absent(&h, 103000));
}

int main(void)
{
    printf("Running test_stall_detection...\n");
    test_stall_detection();

    printf("Running test_single_key_fault_and_recovery...\n");
    test_single_key_fault_and_recovery();

    printf("Running test_recovery_requires_key_seen_up...\n");
    test_recovery_requires_key_seen_up();

    printf("Running test_backoff_growth_and_reset...\n");
    test_backoff_growth_and_reset();

    printf("Running test_probe_pause_not_counted_as_stall...\n");
    test_probe_pause_not_counted_as_stall();

    printf("Running test_keys_absent...\n");
    test_keys_absent();

    printf("All HE health tests passed.\n");
    return 0;
}
