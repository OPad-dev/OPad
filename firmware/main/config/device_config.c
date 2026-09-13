#include "device_config.h"
#include "input/keypad.h"
#include "input/debounce.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "runtime/runtime.h"
#include "counters/counters.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_log.h"
#include <stdio.h>
#include <string.h>

static const char *TAG = "dev_config";
static const char *NVS_NAMESPACE = "osupad_cfg";
static const char *NVS_KEY = "cfg";

static device_config_data_t s_current_config = {
    .version = DEVICE_CONFIG_VERSION,
    .key1_usage = DEVICE_CONFIG_DEFAULT_KEY1,
    .key2_usage = DEVICE_CONFIG_DEFAULT_KEY2,
    .debounce_us = DEVICE_CONFIG_DEFAULT_DEBOUNCE_US,
    .brightness = DEVICE_CONFIG_DEFAULT_BRIGHTNESS,
    .sleep_s = DEVICE_CONFIG_DEFAULT_SLEEP_S,
};

static bool s_dirty = false;

static void set_defaults(device_config_data_t *cfg)
{
    cfg->version = DEVICE_CONFIG_VERSION;
    cfg->key1_usage = DEVICE_CONFIG_DEFAULT_KEY1;
    cfg->key2_usage = DEVICE_CONFIG_DEFAULT_KEY2;
    cfg->debounce_us = DEVICE_CONFIG_DEFAULT_DEBOUNCE_US;
    cfg->brightness = DEVICE_CONFIG_DEFAULT_BRIGHTNESS;
    cfg->sleep_s = DEVICE_CONFIG_DEFAULT_SLEEP_S;
}


static esp_err_t write_to_nvs(const device_config_data_t *cfg)
{
    // STRICT SPEC INVARIANT: zero NVS writes during PLAYING and COOLDOWN
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "NVS write blocked: state != IDLE (deferred to supervisor)");
        s_dirty = true;
        return ESP_OK;
    }

    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s': %s", NVS_NAMESPACE, esp_err_to_name(err));
        return err;
    }

    err = nvs_set_blob(handle, NVS_KEY, cfg, sizeof(*cfg));
    if (err == ESP_OK) {
        err = nvs_commit(handle);
        if (err == ESP_OK) {
            s_dirty = false;
            counters_record_nvs_write();
            ESP_LOGI(TAG, "Device config persisted to NVS: K1=0x%02lx, K2=0x%02lx, debounce=%lu us, brightness=%lu%%, sleep=%lu s",
                     (unsigned long)cfg->key1_usage, (unsigned long)cfg->key2_usage,
                     (unsigned long)cfg->debounce_us, (unsigned long)cfg->brightness,
                     (unsigned long)cfg->sleep_s);
        }
    }
    nvs_close(handle);
    return err;
}

void device_config_apply(const device_config_data_t *cfg)
{
    if (!cfg) return;

    // Apply keycodes and debounce to keypad
    keypad_config_t k_cfg = {
        .keycode1 = (uint8_t)cfg->key1_usage,
        .keycode2 = (uint8_t)cfg->key2_usage,
        .debounce_us = cfg->debounce_us,
    };
    keypad_set_config(&k_cfg);
    usb_hid_set_keycodes(k_cfg.keycode1, k_cfg.keycode2);

    // Apply brightness and sleep to board and UI
    board_backlight_set((uint8_t)cfg->brightness);
    ui_set_brightness((uint8_t)cfg->brightness);
    ui_set_sleep_timeout(cfg->sleep_s);
}

esp_err_t device_config_init(void)
{
    // Initialize NVS if needed
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        nvs_flash_erase();
        err = nvs_flash_init();
    }
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "NVS flash init failed in config: %s (using defaults)", esp_err_to_name(err));
        set_defaults(&s_current_config);
        device_config_apply(&s_current_config);
        return ESP_OK;
    }

    nvs_handle_t handle;
    err = nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle);
    if (err == ESP_OK) {
        device_config_data_t loaded;
        size_t len = sizeof(loaded);
        err = nvs_get_blob(handle, NVS_KEY, &loaded, &len);
        nvs_close(handle);

        if (err == ESP_OK && len == sizeof(loaded) && loaded.version == DEVICE_CONFIG_VERSION &&
            device_config_validate(&loaded, NULL, 0)) {
            s_current_config = loaded;
            ESP_LOGI(TAG, "Loaded config from NVS: K1=0x%02lx, K2=0x%02lx, debounce=%lu us, brightness=%lu%%, sleep=%lu s",
                     (unsigned long)loaded.key1_usage, (unsigned long)loaded.key2_usage,
                     (unsigned long)loaded.debounce_us, (unsigned long)loaded.brightness,
                     (unsigned long)loaded.sleep_s);
        } else {
            ESP_LOGW(TAG, "NVS config missing or invalid, using defaults");
            set_defaults(&s_current_config);
        }
    } else {
        ESP_LOGI(TAG, "No NVS config found, using defaults");
        set_defaults(&s_current_config);
    }

    device_config_apply(&s_current_config);
    return ESP_OK;
}

void device_config_get(device_config_data_t *out_cfg)
{
    if (out_cfg) {
        *out_cfg = s_current_config;
    }
}

esp_err_t device_config_set(const device_config_data_t *cfg)
{
    if (!cfg) return ESP_ERR_INVALID_ARG;

    s_current_config = *cfg;
    device_config_apply(&s_current_config);

    if (runtime_get_state() == OSUPAD_STATE_IDLE) {
        return write_to_nvs(&s_current_config);
    } else {
        s_dirty = true;
        ESP_LOGI(TAG, "Config applied in RAM; NVS write deferred until IDLE");
        return ESP_OK;
    }
}

esp_err_t device_config_flush(void)
{
    if (!s_dirty) {
        return ESP_OK;
    }
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK;
    }
    ESP_LOGI(TAG, "Flushing deferred config to NVS");
    return write_to_nvs(&s_current_config);
}
