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
 * @brief Initialize low-level board key GPIOs.
 */
esp_err_t board_init(void);

/**
 * @brief Initialize LCD backlight PWM (LEDC).
 */
esp_err_t board_backlight_init(void);

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
 * @param[out] out_io Optional handle to the initialized esp_lcd panel IO.
 * @param[out] out_panel Optional handle to the initialized esp_lcd panel.
 */
esp_err_t board_display_init(esp_lcd_panel_io_handle_t *out_io, esp_lcd_panel_handle_t *out_panel);

/**
 * @brief Get the initialized LCD panel handle.
 */
esp_lcd_panel_handle_t board_display_get_panel_handle(void);

/**
 * @brief Get the initialized LCD panel IO handle.
 */
esp_lcd_panel_io_handle_t board_display_get_io_handle(void);

/**
 * @brief Set display backlight brightness (0-100%).
 */
void board_display_set_brightness(uint8_t percent);

/**
 * @brief Put display to sleep (turns off backlight and panel).
 */
void board_display_sleep(void);

/**
 * @brief Wake display from sleep (turns on panel and restores backlight).
 */
void board_display_wake(void);

/**
 * @brief Get GPIO number for physical Key 1.
 */
int board_get_key1_gpio(void);

/**
 * @brief Get GPIO number for physical Key 2.
 */
int board_get_key2_gpio(void);

#ifdef __cplusplus
}
#endif
