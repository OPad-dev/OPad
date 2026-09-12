#include "ui_core.h"

// Built-in layouts, used until the PC designer pushes custom ones. The designer's
// "Reset to default" reads these through the preview library, so this file is the
// single source of truth.

#define BG        0x0E0E16
#define CARD      0x1B1B28
#define PINK      0xFF66AA
#define WHITE     0xFFFFFF
#define MUTED     0x8C8CA6
#define YELLOW    0xFFCC33
#define CYAN      0x66CCFF
#define GREEN     0x44DD88
#define OFF       0x3A3A4C

#define DEC UI_DECIMALS_DEFAULT

static const ui_layout_t s_idle = {
    .background = BG,
    .count = 12,
    .widgets = {
        // Status strip: clock, date, PC and tosu connection
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

        // Lifetime total
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_NONE, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 40, .w = 320, .h = 18, .fg = MUTED, .decimals = DEC, .label = "TOTAL PRESSES" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PAD_TOTAL_LIFETIME, .font = UI_FONT_48, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 58, .w = 320, .h = 54, .fg = WHITE, .decimals = DEC },

        // Per-key lifetime cards: K1 pink / K2 dark at rest, colors swap while pressed
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K1_LIFETIME, .font = UI_FONT_24,
          .x = 12, .y = 118, .w = 144, .h = 64, .fg = BG, .bg = PINK, .accent = CARD, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K1" },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K2_LIFETIME, .font = UI_FONT_24,
          .x = 164, .y = 118, .w = 144, .h = 64, .fg = WHITE, .bg = CARD, .accent = PINK, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K2" },

        // osu! profile
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
        // Header pill
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_NONE, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 96, .y = 4, .w = 128, .h = 22, .fg = BG, .bg = PINK, .radius = 11,
          .flags = UI_FLAG_BG_FILL, .decimals = DEC, .label = "osu! PLAYING" },

        // Map
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TITLE, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 29, .w = 304, .h = 24, .fg = WHITE, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_ARTIST, .font = UI_FONT_14, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 53, .w = 304, .h = 18, .fg = MUTED, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_DIFFICULTY, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 8, .y = 71, .w = 200, .h = 18, .fg = CYAN, .decimals = DEC, .label = "[", .suffix = "]" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_STARS, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 214, .y = 71, .w = 98, .h = 18, .fg = YELLOW, .decimals = DEC, .suffix = " *" },

        // PP
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_PP, .font = UI_FONT_48, .align = UI_ALIGN_CENTER,
          .x = 0, .y = 87, .w = 320, .h = 54, .fg = WHITE, .decimals = DEC, .suffix = "pp" },

        // Accuracy · combo · grade
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_ACCURACY, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 8, .y = 140, .w = 116, .h = 24, .fg = WHITE, .decimals = DEC, .suffix = "%" },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_COMBO, .font = UI_FONT_20, .align = UI_ALIGN_CENTER,
          .x = 124, .y = 140, .w = 84, .h = 24, .fg = WHITE, .decimals = DEC, .suffix = "x" },
        { .kind = UI_WIDGET_GRADE, .source = UI_SRC_PLAY_GRADE, .font = UI_FONT_24, .align = UI_ALIGN_CENTER,
          .x = 208, .y = 138, .w = 104, .h = 28, .fg = WHITE, .decimals = DEC },

        // Map press counters: K1 pink / K2 dark at rest, colors swap while pressed
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K1_MAP, .font = UI_FONT_24,
          .x = 12, .y = 166, .w = 144, .h = 48, .fg = BG, .bg = PINK, .accent = CARD, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K1" },
        { .kind = UI_WIDGET_KEYCARD, .source = UI_SRC_PAD_K2_MAP, .font = UI_FONT_24,
          .x = 164, .y = 166, .w = 144, .h = 48, .fg = WHITE, .bg = CARD, .accent = PINK, .radius = 12,
          .flags = UI_FLAG_BORDER, .decimals = DEC, .label = "K2" },

        // Progress
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TIME_ELAPSED, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 2, .y = 218, .w = 48, .h = 20, .fg = MUTED, .decimals = DEC },
        { .kind = UI_WIDGET_PROGRESS, .source = UI_SRC_MAP_PROGRESS,
          .x = 56, .y = 224, .w = 208, .h = 8, .bg = CARD, .accent = PINK, .radius = 4, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_MAP_TIME_REMAINING, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 270, .y = 218, .w = 48, .h = 20, .fg = MUTED, .decimals = DEC, .label = "-" },

        // Mods and max combo in the corners (hidden when empty)
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_MODS, .font = UI_FONT_14, .align = UI_ALIGN_LEFT,
          .x = 10, .y = 6, .w = 84, .h = 18, .fg = YELLOW, .flags = UI_FLAG_HIDE_WHEN_EMPTY, .decimals = DEC },
        { .kind = UI_WIDGET_TEXT, .source = UI_SRC_PLAY_MAX_COMBO, .font = UI_FONT_14, .align = UI_ALIGN_RIGHT,
          .x = 226, .y = 6, .w = 86, .h = 18, .fg = MUTED, .flags = UI_FLAG_HIDE_WHEN_EMPTY,
          .decimals = DEC, .label = "max ", .suffix = "x" },
    },
};

const ui_layout_t *ui_default_layout(uint8_t screen)
{
    switch (screen) {
    case UI_SCREEN_IDLE: return &s_idle;
    case UI_SCREEN_PLAYING: return &s_playing;
    default: return NULL;
    }
}
