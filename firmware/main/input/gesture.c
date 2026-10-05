#include "gesture.h"
#include <stdlib.h>

void gesture_begin(gesture_tracker_t *g, int16_t x, int16_t y)
{
    g->origin_x = x;
    g->origin_y = y;
    g->axis = GESTURE_AXIS_NONE;
    g->horizontal_fired = false;
}

bool gesture_is_swipe(const gesture_tracker_t *g)
{
    return g->axis != GESTURE_AXIS_NONE;
}

int gesture_distance(const gesture_tracker_t *g, int16_t x, int16_t y)
{
    int ax = abs(x - g->origin_x);
    int ay = abs(y - g->origin_y);
    return ax > ay ? ax : ay;
}

gesture_t gesture_feed(gesture_tracker_t *g, int16_t x, int16_t y)
{
    int dx = x - g->origin_x;
    int dy = y - g->origin_y;

    if (g->axis == GESTURE_AXIS_NONE) {
        int ax = abs(dx);
        int ay = abs(dy);
        if (ax < GESTURE_SWIPE_PX && ay < GESTURE_SWIPE_PX) {
            return GESTURE_NONE;
        }
        // One axis has to clearly lead; a diagonal waits for more movement
        if (2 * ay >= 3 * ax) {
            g->axis = GESTURE_AXIS_VERTICAL;
        } else if (2 * ax >= 3 * ay) {
            g->axis = GESTURE_AXIS_HORIZONTAL;
        } else {
            return GESTURE_NONE;
        }
    }

    if (g->axis == GESTURE_AXIS_VERTICAL) {
        // The origin moves one step at a time, so a long drag gives one step
        // per GESTURE_SWIPE_PX and turning back steps the other way
        if (dy <= -GESTURE_SWIPE_PX) {
            g->origin_y -= GESTURE_SWIPE_PX;
            return GESTURE_UP;
        }
        if (dy >= GESTURE_SWIPE_PX) {
            g->origin_y += GESTURE_SWIPE_PX;
            return GESTURE_DOWN;
        }
        return GESTURE_NONE;
    }

    if (!g->horizontal_fired && abs(dx) >= GESTURE_SWIPE_PX) {
        g->horizontal_fired = true;
        return dx < 0 ? GESTURE_LEFT : GESTURE_RIGHT;
    }
    return GESTURE_NONE;
}
