#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Distance a finger has to travel along one axis before it is a swipe, and
// again for every further vertical step, in screen pixels (320x240 screen)
#define GESTURE_SWIPE_PX    30

typedef enum {
    GESTURE_NONE = 0,
    GESTURE_UP,
    GESTURE_DOWN,
    GESTURE_LEFT,
    GESTURE_RIGHT,
} gesture_t;

typedef enum {
    GESTURE_AXIS_NONE = 0,  // Not moved far enough yet: still a tap or a hold
    GESTURE_AXIS_VERTICAL,
    GESTURE_AXIS_HORIZONTAL,
} gesture_axis_t;

/*
 * Follows one finger from touch-down to release, in screen coordinates (x to
 * the right, y down). A swipe locks to the axis it started on: vertical keeps
 * stepping every GESTURE_SWIPE_PX for as long as the finger moves (one volume
 * step each), horizontal fires once per touch.
 */
typedef struct {
    int16_t origin_x;
    int16_t origin_y;
    gesture_axis_t axis;
    bool horizontal_fired;
} gesture_tracker_t;

void gesture_begin(gesture_tracker_t *g, int16_t x, int16_t y);

/**
 * @brief Feeds the next position of the finger.
 * @return The swipe this position completes, or GESTURE_NONE. One position
 *         can complete several vertical steps (a fast finger, or a sample
 *         read late): call again with the same position until GESTURE_NONE.
 */
gesture_t gesture_feed(gesture_tracker_t *g, int16_t x, int16_t y);

/**
 * @brief True once the touch has turned into a swipe.
 */
bool gesture_is_swipe(const gesture_tracker_t *g);

/**
 * @brief How far (px, the larger axis) the finger is from where it landed,
 *        for telling a resting finger from one that is starting a swipe.
 *        Meaningful until the touch becomes a swipe.
 */
int gesture_distance(const gesture_tracker_t *g, int16_t x, int16_t y);

#ifdef __cplusplus
}
#endif
