#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include "input/he_cal_button.h"

// Screen is 320x240. Button placed at (70, 175) with size 180x44. Max excursion 30 px.
#define BTN_X 70
#define BTN_Y 175
#define BTN_W 180
#define BTN_H 44
#define BTN_MAX_DIST 30

static void test_clean_click(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // Land inside (100, 190)
    bool click = he_cal_button_feed_touch(&btn, true, 100, 190);
    assert(!click);

    // Minor tremor / small movement (dx=4, dy=3)
    click = he_cal_button_feed_touch(&btn, true, 104, 193);
    assert(!click);

    // Lift inside (103, 191)
    click = he_cal_button_feed_touch(&btn, false, 103, 191);
    assert(click);

    printf("✓ test_clean_click passed\n");
}

static void test_excursion_exceeded(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // Land inside
    he_cal_button_feed_touch(&btn, true, 100, 190);

    // Move 35 px horizontally (swipe motion across button)
    he_cal_button_feed_touch(&btn, true, 135, 190);

    // Lift still inside button
    bool click = he_cal_button_feed_touch(&btn, false, 130, 190);
    assert(!click); // Rejected because excursion >= BTN_MAX_DIST (30)

    // Exact boundary: 29 px (< 30) succeeds
    he_cal_button_feed_touch(&btn, true, 100, 190);
    he_cal_button_feed_touch(&btn, true, 129, 190);
    assert(he_cal_button_feed_touch(&btn, false, 129, 190));

    // Exact boundary: 30 px (== 30) is rejected
    he_cal_button_feed_touch(&btn, true, 100, 190);
    he_cal_button_feed_touch(&btn, true, 130, 190);
    assert(!he_cal_button_feed_touch(&btn, false, 130, 190));

    printf("✓ test_excursion_exceeded passed\n");
}

static void test_land_outside(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // Land outside at (50, 190) - left of button
    he_cal_button_feed_touch(&btn, true, 50, 190);

    // Move inside button
    he_cal_button_feed_touch(&btn, true, 80, 190);

    // Lift inside button
    bool click = he_cal_button_feed_touch(&btn, false, 85, 190);
    assert(!click); // Rejected because landing was outside

    printf("✓ test_land_outside passed\n");
}

static void test_release_sample_counts_toward_excursion(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);
    he_cal_button_feed_touch(&btn, true, 100, 190);
    assert(!he_cal_button_feed_touch(&btn, false, 130, 190));
    he_cal_button_feed_touch(&btn, true, 100, 190);
    assert(he_cal_button_feed_touch(&btn, false, 129, 190));
}

static void test_lift_outside(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // Land inside near left border (72, 190)
    he_cal_button_feed_touch(&btn, true, 72, 190);

    // Move 10 px left, leaving the rectangle
    he_cal_button_feed_touch(&btn, true, 62, 190);

    // Lift outside (60, 190)
    bool click = he_cal_button_feed_touch(&btn, false, 60, 190);
    assert(!click); // Rejected because lift was outside

    printf("✓ test_lift_outside passed\n");
}

static void test_boundary_inclusive(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // Exact top-left corner (BTN_X, BTN_Y)
    he_cal_button_feed_touch(&btn, true, BTN_X, BTN_Y);
    bool click = he_cal_button_feed_touch(&btn, false, BTN_X, BTN_Y);
    assert(click);

    // Exact bottom-right corner pixel (BTN_X + BTN_W - 1, BTN_Y + BTN_H - 1)
    he_cal_button_feed_touch(&btn, true, BTN_X + BTN_W - 1, BTN_Y + BTN_H - 1);
    click = he_cal_button_feed_touch(&btn, false, BTN_X + BTN_W - 1, BTN_Y + BTN_H - 1);
    assert(click);

    // Just outside right border
    assert(!he_cal_button_is_inside(&btn.rect, BTN_X + BTN_W, BTN_Y));
    // Just outside bottom border
    assert(!he_cal_button_is_inside(&btn.rect, BTN_X, BTN_Y + BTN_H));
    // Just outside left
    assert(!he_cal_button_is_inside(&btn.rect, BTN_X - 1, BTN_Y));
    // Just outside top
    assert(!he_cal_button_is_inside(&btn.rect, BTN_X, BTN_Y - 1));

    printf("✓ test_boundary_inclusive passed\n");
}

static void test_reset_and_successive_clicks(void)
{
    he_cal_button_t btn;
    he_cal_button_init(&btn, BTN_X, BTN_Y, BTN_W, BTN_H, BTN_MAX_DIST);

    // First valid click
    he_cal_button_feed_touch(&btn, true, 100, 190);
    bool click1 = he_cal_button_feed_touch(&btn, false, 100, 190);
    assert(click1);

    // Reset while down
    he_cal_button_feed_touch(&btn, true, 100, 190);
    he_cal_button_reset(&btn);
    bool click_aborted = he_cal_button_feed_touch(&btn, false, 100, 190);
    assert(!click_aborted);

    // Second valid click
    he_cal_button_feed_touch(&btn, true, 120, 195);
    bool click2 = he_cal_button_feed_touch(&btn, false, 122, 196);
    assert(click2);

    printf("✓ test_reset_and_successive_clicks passed\n");
}

int main(void)
{
    printf("=== Running Hall Effect Calibration Button Host Tests ===\n");
    test_clean_click();
    test_excursion_exceeded();
    test_land_outside();
    test_release_sample_counts_toward_excursion();
    test_lift_outside();
    test_boundary_inclusive();
    test_reset_and_successive_clicks();
    printf("All Hall Effect calibration button tests passed successfully!\n");
    return 0;
}
