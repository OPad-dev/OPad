#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <pthread.h>
#include <time.h>
#include "input/he_cal.h"
#include "input/he_cal_record.h"
#include "runtime/runtime.h"
#include "nvs.h"

// Only the platform boundary is replaced: tests execute the real he_cal.c
// transactions, validation and publication, backed by two in-memory slots.
static he_cal_record_t slots[2];
static bool valid[2];
static int fail_at; // 1 = open, 2 = set_blob, 3 = commit
static unsigned writes;
static bool exercise_concurrency;
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
    assert(h == NVS_READWRITE && len == sizeof(he_cal_record_t));
    if (fail_at == 2) return ESP_FAIL;
    // IDF v5.5.2 set_blob writes immediately; commit is currently a no-op
    // for a valid handle (nvs_api.cpp / nvs_handle_simple.cpp).
    int slot = strcmp(key, "cal_a") == 0 ? 0 : 1;
    memcpy(&slots[slot], in, len);
    valid[slot] = true;
    assert(he_cal_record_validate(&slots[slot]));
    if (exercise_concurrency) {
        const struct timespec delay = {.tv_nsec = 1000000};
        nanosleep(&delay, NULL); // expose interleaved set_blob/commit transactions
    }
    return ESP_OK;
}

esp_err_t nvs_commit(nvs_handle_t h)
{
    assert(h == NVS_READWRITE);
    if (fail_at == 3) return ESP_FAIL;
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

static void *save_repeatedly(void *calibration)
{
    for (int i = 0; i < 10; i++) assert(he_cal_save(calibration) == ESP_OK);
    return NULL;
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

    unsigned expected_writes = 1;
    for (int failure = 1; failure <= 3; failure++) {
        fail_at = failure;
        assert(he_cal_save(next) == ESP_FAIL);
        expect_active(old);
        assert(!he_cal_is_dirty());
        assert(he_cal_flush() == ESP_OK);
        assert(writes == expected_writes);
        assert(he_cal_init() == ESP_OK);
        if (failure == 3) {
            // Defensive injected commit error: RAM stayed old, but set_blob
            // had already persisted a valid newer record. Do not promise
            // rollback across reboot merely because commit returned error.
            expect_active(next);
            fail_at = 0;
            assert(he_cal_save(old) == ESP_OK);
            expected_writes++;
        } else {
            expect_active(old);
        }
    }
    fail_at = 0;
    mode = OSUPAD_STATE_PLAYING;
    assert(he_cal_save(next) == ESP_ERR_INVALID_STATE);
    mode = OSUPAD_STATE_COOLDOWN;
    assert(he_cal_save(next) == ESP_ERR_INVALID_STATE);
    expect_active(old);
    assert(writes == expected_writes);
    mode = OSUPAD_STATE_IDLE;
    assert(he_cal_save(next) == ESP_OK);
    assert(writes == ++expected_writes);
    assert(valid[0] && valid[1]);
    assert(he_cal_init() == ESP_OK);
    expect_active(next);

    // Existing deferred API still flushes once IDLE, serialized with saves.
    mode = OSUPAD_STATE_PLAYING;
    assert(he_cal_set(old) == ESP_OK && he_cal_is_dirty());
    assert(he_cal_flush() == ESP_OK && writes == expected_writes);
    mode = OSUPAD_STATE_IDLE;
    assert(he_cal_flush() == ESP_OK && writes == ++expected_writes);
    assert(!he_cal_is_dirty());
    assert(he_cal_init() == ESP_OK);
    expect_active(old);
    uint32_t before_seq = slots[0].seq > slots[1].seq ? slots[0].seq : slots[1].seq;
    exercise_concurrency = true;
    pthread_t writers[2];
    assert(pthread_create(&writers[0], NULL, save_repeatedly, (void *)old) == 0);
    assert(pthread_create(&writers[1], NULL, save_repeatedly, (void *)next) == 0);
    for (int i = 0; i < 2; i++) assert(pthread_join(writers[i], NULL) == 0);
    assert(writes == expected_writes + 20);
    assert(he_cal_record_validate(&slots[0]) && he_cal_record_validate(&slots[1]));
    assert((slots[0].seq == before_seq + 20 && slots[1].seq == before_seq + 19) ||
           (slots[1].seq == before_seq + 20 && slots[0].seq == before_seq + 19));
    he_calibration_t latest[2];
    he_cal_get_all(latest);
    assert(he_cal_init() == ESP_OK);
    expect_active(latest);
    puts("Calibration storage preserves RAM on errors; reboot follows persisted records.");
    return 0;
}
