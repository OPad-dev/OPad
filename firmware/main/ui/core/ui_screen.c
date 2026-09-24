#include "ui_core.h"
#include "ui_internal.h"
#include <stdio.h>
#include <string.h>

static const lv_font_t *const s_fonts[UI_FONT_COUNT] = {
    [UI_FONT_12] = &lv_font_montserrat_12,
    [UI_FONT_14] = &lv_font_montserrat_14,
    [UI_FONT_16] = &lv_font_montserrat_16,
    [UI_FONT_20] = &lv_font_montserrat_20,
    [UI_FONT_24] = &lv_font_montserrat_24,
    [UI_FONT_32] = &lv_font_montserrat_32,
    [UI_FONT_48] = &lv_font_montserrat_48,
};

static const lv_text_align_t s_text_align[] = {
    [UI_ALIGN_LEFT] = LV_TEXT_ALIGN_LEFT,
    [UI_ALIGN_CENTER] = LV_TEXT_ALIGN_CENTER,
    [UI_ALIGN_RIGHT] = LV_TEXT_ALIGN_RIGHT,
};

static const lv_font_t *widget_font(const ui_widget_t *w)
{
    return s_fonts[w->font < UI_FONT_COUNT ? w->font : UI_FONT_14];
}

// Black or white, whichever is readable on the given background
static lv_color_t contrast_text(uint32_t rgb)
{
    uint32_t r = (rgb >> 16) & 0xFF, g = (rgb >> 8) & 0xFF, b = rgb & 0xFF;
    return (r * 299 + g * 587 + b * 114) / 1000 > 150 ? lv_color_black() : lv_color_white();
}

static lv_obj_t *plain_obj(lv_obj_t *parent)
{
    lv_obj_t *obj = lv_obj_create(parent);
    lv_obj_remove_style_all(obj);
    lv_obj_remove_flag(obj, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);
    return obj;
}

// Single-line label, vertically centered inside the widget box
static lv_obj_t *box_label(lv_obj_t *parent, const ui_widget_t *w, int x, int y, int width, int height)
{
    const lv_font_t *font = widget_font(w);
    int32_t line_h = lv_font_get_line_height(font);
    lv_obj_t *label = lv_label_create(parent);
    lv_obj_remove_style_all(label);
    lv_obj_set_style_text_font(label, font, 0);
    lv_obj_set_style_text_align(label, s_text_align[w->align <= UI_ALIGN_RIGHT ? w->align : 0], 0);
    lv_label_set_long_mode(label, LV_LABEL_LONG_MODE_DOTS);
    lv_obj_set_size(label, width, line_h);
    lv_obj_set_pos(label, x, y + (height - line_h) / 2);
    return label;
}

static void apply_box_style(lv_obj_t *obj, const ui_widget_t *w)
{
    lv_obj_set_style_radius(obj, w->radius, 0);
    if (w->flags & UI_FLAG_BG_FILL) {
        lv_obj_set_style_bg_color(obj, lv_color_hex(w->bg), 0);
        lv_obj_set_style_bg_opa(obj, LV_OPA_COVER, 0);
    }
    if (w->flags & UI_FLAG_BORDER) {
        lv_obj_set_style_border_color(obj, lv_color_hex(w->fg), 0);
        lv_obj_set_style_border_width(obj, 1, 0);
    }
}

// ---- Bound text ------------------------------------------------------------------

static void set_hidden_if_empty(lv_obj_t *root, const ui_widget_t *w)
{
    if (w->flags & UI_FLAG_HIDE_WHEN_EMPTY) {
        lv_obj_set_flag(root, LV_OBJ_FLAG_HIDDEN, ui_data_is_empty(w->source));
    }
}

static void text_observer_cb(lv_observer_t *observer, lv_subject_t *subject)
{
    (void)subject;
    lv_obj_t *label = lv_observer_get_target_obj(observer);
    const ui_widget_t *w = lv_observer_get_user_data(observer);
    char value[UI_STRING_MAX];
    ui_data_format(w->source, w->decimals, value, sizeof(value));
    lv_label_set_text_fmt(label, "%s%s%s", w->label, value, w->suffix);
    set_hidden_if_empty(lv_obj_get_user_data(label), w);
}

// Key card count: the widget label is the card title, so only value + suffix here
static void count_observer_cb(lv_observer_t *observer, lv_subject_t *subject)
{
    (void)subject;
    lv_obj_t *label = lv_observer_get_target_obj(observer);
    const ui_widget_t *w = lv_observer_get_user_data(observer);
    char value[UI_STRING_MAX];
    ui_data_format(w->source, w->decimals, value, sizeof(value));
    lv_label_set_text_fmt(label, "%s%s", value, w->suffix);
}

static void bind_text(lv_obj_t *label, lv_obj_t *root, const ui_widget_t *w)
{
    lv_obj_set_user_data(label, root);
    if (w->source == UI_SRC_NONE) {
        lv_label_set_text_fmt(label, "%s%s", w->label, w->suffix);
        return;
    }
    lv_subject_add_observer_obj(ui_data_subject(w->source), text_observer_cb, label, (void *)w);
}

// ---- Grade -------------------------------------------------------------------------

static uint32_t grade_color(const char *grade)
{
    if (strcmp(grade, "XH") == 0 || strcmp(grade, "SSH") == 0 || strcmp(grade, "SH") == 0) return 0xDDE6F0;
    if (strcmp(grade, "X") == 0 || strcmp(grade, "SS") == 0) return 0xFFE066;
    switch (grade[0]) {
    case 'S': return 0xFFC933;
    case 'A': return 0x88DD44;
    case 'B': return 0x44AAFF;
    case 'C': return 0xDD66FF;
    case 'D': return 0xFF5566;
    default:  return 0x9A9AB0;
    }
}

static void grade_observer_cb(lv_observer_t *observer, lv_subject_t *subject)
{
    lv_obj_t *label = lv_observer_get_target_obj(observer);
    const ui_widget_t *w = lv_observer_get_user_data(observer);
    const char *grade = lv_subject_get_string(subject);
    // osu! shows X/XH as SS
    const char *shown = (strcmp(grade, "X") == 0 || strcmp(grade, "XH") == 0) ? "SS"
                        : (strcmp(grade, "SH") == 0) ? "S" : grade;
    lv_label_set_text_fmt(label, "%s%s%s", w->label, shown, w->suffix);
    lv_obj_set_style_text_color(label, lv_color_hex(grade_color(grade)), 0);
    set_hidden_if_empty(lv_obj_get_user_data(label), w);
}

// ---- Widgets -----------------------------------------------------------------------

static void build_text(lv_obj_t *scr, const ui_widget_t *w)
{
    lv_obj_t *root = scr;
    int x = w->x, y = w->y;
    if (w->flags & (UI_FLAG_BG_FILL | UI_FLAG_BORDER)) {
        root = plain_obj(scr);
        lv_obj_set_pos(root, w->x, w->y);
        lv_obj_set_size(root, w->w, w->h);
        apply_box_style(root, w);
        x = 0;
        y = 0;
    }
    int pad = (root != scr) ? w->radius / 2 + 2 : 0;
    lv_obj_t *label = box_label(root, w, x + pad, y, w->w - 2 * pad, w->h);
    lv_obj_set_style_text_color(label, lv_color_hex(w->fg), 0);
    bind_text(label, root == scr ? label : root, w);
}

static void build_grade(lv_obj_t *scr, const ui_widget_t *w)
{
    lv_obj_t *label = box_label(scr, w, w->x, w->y, w->w, w->h);
    lv_obj_set_user_data(label, label);
    lv_subject_t *subject = ui_data_subject(w->source);
    if (subject && ui_source_info(w->source)->type == UI_VALUE_STRING) {
        lv_subject_add_observer_obj(subject, grade_observer_cb, label, (void *)w);
    } else {
        lv_obj_set_style_text_color(label, lv_color_hex(w->fg), 0);
        bind_text(label, label, w);
    }
}

static void build_progress(lv_obj_t *scr, const ui_widget_t *w)
{
    lv_obj_t *bar = lv_bar_create(scr);
    lv_obj_remove_style_all(bar);
    lv_obj_set_pos(bar, w->x, w->y);
    lv_obj_set_size(bar, w->w, w->h);
    lv_obj_set_style_radius(bar, w->radius, LV_PART_MAIN);
    lv_obj_set_style_bg_color(bar, lv_color_hex(w->bg), LV_PART_MAIN);
    lv_obj_set_style_bg_opa(bar, LV_OPA_COVER, LV_PART_MAIN);
    lv_obj_set_style_radius(bar, w->radius, LV_PART_INDICATOR);
    lv_obj_set_style_bg_color(bar, lv_color_hex(w->accent), LV_PART_INDICATOR);
    lv_obj_set_style_bg_opa(bar, LV_OPA_COVER, LV_PART_INDICATOR);

    const ui_source_info_t *info = ui_source_info(w->source);
    if (info && info->type == UI_VALUE_NUMBER && w->source != UI_SRC_NONE) {
        lv_bar_set_range(bar, 0, info->scale);
        lv_bar_bind_value(bar, ui_data_subject(w->source));
    } else {
        lv_bar_set_range(bar, 0, 100);
        lv_bar_set_value(bar, 50, LV_ANIM_OFF);
    }
}

static uint8_t key_down_source(uint8_t source)
{
    switch (source) {
    case UI_SRC_PAD_K1_MAP:
    case UI_SRC_PAD_K1_LIFETIME:
    case UI_SRC_PAD_K1_LABEL:
        return UI_SRC_PAD_K1_DOWN;
    case UI_SRC_PAD_K2_MAP:
    case UI_SRC_PAD_K2_LIFETIME:
    case UI_SRC_PAD_K2_LABEL:
        return UI_SRC_PAD_K2_DOWN;
    default:
        return UI_SRC_NONE;
    }
}

static void build_keycard(lv_obj_t *scr, const ui_widget_t *w)
{
    lv_obj_t *card = plain_obj(scr);
    lv_obj_set_pos(card, w->x, w->y);
    lv_obj_set_size(card, w->w, w->h);
    lv_obj_set_style_radius(card, w->radius, 0);
    lv_obj_set_style_bg_color(card, lv_color_hex(w->bg), 0);
    lv_obj_set_style_bg_opa(card, LV_OPA_COVER, 0);
    lv_obj_set_style_text_color(card, lv_color_hex(w->fg), 0);
    if (w->flags & UI_FLAG_BORDER) {
        // Border takes the "other" color: accent at rest, bg while pressed
        lv_obj_set_style_border_color(card, lv_color_hex(w->accent), 0);
        lv_obj_set_style_border_color(card, lv_color_hex(w->bg), LV_STATE_CHECKED);
        lv_obj_set_style_border_opa(card, LV_OPA_50, 0);
        lv_obj_set_style_border_width(card, 1, 0);
    }
    // Pressed: bg and accent swap roles; children inherit the contrasting text color
    lv_obj_set_style_bg_color(card, lv_color_hex(w->accent), LV_STATE_CHECKED);
    lv_obj_set_style_text_color(card, contrast_text(w->accent), LV_STATE_CHECKED);

    lv_obj_t *title = lv_label_create(card);
    lv_obj_remove_style_all(title);
    lv_obj_set_style_text_font(title, s_fonts[UI_FONT_14], 0);
    lv_obj_set_style_text_opa(title, LV_OPA_70, 0);
    lv_label_set_text(title, w->label);
    lv_obj_align(title, LV_ALIGN_TOP_MID, 0, 3);

    lv_obj_t *count = lv_label_create(card);
    lv_obj_remove_style_all(count);
    lv_obj_set_style_text_font(count, widget_font(w), 0);
    lv_obj_align(count, LV_ALIGN_CENTER, 0, w->h >= 48 ? 8 : 0);
    if (w->source != UI_SRC_NONE) {
        lv_subject_add_observer_obj(ui_data_subject(w->source), count_observer_cb, count, (void *)w);
    }

    uint8_t down = key_down_source(w->source);
    if (down != UI_SRC_NONE) {
        lv_obj_bind_state_if_eq(card, ui_data_subject(down), LV_STATE_CHECKED, 1);
    }
}

static void build_status_dot(lv_obj_t *scr, const ui_widget_t *w)
{
    const lv_font_t *font = widget_font(w);
    int32_t dot = lv_font_get_line_height(font) / 2;
    if (dot < 6) dot = 6;

    lv_obj_t *d = plain_obj(scr);
    lv_obj_set_size(d, dot, dot);
    lv_obj_set_pos(d, w->x, w->y + (w->h - dot) / 2);
    lv_obj_set_style_radius(d, LV_RADIUS_CIRCLE, 0);
    lv_obj_set_style_bg_opa(d, LV_OPA_COVER, 0);
    lv_obj_set_style_bg_color(d, lv_color_hex(w->bg), 0);
    lv_obj_set_style_bg_color(d, lv_color_hex(w->accent), LV_STATE_CHECKED);
    if (w->source != UI_SRC_NONE) {
        lv_obj_bind_state_if_eq(d, ui_data_subject(w->source), LV_STATE_CHECKED, 1);
    }

    int gap = dot + 5;
    lv_obj_t *label = box_label(scr, w, w->x + gap, w->y, w->w - gap, w->h);
    lv_obj_set_style_text_color(label, lv_color_hex(w->fg), 0);
    lv_label_set_text_fmt(label, "%s%s", w->label, w->suffix);
}

static void build_rect(lv_obj_t *scr, const ui_widget_t *w)
{
    lv_obj_t *rect = plain_obj(scr);
    lv_obj_set_pos(rect, w->x, w->y);
    lv_obj_set_size(rect, w->w, w->h);
    lv_obj_set_style_radius(rect, w->radius, 0);
    lv_obj_set_style_bg_color(rect, lv_color_hex(w->bg), 0);
    lv_obj_set_style_bg_opa(rect, LV_OPA_COVER, 0);
    if (w->flags & UI_FLAG_BORDER) {
        lv_obj_set_style_border_color(rect, lv_color_hex(w->fg), 0);
        lv_obj_set_style_border_width(rect, 1, 0);
    }
}

// ---- Layout ------------------------------------------------------------------------

bool ui_layout_validate(const ui_layout_t *layout, char *err, size_t err_len)
{
    #define FAIL(...) do { if (err && err_len) snprintf(err, err_len, __VA_ARGS__); return false; } while (0)
    if (!layout) FAIL("no layout");
    if (layout->count > UI_MAX_WIDGETS) FAIL("too many widgets (%u > %d)", layout->count, UI_MAX_WIDGETS);
    for (int i = 0; i < layout->count; i++) {
        const ui_widget_t *w = &layout->widgets[i];
        if (w->kind >= UI_WIDGET_KIND_COUNT) FAIL("widget %d: unknown kind %u", i, w->kind);
        if (w->font >= UI_FONT_COUNT) FAIL("widget %d: unknown font %u", i, w->font);
        if (w->align > UI_ALIGN_RIGHT) FAIL("widget %d: unknown align %u", i, w->align);
        if (!ui_source_info(w->source)) FAIL("widget %d: unknown source %u", i, w->source);
        if (w->w <= 0 || w->h <= 0 || w->w > 2 * UI_SCREEN_W || w->h > 2 * UI_SCREEN_H) {
            FAIL("widget %d: bad size %dx%d", i, w->w, w->h);
        }
        if (w->x < -UI_SCREEN_W || w->x > 2 * UI_SCREEN_W || w->y < -UI_SCREEN_H || w->y > 2 * UI_SCREEN_H) {
            FAIL("widget %d: position out of range", i);
        }
        if (memchr(w->label, '\0', UI_LABEL_MAX) == NULL || memchr(w->suffix, '\0', UI_SUFFIX_MAX) == NULL) {
            FAIL("widget %d: unterminated text", i);
        }
    }
    return true;
    #undef FAIL
}

lv_obj_t *ui_screen_create(const ui_layout_t *layout)
{
    if (!ui_layout_validate(layout, NULL, 0)) {
        return NULL;
    }
    // Observers keep pointers to the widget specs: give the screen its own copy,
    // freed by ui_screen_delete once the widgets are gone
    ui_layout_t *copy = lv_malloc(sizeof(ui_layout_t));
    if (!copy) {
        return NULL;
    }
    memcpy(copy, layout, sizeof(ui_layout_t));

    lv_obj_t *scr = lv_obj_create(NULL);
    lv_obj_remove_style_all(scr);
    lv_obj_remove_flag(scr, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_set_size(scr, UI_SCREEN_W, UI_SCREEN_H);
    lv_obj_set_style_bg_color(scr, lv_color_hex(copy->background), 0);
    lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, 0);
    lv_obj_set_user_data(scr, copy);

    for (int i = 0; i < copy->count; i++) {
        const ui_widget_t *w = &copy->widgets[i];
        switch (w->kind) {
        case UI_WIDGET_TEXT:       build_text(scr, w); break;
        case UI_WIDGET_PROGRESS:   build_progress(scr, w); break;
        case UI_WIDGET_KEYCARD:    build_keycard(scr, w); break;
        case UI_WIDGET_STATUS_DOT: build_status_dot(scr, w); break;
        case UI_WIDGET_RECT:       build_rect(scr, w); break;
        case UI_WIDGET_GRADE:      build_grade(scr, w); break;
        default: break;
        }
    }
    return scr;
}

void ui_screen_delete(lv_obj_t *scr)
{
    if (!scr) {
        return;
    }
    // LV_EVENT_DELETE reaches the screen before its children are torn down, so
    // the layout copy is only freed after lv_obj_delete has returned
    ui_layout_t *copy = lv_obj_get_user_data(scr);
    lv_obj_delete(scr);
    lv_free(copy);
}
