// Headless LVGL display for rendering OPad layouts on the host.

#include "ui_core.h"
#include <string.h>
#include <time.h>

static lv_display_t *s_disp;
static uint8_t s_fb[UI_SCREEN_W * UI_SCREEN_H * 2];   // RGB565, full frame (direct mode)
static uint32_t s_fake_ms;

static uint32_t preview_tick(void)
{
    return s_fake_ms;
}

static void flush_cb(lv_display_t *disp, const lv_area_t *area, uint8_t *px_map)
{
    (void)area;
    (void)px_map;  // direct mode: LVGL renders straight into s_fb
    lv_display_flush_ready(disp);
}

void preview_init(void)
{
    if (s_disp) {
        return;
    }
    lv_init();
    lv_tick_set_cb(preview_tick);
    s_disp = lv_display_create(UI_SCREEN_W, UI_SCREEN_H);
    lv_display_set_color_format(s_disp, LV_COLOR_FORMAT_RGB565);
    lv_display_set_buffers(s_disp, s_fb, NULL, sizeof(s_fb), LV_DISPLAY_RENDER_MODE_DIRECT);
    lv_display_set_flush_cb(s_disp, flush_cb);
    ui_data_init();
}

// Build the layout, render one frame and write RGBA8888 into out (320*240*4 bytes).
// Returns 0 on success, -1 if the layout is invalid.
int preview_render(const ui_layout_t *layout, uint8_t *out_rgba)
{
    lv_obj_t *scr = ui_screen_create(layout);
    if (!scr) {
        return -1;
    }
    lv_obj_t *old = lv_screen_active();
    lv_screen_load(scr);
    if (old && old != scr) {
        ui_screen_delete(old);
    }
    s_fake_ms += 1000;
    lv_obj_invalidate(scr);
    lv_refr_now(s_disp);

    for (int i = 0; i < UI_SCREEN_W * UI_SCREEN_H; i++) {
        uint16_t c = (uint16_t)(s_fb[2 * i] | (s_fb[2 * i + 1] << 8));
        uint8_t r = (c >> 11) & 0x1F, g = (c >> 5) & 0x3F, b = c & 0x1F;
        out_rgba[4 * i] = (uint8_t)((r * 527 + 23) >> 6);
        out_rgba[4 * i + 1] = (uint8_t)((g * 259 + 33) >> 6);
        out_rgba[4 * i + 2] = (uint8_t)((b * 527 + 23) >> 6);
        out_rgba[4 * i + 3] = 0xFF;
    }
    return 0;
}

size_t preview_sizeof_widget(void) { return sizeof(ui_widget_t); }
size_t preview_sizeof_layout(void) { return sizeof(ui_layout_t); }
