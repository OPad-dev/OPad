#pragma once

#include "esp_err.h"

/**
 * @brief Start streaming the HE module's raw sensor readings over CDC
 * (CONFIG_OSUPAD_HALL_BENCH builds only; see hall_bench.c).
 */
esp_err_t hall_bench_start(void);
