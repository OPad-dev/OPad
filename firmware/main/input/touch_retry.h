#pragma once

#include "esp_err.h"
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize the capacitive touchscreen quick-retry driver.
 *        Polls the CST816 I2C controller on Core 1 at 100 Hz.
 *        Tapping anywhere on the glass sends USB HID Keycode 0x35 ('`' / Quick Retry).
 */
esp_err_t touch_retry_init(void);

/**
 * @brief Returns true if the screen is currently being touched.
 */
bool touch_retry_is_pressed(void);

#ifdef __cplusplus
}
#endif
