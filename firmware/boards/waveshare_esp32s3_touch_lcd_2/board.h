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
 * @brief Configure (or move) the key switch GPIOs: input, pull-up, any-edge interrupt.
 * Moves an already registered key ISR to the new pins. Call from one task only.
 */
esp_err_t board_keys_set_gpio(int key1_gpio, int key2_gpio);

/**
 * @brief Leave the key pins analog: an HE module's sensors drive them. Call
 * before keypad_init. board_keys_set_gpio then records pins without arming
 * them (no pull, no digital input, no interrupt) and no key ISR is attached.
 */
void board_keys_set_analog(bool analog);

/**
 * @brief Initialize LCD backlight PWM (LEDC).
 */
esp_err_t board_backlight_init(void);

/**
 * @brief Register ISR handler for the physical key inputs (arg is the 1-based key index).
 * Requires board_keys_set_gpio first.
 */
esp_err_t board_keys_register_isr(gpio_isr_t isr_handler);

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
 * @brief Wake display from sleep (turns on the panel). The backlight stays off
 * until the caller sets the brightness it wants.
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

/**
 * @brief Detected input module type from the ID voltage divider on GPIO8.
 */
typedef enum {
    BOARD_MODULE_NONE = 0,   /**< No module detected (ID line floating) */
    BOARD_MODULE_MX   = 1,   /**< MX mechanical switch module (~0.30 V) */
    BOARD_MODULE_HE   = 2,   /**< Hall Effect rapid-trigger module (~1.06 V) */
} board_module_type_t;

/**
 * @brief Read the module ID voltage on GPIO8 (ADC1_CH7) and identify the
 *        input module on the carrier's connector. BOARD_MODULE_NONE also
 *        covers hand-wired pads, where nothing drives GPIO8.
 *        Call once at boot before keypad_init.
 */
board_module_type_t board_detect_module(void);

/**
 * @brief What board_detect_module found at boot (NONE before it ran).
 */
board_module_type_t board_boot_module(void);

/**
 * @brief Measure the module ID voltage on GPIO8: with the internal pull-down
 *        off (open) and on (loaded), in mV, -1 where a read failed. Takes
 *        about 10 ms. ADC1 must be free (nothing else here uses it).
 */
esp_err_t board_read_module_id(int *out_open_mv, int *out_loaded_mv);

/**
 * @brief Which module an ID reading names (the thresholds board_detect_module uses).
 */
board_module_type_t board_module_from_id(int open_mv, int loaded_mv);

/**
 * @brief ID voltage (mV) with GPIO2 pulled up and ID pulled down: about
 *        1.5 V when an MX module hangs on a pin 1 <-> 8 reversed cable,
 *        near 0 V or the module's divider otherwise. GPIO2 is reset after.
 *        Call only when GPIO2 is not a key pin.
 */
esp_err_t board_probe_reversed_cable(int *out_mv);

#ifdef __cplusplus
}
#endif
