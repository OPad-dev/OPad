#pragma once

#include "esp_err.h"

/**
 * @brief Start streaming the HE module's raw sensor readings over CDC
 * (CONFIG_OSUPAD_HALL_BENCH builds only; see hall_bench.c).
 */
esp_err_t hall_bench_start(void);

/**
 * @brief Cover the LCD with a live view of both sensors (voltage, change from
 * rest, a travel bar). Call after ui_init; does nothing without a UI.
 */
void hall_bench_show_ui(void);
