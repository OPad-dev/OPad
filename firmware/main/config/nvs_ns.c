#include "nvs_ns.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "esp_log.h"
#include "nvs.h"

static const char *TAG = "nvs_ns";

typedef struct {
    const char *name;
    const char *legacy;  // before the rename; read only, never written again
} ns_names_t;

static const ns_names_t NAMES[NVS_NS_COUNT] = {
    [NVS_NS_COUNTERS] = {"opad", "osupad"},
    [NVS_NS_CONFIG] = {"opad_cfg", "osupad_cfg"},
    [NVS_NS_CAL] = {"opad_cal", "osupad_cal"},
    [NVS_NS_CAL3] = {"opad_cal3", "osupad_cal3"},
    [NVS_NS_UI] = {"opad_ui", "osupad_ui"},
};

// Until migrated, a namespace is opened under its old name, so a pad whose
// copy failed keeps working exactly as before
static bool s_migrated[NVS_NS_COUNT];

const char *nvs_ns_name(nvs_ns_id_t id)
{
    if (id >= NVS_NS_COUNT) {
        return NULL;
    }
    return s_migrated[id] ? NAMES[id].name : NAMES[id].legacy;
}

const char *nvs_ns_legacy_name(nvs_ns_id_t id)
{
    return id < NVS_NS_COUNT ? NAMES[id].legacy : NULL;
}

static bool has_marker(nvs_handle_t h)
{
    uint8_t v = 0;
    return nvs_get_u8(h, NVS_NS_MIGRATED_KEY, &v) == ESP_OK && v == 1;
}

// Copies one entry of any type; buffers for strings and blobs are sized from NVS
static esp_err_t copy_entry(nvs_handle_t from, nvs_handle_t to, const char *key, nvs_type_t type)
{
    esp_err_t err;
    switch (type) {
#define COPY_INT(T, NVS_T, get, set) \
    case NVS_T: {                    \
        T v;                         \
        err = get(from, key, &v);    \
        return err == ESP_OK ? set(to, key, v) : err; \
    }
        COPY_INT(uint8_t, NVS_TYPE_U8, nvs_get_u8, nvs_set_u8)
        COPY_INT(int8_t, NVS_TYPE_I8, nvs_get_i8, nvs_set_i8)
        COPY_INT(uint16_t, NVS_TYPE_U16, nvs_get_u16, nvs_set_u16)
        COPY_INT(int16_t, NVS_TYPE_I16, nvs_get_i16, nvs_set_i16)
        COPY_INT(uint32_t, NVS_TYPE_U32, nvs_get_u32, nvs_set_u32)
        COPY_INT(int32_t, NVS_TYPE_I32, nvs_get_i32, nvs_set_i32)
        COPY_INT(uint64_t, NVS_TYPE_U64, nvs_get_u64, nvs_set_u64)
        COPY_INT(int64_t, NVS_TYPE_I64, nvs_get_i64, nvs_set_i64)
#undef COPY_INT
    case NVS_TYPE_STR: {
        size_t len = 0;
        err = nvs_get_str(from, key, NULL, &len);
        if (err != ESP_OK) {
            return err;
        }
        char *buf = malloc(len);
        if (!buf) {
            return ESP_ERR_NO_MEM;
        }
        err = nvs_get_str(from, key, buf, &len);
        if (err == ESP_OK) {
            err = nvs_set_str(to, key, buf);
        }
        free(buf);
        return err;
    }
    case NVS_TYPE_BLOB: {
        size_t len = 0;
        err = nvs_get_blob(from, key, NULL, &len);
        if (err != ESP_OK) {
            return err;
        }
        void *buf = malloc(len ? len : 1);
        if (!buf) {
            return ESP_ERR_NO_MEM;
        }
        err = nvs_get_blob(from, key, buf, &len);
        if (err == ESP_OK) {
            err = nvs_set_blob(to, key, buf, len);
        }
        free(buf);
        return err;
    }
    default:
        return ESP_ERR_NOT_SUPPORTED;
    }
}

// Copies legacy into name. On any failure the half-written new namespace is
// erased, so the next boot starts the copy again from scratch.
static esp_err_t migrate_one(const ns_names_t *ns)
{
    nvs_handle_t to;
    esp_err_t err = nvs_open(ns->name, NVS_READWRITE, &to);
    if (err != ESP_OK) {
        return err;
    }
    if (has_marker(to)) {
        nvs_close(to);
        return ESP_OK;
    }
    // A copy cut short by a reset left entries but no marker: start over
    err = nvs_erase_all(to);

    unsigned copied = 0;
    nvs_handle_t from;
    bool have_legacy = err == ESP_OK && nvs_open(ns->legacy, NVS_READONLY, &from) == ESP_OK;
    if (have_legacy) {
        nvs_iterator_t it = NULL;
        esp_err_t found = nvs_entry_find(NVS_DEFAULT_PART_NAME, ns->legacy, NVS_TYPE_ANY, &it);
        while (err == ESP_OK && found == ESP_OK) {
            nvs_entry_info_t info;
            nvs_entry_info(it, &info);
            err = copy_entry(from, to, info.key, info.type);
            if (err == ESP_OK) {
                copied++;
                found = nvs_entry_next(&it);
            } else {
                ESP_LOGE(TAG, "Copying %s/%s failed: %s", ns->legacy, info.key, esp_err_to_name(err));
            }
        }
        nvs_release_iterator(it);
        nvs_close(from);
    }
    if (err == ESP_OK) {
        err = nvs_commit(to);
    }
    // The marker goes in only after every entry is committed
    if (err == ESP_OK) {
        err = nvs_set_u8(to, NVS_NS_MIGRATED_KEY, 1);
    }
    if (err == ESP_OK) {
        err = nvs_commit(to);
    }
    if (err != ESP_OK) {
        nvs_erase_all(to);
        nvs_commit(to);
    } else if (copied > 0) {
        ESP_LOGI(TAG, "Copied %u entries from %s to %s", copied, ns->legacy, ns->name);
    }
    nvs_close(to);
    return err;
}

void nvs_ns_migrate(void)
{
    for (int i = 0; i < NVS_NS_COUNT; i++) {
        esp_err_t err = migrate_one(&NAMES[i]);
        s_migrated[i] = err == ESP_OK;
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "Keeping %s for now (copy to %s failed: %s)", NAMES[i].legacy,
                     NAMES[i].name, esp_err_to_name(err));
        }
    }
}
