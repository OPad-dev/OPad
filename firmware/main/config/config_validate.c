#include "device_config.h"
#include <stdio.h>

#define DEBOUNCE_MIN_US     500
#define DEBOUNCE_MAX_US     20000

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
    return true;
}
