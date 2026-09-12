#include "counters.h"
#include "input/keypad.h"
#include "runtime/runtime.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_log.h"
#include <string.h>

static const char *TAG = "counters";
static const char *NVS_NAMESPACE = "osupad";

static uint32_t s_generation = 1;
static uint64_t s_last_saved_key1 = 0;
static uint64_t s_last_saved_key2 = 0;
static bool s_initialized = false;

esp_err_t counters_init(void)
{
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW(TAG, "Erasing corrupted/outdated NVS flash...");
        ESP_ERROR_CHECK(nvs_flash_erase());
        err = nvs_flash_init();
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to initialize NVS flash: %s", esp_err_to_name(err));
        return err;
    }

    nvs_handle_t handle;
    err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s': %s", NVS_NAMESPACE, esp_err_to_name(err));
        return err;
    }

    err = nvs_get_u32(handle, "generation", &s_generation);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        s_generation = 1;
        nvs_set_u32(handle, "generation", s_generation);
    }

    uint64_t k1 = 0;
    err = nvs_get_u64(handle, "key1_cnt", &k1);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        k1 = 0;
        nvs_set_u64(handle, "key1_cnt", k1);
    }

    uint64_t k2 = 0;
    err = nvs_get_u64(handle, "key2_cnt", &k2);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        k2 = 0;
        nvs_set_u64(handle, "key2_cnt", k2);
    }

    nvs_commit(handle);
    nvs_close(handle);

    keypad_set_lifetime_presses(k1, k2);

    s_last_saved_key1 = k1;
    s_last_saved_key2 = k2;
    s_initialized = true;

    ESP_LOGI(TAG, "Counters initialized: Gen=%lu, Key1=%llu, Key2=%llu",
             (unsigned long)s_generation,
             (unsigned long long)k1,
             (unsigned long long)k2);
    return ESP_OK;
}

void counters_get(counters_snapshot_t *snapshot)
{
    if (!snapshot) {
        return;
    }

    uint64_t ram_k1 = 0;
    uint64_t ram_k2 = 0;
    keypad_get_lifetime_presses(&ram_k1, &ram_k2);

    snapshot->generation = s_generation;
    snapshot->lifetime_key1 = ram_k1;
    snapshot->lifetime_key2 = ram_k2;
}

esp_err_t counters_sync_from_host(uint32_t generation, uint64_t k1, uint64_t k2, bool force)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Cannot sync counters while active (state != IDLE)");
        return ESP_ERR_INVALID_STATE;
    }

    // Monotonicity / generation validation unless forced
    counters_snapshot_t current;
    counters_get(&current);

    if (!force) {
        if (generation < current.generation) {
            ESP_LOGW(TAG, "Rejected sync: generation (%lu) < current (%lu)",
                     (unsigned long)generation, (unsigned long)current.generation);
            return ESP_ERR_INVALID_ARG;
        }
        if (generation == current.generation && (k1 < current.lifetime_key1 || k2 < current.lifetime_key2)) {
            ESP_LOGW(TAG, "Rejected sync: non-monotonic counters on same generation");
            return ESP_ERR_INVALID_ARG;
        }
    }

    s_generation = generation;
    keypad_set_lifetime_presses(k1, k2);

    return counters_checkpoint(true);
}

esp_err_t counters_checkpoint(bool force)
{
    if (!s_initialized) {
        return ESP_ERR_INVALID_STATE;
    }

    // STRICT SPEC INVARIANT: Zero flash writes during gameplay and cooldown
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK; // Silently skip write
    }

    counters_snapshot_t current;
    counters_get(&current);

    if (!force && current.lifetime_key1 == s_last_saved_key1 && current.lifetime_key2 == s_last_saved_key2) {
        return ESP_OK; // Clean, no writes needed
    }

    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS for checkpoint: %s", esp_err_to_name(err));
        return err;
    }

    nvs_set_u32(handle, "generation", current.generation);
    nvs_set_u64(handle, "key1_cnt", current.lifetime_key1);
    nvs_set_u64(handle, "key2_cnt", current.lifetime_key2);
    nvs_commit(handle);
    nvs_close(handle);

    s_last_saved_key1 = current.lifetime_key1;
    s_last_saved_key2 = current.lifetime_key2;

    ESP_LOGI(TAG, "Counters checkpointed to NVS: Gen=%lu, K1=%llu, K2=%llu",
             (unsigned long)current.generation,
             (unsigned long long)current.lifetime_key1,
             (unsigned long long)current.lifetime_key2);
    return ESP_OK;
}

esp_err_t counters_reset(void)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_ERR_INVALID_STATE;
    }

    s_generation++;
    keypad_set_lifetime_presses(0, 0);

    return counters_checkpoint(true);
}
