#include "input/he_health.h"

#include <stddef.h>

void he_health_init(he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return;
    }
    h->last_frame_us = now;
    h->keys = 2;
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        h->last_valid_us[k] = now;
        h->key_faulted[k] = false;
        h->fault_since_us[k] = 0;
    }
    h->adc_ok = false;
    h->backoff_us = HE_RESTART_BACKOFF_MIN_US;
    h->retry_at_us = 0;
    h->healthy_since_us = now;
}

void he_health_set_keys(he_health_t *h, uint8_t keys)
{
    if (h != NULL && keys >= 1 && keys <= HE_KEYS_MAX) {
        h->keys = keys;
    }
}

void he_health_resync(he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return;
    }
    h->last_frame_us = now;
    for (int k = 0; k < h->keys; k++) {
        h->last_valid_us[k] = now;
        h->key_faulted[k] = false;
    }
}

he_health_check_result_t he_health_check(he_health_t *h, int64_t now)
{
    he_health_check_result_t res = {0};
    if (h == NULL) {
        return res;
    }
    if (now - h->last_frame_us > HE_STALL_US) {
        res.adc_stalled = true;
        for (int k = 0; k < h->keys; k++) {
            if (!h->key_faulted[k]) {
                h->fault_since_us[k] = now;
                h->key_faulted[k] = true;
                res.key_fault_started[k] = true;
            }
        }
    } else {
        for (int k = 0; k < h->keys; k++) {
            bool stale = now - h->last_valid_us[k] > HE_STALL_US;
            if (stale && !h->key_faulted[k]) {
                h->fault_since_us[k] = now;
                h->key_faulted[k] = true;
                res.key_fault_started[k] = true;
            } else if (!stale && h->key_faulted[k]) {
                h->key_faulted[k] = false;
                res.key_fault_cleared[k] = true;
            }
        }
    }
    return res;
}

bool he_health_keys_absent(const he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return false;
    }
    // Every key faulted, the last of them for HE_ABSENT_US
    int64_t since = 0;
    for (int k = 0; k < h->keys; k++) {
        if (!h->key_faulted[k]) {
            return false;
        }
        if (h->fault_since_us[k] > since) {
            since = h->fault_since_us[k];
        }
    }
    return now - since >= HE_ABSENT_US;
}

void he_health_on_adc_stalled(he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return;
    }
    h->adc_ok = false;
    h->retry_at_us = now + h->backoff_us;
}

bool he_health_should_retry_adc(const he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return false;
    }
    return !h->adc_ok && now >= h->retry_at_us;
}

void he_health_on_adc_restart_result(he_health_t *h, int64_t now, bool success)
{
    if (h == NULL) {
        return;
    }
    if (success) {
        h->adc_ok = true;
        h->healthy_since_us = now;
        he_health_resync(h, now);
    } else {
        h->retry_at_us = now + h->backoff_us;
    }
    int64_t next = h->backoff_us * 2;
    h->backoff_us = next > HE_RESTART_BACKOFF_MAX_US ? HE_RESTART_BACKOFF_MAX_US : next;
}

void he_health_step_backoff(he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return;
    }
    if (h->adc_ok && h->backoff_us != HE_RESTART_BACKOFF_MIN_US &&
        now - h->healthy_since_us > HE_HEALTHY_RESET_US) {
        h->backoff_us = HE_RESTART_BACKOFF_MIN_US;
    }
}

void he_health_on_probe_done(he_health_t *h, int64_t now)
{
    if (h == NULL) {
        return;
    }
    h->last_frame_us = now;
}
