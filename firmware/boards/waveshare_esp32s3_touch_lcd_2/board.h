#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_vendor.h"
#include "esp_lcd_panel_ops.h"
#include "board_pins.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize low-level board peripherals (GPIOs, backlight timer).
 */
esp_err_t board_init(void);

/**
 * @brief Register ISR handler for the physical key inputs.
 */
esp_err_t board_keys_register_isr(gpio_isr_t isr_handler, void *arg);

/**
 * @brief Read raw physical state of Key 1 (true = pressed / active low).
 */
bool board_key1_read(void);

/**
 * @brief Read raw physical state of Key 2 (true = pressed / active low).
 */
bool board_key2_read(void);

/**
 * @brief Set LCD backlight brightness (0-100%).
 */
void board_backlight_set(uint8_t percent);

/**
 * @brief Get current LCD backlight brightness percentage.
 */
uint8_t board_backlight_get(void);

/**
 * @brief Initialize ST7789 LCD panel.
 * @param[out] out_panel Handle to the initialized esp_lcd panel.
 */
esp_err_t board_display_init(esp_lcd_panel_handle_t *out_panel);

#ifdef __cplusplus
}
#endif
