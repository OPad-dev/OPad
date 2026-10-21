#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include "input/he_cal.h"
#include "input/he_cal_record.h"
#include "runtime/runtime.h"
#include "nvs.h"

// Only the platform boundary is replaced: tests execute the real he_cal.c
// transactions, validation and publication, backed by two in-memory slots.
static he_cal_record_t slots[2];
static bool valid[2];
static he_cal_record_t pending;
static int pending_slot;
static int fail_at; // 1 = open, 2 = set_blob, 3 = commit
static unsigned writes;
static osupad_state_t mode = OSUPAD_STATE_IDLE;

osupad_state_t runtime_get_state(void) { return mode; }
void counters_record_nvs_write(void) { writes++; }

esp_err_t nvs_open(const char *name, int access, nvs_handle_t *handle)
{
    assert(strcmp(name, "osupad_cal") == 0);
    *handle = access;
    return access == NVS_READWRITE && fail_at == 1 ? ESP_FAIL : ESP_OK;
}

esp_err_t nvs_get_blob(nvs_handle_t h, const char *key, void *out, size_t *len)
{
    assert(h == NVS_READONLY && *len == sizeof(he_cal_record_t));
    int slot = strcmp(key, "cal_a") == 0 ? 0 : 1;
    if (!valid[slot]) return ESP_FAIL;
    memcpy(out, &slots[slot], *len);
    return ESP_OK;
}

esp_err_t nvs_set_blob(nvs_handle_t h, const char *key, const void *in, size_t len)
{
    assert(h == NVS_READWRITE && len == sizeof(pending));
    if (fail_at == 2) return ESP_FAIL;
    pending_slot = strcmp(key, "cal_a") == 0 ? 0 : 1;
    memcpy(&pending, in, len);
    assert(he_cal_record_validate(&pending));
    return ESP_OK;
}

esp_err_t nvs_commit(nvs_handle_t h)
{
    assert(h == NVS_READWRITE);
    if (fail_at == 3) return ESP_FAIL;
    slots[pending_slot] = pending;
    valid[pending_slot] = true;
    return ESP_OK;
}

void nvs_close(nvs_handle_t h) { (void)h; }

static void expect_active(const he_calibration_t expected[2])
{
    he_calibration_t got[2];
    he_cal_get_all(got);
    for (int k = 0; k < 2; k++) {
        assert(got[k].rest == expected[k].rest);
        assert(got[k].full == expected[k].full);
        assert(got[k].valid == expected[k].valid);
    }
}

int main(void)
{
    const he_calibration_t old[2] = {{.rest = 2080, .full = 2540, .valid = true},
                                      {.rest = 2090, .full = 2580, .valid = true}};
    const he_calibration_t next[2] = {{.rest = 2086, .full = 2566, .valid = true},
                                       {.rest = 2087, .full = 2590, .valid = true}};
    assert(he_cal_init() == ESP_OK);
    he_calibration_t uncalibrated[2];
    he_cal_get_all(uncalibrated);
    assert(!uncalibrated[0].valid && !uncalibrated[1].valid);
    fail_at = 3;
    assert(he_cal_save(old) == ESP_FAIL);
    expect_active(uncalibrated);
    fail_at = 0;
    assert(he_cal_save(old) == ESP_OK);
    expect_active(old);
    assert(writes == 1);

    for (fail_at = 1; fail_at <= 3; fail_at++) {
        assert(he_cal_save(next) == ESP_FAIL);
        expect_active(old);
        assert(!he_cal_is_dirty());
        assert(he_cal_flush() == ESP_OK);
        assert(writes == 1);
        assert(he_cal_init() == ESP_OK);
        expect_active(old);
    }
    fail_at = 0;
    mode = OSUPAD_STATE_PLAYING;
    assert(he_cal_save(next) == ESP_ERR_INVALID_STATE);
    mode = OSUPAD_STATE_COOLDOWN;
    assert(he_cal_save(next) == ESP_ERR_INVALID_STATE);
    expect_active(old);
    assert(writes == 1);
    mode = OSUPAD_STATE_IDLE;
    assert(he_cal_save(next) == ESP_OK);
    assert(writes == 2);
    assert(valid[0] && valid[1]);
    assert(he_cal_init() == ESP_OK);
    expect_active(next);

    // Existing deferred API still flushes once IDLE, serialized with saves.
    mode = OSUPAD_STATE_PLAYING;
    assert(he_cal_set(old) == ESP_OK && he_cal_is_dirty());
    assert(he_cal_flush() == ESP_OK && writes == 2);
    mode = OSUPAD_STATE_IDLE;
    assert(he_cal_flush() == ESP_OK && writes == 3);
    assert(!he_cal_is_dirty());
    assert(he_cal_init() == ESP_OK);
    expect_active(old);
    puts("Calibration storage keeps old values after open/set/commit failures.");
    return 0;
}
