// Headless stub for osupad-ui-preview when firmware LVGL sources are missing
// (e.g. on clean CI checkouts where ESP-IDF components have not been fetched).
// Provides valid default layouts, source metadata, layout validation, and mock frame rendering
// so cargo build and cargo test pass without requiring a full ESP-IDF setup.

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "ui_ids.h"

#define UI_SCREEN_W 320
#define UI_SCREEN_H 240
#define UI_MAX_WIDGETS 32
#define UI_LABEL_MAX 32
#define UI_SUFFIX_MAX 12
#define UI_STRING_MAX 64
#define UI_DECIMALS_DEFAULT 0xFF

typedef struct {
    uint8_t kind;
    uint8_t source;
    uint8_t font;
    uint8_t align;
    int16_t x, y, w, h;
    uint32_t fg;
    uint32_t bg;
    uint32_t accent;
    uint8_t radius;
    uint8_t decimals;
    uint8_t flags;
    uint8_t reserved;
    char label[UI_LABEL_MAX];
    char suffix[UI_SUFFIX_MAX];
} ui_widget_t;

typedef struct {
    uint32_t background;
    uint8_t count;
    ui_widget_t widgets[UI_MAX_WIDGETS];
} ui_layout_t;

typedef struct {
    const char *name;
    uint8_t type;
    uint8_t fmt;
    int32_t scale;
    uint8_t decimals;
} ui_source_info_t;

#define S(nm, fmt, scale, dec) { nm, 0, fmt, scale, dec }
#define N(nm, fmt, scale, dec) { nm, 1, fmt, scale, dec }

static const ui_source_info_t s_sources[UI_SRC_COUNT] = {
    [UI_SRC_NONE]                = S("none", 0, 1, 0),

    [UI_SRC_MAP_TITLE]           = S("map.title", 0, 1, 0),
    [UI_SRC_MAP_ARTIST]          = S("map.artist", 0, 1, 0),
    [UI_SRC_MAP_MAPPER]          = S("map.mapper", 0, 1, 0),
    [UI_SRC_MAP_DIFFICULTY]      = S("map.difficulty", 0, 1, 0),
    [UI_SRC_MAP_STATUS]          = S("map.status", 0, 1, 0),
    [UI_SRC_MAP_STARS]           = N("map.stars", 3, 100, 2),
    [UI_SRC_MAP_STARS_LIVE]      = N("map.stars_live", 3, 100, 2),
    [UI_SRC_MAP_AR]              = N("map.ar", 3, 100, 1),
    [UI_SRC_MAP_CS]              = N("map.cs", 3, 100, 1),
    [UI_SRC_MAP_OD]              = N("map.od", 3, 100, 1),
    [UI_SRC_MAP_HP]              = N("map.hp", 3, 100, 1),
    [UI_SRC_MAP_BPM]             = N("map.bpm", 1, 1, 0),
    [UI_SRC_MAP_OBJECTS]         = N("map.objects", 2, 1, 0),
    [UI_SRC_MAP_MAX_COMBO]       = N("map.max_combo", 2, 1, 0),
    [UI_SRC_MAP_LENGTH]          = N("map.length", 4, 1, 0),
    [UI_SRC_MAP_TIME_ELAPSED]    = N("map.time_elapsed", 4, 1, 0),
    [UI_SRC_MAP_TIME_REMAINING]  = N("map.time_remaining", 4, 1, 0),
    [UI_SRC_MAP_PROGRESS]        = N("map.progress", 5, 1000, 0),
    [UI_SRC_MAP_KIAI]            = N("map.kiai", 1, 1, 0),

    [UI_SRC_PLAY_PP]             = N("play.pp", 3, 100, 0),
    [UI_SRC_PLAY_PP_FC]          = N("play.pp_fc", 3, 100, 0),
    [UI_SRC_PLAY_PP_MAX]         = N("play.pp_max", 3, 100, 0),
    [UI_SRC_PLAY_ACCURACY]       = N("play.accuracy", 3, 100, 2),
    [UI_SRC_PLAY_SCORE]          = N("play.score", 2, 1, 0),
    [UI_SRC_PLAY_COMBO]          = N("play.combo", 2, 1, 0),
    [UI_SRC_PLAY_MAX_COMBO]      = N("play.max_combo", 2, 1, 0),
    [UI_SRC_PLAY_GRADE]          = S("play.grade", 0, 1, 0),
    [UI_SRC_PLAY_HITS_300]       = N("play.hits_300", 2, 1, 0),
    [UI_SRC_PLAY_HITS_100]       = N("play.hits_100", 2, 1, 0),
    [UI_SRC_PLAY_HITS_50]        = N("play.hits_50", 2, 1, 0),
    [UI_SRC_PLAY_HITS_MISS]      = N("play.hits_miss", 2, 1, 0),
    [UI_SRC_PLAY_SLIDER_BREAKS]  = N("play.slider_breaks", 2, 1, 0),
    [UI_SRC_PLAY_UR]             = N("play.ur", 3, 100, 1),
    [UI_SRC_PLAY_HEALTH]         = N("play.health", 5, 1000, 0),
    [UI_SRC_PLAY_MODS]           = S("play.mods", 0, 1, 0),
    [UI_SRC_PLAY_PLAYER]         = S("play.player", 0, 1, 0),
    [UI_SRC_PLAY_FAILED]         = N("play.failed", 1, 1, 0),

    [UI_SRC_PROFILE_NAME]        = S("profile.name", 0, 1, 0),
    [UI_SRC_PROFILE_RANK]        = N("profile.rank", 2, 1, 0),
    [UI_SRC_PROFILE_PP]          = N("profile.pp", 3, 100, 0),
    [UI_SRC_PROFILE_ACCURACY]    = N("profile.accuracy", 3, 100, 2),
    [UI_SRC_PROFILE_PLAYCOUNT]   = N("profile.playcount", 2, 1, 0),
    [UI_SRC_PROFILE_LEVEL]       = N("profile.level", 3, 100, 0),
    [UI_SRC_PROFILE_COUNTRY]     = S("profile.country", 0, 1, 0),
    [UI_SRC_SESSION_PLAYTIME]    = N("session.playtime", 4, 1, 0),
    [UI_SRC_SESSION_PLAYCOUNT]   = N("session.playcount", 2, 1, 0),
    [UI_SRC_GAME_STATE]          = S("game.state", 0, 1, 0),

    [UI_SRC_PAD_K1_MAP]          = N("pad.k1_map", 2, 1, 0),
    [UI_SRC_PAD_K2_MAP]          = N("pad.k2_map", 2, 1, 0),
    [UI_SRC_PAD_TOTAL_MAP]       = N("pad.total_map", 2, 1, 0),
    [UI_SRC_PAD_K1_LIFETIME]     = N("pad.k1_lifetime", 2, 1, 0),
    [UI_SRC_PAD_K2_LIFETIME]     = N("pad.k2_lifetime", 2, 1, 0),
    [UI_SRC_PAD_TOTAL_LIFETIME]  = N("pad.total_lifetime", 2, 1, 0),
    [UI_SRC_PAD_KPS]             = N("pad.kps", 1, 1, 0),
    [UI_SRC_PAD_K1_LABEL]        = S("pad.k1_label", 0, 1, 0),
    [UI_SRC_PAD_K2_LABEL]        = S("pad.k2_label", 0, 1, 0),
    [UI_SRC_PAD_CLOCK]           = S("pad.clock", 0, 1, 0),
    [UI_SRC_PAD_DATE]            = S("pad.date", 0, 1, 0),
    [UI_SRC_PAD_UPTIME]          = N("pad.uptime", 4, 1, 0),
    [UI_SRC_PAD_K1_DOWN]         = N("pad.k1_down", 1, 1, 0),
    [UI_SRC_PAD_K2_DOWN]         = N("pad.k2_down", 1, 1, 0),

    [UI_SRC_STATUS_PC]           = N("status.pc", 1, 1, 0),
    [UI_SRC_STATUS_TOSU]         = N("status.tosu", 1, 1, 0),
    [UI_SRC_STATUS_OSU]          = N("status.osu", 1, 1, 0),
};

#undef S
#undef N

#define BG        0x0E0E16
#define CARD      0x1B1B28
#define PINK      0xFF66AA
#define WHITE     0xFFFFFF
#define MUTED     0x8C8CA6
#define YELLOW    0xFFCC33
#define CYAN      0x66CCFF
#define GREEN     0x44DD88
#define OFF       0x3A3A4C
#define DEC       UI_DECIMALS_DEFAULT

static const ui_layout_t s_idle = {
    .background = BG,
    .count = 12,
    .widgets = {
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PAD_CLOCK, .font = UI_FONT_24, .align = UI_ALIGN_LEFT,
          .x = 12, .y = 4, .w = 76, .h = 28, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PAD_DATE, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 92, .y = 8, .w = 104, .h = 22, .fg = MUTED, .decimals = DEC },
        { .kind = UI_WIDGET_STATUS_DOT, .source = UI_SRC_STATUS_PC, .font = UI_FONT_14,
          .x = 198, .y = 8, .w = 44, .h = 22, .fg = MUTED, .bg = OFF, .accent = GREEN, .decimals = DEC,
          .label = "PC" },
        { .kind = UI_WIDGET_STATUS_DOT, .source = UI_SRC_STATUS_TOSU, .font = UI_FONT_14,
          .x = 248, .y = 8, .w = 64, .h = 22, .fg = MUTED, .bg = OFF, .accent = GREEN, .decimals = DEC,
          .label = "tosu" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_NONE, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 40, .w = 320, .h = 18, .fg = MUTED, .decimals = DEC, .label = "TOTAL PRESSES" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PAD_TOTAL_LIFETIME, .font = UI_FONT_48, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 58, .w = 320, .h = 54, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K1_LIFETIME, .font = UI_FONT_24,
          .x = 12, .y = 118, .w = 144, .h = 64, .fg = BG, .bg = PINK, .accent = CARD, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K1" },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K2_LIFETIME, .font = UI_FONT_24,
          .x = 164, .y = 118, .w = 144, .h = 64, .fg = WHITE, .bg = CARD, .accent = PINK, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K2" },
        { .kind = UI_WIDGET_RECT, .source = UI_SRC_NONE, .x = 12, .y = 190, .w = 296, .h = 42,
          .bg = CARD, .radius = 12, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PROFILE_NAME, .font = UI_FONT_20, .align = UI_ALIGN_LEFT,
          .x = 24, .y = 190, .w = 116, .h = 42, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PROFILE_RANK, .font = UI_FONT_16, .align = UI_ALIGN_CENTER,
          .x = 140, .y = 190, .w = 86, .h = 42, .fg = MUTED, .flags = UI_FLAG_HIDE_WHEN_EMPTY,
          .decimals = DEC, .label = "#" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PROFILE_PP, .font = UI_FONT_16, .align = UI_ALIGN_RIGHT,
          .x = 226, .y = 190, .w = 70, .h = 42, .fg = PINK, .flags = UI_FLAG_HIDE_WHEN_EMPTY,
          .decimals = DEC, .suffix = "pp" },
    },
};

static const ui_layout_t s_playing = {
    .background = BG,
    .count = 16,
    .widgets = {
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_NONE, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 96, .y = 4, .w = 128, .h = 22, .fg = BG, .bg = PINK, .radius = 11,
          .flags = UI_FLAG_BG_FILL, .decimals = DEC, .label = "osu! PLAYING" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TITLE, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 29, .w = 304, .h = 24, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_ARTIST, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 53, .w = 304, .h = 18, .fg = MUTED, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_DIFFICULTY, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 8, .y = 71, .w = 200, .h = 18, .fg = CYAN, .decimals = DEC, .label = "[", .suffix = "]" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_STARS, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 214, .y = 71, .w = 98, .h = 18, .fg = YELLOW, .decimals = DEC, .suffix = " *" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_PP, .font = UI_FONT_48, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 87, .w = 320, .h = 54, .fg = WHITE, .decimals = DEC, .suffix = "pp" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_ACCURACY, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 140, .w = 116, .h = 24, .fg = WHITE, .decimals = DEC, .suffix = "%" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_COMBO, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 124, .y = 140, .w = 84, .h = 24, .fg = WHITE, .decimals = DEC, .suffix = "x" },
        { .kind = UI_WIDGET_GRADE, .source = UI_SRC_PLAY_GRADE, .font = UI_FONT_24, .align = UI_ALIGN_CENTER,
          .x = 208, .y = 138, .w = 104, .h = 28, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K1_MAP, .font = UI_FONT_24,
          .x = 12, .y = 166, .w = 144, .h = 48, .fg = BG, .bg = PINK, .accent = CARD, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K1" },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K2_MAP, .font = UI_FONT_24,
          .x = 164, .y = 166, .w = 144, .h = 48, .fg = WHITE, .bg = CARD, .accent = PINK, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K2" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TIME_ELAPSED, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 2, .y = 218, .w = 48, .h = 20, .fg = MUTED, .decimals = DEC },
        { .kind = UI_WIDGET_PROGRESS, .source = UI_SRC_MAP_PROGRESS,
          .x = 56, .y = 224, .w = 208, .h = 8, .bg = CARD, .accent = PINK, .radius = 4, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TIME_REMAINING, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 270, .y = 218, .w = 48, .h = 20, .fg = MUTED, .decimals = DEC, .label = "-" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_MODS, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 10, .y = 6, .w = 84, .h = 18, .fg = YELLOW, .flags = UI_FLAG_HIDE_WHEN_EMPTY, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_MAX_COMBO, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 226, .y = 6, .w = 86, .h = 18, .fg = MUTED, .flags = UI_FLAG_HIDE_WHEN_EMPTY,
          .decimals = DEC, .label = "max ", .suffix = "x" },
    },
};

void preview_init(void) {}

bool ui_layout_validate(const ui_layout_t *layout)
{
    if (!layout) return false;
    if (layout->count > UI_MAX_WIDGETS) return false;
    for (int i = 0; i < layout->count; i++) {
        const ui_widget_t *w = &layout->widgets[i];
        if (w->kind >= UI_WIDGET_KIND_COUNT) return false;
        if (w->font >= UI_FONT_COUNT) return false;
        if (w->align > UI_ALIGN_RIGHT) return false;
        if (w->source >= UI_SRC_COUNT || s_sources[w->source].name == NULL) return false;
        if (w->w <= 0 || w->h <= 0 || w->w > 2 * UI_SCREEN_W || w->h > 2 * UI_SCREEN_H) return false;
        if (w->x < -UI_SCREEN_W || w->x > 2 * UI_SCREEN_W || w->y < -UI_SCREEN_H || w->y > 2 * UI_SCREEN_H) return false;
        if (memchr(w->label, '\0', UI_LABEL_MAX) == NULL || memchr(w->suffix, '\0', UI_SUFFIX_MAX) == NULL) return false;
    }
    return true;
}

int preview_render(const ui_layout_t *layout, uint8_t *out_rgba)
{
    if (!ui_layout_validate(layout) || !out_rgba) {
        return -1;
    }
    uint32_t bg = layout->background;
    uint8_t r = (uint8_t)((bg >> 16) & 0xFF);
    uint8_t g = (uint8_t)((bg >> 8) & 0xFF);
    uint8_t b = (uint8_t)(bg & 0xFF);
    for (int i = 0; i < UI_SCREEN_W * UI_SCREEN_H; i++) {
        out_rgba[4 * i] = r;
        out_rgba[4 * i + 1] = g;
        out_rgba[4 * i + 2] = b;
        out_rgba[4 * i + 3] = 0xFF;
    }
    for (int i = 0; i < layout->count; i++) {
        const ui_widget_t *w = &layout->widgets[i];
        int x0 = w->x, y0 = w->y, x1 = w->x + w->w, y1 = w->y + w->h;
        if (x0 < 0) x0 = 0;
        if (y0 < 0) y0 = 0;
        if (x1 > UI_SCREEN_W) x1 = UI_SCREEN_W;
        if (y1 > UI_SCREEN_H) y1 = UI_SCREEN_H;
        uint32_t color = w->fg ? w->fg : (w->bg ? w->bg : 0xFFFFFF);
        uint8_t wr = (uint8_t)((color >> 16) & 0xFF);
        uint8_t wg = (uint8_t)((color >> 8) & 0xFF);
        uint8_t wb = (uint8_t)(color & 0xFF);
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                int px = y * UI_SCREEN_W + x;
                out_rgba[4 * px] = wr;
                out_rgba[4 * px + 1] = wg;
                out_rgba[4 * px + 2] = wb;
                out_rgba[4 * px + 3] = 0xFF;
            }
        }
    }
    return 0;
}

size_t preview_sizeof_widget(void) { return sizeof(ui_widget_t); }
size_t preview_sizeof_layout(void) { return sizeof(ui_layout_t); }

const ui_layout_t *ui_default_layout(uint8_t screen)
{
    switch (screen) {
    case UI_SCREEN_IDLE: return &s_idle;
    case UI_SCREEN_PLAYING: return &s_playing;
    default: return NULL;
    }
}

void ui_data_set_number(uint8_t source, double value) { (void)source; (void)value; }
void ui_data_set_string(uint8_t source, const char *value) { (void)source; (void)value; }
void ui_data_clear(uint8_t source) { (void)source; }

const ui_source_info_t *ui_source_info(uint8_t source)
{
    if (source < UI_SRC_COUNT && s_sources[source].name != NULL) {
        return &s_sources[source];
    }
    return NULL;
}
