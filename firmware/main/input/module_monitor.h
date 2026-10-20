#pragma once

#include "esp_err.h"

/**
 * @brief Watch the input module connector for a module swapped while powered
 * (module_monitor.c, decisions in module_watch.h). Call once at boot, after
 * keypad_init and ui_init.
 */
esp_err_t module_monitor_start(void);
