#pragma once

// Device UI: LVGL on core 1 (esp_lvgl_port) rendering the idle and playing layouts.
// Nothing here runs on core 0; the keypad task only calls ui_notify_activity().

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "ui/core/ui_core.h"

#ifdef __cplusplus
extern "C" {
#endif

esp_err_t ui_init(void);

/** Lock-free timestamp write; safe from the keypad task. */
void ui_notify_activity(void);

void ui_set_time(uint32_t year, uint32_t month, uint32_t day, uint32_t hour, uint32_t minute, uint32_t second);
void ui_set_brightness(uint8_t percent);
void ui_set_sleep_timeout(uint32_t seconds);
bool ui_is_asleep(void);

/** tosu connection state reported by the host. Clears tosu-derived values on disconnect. */
void ui_set_tosu_connected(bool connected);

/** Hold the LVGL lock around a batch of ui_data_* calls (ui/core/ui_core.h). */
void ui_lock(void);
void ui_unlock(void);

/** Update data sources from the protocol task (takes the LVGL lock). */
void ui_set_number(uint8_t source, double value);
void ui_set_string(uint8_t source, const char *value);

/** Replace a screen layout. Returns false with a reason if it is invalid. */
bool ui_set_layout(uint8_t screen, const ui_layout_t *layout, char *err, size_t err_len);

#ifdef __cplusplus
}
#endif
