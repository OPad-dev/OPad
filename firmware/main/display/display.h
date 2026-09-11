#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "protocol/osupad.pb.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize display engine, create ST7789 panel, and spawn background render task.
 */
esp_err_t display_init(void);

/**
 * @brief Update real-time clock from TimeSync message.
 */
void display_set_time(uint32_t year, uint32_t month, uint32_t day, uint32_t hour, uint32_t minute, uint32_t second);

/**
 * @brief Update live gameplay telemetry HUD state.
 */
void display_update_gameplay_state(const osupad_GameplayDisplayState *state);

/**
 * @brief Notify display system of a user input activity (resets idle timer and wakes screen).
 * MUST be safe to invoke asynchronously from low or high priority tasks.
 */
void display_notify_activity(void);

/**
 * @brief Set display sleep timeout in seconds (0 = never sleep).
 */
void display_set_sleep_timeout(uint32_t seconds);

/**
 * @brief Set display brightness percentage (0 - 100).
 */
void display_set_brightness(uint8_t brightness);

/**
 * @brief Check if display is currently asleep.
 */
bool display_is_asleep(void);

#ifdef __cplusplus
}
#endif
