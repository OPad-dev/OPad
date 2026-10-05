#pragma once

#include "esp_err.h"
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize the capacitive touchscreen driver.
 *        A Core 1 task sleeps on the CST816's INT line and, once a touch is
 *        down, polls the controller at 100 Hz until it is released.
 *        During a map a touch holds HID Keycode 0x35 ('`' / Quick Retry) and
 *        swipes set the volume or run the left/right actions; outside one only
 *        the swipes work. With no host saying which (no daemon or no tosu),
 *        every touch is Quick Retry, as before swipes existed.
 */
esp_err_t touch_retry_init(void);

/**
 * @brief Returns true if the screen is currently being touched.
 */
bool touch_retry_is_pressed(void);

/**
 * @brief From each HostStatus: whether the host can tell a map from a menu
 *        (tosu connected), and whether volume swipes go to osu!.
 */
void touch_retry_set_host_status(bool tosu_connected, bool osu_active);

/**
 * @brief Left/right swipe actions (swipe_action_t) and their keyboard keys.
 */
void touch_retry_set_swipe_actions(uint8_t left_action, uint8_t left_key,
                                   uint8_t right_action, uint8_t right_key);

#ifdef __cplusplus
}
#endif
