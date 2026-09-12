#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    KEY_ID_1 = 0,
    KEY_ID_2 = 1,
    KEY_ID_COUNT = 2
} keypad_key_id_t;

typedef struct {
    uint8_t keycode1;       // USB HID Keycode for Key 1 (default 0x1D = 'z')
    uint8_t keycode2;       // USB HID Keycode for Key 2 (default 0x1B = 'x')
    uint16_t debounce_ms;   // Debounce interval in milliseconds (default 5ms)
} keypad_config_t;

typedef void (*keypad_state_callback_t)(uint8_t key_index, bool pressed);

/**
 * @brief Initialize keypad subsystem, GPIOs, ISR, and high-priority input processing task.
 */
esp_err_t keypad_init(const keypad_config_t *config);

/**
 * @brief Register callback for key state changes (dispatched with minimal latency).
 */
void keypad_set_state_callback(keypad_state_callback_t cb);

/**
 * @brief Get current physical switch state.
 */
bool keypad_is_pressed(keypad_key_id_t key_id);

/**
 * @brief Get RAM lifetime press counters.
 */
void keypad_get_lifetime_presses(uint64_t *key1_presses, uint64_t *key2_presses);

/**
 * @brief Set RAM lifetime press counters (used on boot load, sync, or reset).
 */
void keypad_set_lifetime_presses(uint64_t key1_presses, uint64_t key2_presses);

/**
 * @brief Get timestamp in microseconds of the last keypress down event.
 */
int64_t keypad_get_last_press_us(keypad_key_id_t key_id);

/**
 * @brief Update keypad configuration (debouncing and key mappings).
 */
void keypad_set_config(const keypad_config_t *config);

/**
 * @brief Get current keypad configuration.
 */
void keypad_get_config(keypad_config_t *out_config);

#ifdef __cplusplus
}
#endif
