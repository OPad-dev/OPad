#include "ui_anim.h"
#include "ui_internal.h"
#include <string.h>

// One animation at a time: a full-screen root on the top layer that dims the
// screen below, two rings, and the grade on a dark disc. Every frame is computed from the time
// since the start alone, so seeking (the PC preview) shows exactly what the
// pad draws at that moment.

#define DIM_OPA 200
#define FADE_IN_MS 200
#define EXIT_MS 350
#define EXIT_START_MS (UI_ANIM_DURATION_MS - EXIT_MS)
#define LETTER_SCALE (LV_SCALE_NONE * 24 / 10)  // Montserrat 48 to about 115 px
#define LETTER_MAX_W 112                         // "SS" is scaled down to fit the disc
#define DISC_D 160
#define DISC_COLOR 0x14141E
#define RING_COUNT 2

static lv_obj_t *s_root;
static lv_obj_t *s_rings[RING_COUNT];
static lv_obj_t *s_disc;
static lv_obj_t *s_letter;
static int32_t s_letter_scale;
static uint8_t s_id;

// ---- Easing (x in 0..1) -------------------------------------------------------------

static float progress(uint32_t t, uint32_t start, uint32_t len)
{
    if (t <= start) return 0.0f;
    if (t >= start + len) return 1.0f;
    return (float)(t - start) / (float)len;
}

static float ease_out_cubic(float x)
{
    float i = 1.0f - x;
    return 1.0f - i * i * i;
}

static float ease_in_cubic(float x)
{
    return x * x * x;
}

static float ease_out_back(float x)
{
    const float c1 = 1.70158f, c3 = c1 + 1.0f;
    float i = x - 1.0f;
    return 1.0f + c3 * i * i * i + c1 * i * i;
}

static float ease_out_bounce(float x)
{
    const float n = 7.5625f, d = 2.75f;
    if (x < 1.0f / d) return n * x * x;
    if (x < 2.0f / d) { x -= 1.5f / d;   return n * x * x + 0.75f; }
    if (x < 2.5f / d) { x -= 2.25f / d;  return n * x * x + 0.9375f; }
    x -= 2.625f / d;
    return n * x * x + 0.984375f;
}

static lv_opa_t opa_of(float x)
{
    if (x <= 0.0f) return LV_OPA_TRANSP;
    if (x >= 1.0f) return LV_OPA_COVER;
    return (lv_opa_t)(x * 255.0f);
}

// ---- Frames -------------------------------------------------------------------------

// The grade and its disc move together; only the letter needs a transform
static void set_letter(float scale, int32_t dy, float opa)
{
    lv_opa_t o = opa_of(scale > 0.0f ? opa : 0.0f);
    int32_t s = (int32_t)((float)s_letter_scale * scale);
    lv_obj_set_style_transform_scale(s_letter, s > 1 ? s : 1, 0);
    lv_obj_set_style_translate_y(s_letter, dy, 0);
    lv_obj_set_style_opa(s_letter, o, 0);

    int32_t d = (int32_t)(DISC_D * scale);
    lv_obj_set_size(s_disc, d > 1 ? d : 1, d > 1 ? d : 1);
    lv_obj_set_style_translate_y(s_disc, dy, 0);
    lv_obj_set_style_opa(s_disc, o, 0);
}

static void set_ring(int i, uint32_t t, uint32_t start)
{
    const uint32_t len = 700;
    lv_obj_t *ring = s_rings[i];
    if (t < start || t >= start + len) {
        lv_obj_add_flag(ring, LV_OBJ_FLAG_HIDDEN);
        return;
    }
    float p = progress(t, start, len);
    int32_t d = 40 + (int32_t)(300.0f * ease_out_cubic(p));
    lv_obj_remove_flag(ring, LV_OBJ_FLAG_HIDDEN);
    lv_obj_set_size(ring, d, d);
    lv_obj_set_style_border_width(ring, 2 + (int32_t)(8.0f * (1.0f - p)), 0);
    lv_obj_set_style_border_opa(ring, opa_of(1.0f - p), 0);
}

static void apply(uint32_t t)
{
    if (!s_root) return;

    float out = progress(t, EXIT_START_MS, EXIT_MS);
    lv_obj_set_style_bg_opa(s_root, (lv_opa_t)(DIM_OPA * progress(t, 0, FADE_IN_MS) * (1.0f - out)), 0);

    for (int i = 0; i < RING_COUNT; i++) {
        if (s_id == UI_ANIM_BURST) {
            set_ring(i, t, 120 * (uint32_t)i);
        } else {
            lv_obj_add_flag(s_rings[i], LV_OBJ_FLAG_HIDDEN);
        }
    }

    switch (s_id) {
    case UI_ANIM_DROP: {
        float in = ease_out_bounce(progress(t, 0, 700));
        int32_t dy = (int32_t)(-170.0f * (1.0f - in) + 60.0f * ease_in_cubic(out));
        set_letter(1.0f, dy, t == 0 ? 0.0f : 1.0f - out);
        break;
    }
    case UI_ANIM_BURST: {
        float in = progress(t, 80, 300);
        float scale = in > 0.0f ? 0.4f + 0.6f * ease_out_back(in) : 0.0f;
        set_letter(scale * (1.0f + 0.25f * out), 0, 1.0f - out);
        break;
    }
    case UI_ANIM_POP:
    default: {
        float scale = ease_out_back(progress(t, 0, 450));
        set_letter(scale * (1.0f + 0.25f * out), 0, 1.0f - out);
        break;
    }
    }
}

// ---- Lifetime -----------------------------------------------------------------------

static void anim_exec_cb(void *var, int32_t value)
{
    (void)var;
    apply(value > 0 ? (uint32_t)value : 0);
}

static void delete_objects(void)
{
    if (s_root) {
        lv_obj_delete(s_root);
    }
    s_root = NULL;
    s_disc = NULL;
    s_letter = NULL;
    memset(s_rings, 0, sizeof(s_rings));
    s_id = UI_ANIM_NONE;
}

static void anim_completed_cb(lv_anim_t *a)
{
    (void)a;
    delete_objects();  // LVGL is already removing this lv_anim
}

static lv_obj_t *plain(lv_obj_t *parent)
{
    lv_obj_t *obj = lv_obj_create(parent);
    lv_obj_remove_style_all(obj);
    lv_obj_remove_flag(obj, LV_OBJ_FLAG_CLICKABLE | LV_OBJ_FLAG_SCROLLABLE);
    return obj;
}

void ui_anim_stop(void)
{
    if (s_root) {
        lv_anim_delete(s_root, anim_exec_cb);
    }
    delete_objects();
}

bool ui_anim_active(void)
{
    return s_root != NULL;
}

bool ui_anim_play(uint8_t id, const char *grade)
{
    ui_anim_stop();
    if (id == UI_ANIM_NONE) {
        return false;
    }
    if (id > UI_ANIM_BUILTIN_COUNT) {
        id = UI_ANIM_POP;  // a newer built-in, or an upload slot this pad does not have
    }
    if (!grade) {
        grade = "";
    }
    uint32_t color = ui_grade_color(grade);

    s_id = id;
    s_root = plain(lv_layer_top());
    lv_obj_set_size(s_root, UI_SCREEN_W, UI_SCREEN_H);
    lv_obj_set_style_bg_color(s_root, lv_color_black(), 0);

    for (int i = 0; i < RING_COUNT; i++) {
        lv_obj_t *ring = plain(s_root);
        lv_obj_set_style_radius(ring, LV_RADIUS_CIRCLE, 0);
        lv_obj_set_style_border_color(ring, lv_color_hex(color), 0);
        lv_obj_align(ring, LV_ALIGN_CENTER, 0, 0);
        s_rings[i] = ring;
    }

    s_disc = plain(s_root);
    lv_obj_set_style_radius(s_disc, LV_RADIUS_CIRCLE, 0);
    lv_obj_set_style_bg_color(s_disc, lv_color_hex(DISC_COLOR), 0);
    lv_obj_set_style_bg_opa(s_disc, LV_OPA_COVER, 0);
    lv_obj_set_style_border_color(s_disc, lv_color_hex(color), 0);
    lv_obj_set_style_border_width(s_disc, 4, 0);
    lv_obj_align(s_disc, LV_ALIGN_CENTER, 0, 0);

    s_letter = lv_label_create(s_root);
    lv_obj_set_style_text_font(s_letter, &lv_font_montserrat_48, 0);
    lv_obj_set_style_text_color(s_letter, lv_color_hex(color), 0);
    lv_obj_set_style_transform_pivot_x(s_letter, lv_pct(50), 0);
    lv_obj_set_style_transform_pivot_y(s_letter, lv_pct(50), 0);
    lv_label_set_text(s_letter, ui_grade_shown(grade));
    lv_obj_align(s_letter, LV_ALIGN_CENTER, 0, 0);
    lv_obj_update_layout(s_letter);
    int32_t w = lv_obj_get_width(s_letter);
    s_letter_scale = LETTER_SCALE;
    if (w > 0 && w * LETTER_SCALE / LV_SCALE_NONE > LETTER_MAX_W) {
        s_letter_scale = LETTER_MAX_W * LV_SCALE_NONE / w;
    }

    apply(0);

    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, s_root);
    lv_anim_set_values(&a, 0, UI_ANIM_DURATION_MS);
    lv_anim_set_duration(&a, UI_ANIM_DURATION_MS);
    lv_anim_set_exec_cb(&a, anim_exec_cb);
    lv_anim_set_completed_cb(&a, anim_completed_cb);
    lv_anim_start(&a);
    return true;
}

void ui_anim_seek(uint32_t t_ms)
{
    if (!s_root) return;
    lv_anim_delete(s_root, anim_exec_cb);
    apply(t_ms < UI_ANIM_DURATION_MS ? t_ms : UI_ANIM_DURATION_MS);
}
