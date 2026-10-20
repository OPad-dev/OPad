#include "he_config_record.h"
#include <string.h>
#include <stdio.h>

uint32_t he_config_crc32(const void *data, size_t len)
{
    const uint8_t *p = (const uint8_t *)data;
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= p[i];
        for (int b = 0; b < 8; b++) {
            crc = (crc >> 1) ^ (0xEDB88320u & -(crc & 1u));
        }
    }
    return ~crc;
}

void he_config_default_settings(he_key_settings_t settings[2])
{
    if (!settings) {
        return;
    }
    for (int k = 0; k < 2; k++) {
        settings[k].actuation = HE_CONFIG_DEFAULT_ACTUATION;
        settings[k].release_hyst = HE_CONFIG_DEFAULT_HYSTERESIS;
        settings[k].top_reset = HE_CONFIG_DEFAULT_TOP_RESET;
        settings[k].rt_press = HE_CONFIG_DEFAULT_RT_PRESS;
        settings[k].rt_release = HE_CONFIG_DEFAULT_RT_RELEASE;
        settings[k].bottom_dz = HE_CONFIG_DEFAULT_BOTTOM_DZ;
        settings[k].rapid_trigger = HE_CONFIG_DEFAULT_RAPID_TRIGGER;
        settings[k].continuous = HE_CONFIG_DEFAULT_CONTINUOUS;
    }
}

bool he_config_validate_settings(const he_key_settings_t *s, char *err_msg, size_t err_msg_len)
{
    if (!s) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "null he settings");
        return false;
    }
    if (s->actuation > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "actuation %u out of range [0, %d]", s->actuation, HE_PCT_FULL);
        return false;
    }
    if (s->release_hyst > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "hysteresis %u out of range [0, %d]", s->release_hyst, HE_PCT_FULL);
        return false;
    }
    if (s->top_reset > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "top_reset %u out of range [0, %d]", s->top_reset, HE_PCT_FULL);
        return false;
    }
    if (s->rt_press > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "rt_press %u out of range [0, %d]", s->rt_press, HE_PCT_FULL);
        return false;
    }
    if (s->rt_release > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "rt_release %u out of range [0, %d]", s->rt_release, HE_PCT_FULL);
        return false;
    }
    if (s->bottom_dz > HE_PCT_FULL) {
        if (err_msg && err_msg_len) snprintf(err_msg, err_msg_len, "bottom_dz %u out of range [0, %d]", s->bottom_dz, HE_PCT_FULL);
        return false;
    }
    return true;
}

void he_config_record_pack(he_config_record_t *rec, const he_key_settings_t settings[2])
{
    if (!rec) {
        return;
    }
    memset(rec, 0, sizeof(*rec));
    rec->magic = HE_CONFIG_RECORD_MAGIC;
    rec->version = HE_CONFIG_RECORD_VERSION;
    rec->reserved = 0;
    for (int k = 0; k < 2; k++) {
        rec->keys[k].actuation = settings[k].actuation;
        rec->keys[k].release_hyst = settings[k].release_hyst;
        rec->keys[k].rt_press = settings[k].rt_press;
        rec->keys[k].rt_release = settings[k].rt_release;
        rec->keys[k].top_reset = settings[k].top_reset;
        rec->keys[k].bottom_dz = settings[k].bottom_dz;
        rec->keys[k].rapid_trigger = settings[k].rapid_trigger ? 1 : 0;
        rec->keys[k].continuous = settings[k].continuous ? 1 : 0;
    }
    rec->crc32 = he_config_crc32(rec, offsetof(he_config_record_t, crc32));
}

bool he_config_record_unpack(const void *buf, size_t len, he_key_settings_t settings[2])
{
    if (!settings) {
        return false;
    }
    he_config_default_settings(settings);

    if (!buf || len != sizeof(he_config_record_t)) {
        return false;
    }

    const he_config_record_t *rec = (const he_config_record_t *)buf;
    if (rec->magic != HE_CONFIG_RECORD_MAGIC || rec->version != HE_CONFIG_RECORD_VERSION) {
        return false;
    }

    uint32_t expected_crc = he_config_crc32(rec, offsetof(he_config_record_t, crc32));
    if (rec->crc32 != expected_crc) {
        return false;
    }

    for (int k = 0; k < 2; k++) {
        if (rec->keys[k].actuation > HE_PCT_FULL ||
            rec->keys[k].release_hyst > HE_PCT_FULL ||
            rec->keys[k].rt_press > HE_PCT_FULL ||
            rec->keys[k].rt_release > HE_PCT_FULL ||
            rec->keys[k].top_reset > HE_PCT_FULL ||
            rec->keys[k].bottom_dz > HE_PCT_FULL) {
            he_config_default_settings(settings);
            return false;
        }
    }

    for (int k = 0; k < 2; k++) {
        settings[k].actuation = rec->keys[k].actuation;
        settings[k].release_hyst = rec->keys[k].release_hyst;
        settings[k].rt_press = rec->keys[k].rt_press;
        settings[k].rt_release = rec->keys[k].rt_release;
        settings[k].top_reset = rec->keys[k].top_reset;
        settings[k].bottom_dz = rec->keys[k].bottom_dz;
        settings[k].rapid_trigger = rec->keys[k].rapid_trigger != 0;
        settings[k].continuous = rec->keys[k].continuous != 0;
    }

    return true;
}
