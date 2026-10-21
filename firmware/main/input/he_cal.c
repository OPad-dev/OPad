#include "he_cal.h"
#include "he_cal_record.h"
#include "he_backend.h"
#include "runtime/runtime.h"
#include "counters/counters.h"
#include "esp_log.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "freertos/FreeRTOS.h"
#include "freertos/portmacro.h"
#include "freertos/semphr.h"
#include <string.h>

static const char *TAG = "he_cal";
static const char *NVS_NAMESPACE = "osupad_cal";
static const char *NVS_KEY_A = "cal_a";
static const char *NVS_KEY_B = "cal_b";

static portMUX_TYPE s_cal_lock = portMUX_INITIALIZER_UNLOCKED;
static he_calibration_t s_active_cal[2];
static int s_active_slot = -1;  // 0 = A, 1 = B, -1 = neither
static uint32_t s_active_seq = 0;
static bool s_dirty = false;
// NVS runs outside the spinlock; serialize whole transactions with the
// runtime supervisor's deferred flushes. No ISR or key task takes this mutex.
static StaticSemaphore_t s_write_mutex_buf;
static SemaphoreHandle_t s_write_mutex;

static void set_factory_defaults_locked(void)
{
    for (int k = 0; k < 2; k++) {
        s_active_cal[k].rest = HE_CAL_FACTORY_REST;
        s_active_cal[k].full = HE_CAL_FACTORY_FULL;
        s_active_cal[k].valid = false;
    }
    s_active_slot = -1;
    s_active_seq = 0;
    s_dirty = false;
}

esp_err_t he_cal_init(void)
{
    if (!s_write_mutex) s_write_mutex = xSemaphoreCreateMutexStatic(&s_write_mutex_buf);
    portENTER_CRITICAL(&s_cal_lock);
    set_factory_defaults_locked();
    portEXIT_CRITICAL(&s_cal_lock);

    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle);
    if (err != ESP_OK) {
        ESP_LOGI(TAG, "No NVS calibration namespace found (%s), using factory defaults", esp_err_to_name(err));
        return ESP_OK;
    }

    he_cal_record_t slot_a = {0};
    he_cal_record_t slot_b = {0};
    size_t len_a = sizeof(slot_a);
    size_t len_b = sizeof(slot_b);

    bool a_valid = false;
    if (nvs_get_blob(handle, NVS_KEY_A, &slot_a, &len_a) == ESP_OK && len_a == sizeof(slot_a)) {
        a_valid = he_cal_record_validate(&slot_a);
    }

    bool b_valid = false;
    if (nvs_get_blob(handle, NVS_KEY_B, &slot_b, &len_b) == ESP_OK && len_b == sizeof(slot_b)) {
        b_valid = he_cal_record_validate(&slot_b);
    }
    nvs_close(handle);

    int active = he_cal_record_select_active(&slot_a, a_valid, &slot_b, b_valid);
    const he_cal_record_t *chosen = active == 0 ? &slot_a : active == 1 ? &slot_b : NULL;
    he_calibration_t loaded[2];
    portENTER_CRITICAL(&s_cal_lock);
    if (chosen) {
        he_cal_record_unpack(chosen, s_active_cal);
        s_active_slot = active;
        s_active_seq = chosen->seq;
    } else {
        set_factory_defaults_locked();
    }
    loaded[0] = s_active_cal[0];
    loaded[1] = s_active_cal[1];
    portEXIT_CRITICAL(&s_cal_lock);

    // Logging takes locks, so never inside the critical section (a first
    // printf there aborts: newlib initialises its lock lazily)
    if (chosen) {
        ESP_LOGI(TAG, "Loaded calibration from slot %c (seq=%lu): K1=[%ld..%ld, v=%d], K2=[%ld..%ld, v=%d]",
                 active == 0 ? 'A' : 'B', (unsigned long)chosen->seq, (long)loaded[0].rest,
                 (long)loaded[0].full, loaded[0].valid, (long)loaded[1].rest, (long)loaded[1].full,
                 loaded[1].valid);
    } else {
        ESP_LOGI(TAG, "No valid calibration record in NVS, using factory defaults");
    }
    (void)loaded;  // host tests build ESP_LOGI as nothing

    return ESP_OK;
}

void he_cal_get(uint8_t key_index, he_calibration_t *out)
{
    if (key_index >= 2 || !out) {
        return;
    }
    portENTER_CRITICAL(&s_cal_lock);
    *out = s_active_cal[key_index];
    portEXIT_CRITICAL(&s_cal_lock);
}

void he_cal_get_all(he_calibration_t out[2])
{
    if (!out) {
        return;
    }
    portENTER_CRITICAL(&s_cal_lock);
    out[0] = s_active_cal[0];
    out[1] = s_active_cal[1];
    portEXIT_CRITICAL(&s_cal_lock);
}

bool he_cal_is_dirty(void)
{
    portENTER_CRITICAL(&s_cal_lock);
    bool dirty = s_dirty;
    portEXIT_CRITICAL(&s_cal_lock);
    return dirty;
}

static esp_err_t persist_locked(const he_calibration_t snapshot[2])
{
    portENTER_CRITICAL(&s_cal_lock);
    int cur_slot = s_active_slot;
    uint32_t cur_seq = s_active_seq;
    portEXIT_CRITICAL(&s_cal_lock);

    int next_slot = he_cal_record_next_slot(cur_slot);
    uint32_t next_seq = he_cal_record_next_seq(cur_slot, cur_seq);
    he_cal_record_t rec;
    he_cal_record_pack(&rec, next_seq, snapshot);

    const char *key = (next_slot == 0) ? NVS_KEY_A : NVS_KEY_B;
    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s' for writing: %s", NVS_NAMESPACE, esp_err_to_name(err));
        return err;
    }

    err = nvs_set_blob(handle, key, &rec, sizeof(rec));
    if (err == ESP_OK) {
        err = nvs_commit(handle);
        if (err == ESP_OK) {
            portENTER_CRITICAL(&s_cal_lock);
            s_active_slot = next_slot;
            s_active_seq = next_seq;
            portEXIT_CRITICAL(&s_cal_lock);
            counters_record_nvs_write();
            ESP_LOGI(TAG, "Calibration persisted to NVS slot %c (seq=%lu): K1=[%ld..%ld, v=%d], K2=[%ld..%ld, v=%d]",
                     (next_slot == 0) ? 'A' : 'B', (unsigned long)next_seq,
                     (long)snapshot[0].rest, (long)snapshot[0].full, snapshot[0].valid,
                     (long)snapshot[1].rest, (long)snapshot[1].full, snapshot[1].valid);
        } else {
            ESP_LOGE(TAG, "Failed to commit calibration to NVS: %s", esp_err_to_name(err));
        }
    } else {
        ESP_LOGE(TAG, "Failed to set calibration blob in NVS: %s", esp_err_to_name(err));
    }
    nvs_close(handle);
    return err;
}

static esp_err_t flush_locked(void)
{
    portENTER_CRITICAL(&s_cal_lock);
    bool dirty = s_dirty;
    he_calibration_t snapshot[2] = {s_active_cal[0], s_active_cal[1]};
    portEXIT_CRITICAL(&s_cal_lock);
    if (!dirty || runtime_get_state() != OSUPAD_STATE_IDLE) return ESP_OK;
    esp_err_t err = persist_locked(snapshot);
    if (err == ESP_OK) {
        portENTER_CRITICAL(&s_cal_lock);
        s_dirty = false;
        portEXIT_CRITICAL(&s_cal_lock);
    }
    return err;
}

esp_err_t he_cal_flush(void)
{
    xSemaphoreTake(s_write_mutex, portMAX_DELAY);
    esp_err_t err = flush_locked();
    xSemaphoreGive(s_write_mutex);
    return err;
}

static bool calibration_valid(const he_calibration_t in[2])
{
    if (!in) return false;
    for (int k = 0; k < 2; k++) {
        if (in[k].valid) {
            if (in[k].rest < 0 || in[k].rest > 4095 ||
                in[k].full < 0 || in[k].full > 4095 ||
                in[k].full <= in[k].rest) {
                return false;
            }
        }
    }
    return true;
}

esp_err_t he_cal_set(const he_calibration_t in[2])
{
    if (!calibration_valid(in)) return ESP_ERR_INVALID_ARG;
    xSemaphoreTake(s_write_mutex, portMAX_DELAY);

    portENTER_CRITICAL(&s_cal_lock);
    s_active_cal[0] = in[0];
    s_active_cal[1] = in[1];
    s_dirty = true;
    portEXIT_CRITICAL(&s_cal_lock);
    esp_err_t err = flush_locked();
    xSemaphoreGive(s_write_mutex);

#ifdef CONFIG_OSUPAD_HE_BACKEND
    if (he_backend_running()) {
        he_backend_reload_params();
    }
#endif

    return err;
}

esp_err_t he_cal_save(const he_calibration_t in[2])
{
    if (!calibration_valid(in)) return ESP_ERR_INVALID_ARG;
    xSemaphoreTake(s_write_mutex, portMAX_DELAY);
    esp_err_t err = runtime_get_state() == OSUPAD_STATE_IDLE ?
        persist_locked(in) : ESP_ERR_INVALID_STATE;
    if (err == ESP_OK) {
        // Publish only after both NVS calls succeed. An error keeps old RAM
        // and the other slot; set_blob may already have persisted the candidate,
        // so an error alone does not promise rollback across reboot.
        portENTER_CRITICAL(&s_cal_lock);
        s_active_cal[0] = in[0];
        s_active_cal[1] = in[1];
        s_dirty = false;
        portEXIT_CRITICAL(&s_cal_lock);
    }
    xSemaphoreGive(s_write_mutex);
    return err;
}
