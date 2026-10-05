#include "device_config.h"
#include "input/keypad.h"
#include "input/debounce.h"
#include "input/touch_retry.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "runtime/runtime.h"
#include "counters/counters.h"
#include "nvs_flash.h"
#include "nvs.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "diag/diag.h"
#include <stddef.h>
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
    .gameplay_display_hz = DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ,
    .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
    .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
    .swipe_left_action = DEVICE_CONFIG_DEFAULT_SWIPE_LEFT,
    .swipe_right_action = DEVICE_CONFIG_DEFAULT_SWIPE_RIGHT,
    .swipe_up_action = DEVICE_CONFIG_DEFAULT_SWIPE_UP,
    .swipe_down_action = DEVICE_CONFIG_DEFAULT_SWIPE_DOWN,
};

// v1 blobs end before key1_gpio; they load with the default pins
#define DEVICE_CONFIG_V1_SIZE offsetof(device_config_data_t, key1_gpio)
// v2 blobs end before owner_id; they load unclaimed, which is correct — a pad
// that predates §W3-2 has never been claimed by anyone
#define DEVICE_CONFIG_V2_SIZE offsetof(device_config_data_t, owner_id)
// v3 blobs end before the swipe actions; they load with the defaults
#define DEVICE_CONFIG_V3_SIZE offsetof(device_config_data_t, swipe_left_action)
// v4 blobs end before the up/down actions; they load with the defaults
#define DEVICE_CONFIG_V4_SIZE offsetof(device_config_data_t, swipe_up_action)
// v5 blobs add one byte, "up/down inverted", read into swipe_up_action
#define DEVICE_CONFIG_V5_SIZE (DEVICE_CONFIG_V4_SIZE + 1)
// v6 blobs end before the swipe key modifiers; they load as none
#define DEVICE_CONFIG_V6_SIZE offsetof(device_config_data_t, swipe_mods)

static bool s_dirty = false;
// Guards s_current_config and s_dirty: written by the protocol task, read by the
// runtime task (flush) and others. Only copies happen under it, never NVS I/O.
static portMUX_TYPE s_config_lock = portMUX_INITIALIZER_UNLOCKED;

static device_config_data_t config_snapshot(void)
{
    portENTER_CRITICAL(&s_config_lock);
    device_config_data_t cfg = s_current_config;
    portEXIT_CRITICAL(&s_config_lock);
    return cfg;
}

static void config_store(const device_config_data_t *cfg)
{
    portENTER_CRITICAL(&s_config_lock);
    s_current_config = *cfg;
    portEXIT_CRITICAL(&s_config_lock);
}

static void set_dirty(bool dirty)
{
    portENTER_CRITICAL(&s_config_lock);
    s_dirty = dirty;
    portEXIT_CRITICAL(&s_config_lock);
}

static void set_defaults(device_config_data_t *cfg)
{
    cfg->version = DEVICE_CONFIG_VERSION;
    cfg->key1_usage = DEVICE_CONFIG_DEFAULT_KEY1;
    cfg->key2_usage = DEVICE_CONFIG_DEFAULT_KEY2;
    cfg->debounce_us = DEVICE_CONFIG_DEFAULT_DEBOUNCE_US;
    cfg->brightness = DEVICE_CONFIG_DEFAULT_BRIGHTNESS;
    cfg->sleep_s = DEVICE_CONFIG_DEFAULT_SLEEP_S;
    cfg->gameplay_display_hz = DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ;
    cfg->key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO;
    cfg->key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO;
    memset(cfg->owner_id, 0, sizeof(cfg->owner_id));
    cfg->swipe_left_action = DEVICE_CONFIG_DEFAULT_SWIPE_LEFT;
    cfg->swipe_right_action = DEVICE_CONFIG_DEFAULT_SWIPE_RIGHT;
    cfg->swipe_left_key = 0;
    cfg->swipe_right_key = 0;
    cfg->swipe_up_action = DEVICE_CONFIG_DEFAULT_SWIPE_UP;
    cfg->swipe_down_action = DEVICE_CONFIG_DEFAULT_SWIPE_DOWN;
    cfg->swipe_up_key = 0;
    cfg->swipe_down_key = 0;
    memset(cfg->swipe_mods, 0, sizeof(cfg->swipe_mods));
}


static esp_err_t write_to_nvs(const device_config_data_t *cfg)
{
    // STRICT SPEC INVARIANT: zero NVS writes during PLAYING and COOLDOWN
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "NVS write blocked: state != IDLE (deferred to supervisor)");
        set_dirty(true);
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
            set_dirty(false);
            counters_record_nvs_write();
            ESP_LOGI(TAG, "Device config persisted to NVS: K1=0x%02lx@GPIO%lu, K2=0x%02lx@GPIO%lu, debounce=%lu us, brightness=%lu%%, sleep=%lu s",
                     (unsigned long)cfg->key1_usage, (unsigned long)cfg->key1_gpio,
                     (unsigned long)cfg->key2_usage, (unsigned long)cfg->key2_gpio,
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
        .key1_gpio = (uint8_t)cfg->key1_gpio,
        .key2_gpio = (uint8_t)cfg->key2_gpio,
    };
    // Keycodes reach usb_hid through the keypad, never while a key is held
    keypad_set_config(&k_cfg);

    // The UI owns the backlight: it knows whether the display is asleep
    ui_set_brightness((uint8_t)cfg->brightness);
    ui_set_sleep_timeout(cfg->sleep_s);

    // In gesture_t order: up, down, left, right
    const uint8_t actions[4] = {cfg->swipe_up_action, cfg->swipe_down_action,
                                cfg->swipe_left_action, cfg->swipe_right_action};
    const uint8_t keys[4] = {cfg->swipe_up_key, cfg->swipe_down_key,
                             cfg->swipe_left_key, cfg->swipe_right_key};
    touch_retry_set_swipe_actions(actions, keys, cfg->swipe_mods);
}

esp_err_t device_config_init(void)
{
    // Initialize NVS if needed
    esp_err_t err = nvs_flash_init();
    if (err == ESP_ERR_NVS_NO_FREE_PAGES || err == ESP_ERR_NVS_NEW_VERSION_FOUND) {
        ESP_LOGW(TAG, "Erasing corrupted/outdated NVS flash in config init...");
        diag_record(DIAG_EVENT_NVS_ERASED, 2 /* WARN */, 0, 0);
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
        set_defaults(&loaded);
        size_t len = sizeof(loaded);
        err = nvs_get_blob(handle, NVS_KEY, &loaded, &len);
        nvs_close(handle);

        bool layout_ok = (len == sizeof(loaded) && loaded.version == DEVICE_CONFIG_VERSION) ||
                         (len == DEVICE_CONFIG_V6_SIZE && loaded.version == 6) ||
                         (len == DEVICE_CONFIG_V5_SIZE && loaded.version == 5) ||
                         (len == DEVICE_CONFIG_V4_SIZE && loaded.version == 4) ||
                         (len == DEVICE_CONFIG_V3_SIZE && loaded.version == 3) ||
                         (len == DEVICE_CONFIG_V2_SIZE && loaded.version == 2) ||
                         (len == DEVICE_CONFIG_V1_SIZE && loaded.version == 1);
        if (err == ESP_OK && layout_ok && loaded.version == 5) {
            bool inverted = loaded.swipe_up_action != 0;
            loaded.swipe_up_action = inverted ? SWIPE_ACTION_VOLUME_DOWN : SWIPE_ACTION_VOLUME_UP;
            loaded.swipe_down_action = inverted ? SWIPE_ACTION_VOLUME_UP : SWIPE_ACTION_VOLUME_DOWN;
        }
        if (err == ESP_OK && layout_ok && device_config_validate(&loaded, NULL, 0)) {
            if (loaded.version != DEVICE_CONFIG_VERSION) {
                ESP_LOGI(TAG, "Migrating NVS config v%lu -> v%d",
                         (unsigned long)loaded.version, DEVICE_CONFIG_VERSION);
                // An older blob stops short, so the fields beyond it keep what
                // set_defaults left: unclaimed, default swipe actions.
                loaded.version = DEVICE_CONFIG_VERSION;
            }
            s_current_config = loaded;
            ESP_LOGI(TAG, "Loaded config from NVS: K1=0x%02lx@GPIO%lu, K2=0x%02lx@GPIO%lu, debounce=%lu us, brightness=%lu%%, sleep=%lu s",
                     (unsigned long)loaded.key1_usage, (unsigned long)loaded.key1_gpio,
                     (unsigned long)loaded.key2_usage, (unsigned long)loaded.key2_gpio,
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
        *out_cfg = config_snapshot();
    }
}

esp_err_t device_config_set(const device_config_data_t *cfg)
{
    if (!cfg) return ESP_ERR_INVALID_ARG;

    device_config_data_t next = *cfg;
    config_store(&next);
    device_config_apply(&next);

    // Outside IDLE this only marks the config dirty for the supervisor
    return write_to_nvs(&next);
}

void device_config_get_owner(uint8_t out_owner[OWNER_ID_LEN])
{
    if (out_owner) {
        device_config_data_t cfg = config_snapshot();
        memcpy(out_owner, cfg.owner_id, OWNER_ID_LEN);
    }
}

esp_err_t device_config_claim_owner(const uint8_t owner[OWNER_ID_LEN])
{
    owner_runtime_state_t state = (runtime_get_state() == OSUPAD_STATE_IDLE)
                                      ? OWNER_STATE_IDLE
                                      : OWNER_STATE_ACTIVE;

    device_config_data_t next = config_snapshot();
    switch (owner_claim_decide(next.owner_id, owner, state)) {
    case OWNER_CLAIM_ALREADY_OWNED:
        // Every connect would otherwise cost a flash write for no change
        return ESP_OK;

    case OWNER_CLAIM_REJECT_ACTIVE:
        ESP_LOGW(TAG, "Ownership claim refused: a map is running (P1-3)");
        return ESP_ERR_INVALID_STATE;

    case OWNER_CLAIM_REJECT_INVALID:
        ESP_LOGW(TAG, "Ownership claim refused: missing or all-zero owner id");
        return ESP_ERR_INVALID_ARG;

    case OWNER_CLAIM_APPLY:
        break;
    }

    memcpy(next.owner_id, owner, OWNER_ID_LEN);
    config_store(&next);
    ESP_LOGI(TAG, "Pad claimed by a new host install");
    // IDLE is guaranteed by the decision above, so this writes rather than
    // deferring — the host claims at connect time and expects it to stick.
    return write_to_nvs(&next);
}

esp_err_t device_config_flush(void)
{
    portENTER_CRITICAL(&s_config_lock);
    bool dirty = s_dirty;
    portEXIT_CRITICAL(&s_config_lock);
    if (!dirty) {
        return ESP_OK;
    }
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK;
    }
    ESP_LOGI(TAG, "Flushing deferred config to NVS");
    device_config_data_t cfg = config_snapshot();
    return write_to_nvs(&cfg);
}
