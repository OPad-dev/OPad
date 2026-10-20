#include "he_cal_button.h"
#include <stdlib.h>

void he_cal_button_init(he_cal_button_t *btn, int16_t x, int16_t y, int16_t w, int16_t h, int16_t max_dist)
{
    if (!btn) {
        return;
    }
    btn->rect.x = x;
    btn->rect.y = y;
    btn->rect.w = w;
    btn->rect.h = h;
    btn->max_dist = max_dist;
    btn->is_down = false;
    btn->down_x = 0;
    btn->down_y = 0;
    btn->max_excursion = 0;
    btn->started_inside = false;
}

bool he_cal_button_is_inside(const he_cal_rect_t *rect, int16_t x, int16_t y)
{
    if (!rect || rect->w <= 0 || rect->h <= 0) {
        return false;
    }
    return (x >= rect->x && x < (rect->x + rect->w) &&
            y >= rect->y && y < (rect->y + rect->h));
}

bool he_cal_button_feed_touch(he_cal_button_t *btn, bool down, int16_t x, int16_t y)
{
    if (!btn) {
        return false;
    }

    if (!btn->is_down) {
        if (down) {
            btn->is_down = true;
            btn->down_x = x;
            btn->down_y = y;
            btn->max_excursion = 0;
            btn->started_inside = he_cal_button_is_inside(&btn->rect, x, y);
        }
        return false;
    }

    // Touch is already down
    if (down) {
        int16_t dx = (int16_t)abs(x - btn->down_x);
        int16_t dy = (int16_t)abs(y - btn->down_y);
        int16_t dist = dx > dy ? dx : dy;
        if (dist > btn->max_excursion) {
            btn->max_excursion = dist;
        }
        return false;
    }

    // Touch was released (finger lifted)
    btn->is_down = false;
    bool inside = he_cal_button_is_inside(&btn->rect, x, y);
    bool valid_click = btn->started_inside && inside && (btn->max_excursion < btn->max_dist);
    return valid_click;
}

void he_cal_button_reset(he_cal_button_t *btn)
{
    if (!btn) {
        return;
    }
    btn->is_down = false;
    btn->down_x = 0;
    btn->down_y = 0;
    btn->max_excursion = 0;
    btn->started_inside = false;
}
