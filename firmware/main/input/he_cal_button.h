#pragma once

/*
 * Hall Effect calibration UI button hit testing.
 * docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Touch button hit test rules:
 * - Touch must land inside button rectangle.
 * - During the touch, max excursion must be strictly less than GESTURE_SWIPE_PX (30 px).
 * - Touch must lift inside the button rectangle.
 *
 * Pure C, no ESP-IDF/FreeRTOS dependencies, host-testable.
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    int16_t x;
    int16_t y;
    int16_t w;
    int16_t h;
} he_cal_rect_t;

typedef struct {
    he_cal_rect_t rect;
    int16_t max_dist;       // Max excursion allowed (< max_dist), e.g. GESTURE_SWIPE_PX (30)
    bool is_down;
    int16_t down_x;
    int16_t down_y;
    int16_t max_excursion;
    bool started_inside;
} he_cal_button_t;

/**
 * @brief Initialize button tracker with bounds and max distance.
 */
void he_cal_button_init(he_cal_button_t *btn, int16_t x, int16_t y, int16_t w, int16_t h, int16_t max_dist);

/**
 * @brief Test if coordinate (x, y) is inside the rectangle bounds [x, x+w) x [y, y+h).
 */
bool he_cal_button_is_inside(const he_cal_rect_t *rect, int16_t x, int16_t y);

/**
 * @brief Feed a touch sample.
 * @param btn Pointer to button tracker.
 * @param down True if finger is currently down, false if up/released.
 * @param x Touch X coordinate.
 * @param y Touch Y coordinate.
 * @return True on the exact frame the finger lifts inside after starting inside and moving < max_dist.
 */
bool he_cal_button_feed_touch(he_cal_button_t *btn, bool down, int16_t x, int16_t y);

/**
 * @brief Cancel / reset active touch tracking (e.g. on screen switch or timeout).
 */
void he_cal_button_reset(he_cal_button_t *btn);

#ifdef __cplusplus
}
#endif
