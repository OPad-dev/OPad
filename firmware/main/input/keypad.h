#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "input/key_event.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint8_t keycode1;       // USB HID Keycode for Key 1 (default 0x1D = 'z')
    uint8_t keycode2;       // USB HID Keycode for Key 2 (default 0x1B = 'x')
    uint32_t debounce_us;   // Debounce interval in microseconds (default 5000us)
    uint8_t key1_gpio;      // Switch GPIO for Key 1 (default 14)
    uint8_t key2_gpio;      // Switch GPIO for Key 2 (default 9)
} keypad_config_t;

/**
 * @brief Initialize keypad subsystem, GPIOs, ISR, and high-priority input processing task.
 */
esp_err_t keypad_init(const keypad_config_t *config);

/**
 * @brief Update keypad configuration (debouncing, key mappings and switch GPIOs).
 * Once the keypad task runs, applied by it while both keys are released.
 */
void keypad_set_config(const keypad_config_t *config);

/**
 * @brief Get current keypad configuration.
 */
void keypad_get_config(keypad_config_t *out_config);

/**
 * @brief Start scanning the allowed key GPIO pins (pulled up) for one pressed to GND.
 * The scan runs on the keypad task and replaces one still running, which then
 * never reports a result. Returns at once.
 * @param timeout_ms How long to scan, in milliseconds.
 * @param exclude_gpio Pin number to ignore (e.g. key1 when detecting key2), or 0 for none.
 * @return Scan id for keypad_detect_pin_result, or 0 if it cannot run (before keypad_init).
 */
uint32_t keypad_detect_pin_start(uint32_t timeout_ms, uint32_t exclude_gpio);

/**
 * @brief True once the scan `id` has finished; *out_pin is then the detected
 * GPIO number (> 0), or -1 if none was pressed before the timeout.
 */
bool keypad_detect_pin_result(uint32_t id, int *out_pin);

/** Result of keypad_line_test */
typedef struct {
    bool ran;                                 // false: key input is off (Hall Effect module)
    uint8_t gpio[KEY_ID_COUNT];               // the pins tested
    bool high_with_pullup[KEY_ID_COUNT];      // false: switch held, or line shorted to GND
    bool high_with_pulldown[KEY_ID_COUNT];    // true: an external pull-up (MX module) reaches the pin
} keypad_line_test_t;

/**
 * @brief Test the key lines for a short to GND and for the module's pull-up.
 * Runs on the keypad task (key edges ignored for about 1 ms, nothing reported
 * in between) and blocks the caller until it has, up to 100 ms. Board test
 * only: call from the protocol task, in IDLE.
 */
esp_err_t keypad_line_test(keypad_line_test_t *out);

/**
 * @brief Turn key input off (keys released, pins parked analog with no
 * interrupt) or back on (pins armed again). Used when the connected input
 * module is not one this firmware can read, or was swapped while powered.
 */
void keypad_set_input_enabled(bool enabled);

#ifdef __cplusplus
}
#endif
