#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "input/he_params.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_CONFIG_RECORD_MAGIC   0x48454346u  // "HECF" (Hall Effect ConFig)
#define HE_CONFIG_RECORD_VERSION 1u

// PROVISIONAL factory defaults for HE keys (docs/specs/v2-hall-effect-decisions.md D4, OPEN C/D)
#define HE_CONFIG_DEFAULT_ACTUATION     500 // 50.0% of calibrated span (provisional)
#define HE_CONFIG_DEFAULT_HYSTERESIS    100 // 10.0% of calibrated span (provisional)
#define HE_CONFIG_DEFAULT_TOP_RESET     100 // 10.0% of calibrated span (provisional)
#define HE_CONFIG_DEFAULT_RT_PRESS      150 // 15.0% of calibrated span (provisional)
#define HE_CONFIG_DEFAULT_RT_RELEASE    150 // 15.0% of calibrated span (provisional)
#define HE_CONFIG_DEFAULT_BOTTOM_DZ     0   // Off
#define HE_CONFIG_DEFAULT_RAPID_TRIGGER false
#define HE_CONFIG_DEFAULT_CONTINUOUS    false

typedef struct __attribute__((packed)) {
    uint16_t actuation;
    uint16_t release_hyst;
    uint16_t rt_press;
    uint16_t rt_release;
    uint16_t top_reset;
    uint16_t bottom_dz;
    uint8_t rapid_trigger;
    uint8_t continuous;
} he_key_config_record_t;

typedef struct __attribute__((packed)) {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved;
    he_key_config_record_t keys[2];
    uint32_t crc32;
} he_config_record_t;

/**
 * Standard CRC-32 (IEEE 802.3, polynomial 0xEDB88320).
 */
uint32_t he_config_crc32(const void *data, size_t len);

/**
 * Initialize he_key_settings_t with factory defaults.
 */
void he_config_default_settings(he_key_settings_t settings[2]);

/**
 * Validate a key settings struct against allowable bounds.
 */
bool he_config_validate_settings(const he_key_settings_t *settings, char *err_msg, size_t err_msg_len);

/**
 * Pack in-memory he_key_settings_t into on-disk fixed record with magic, version, and CRC.
 */
void he_config_record_pack(he_config_record_t *rec, const he_key_settings_t settings[2]);

/**
 * Unpack on-disk fixed record into in-memory he_key_settings_t.
 * Returns true if record magic, version, CRC and value ranges are valid;
 * false otherwise (populating settings with defaults).
 */
bool he_config_record_unpack(const void *buf, size_t len, he_key_settings_t settings[2]);

#ifdef __cplusplus
}
#endif
