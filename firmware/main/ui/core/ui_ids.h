#pragma once

// Identifiers shared by the firmware, the host protocol (protocol/osupad.proto) and the
// PC designer preview. Values are wire format: append only, never renumber.

typedef enum {
    UI_WIDGET_TEXT = 0,       // Static label, or bound value with prefix (label) and suffix
    UI_WIDGET_PROGRESS = 1,   // Horizontal bar, source value 0..1
    UI_WIDGET_KEYCARD = 2,    // K1/K2 counter card, filled with accent while the key is down
    UI_WIDGET_STATUS_DOT = 3, // Dot (accent = on, fg = off) followed by the label
    UI_WIDGET_RECT = 4,       // Rounded rectangle (bg), optional border in fg
    UI_WIDGET_GRADE = 5,      // Rank letter drawn in the grade's color
    UI_WIDGET_KIND_COUNT
} ui_widget_kind_t;

// LVGL built-in Montserrat sizes
typedef enum {
    UI_FONT_12 = 0,
    UI_FONT_14 = 1,
    UI_FONT_16 = 2,
    UI_FONT_20 = 3,
    UI_FONT_24 = 4,
    UI_FONT_32 = 5,
    UI_FONT_48 = 6,
    UI_FONT_COUNT
} ui_font_id_t;

typedef enum {
    UI_ALIGN_LEFT = 0,
    UI_ALIGN_CENTER = 1,
    UI_ALIGN_RIGHT = 2,
} ui_align_t;

typedef enum {
    UI_SCREEN_IDLE = 0,
    UI_SCREEN_PLAYING = 1,
    UI_SCREEN_COUNT
} ui_screen_id_t;

// Widget flags
#define UI_FLAG_BG_FILL          0x01  // Fill the widget with bg
#define UI_FLAG_HIDE_WHEN_EMPTY  0x02  // Hidden while the bound source has no value
#define UI_FLAG_BORDER           0x04  // 1px border in fg (RECT, KEYCARD)

typedef enum {
    UI_SRC_NONE = 0,              // Static text: label only

    // Map (tosu beatmap)
    UI_SRC_MAP_TITLE = 1,
    UI_SRC_MAP_ARTIST = 2,
    UI_SRC_MAP_MAPPER = 3,
    UI_SRC_MAP_DIFFICULTY = 4,
    UI_SRC_MAP_STATUS = 5,        // "ranked", "loved", ...
    UI_SRC_MAP_STARS = 6,         // total, with mods
    UI_SRC_MAP_STARS_LIVE = 7,
    UI_SRC_MAP_AR = 8,
    UI_SRC_MAP_CS = 9,
    UI_SRC_MAP_OD = 10,
    UI_SRC_MAP_HP = 11,
    UI_SRC_MAP_BPM = 12,
    UI_SRC_MAP_OBJECTS = 13,
    UI_SRC_MAP_MAX_COMBO = 14,
    UI_SRC_MAP_LENGTH = 15,       // ms
    UI_SRC_MAP_TIME_ELAPSED = 16, // ms
    UI_SRC_MAP_TIME_REMAINING = 17, // ms
    UI_SRC_MAP_PROGRESS = 18,     // 0..1
    UI_SRC_MAP_KIAI = 19,         // 0/1

    // Live play (tosu play)
    UI_SRC_PLAY_PP = 20,
    UI_SRC_PLAY_PP_FC = 21,
    UI_SRC_PLAY_PP_MAX = 22,      // max achievable this play
    UI_SRC_PLAY_ACCURACY = 23,    // percent
    UI_SRC_PLAY_SCORE = 24,
    UI_SRC_PLAY_COMBO = 25,
    UI_SRC_PLAY_MAX_COMBO = 26,
    UI_SRC_PLAY_GRADE = 27,       // "SS", "S", "A", ...
    UI_SRC_PLAY_HITS_300 = 28,
    UI_SRC_PLAY_HITS_100 = 29,
    UI_SRC_PLAY_HITS_50 = 30,
    UI_SRC_PLAY_HITS_MISS = 31,
    UI_SRC_PLAY_SLIDER_BREAKS = 32,
    UI_SRC_PLAY_UR = 33,
    UI_SRC_PLAY_HEALTH = 34,      // 0..1
    UI_SRC_PLAY_MODS = 35,        // "HDDT"
    UI_SRC_PLAY_PLAYER = 36,
    UI_SRC_PLAY_FAILED = 37,      // 0/1

    // Profile & session (tosu)
    UI_SRC_PROFILE_NAME = 40,
    UI_SRC_PROFILE_RANK = 41,
    // 42 reserved
    UI_SRC_PROFILE_PP = 43,
    UI_SRC_PROFILE_ACCURACY = 44,
    UI_SRC_PROFILE_PLAYCOUNT = 45,
    UI_SRC_PROFILE_LEVEL = 46,
    UI_SRC_PROFILE_COUNTRY = 47,
    UI_SRC_SESSION_PLAYTIME = 48, // ms
    UI_SRC_SESSION_PLAYCOUNT = 49,
    UI_SRC_GAME_STATE = 50,       // "Playing", "Song select", ...

    // Pad (device-local)
    UI_SRC_PAD_K1_MAP = 60,
    UI_SRC_PAD_K2_MAP = 61,
    UI_SRC_PAD_TOTAL_MAP = 62,
    UI_SRC_PAD_K1_LIFETIME = 63,
    UI_SRC_PAD_K2_LIFETIME = 64,
    UI_SRC_PAD_TOTAL_LIFETIME = 65,
    UI_SRC_PAD_KPS = 66,          // taps per second
    UI_SRC_PAD_K1_LABEL = 67,     // bound key character, e.g. "Z"
    UI_SRC_PAD_K2_LABEL = 68,
    UI_SRC_PAD_CLOCK = 69,        // "21:07"
    UI_SRC_PAD_DATE = 70,         // "Sat 12 Sep"
    UI_SRC_PAD_UPTIME = 71,       // ms
    UI_SRC_PAD_K1_DOWN = 72,      // 0/1, includes a short hold so taps are visible
    UI_SRC_PAD_K2_DOWN = 73,

    // Connection status (0/1)
    UI_SRC_STATUS_PC = 80,        // daemon connected over USB CDC
    UI_SRC_STATUS_TOSU = 81,
    UI_SRC_STATUS_OSU = 82,       // tosu sees a running osu!

    UI_SRC_COUNT = 96
} ui_source_t;
