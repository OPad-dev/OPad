#include "device_config.h"
#include "input/debounce.h"
#include <stdio.h>

#ifdef ESP_PLATFORM
#include "sdkconfig.h"
#endif

// Header GPIOs of the Waveshare ESP32-S3-Touch-LCD-2 (schematic, P1/P2) that can take a
// switch to GND with the internal pull-up. Keep in sync with KEY_PINS in opad-model.
// Left out: 19/20 (USB D-/D+), 43/44 (UART0 console), 47/48 (touch + IMU I2C),
// 17 (CAM_PWDN, 10k pull-down to GND), 8 (module-ID ADC, 10k to GND through the divider).
// The rest are camera pins, free with no camera fitted.
const uint8_t KEY_GPIO_ALLOWED[] = {2, 4, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 18, 21};
const size_t KEY_GPIO_ALLOWED_COUNT = sizeof(KEY_GPIO_ALLOWED) / sizeof(KEY_GPIO_ALLOWED[0]);

bool device_config_key_gpio_supported(uint32_t gpio)
{
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO)
    if (gpio == (uint32_t)CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM) {
        return false;
    }
#endif
    for (size_t i = 0; i < KEY_GPIO_ALLOWED_COUNT; i++) {
        if (KEY_GPIO_ALLOWED[i] == gpio) {
            return true;
        }
    }
    return false;
}

// A key is optional (0) unless the action sends it
static bool swipe_valid(uint8_t action, uint8_t key)
{
    if (action < SWIPE_ACTION_NONE || action > SWIPE_ACTION_VOLUME_DOWN) {
        return false;
    }
    if (key != 0 && (key < 0x04 || key > 0xE7)) {
        return false;
    }
    return action != SWIPE_ACTION_KEY || key != 0;
}

bool device_config_validate(const device_config_data_t *cfg, char *err_msg, size_t err_msg_len)
{
    if (!cfg) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "null config");
        return false;
    }
    if (cfg->key1_usage < 0x04 || cfg->key1_usage > 0xE7) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid key1 usage 0x%02lx", (unsigned long)cfg->key1_usage);
        return false;
    }
    if (cfg->key2_usage < 0x04 || cfg->key2_usage > 0xE7) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid key2 usage 0x%02lx", (unsigned long)cfg->key2_usage);
        return false;
    }
    if (cfg->debounce_us < DEBOUNCE_MIN_US || cfg->debounce_us > DEBOUNCE_MAX_US) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "debounce %lu us out of range [500, 20000]", (unsigned long)cfg->debounce_us);
        return false;
    }
    if (cfg->brightness > 100) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "brightness %lu out of range [0, 100]", (unsigned long)cfg->brightness);
        return false;
    }
    if (cfg->sleep_s != 0 && (cfg->sleep_s < 10 || cfg->sleep_s > 86400)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "sleep %lu s out of range (0 or 10-86400)", (unsigned long)cfg->sleep_s);
        return false;
    }
    if (cfg->gameplay_display_hz != 0 && (cfg->gameplay_display_hz < 1 || cfg->gameplay_display_hz > 60)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "gameplay display hz %lu out of range [1, 60]", (unsigned long)cfg->gameplay_display_hz);
        return false;
    }
    if (!device_config_key_gpio_supported(cfg->key1_gpio)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "unsupported key1 gpio %lu", (unsigned long)cfg->key1_gpio);
        return false;
    }
    if (!device_config_key_gpio_supported(cfg->key2_gpio)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "unsupported key2 gpio %lu", (unsigned long)cfg->key2_gpio);
        return false;
    }
    if (!swipe_valid(cfg->swipe_left_action, cfg->swipe_left_key)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid left swipe action %u key 0x%02x",
                                             cfg->swipe_left_action, cfg->swipe_left_key);
        return false;
    }
    if (!swipe_valid(cfg->swipe_right_action, cfg->swipe_right_key)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid right swipe action %u key 0x%02x",
                                             cfg->swipe_right_action, cfg->swipe_right_key);
        return false;
    }
    if (!swipe_valid(cfg->swipe_up_action, cfg->swipe_up_key)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid up swipe action %u key 0x%02x",
                                             cfg->swipe_up_action, cfg->swipe_up_key);
        return false;
    }
    if (!swipe_valid(cfg->swipe_down_action, cfg->swipe_down_key)) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "invalid down swipe action %u key 0x%02x",
                                             cfg->swipe_down_action, cfg->swipe_down_key);
        return false;
    }
    if (cfg->key1_gpio == cfg->key2_gpio) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "key1 and key2 share gpio %lu", (unsigned long)cfg->key1_gpio);
        return false;
    }
    return true;
}

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

bool device_config_blob_layout_ok(size_t len, uint32_t version)
{
    return (len == sizeof(device_config_data_t) && version == DEVICE_CONFIG_VERSION) ||
           (len == DEVICE_CONFIG_V6_SIZE && version == 6) ||
           (len == DEVICE_CONFIG_V5_SIZE && version == 5) ||
           (len == DEVICE_CONFIG_V4_SIZE && version == 4) ||
           (len == DEVICE_CONFIG_V3_SIZE && version == 3) ||
           (len == DEVICE_CONFIG_V2_SIZE && version == 2) ||
           (len == DEVICE_CONFIG_V1_SIZE && version == 1);
}

bool device_config_validate_he(const he_key_settings_t in[2], char *err_msg, size_t err_msg_len)
{
    if (!in) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "null he settings");
        return false;
    }
    for (int k = 0; k < 2; k++) {
        if (in[k].actuation > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] actuation %u out of range [0, %d]", k, in[k].actuation, HE_PCT_FULL);
            return false;
        }
        if (in[k].release_hyst > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] hysteresis %u out of range [0, %d]", k, in[k].release_hyst, HE_PCT_FULL);
            return false;
        }
        if (in[k].top_reset > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] top_reset %u out of range [0, %d]", k, in[k].top_reset, HE_PCT_FULL);
            return false;
        }
        if (in[k].rt_press > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] rt_press %u out of range [0, %d]", k, in[k].rt_press, HE_PCT_FULL);
            return false;
        }
        if (in[k].rt_release > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] rt_release %u out of range [0, %d]", k, in[k].rt_release, HE_PCT_FULL);
            return false;
        }
        if (in[k].bottom_dz > HE_PCT_FULL) {
            if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "he_key[%d] bottom_dz %u out of range [0, %d]", k, in[k].bottom_dz, HE_PCT_FULL);
            return false;
        }
    }
    return true;
}
