#pragma once

/*
 * Pure-C Hall Effect health monitoring and back-off state machine.
 * docs/specs/v2-hall-effect-decisions.md D1, OPEN G.
 *
 * Decisions:
 * 1. Key fault: a key without a valid conversion for HE_STALL_US is faulted
 *    and released. Once readings return, it must be seen up (he_rt_fault)
 *    before it can press again.
 * 2. ADC stall: no DMA frames for HE_STALL_US faults both keys and triggers
 *    an ADC restart with exponential back-off (10 ms -> 1 s).
 * 3. Module probe pause: pausing the ADC to read module ID is not counted
 *    as an ADC stall.
 * 4. Module absence: both keys faulted for HE_ABSENT_US indicates module removed.
 * 5. Back-off reset: 1 second of continuous healthy ADC operation resets back-off.
 *
 * Pure C, no ESP-IDF/FreeRTOS, fully host-tested.
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// PROVISIONAL (OPEN G): to be validated under display, USB and flash load
#define HE_STALL_US               5000
#define HE_RESTART_BACKOFF_MIN_US 10000
#define HE_RESTART_BACKOFF_MAX_US 1000000
#define HE_ABSENT_US              100000
#define HE_HEALTHY_RESET_US       1000000

typedef struct {
    int64_t last_frame_us;
    int64_t last_valid_us[2];
    bool    key_faulted[2];
    int64_t fault_since_us[2];

    bool    adc_ok;
    int64_t backoff_us;
    int64_t retry_at_us;
    int64_t healthy_since_us;
} he_health_t;

typedef struct {
    bool adc_stalled;
    bool key_fault_started[2];
    bool key_fault_cleared[2];
} he_health_check_result_t;

/**
 * Initialize health state machine.
 */
void he_health_init(he_health_t *h, int64_t now);

/**
 * Resync health: marks all readings fresh from now on.
 */
void he_health_resync(he_health_t *h, int64_t now);

/**
 * Inlined ISR hooks to update timestamps without function call overhead.
 */
static inline void he_health_record_frame(he_health_t *h, int64_t now)
{
    h->last_frame_us = now;
}

static inline void he_health_record_valid(he_health_t *h, int k, int64_t now)
{
    h->last_valid_us[k] = now;
}

static inline bool he_health_is_key_faulted(const he_health_t *h, int k)
{
    return h->key_faulted[k];
}

/**
 * Periodic health check (runs in he_task).
 */
he_health_check_result_t he_health_check(he_health_t *h, int64_t now);

/**
 * Check if both keys have been absent/faulted long enough to count as unplugged.
 */
bool he_health_keys_absent(const he_health_t *h, int64_t now);

/**
 * Handle ADC stall detected: schedules restart retry.
 */
void he_health_on_adc_stalled(he_health_t *h, int64_t now);

/**
 * Check if an ADC restart should be attempted.
 */
bool he_health_should_retry_adc(const he_health_t *h, int64_t now);

/**
 * Record result of an ADC restart attempt.
 */
void he_health_on_adc_restart_result(he_health_t *h, int64_t now, bool success);

/**
 * Periodic back-off step: resets back-off after 1s of healthy operation.
 */
void he_health_step_backoff(he_health_t *h, int64_t now);

/**
 * Module probe handshake done: updates frame timestamp so pause is not a stall.
 */
void he_health_on_probe_done(he_health_t *h, int64_t now);

#ifdef __cplusplus
}
#endif
