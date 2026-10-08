#pragma once

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"

#ifdef __cplusplus
extern "C" {
#endif

// Measurements of the carrier and input module PCBs (BoardTestResult on the
// wire). The host turns them into pass/fail checks. GPIO masks: bit n = GPIOn.
typedef struct {
    board_module_type_t boot_module;
    board_module_type_t module;
    int id_mv;                  // -1 = not measured
    int id_loaded_mv;
    int reverse_probe_mv;       // -1 = not run (a module answered, or GPIO2 is a key pin)
    uint8_t key1_gpio;
    uint8_t key2_gpio;
    bool keys_enabled;
    uint64_t tested_gpios;
    uint64_t high_with_pullup;
    uint64_t high_with_pulldown;
    uint64_t bridged_gpios;
} board_test_result_t;

/**
 * @brief Run the board test: module ID, reversed cable, key line pull-ups,
 * spare connector lines. About 30 ms; key edges are ignored for about 1 ms of
 * it. Protocol task only, and only in IDLE: it reconfigures pins and blocks.
 */
esp_err_t board_test_run(board_test_result_t *out);

#ifdef __cplusplus
}
#endif
