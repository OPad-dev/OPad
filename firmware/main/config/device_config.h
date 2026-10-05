#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "config/owner.h"

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

#define DEVICE_CONFIG_VERSION               5
#define DEVICE_CONFIG_DEFAULT_KEY1          0x1D // 'Z'
#define DEVICE_CONFIG_DEFAULT_KEY2          0x1B // 'X'
#define DEVICE_CONFIG_DEFAULT_DEBOUNCE_US   5000
#define DEVICE_CONFIG_DEFAULT_BRIGHTNESS    100
#define DEVICE_CONFIG_DEFAULT_SLEEP_S       600
#define DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ 10
#define DEVICE_CONFIG_DEFAULT_KEY1_GPIO     14   // Header P2, pin 11
#define DEVICE_CONFIG_DEFAULT_KEY2_GPIO     9    // Header P2, pin 12

// What a left/right swipe does. Values are osupad.proto's SwipeAction; 0
// (keep current) is wire only and never stored.
typedef enum {
    SWIPE_ACTION_NONE = 1,
    SWIPE_ACTION_PREV_TRACK = 2,
    SWIPE_ACTION_NEXT_TRACK = 3,
    SWIPE_ACTION_PLAY_PAUSE = 4,
    SWIPE_ACTION_MUTE = 5,
    SWIPE_ACTION_KEY = 6,   // swipe_*_key, a keyboard key
} swipe_action_t;

#define DEVICE_CONFIG_DEFAULT_SWIPE_LEFT    SWIPE_ACTION_PREV_TRACK
#define DEVICE_CONFIG_DEFAULT_SWIPE_RIGHT   SWIPE_ACTION_NEXT_TRACK

typedef struct __attribute__((packed)) {
    uint32_t version;
    uint32_t key1_usage;
    uint32_t key2_usage;
    uint32_t debounce_us;
    uint32_t brightness;
    uint32_t sleep_s;
    uint32_t gameplay_display_hz;
    // v2
    uint32_t key1_gpio;
    uint32_t key2_gpio;
    // v3: which host install owns this pad (§W3-1, §W3-2). All zero =
    // unclaimed, which is what every pad flashed before v3 reads as.
    uint8_t owner_id[OWNER_ID_LEN];
    // v4: left/right swipe actions (swipe_action_t) and their keyboard keys
    // (HID usage, 0 = none), used when the action is SWIPE_ACTION_KEY
    uint8_t swipe_left_action;
    uint8_t swipe_right_action;
    uint8_t swipe_left_key;
    uint8_t swipe_right_key;
    // v5: 1 = up/down swipes inverted (swipe up = volume down)
    uint8_t swipe_invert_vertical;
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
 * @brief Header GPIOs a key switch can be wired to. The bench debug GPIO may be
 * among them; check device_config_key_gpio_supported() before using one.
 */
extern const uint8_t KEY_GPIO_ALLOWED[];
extern const size_t KEY_GPIO_ALLOWED_COUNT;

/**
 * @brief True if a key switch can be wired to this GPIO on the board's headers.
 */
bool device_config_key_gpio_supported(uint32_t gpio);

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

/**
 * @brief Copies the current owner id out. All zero when unclaimed.
 */
void device_config_get_owner(uint8_t out_owner[OWNER_ID_LEN]);

/**
 * @brief Records a new owner (§W3-2).
 *
 * An NVS write, so it is honoured only in IDLE (P1-3). Re-claiming by the
 * current owner is a no-op rather than a write. Returns ESP_OK when the pad
 * ends up owned by @p owner, ESP_ERR_INVALID_STATE while a map is running, and
 * ESP_ERR_INVALID_ARG for a missing or all-zero id.
 */
esp_err_t device_config_claim_owner(const uint8_t owner[OWNER_ID_LEN]);

#ifdef __cplusplus
}
#endif
