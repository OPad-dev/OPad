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
static bool s_counters_nvs_ok = false;
static uint32_t s_nvs_writes = 0;

bool counters_is_nvs_ok(void)
{
    return s_counters_nvs_ok;
}

uint32_t counters_get_nvs_writes(void)
{
    return s_nvs_writes;
}

esp_err_t counters_init(void)
{
#if defined(CONFIG_OSUPAD_TEST_FAIL_NVS) || defined(OSUPAD_TEST_FAIL_NVS)
    ESP_LOGW(TAG, "FORCED NVS FAILURE (OSUPAD_TEST_FAIL_NVS active)");
    s_counters_nvs_ok = false;
    return ESP_FAIL;
#endif

    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW(TAG, "Erasing corrupted/outdated NVS flash...");
        esp_err_t erase_err = nvs_flash_erase();
        if (erase_err != ESP_OK) {
            ESP_LOGE(TAG, "Failed to erase NVS: %s", esp_err_to_name(erase_err));
            s_counters_nvs_ok = false;
            return erase_err;
        }
        err = nvs_flash_init();
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to initialize NVS flash: %s (continuing with RAM counters)", esp_err_to_name(err));
        s_counters_nvs_ok = false;
        return err;
    }

    nvs_handle_t handle;
    err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s': %s (continuing with RAM counters)", NVS_NAMESPACE, esp_err_to_name(err));
        s_counters_nvs_ok = false;
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

    // Keypad starts with RAM counters at 0. counters_init adds NVS values
    // (keypad_add_lifetime_presses) rather than overwriting, so any keypresses
    // that occurred between HID-ready and counters_init are preserved.
    keypad_add_lifetime_presses(k1, k2);

    s_last_saved_key1 = k1;
    s_last_saved_key2 = k2;
    s_initialized = true;
    s_counters_nvs_ok = true;

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

esp_err_t counters_sync_from_host(uint32_t generation, uint64_t k1, uint64_t k2, bool force, char *err_msg, size_t err_msg_len)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Cannot sync counters while active (state != IDLE)");
        if (err_msg && err_msg_len > 0) {
            snprintf(err_msg, err_msg_len, "not idle");
        }
        return ESP_ERR_INVALID_STATE;
    }

    if (!s_counters_nvs_ok) {
        ESP_LOGW(TAG, "Cannot sync counters: NVS unavailable");
        if (err_msg && err_msg_len > 0) {
            snprintf(err_msg, err_msg_len, "nvs unavailable");
        }
        return ESP_ERR_NOT_SUPPORTED;
    }

    // Monotonicity / generation validation unless forced
    counters_snapshot_t current;
    counters_get(&current);

    if (!force) {
        if (generation < current.generation) {
            ESP_LOGW(TAG, "Rejected sync: generation (%lu) < current (%lu)",
                     (unsigned long)generation, (unsigned long)current.generation);
            if (err_msg && err_msg_len > 0) {
                snprintf(err_msg, err_msg_len, "stale generation");
            }
            return ESP_ERR_INVALID_ARG;
        }
        if (generation == current.generation && (k1 < current.lifetime_key1 || k2 < current.lifetime_key2)) {
            ESP_LOGW(TAG, "Rejected sync: non-monotonic counters on same generation");
            if (err_msg && err_msg_len > 0) {
                snprintf(err_msg, err_msg_len, "non-monotonic");
            }
            return ESP_ERR_INVALID_ARG;
        }
    }

    s_generation = generation;
    keypad_set_lifetime_presses(k1, k2);

    if (err_msg && err_msg_len > 0) {
        err_msg[0] = '\0';
    }

    return counters_checkpoint(true);
}

esp_err_t counters_checkpoint(bool force)
{
    if (!s_initialized || !s_counters_nvs_ok) {
        return ESP_OK; // Silently skip write if uninitialized or NVS failed
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

    s_nvs_writes++;
    s_last_saved_key1 = current.lifetime_key1;
    s_last_saved_key2 = current.lifetime_key2;

    ESP_LOGI(TAG, "Counters checkpointed to NVS: Gen=%lu, K1=%llu, K2=%llu (total writes=%lu)",
             (unsigned long)current.generation,
             (unsigned long long)current.lifetime_key1,
             (unsigned long long)current.lifetime_key2,
             (unsigned long)s_nvs_writes);
    return ESP_OK;
}

esp_err_t counters_reset(void)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_ERR_INVALID_STATE;
    }
    if (!s_counters_nvs_ok) {
        return ESP_ERR_NOT_SUPPORTED;
    }

    s_generation++;
    keypad_set_lifetime_presses(0, 0);

    return counters_checkpoint(true);
}
