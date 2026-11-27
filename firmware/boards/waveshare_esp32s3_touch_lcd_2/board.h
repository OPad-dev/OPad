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
 * @brief Leave the key pins alone entirely: an HE V3 module's SPI lines run
 * on them. Call before keypad_init. board_keys_set_gpio then only records
 * the pins, and no key ISR is attached.
 */
void board_keys_set_detached(bool detached);

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
 * @brief Give key 3 a pin (MX V3: GPIO6, fixed by the module). Once, before
 * keypad_init; the pin stays untouched until board_key3_arm.
 */
esp_err_t board_key3_set_gpio(int gpio);

/**
 * @brief Arm key 3's pin (input, pull-up, any-edge interrupt, the key ISR
 * with arg 3) or park it analog. No-op without board_key3_set_gpio. Keypad task only.
 */
esp_err_t board_key3_arm(bool armed);

/**
 * @brief Read raw physical state of Key 3 (true = pressed); false while not armed.
 */
bool board_key3_read(void);

/** @brief Key 3's pin, or -1 when the module has none the switches drive. */
int board_get_key3_gpio(void);

/**
 * @brief Set LCD backlight brightness (0-100%).
 */
void board_backlight_set(uint8_t percent);

/**
 * @brief Light the backlight fully and latch the pin, so it stays on through
 * a restart into the ROM bootloader (flashing). Released by
 * board_backlight_init.
 */
void board_backlight_hold_on(void);

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
 * @brief Detected input module type from the ID voltage divider (GPIO8, or
 * GPIO7 for a V3 module on a turned-over FFC). Values match module_id_t
 * (input/module_id.h), which holds the thresholds.
 */
typedef enum {
    BOARD_MODULE_NONE  = 0,  /**< No module detected (ID line floating) */
    BOARD_MODULE_MX    = 1,  /**< MX V1, two keys (~0.30 V) */
    BOARD_MODULE_HE    = 2,  /**< Hall Effect V1, analog on the key pins (~1.06 V) */
    BOARD_MODULE_HE_V3 = 3,  /**< Hall Effect V3, ADS7028 over SPI (~1.65 V) */
    BOARD_MODULE_MX_V3 = 4,  /**< MX V3, three keys (~2.24 V) */
} board_module_type_t;

/**
 * @brief Identify the input module on the carrier's connector: ID on GPIO8
 *        (ADC1_CH7), and when nothing is there, GPIO7 (ADC1_CH6) for a V3
 *        module on a turned-over FFC. BOARD_MODULE_NONE also covers
 *        hand-wired pads, where nothing drives either. Later ID reads use
 *        the pin the module was found on. Call once at boot before keypad_init.
 */
board_module_type_t board_detect_module(void);

/**
 * @brief What board_detect_module found at boot (NONE before it ran).
 */
board_module_type_t board_boot_module(void);

/**
 * @brief True when board_detect_module found a V3 module's ID on GPIO7: its
 *        FFC is turned over (contact k to 15 - k), so every module line sits on
 *        another GPIO (board_pins.h, BOARD_V3_*).
 */
bool board_boot_cable_reversed(void);

/**
 * @brief GPIO the module ID is read from: GPIO8, or GPIO7 after
 *        board_detect_module found a turned-over V3 cable.
 */
int board_module_id_gpio(void);

/**
 * @brief Measure the module ID voltage (board_module_id_gpio): with the internal pull-down
 *        off (open) and on (loaded), in mV, -1 where a read failed. Takes
 *        about 10 ms. ADC1 must be free (nothing else here uses it).
 */
esp_err_t board_read_module_id(int *out_open_mv, int *out_loaded_mv);

/**
 * @brief Whether anything is on the ID line, from one conversion with the
 *        pull-down on (~50 us): for noticing a module plugged in or out fast.
 *        It cannot tell MX from HE; board_read_module_id does that.
 */
esp_err_t board_module_present_fast(bool *out_present, int *out_mv);

/**
 * @brief Close the ADC1 oneshot unit the ID reads keep open, so ADC1
 *        continuous mode (HE input) can take the ADC again.
 */
void board_module_id_release(void);

/**
 * @brief Which module an ID reading names (module_id_classify).
 */
board_module_type_t board_module_from_id(int open_mv, int loaded_mv);

/**
 * @brief Measure the ID voltage on one pin (GPIO8 or GPIO7), as
 *        board_read_module_id does: the board test reads both on a V3 carrier.
 */
esp_err_t board_read_module_id_on(int gpio, int *out_open_mv, int *out_loaded_mv);

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
