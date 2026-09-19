#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize USB HID Keyboard driver.
 */
esp_err_t usb_hid_init(void);

/**
 * @brief Check if USB HID interface is mounted and ready to transmit.
 */
bool usb_hid_is_ready(void);

/**
 * @brief Update configured HID keycodes for Key 1 and Key 2.
 */
void usb_hid_set_keycodes(uint8_t key1_code, uint8_t key2_code);

/**
 * @brief Update configured HID keycode for Key 3 (Touch Retry).
 */
void usb_hid_set_key3_code(uint8_t key3_code);

/**
 * @brief Set the touchscreen quick retry key state (pressed/released).
 */
void usb_hid_set_touch_retry(bool pressed);

/**
 * @brief Handler dispatched when keypad physical state changes.
 */
bool usb_hid_handle_key_event(uint8_t key_index, bool pressed, int64_t edge_us);

#ifdef __cplusplus
}
#endif
