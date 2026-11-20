#include "config/nvs_ns.h"
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
// V1 and V3 modules read different raw values, so each keeps its own
// calibration: a V1 calibration never drives a V3 module (he_cal_use_v3_store)
static nvs_ns_id_t s_namespace = NVS_NS_CAL;  // V1's, the one pads always had
// Keys 1 and 2 in one A/B record pair, as always. Key 3 (V3) in a second pair
// of the same record type (its keys[0]), so the first pair's format never
// changes and older firmware still reads keys 1 and 2.
typedef struct {
    const char *key_a;
    const char *key_b;
    int slot;       // 0 = A, 1 = B, -1 = neither
    uint32_t seq;
} cal_store_t;

static cal_store_t s_store12 = {.key_a = "cal_a", .key_b = "cal_b", .slot = -1};
static cal_store_t s_store3 = {.key_a = "cal3_a", .key_b = "cal3_b", .slot = -1};

static portMUX_TYPE s_cal_lock = portMUX_INITIALIZER_UNLOCKED;
static he_calibration_t s_active_cal[HE_KEYS_MAX];
static uint8_t s_keys = 2;  // 3 with he_cal_use_v3_store
static bool s_dirty = false;
// NVS runs outside the spinlock; serialize whole transactions with the
// runtime supervisor's deferred flushes. No ISR or key task takes this mutex.
static StaticSemaphore_t s_write_mutex_buf;
static SemaphoreHandle_t s_write_mutex;

static void set_factory_defaults_locked(void)
{
    for (int k = 0; k < HE_KEYS_MAX; k++) {
        s_active_cal[k].rest = HE_CAL_FACTORY_REST;
        s_active_cal[k].full = HE_CAL_FACTORY_FULL;
        s_active_cal[k].valid = false;
    }
    s_store12.slot = -1;
    s_store12.seq = 0;
    s_store3.slot = -1;
    s_store3.seq = 0;
    s_dirty = false;
}

void he_cal_use_v3_store(void)
{
    s_namespace = NVS_NS_CAL3;
    s_keys = 3;
}

uint8_t he_cal_key_count(void)
{
    return s_keys;
}

// The newest valid record of a store, or NULL
static const he_cal_record_t *load_store(nvs_handle_t handle, cal_store_t *st, he_cal_record_t slots[2])
{
    size_t len_a = sizeof(slots[0]);
    size_t len_b = sizeof(slots[1]);
    bool a_valid = nvs_get_blob(handle, st->key_a, &slots[0], &len_a) == ESP_OK &&
                   len_a == sizeof(slots[0]) && he_cal_record_validate(&slots[0]);
    bool b_valid = nvs_get_blob(handle, st->key_b, &slots[1], &len_b) == ESP_OK &&
                   len_b == sizeof(slots[1]) && he_cal_record_validate(&slots[1]);
    int active = he_cal_record_select_active(&slots[0], a_valid, &slots[1], b_valid);
    if (active < 0) {
        return NULL;
    }
    st->slot = active;
    st->seq = slots[active].seq;
    return &slots[active];
}

esp_err_t he_cal_init(void)
{
    if (!s_write_mutex) s_write_mutex = xSemaphoreCreateMutexStatic(&s_write_mutex_buf);
    portENTER_CRITICAL(&s_cal_lock);
    set_factory_defaults_locked();
    portEXIT_CRITICAL(&s_cal_lock);

    nvs_handle_t handle;
    esp_err_t err = nvs_open(nvs_ns_name(s_namespace), NVS_READONLY, &handle);
    if (err != ESP_OK) {
        ESP_LOGI(TAG, "No NVS calibration namespace found (%s), using factory defaults", esp_err_to_name(err));
        return ESP_OK;
    }

    he_cal_record_t slots12[2] = {0};
    he_cal_record_t slots3[2] = {0};
    const he_cal_record_t *rec12 = load_store(handle, &s_store12, slots12);
    const he_cal_record_t *rec3 = s_keys > 2 ? load_store(handle, &s_store3, slots3) : NULL;
    nvs_close(handle);

    he_calibration_t loaded[HE_KEYS_MAX];
    portENTER_CRITICAL(&s_cal_lock);
    if (rec12) {
        he_cal_record_unpack(rec12, s_active_cal);
    }
    if (rec3) {
        he_calibration_t k3[2];
        he_cal_record_unpack(rec3, k3);
        s_active_cal[2] = k3[0];
    }
    memcpy(loaded, s_active_cal, sizeof(loaded));
    portEXIT_CRITICAL(&s_cal_lock);

    // Logging takes locks, so never inside the critical section (a first
    // printf there aborts: newlib initialises its lock lazily)
    ESP_LOGI(TAG, "Calibration (%s): K1=[%ld..%ld, v=%d], K2=[%ld..%ld, v=%d], K3=[%ld..%ld, v=%d]%s",
             nvs_ns_name(s_namespace), (long)loaded[0].rest, (long)loaded[0].full, loaded[0].valid,
             (long)loaded[1].rest, (long)loaded[1].full, loaded[1].valid, (long)loaded[2].rest,
             (long)loaded[2].full, loaded[2].valid, rec12 ? "" : " (no keys 1-2 record)");
    (void)loaded;  // host tests build ESP_LOGI as nothing

    return ESP_OK;
}

void he_cal_get(uint8_t key_index, he_calibration_t *out)
{
    if (key_index >= HE_KEYS_MAX || !out) {
        return;
    }
    portENTER_CRITICAL(&s_cal_lock);
    *out = s_active_cal[key_index];
    portEXIT_CRITICAL(&s_cal_lock);
}

void he_cal_get_all(he_calibration_t out[HE_KEYS_MAX])
{
    if (!out) {
        return;
    }
    portENTER_CRITICAL(&s_cal_lock);
    memcpy(out, s_active_cal, sizeof(s_active_cal));
    portEXIT_CRITICAL(&s_cal_lock);
}

bool he_cal_is_dirty(void)
{
    portENTER_CRITICAL(&s_cal_lock);
    bool dirty = s_dirty;
    portEXIT_CRITICAL(&s_cal_lock);
    return dirty;
}

// Write one store's next A/B slot; its slot and seq move on success
static esp_err_t persist_store(nvs_handle_t handle, cal_store_t *st, const he_calibration_t pair[2])
{
    portENTER_CRITICAL(&s_cal_lock);
    int cur_slot = st->slot;
    uint32_t cur_seq = st->seq;
    portEXIT_CRITICAL(&s_cal_lock);

    int next_slot = he_cal_record_next_slot(cur_slot);
    uint32_t next_seq = he_cal_record_next_seq(cur_slot, cur_seq);
    he_cal_record_t rec;
    he_cal_record_pack(&rec, next_seq, pair);

    esp_err_t err = nvs_set_blob(handle, next_slot == 0 ? st->key_a : st->key_b, &rec, sizeof(rec));
    if (err == ESP_OK) {
        err = nvs_commit(handle);
    }
    if (err == ESP_OK) {
        portENTER_CRITICAL(&s_cal_lock);
        st->slot = next_slot;
        st->seq = next_seq;
        portEXIT_CRITICAL(&s_cal_lock);
        counters_record_nvs_write();
    } else {
        ESP_LOGE(TAG, "Failed to write calibration (%s): %s", st->key_a, esp_err_to_name(err));
    }
    return err;
}

static esp_err_t persist_locked(const he_calibration_t snapshot[HE_KEYS_MAX])
{
    nvs_handle_t handle;
    esp_err_t err = nvs_open(nvs_ns_name(s_namespace), NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s' for writing: %s", nvs_ns_name(s_namespace), esp_err_to_name(err));
        return err;
    }
    err = persist_store(handle, &s_store12, snapshot);
    if (err == ESP_OK && s_keys > 2) {
        const he_calibration_t k3[2] = {snapshot[2], snapshot[2]};
        err = persist_store(handle, &s_store3, k3);
    }
    nvs_close(handle);
    if (err == ESP_OK) {
        ESP_LOGI(TAG, "Calibration persisted (%s): K1=[%ld..%ld, v=%d], K2=[%ld..%ld, v=%d], K3=[%ld..%ld, v=%d]",
                 nvs_ns_name(s_namespace), (long)snapshot[0].rest, (long)snapshot[0].full, snapshot[0].valid,
                 (long)snapshot[1].rest, (long)snapshot[1].full, snapshot[1].valid,
                 (long)snapshot[2].rest, (long)snapshot[2].full, snapshot[2].valid);
    }
    return err;
}

static esp_err_t flush_locked(void)
{
    he_calibration_t snapshot[HE_KEYS_MAX];
    portENTER_CRITICAL(&s_cal_lock);
    bool dirty = s_dirty;
    memcpy(snapshot, s_active_cal, sizeof(snapshot));
    portEXIT_CRITICAL(&s_cal_lock);
    if (!dirty || runtime_get_state() != OPAD_STATE_IDLE) return ESP_OK;
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

static bool calibration_valid(const he_calibration_t in[HE_KEYS_MAX])
{
    if (!in) return false;
    for (int k = 0; k < s_keys; k++) {
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

esp_err_t he_cal_set(const he_calibration_t in[HE_KEYS_MAX])
{
    if (!calibration_valid(in)) return ESP_ERR_INVALID_ARG;
    xSemaphoreTake(s_write_mutex, portMAX_DELAY);

    portENTER_CRITICAL(&s_cal_lock);
    for (int k = 0; k < s_keys; k++) {
        s_active_cal[k] = in[k];
    }
    s_dirty = true;
    portEXIT_CRITICAL(&s_cal_lock);
    esp_err_t err = flush_locked();
    xSemaphoreGive(s_write_mutex);

#ifdef OPAD_HE_INPUT
    if (he_backend_running()) {
        he_backend_reload_params();
    }
#endif

    return err;
}

esp_err_t he_cal_save(const he_calibration_t in[HE_KEYS_MAX])
{
    if (!calibration_valid(in)) return ESP_ERR_INVALID_ARG;
    xSemaphoreTake(s_write_mutex, portMAX_DELAY);
    // Keys the module does not have keep what RAM holds
    he_calibration_t full[HE_KEYS_MAX];
    portENTER_CRITICAL(&s_cal_lock);
    memcpy(full, s_active_cal, sizeof(full));
    portEXIT_CRITICAL(&s_cal_lock);
    for (int k = 0; k < s_keys; k++) {
        full[k] = in[k];
    }
    esp_err_t err = runtime_get_state() == OPAD_STATE_IDLE ?
        persist_locked(full) : ESP_ERR_INVALID_STATE;
    if (err == ESP_OK) {
        // Publish only after both NVS calls succeed. An error keeps old RAM
        // and the other slot; set_blob may already have persisted the candidate,
        // so an error alone does not promise rollback across reboot.
        portENTER_CRITICAL(&s_cal_lock);
        memcpy(s_active_cal, full, sizeof(full));
        s_dirty = false;
        portEXIT_CRITICAL(&s_cal_lock);
    }
    xSemaphoreGive(s_write_mutex);
    return err;
}
