#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize easter egg resources (allocate PSRAM memory pool for LVGL GIF decoding).
 */
void easter_egg_init(void);

/**
 * @brief Trigger the 67 freaky cat easter egg animation.
 * Must be called with LVGL lock held (e.g. inside pad_timer_cb or under lvgl_port_lock).
 */
void easter_egg_trigger(void);

#ifdef __cplusplus
}
#endif
