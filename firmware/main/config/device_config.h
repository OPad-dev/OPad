#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

#ifdef ESP_PLATFORM
#include "esp_err.h"
#else
typedef int esp_err_t;
#define ESP_OK 0
#define ESP_FAIL -1
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define DEVICE_CONFIG_VERSION               1
#define DEVICE_CONFIG_DEFAULT_KEY1          0x1D // 'Z'
#define DEVICE_CONFIG_DEFAULT_KEY2          0x1B // 'X'
#define DEVICE_CONFIG_DEFAULT_DEBOUNCE_US   3000
#define DEVICE_CONFIG_DEFAULT_BRIGHTNESS    100
#define DEVICE_CONFIG_DEFAULT_SLEEP_S       600
#define DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ 10

typedef struct __attribute__((packed)) {
    uint32_t version;
    uint32_t key1_usage;
    uint32_t key2_usage;
    uint32_t debounce_us;
    uint32_t brightness;
    uint32_t sleep_s;
    uint32_t gameplay_display_hz;
} device_config_data_t;

/**
 * @brief Initialize device configuration from NVS (or set defaults).
 * Non-fatal: falls back to safe defaults on any NVS or validation failure.
 */
esp_err_t device_config_init(void);

/**
 * @brief Validate configuration parameters.
 */
bool device_config_validate(const device_config_data_t *cfg, char *err_msg, size_t err_msg_len);

/**
 * @brief Get current configuration snapshot.
 */
void device_config_get(device_config_data_t *out_cfg);

/**
 * @brief Apply configuration to hardware and subsystems in RAM.
 */
void device_config_apply(const device_config_data_t *cfg);

/**
 * @brief Set and save new configuration.
 * Applies to RAM immediately; if state is IDLE, commits to NVS immediately.
 * If active (PLAYING or COOLDOWN), marks dirty and defers NVS write until IDLE.
 */
esp_err_t device_config_set(const device_config_data_t *cfg);

/**
 * @brief Flush dirty configuration to NVS if in IDLE state.
 */
esp_err_t device_config_flush(void);

#ifdef __cplusplus
}
#endif
