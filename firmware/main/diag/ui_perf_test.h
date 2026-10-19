#pragma once

/**
 * @brief CONFIG_OSUPAD_UI_PERF_TEST builds only: loop a GIF over the UI with
 * LVGL's FPS and memory monitors (see ui_perf_test.c). Call after ui_init.
 */
void ui_perf_test_start(void);
