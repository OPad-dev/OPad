#include "device_config.h"
#include "input/keypad.h"
#include "input/debounce.h"
#include "input/touch_retry.h"
#include "input/he_backend.h"
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
static const char *NVS_HE_KEY = "he_cfg";

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

static he_key_settings_t s_current_he_keys[2] = {
    {
        .actuation = HE_CONFIG_DEFAULT_ACTUATION,
        .release_hyst = HE_CONFIG_DEFAULT_HYSTERESIS,
        .top_reset = HE_CONFIG_DEFAULT_TOP_RESET,
        .rt_press = HE_CONFIG_DEFAULT_RT_PRESS,
        .rt_release = HE_CONFIG_DEFAULT_RT_RELEASE,
        .bottom_dz = HE_CONFIG_DEFAULT_BOTTOM_DZ,
        .rapid_trigger = HE_CONFIG_DEFAULT_RAPID_TRIGGER,
        .continuous = HE_CONFIG_DEFAULT_CONTINUOUS,
    },
    {
        .actuation = HE_CONFIG_DEFAULT_ACTUATION,
        .release_hyst = HE_CONFIG_DEFAULT_HYSTERESIS,
        .top_reset = HE_CONFIG_DEFAULT_TOP_RESET,
        .rt_press = HE_CONFIG_DEFAULT_RT_PRESS,
        .rt_release = HE_CONFIG_DEFAULT_RT_RELEASE,
        .bottom_dz = HE_CONFIG_DEFAULT_BOTTOM_DZ,
        .rapid_trigger = HE_CONFIG_DEFAULT_RAPID_TRIGGER,
        .continuous = HE_CONFIG_DEFAULT_CONTINUOUS,
    },
};

static bool s_dirty = false;
static bool s_he_dirty = false;
// Guards s_current_config, s_current_he_keys, s_dirty, s_he_dirty: written by the protocol task, read by the
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

void device_config_apply_he(const he_key_settings_t in[2])
{
    if (!in) return;
    he_key_settings_t he_copy[2] = {in[0], in[1]};
    he_backend_apply_settings(he_copy);
}

static esp_err_t write_he_to_nvs(const he_key_settings_t in[2])
{
    // STRICT SPEC INVARIANT: zero NVS writes during PLAYING and COOLDOWN
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "HE NVS write blocked: state != IDLE (deferred to supervisor)");
        portENTER_CRITICAL(&s_config_lock);
        s_he_dirty = true;
        portEXIT_CRITICAL(&s_config_lock);
        return ESP_OK;
    }

    he_config_record_t rec;
    he_config_record_pack(&rec, in);

    nvs_handle_t handle;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to open NVS namespace '%s' for he_cfg: %s", NVS_NAMESPACE, esp_err_to_name(err));
        return err;
    }

    err = nvs_set_blob(handle, NVS_HE_KEY, &rec, sizeof(rec));
    if (err == ESP_OK) {
        err = nvs_commit(handle);
        if (err == ESP_OK) {
            portENTER_CRITICAL(&s_config_lock);
            s_he_dirty = false;
            portEXIT_CRITICAL(&s_config_lock);
            counters_record_nvs_write();
            ESP_LOGI(TAG, "HE config persisted to NVS");
        }
    }
    nvs_close(handle);
    return err;
}

void device_config_get_he(he_key_settings_t out[2])
{
    if (out) {
        portENTER_CRITICAL(&s_config_lock);
        out[0] = s_current_he_keys[0];
        out[1] = s_current_he_keys[1];
        portEXIT_CRITICAL(&s_config_lock);
    }
}

esp_err_t device_config_set_he(const he_key_settings_t in[2])
{
    if (!in) return ESP_ERR_INVALID_ARG;

    he_key_settings_t next[2];
    portENTER_CRITICAL(&s_config_lock);
    // The app echoes the HE fields back in every SetConfig: no write when unchanged
    bool unchanged = !s_he_dirty && he_config_settings_equal(s_current_he_keys, in);
    if (unchanged) {
        portEXIT_CRITICAL(&s_config_lock);
        return ESP_OK;
    }
    s_current_he_keys[0] = in[0];
    s_current_he_keys[1] = in[1];
    next[0] = in[0];
    next[1] = in[1];
    portEXIT_CRITICAL(&s_config_lock);

    device_config_apply_he(next);

    return write_he_to_nvs(next);
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
        he_config_default_settings(s_current_he_keys);
        device_config_apply(&s_current_config);
        device_config_apply_he(s_current_he_keys);
        return ESP_OK;
    }

    nvs_handle_t handle;
    err = nvs_open(NVS_NAMESPACE, NVS_READONLY, &handle);
    if (err == ESP_OK) {
        device_config_data_t loaded;
        set_defaults(&loaded);
        size_t len = sizeof(loaded);
        err = nvs_get_blob(handle, NVS_KEY, &loaded, &len);

        bool layout_ok = device_config_blob_layout_ok(len, loaded.version);
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

        // Load "he_cfg"
        he_key_settings_t he_loaded[2];
        he_config_record_t rec;
        size_t he_len = sizeof(rec);
        esp_err_t he_err = nvs_get_blob(handle, NVS_HE_KEY, &rec, &he_len);
        if (he_err == ESP_OK && he_config_record_unpack(&rec, he_len, he_loaded)) {
            s_current_he_keys[0] = he_loaded[0];
            s_current_he_keys[1] = he_loaded[1];
            ESP_LOGI(TAG, "Loaded HE config from NVS");
        } else {
            if (he_err != ESP_ERR_NVS_NOT_FOUND) {
                ESP_LOGW(TAG, "NVS HE config corrupt or invalid, using defaults");
            } else {
                ESP_LOGI(TAG, "No NVS HE config found, using defaults");
            }
            he_config_default_settings(s_current_he_keys);
        }

        nvs_close(handle);
    } else {
        ESP_LOGI(TAG, "No NVS config found, using defaults");
        set_defaults(&s_current_config);
        he_config_default_settings(s_current_he_keys);
    }

    device_config_apply(&s_current_config);
    device_config_apply_he(s_current_he_keys);
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
    bool he_dirty = s_he_dirty;
    portEXIT_CRITICAL(&s_config_lock);
    if (!dirty && !he_dirty) {
        return ESP_OK;
    }
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK;
    }
    esp_err_t res = ESP_OK;
    if (dirty) {
        ESP_LOGI(TAG, "Flushing deferred config to NVS");
        device_config_data_t cfg = config_snapshot();
        res = write_to_nvs(&cfg);
    }
    if (he_dirty) {
        ESP_LOGI(TAG, "Flushing deferred HE config to NVS");
        he_key_settings_t he_keys[2];
        device_config_get_he(he_keys);
        esp_err_t err_he = write_he_to_nvs(he_keys);
        if (res == ESP_OK) {
            res = err_he;
        }
    }
    return res;
}
