#include "ui_core.h"
#include "ui_internal.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define S(nm, fmt, scale, dec) { nm, UI_VALUE_STRING, fmt, scale, dec }
#define N(nm, fmt, scale, dec) { nm, UI_VALUE_NUMBER, fmt, scale, dec }

static const ui_source_info_t s_sources[UI_SRC_COUNT] = {
    [UI_SRC_NONE]                = S("none", UI_FMT_TEXT, 1, 0),

    [UI_SRC_MAP_TITLE]           = S("map.title", UI_FMT_TEXT, 1, 0),
    [UI_SRC_MAP_ARTIST]          = S("map.artist", UI_FMT_TEXT, 1, 0),
    [UI_SRC_MAP_MAPPER]          = S("map.mapper", UI_FMT_TEXT, 1, 0),
    [UI_SRC_MAP_DIFFICULTY]      = S("map.difficulty", UI_FMT_TEXT, 1, 0),
    [UI_SRC_MAP_STATUS]          = S("map.status", UI_FMT_TEXT, 1, 0),
    [UI_SRC_MAP_STARS]           = N("map.stars", UI_FMT_FIXED, 100, 2),
    [UI_SRC_MAP_STARS_LIVE]      = N("map.stars_live", UI_FMT_FIXED, 100, 2),
    [UI_SRC_MAP_AR]              = N("map.ar", UI_FMT_FIXED, 100, 1),
    [UI_SRC_MAP_CS]              = N("map.cs", UI_FMT_FIXED, 100, 1),
    [UI_SRC_MAP_OD]              = N("map.od", UI_FMT_FIXED, 100, 1),
    [UI_SRC_MAP_HP]              = N("map.hp", UI_FMT_FIXED, 100, 1),
    [UI_SRC_MAP_BPM]             = N("map.bpm", UI_FMT_INT, 1, 0),
    [UI_SRC_MAP_OBJECTS]         = N("map.objects", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_MAP_MAX_COMBO]       = N("map.max_combo", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_MAP_LENGTH]          = N("map.length", UI_FMT_DURATION, 1, 0),
    [UI_SRC_MAP_TIME_ELAPSED]    = N("map.time_elapsed", UI_FMT_DURATION, 1, 0),
    [UI_SRC_MAP_TIME_REMAINING]  = N("map.time_remaining", UI_FMT_DURATION, 1, 0),
    [UI_SRC_MAP_PROGRESS]        = N("map.progress", UI_FMT_PERCENT, 1000, 0),
    [UI_SRC_MAP_KIAI]            = N("map.kiai", UI_FMT_INT, 1, 0),

    [UI_SRC_PLAY_PP]             = N("play.pp", UI_FMT_FIXED, 100, 0),
    [UI_SRC_PLAY_PP_FC]          = N("play.pp_fc", UI_FMT_FIXED, 100, 0),
    [UI_SRC_PLAY_PP_MAX]         = N("play.pp_max", UI_FMT_FIXED, 100, 0),
    [UI_SRC_PLAY_ACCURACY]       = N("play.accuracy", UI_FMT_FIXED, 100, 2),
    [UI_SRC_PLAY_SCORE]          = N("play.score", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_COMBO]          = N("play.combo", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_MAX_COMBO]      = N("play.max_combo", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_GRADE]          = S("play.grade", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PLAY_HITS_300]       = N("play.hits_300", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_HITS_100]       = N("play.hits_100", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_HITS_50]        = N("play.hits_50", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_HITS_MISS]      = N("play.hits_miss", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_SLIDER_BREAKS]  = N("play.slider_breaks", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PLAY_UR]             = N("play.ur", UI_FMT_FIXED, 100, 1),
    [UI_SRC_PLAY_HEALTH]         = N("play.health", UI_FMT_PERCENT, 1000, 0),
    [UI_SRC_PLAY_MODS]           = S("play.mods", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PLAY_PLAYER]         = S("play.player", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PLAY_FAILED]         = N("play.failed", UI_FMT_INT, 1, 0),

    [UI_SRC_PROFILE_NAME]        = S("profile.name", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PROFILE_RANK]        = N("profile.rank", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PROFILE_PP]          = N("profile.pp", UI_FMT_FIXED, 100, 0),
    [UI_SRC_PROFILE_ACCURACY]    = N("profile.accuracy", UI_FMT_FIXED, 100, 2),
    [UI_SRC_PROFILE_PLAYCOUNT]   = N("profile.playcount", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PROFILE_LEVEL]       = N("profile.level", UI_FMT_FIXED, 100, 0),
    [UI_SRC_PROFILE_COUNTRY]     = S("profile.country", UI_FMT_TEXT, 1, 0),
    [UI_SRC_SESSION_PLAYTIME]    = N("session.playtime", UI_FMT_DURATION, 1, 0),
    [UI_SRC_SESSION_PLAYCOUNT]   = N("session.playcount", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_GAME_STATE]          = S("game.state", UI_FMT_TEXT, 1, 0),

    [UI_SRC_PAD_K1_MAP]          = N("pad.k1_map", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_K2_MAP]          = N("pad.k2_map", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_TOTAL_MAP]       = N("pad.total_map", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_K1_LIFETIME]     = N("pad.k1_lifetime", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_K2_LIFETIME]     = N("pad.k2_lifetime", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_TOTAL_LIFETIME]  = N("pad.total_lifetime", UI_FMT_GROUPED, 1, 0),
    [UI_SRC_PAD_KPS]             = N("pad.kps", UI_FMT_INT, 1, 0),
    [UI_SRC_PAD_K1_LABEL]        = S("pad.k1_label", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PAD_K2_LABEL]        = S("pad.k2_label", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PAD_CLOCK]           = S("pad.clock", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PAD_DATE]            = S("pad.date", UI_FMT_TEXT, 1, 0),
    [UI_SRC_PAD_UPTIME]          = N("pad.uptime", UI_FMT_DURATION, 1, 0),
    [UI_SRC_PAD_K1_DOWN]         = N("pad.k1_down", UI_FMT_INT, 1, 0),
    [UI_SRC_PAD_K2_DOWN]         = N("pad.k2_down", UI_FMT_INT, 1, 0),

    [UI_SRC_STATUS_PC]           = N("status.pc", UI_FMT_INT, 1, 0),
    [UI_SRC_STATUS_TOSU]         = N("status.tosu", UI_FMT_INT, 1, 0),
    [UI_SRC_STATUS_OSU]          = N("status.osu", UI_FMT_INT, 1, 0),
};

#undef S
#undef N

static lv_subject_t s_subjects[UI_SRC_COUNT];
static char s_str_buf[UI_SRC_COUNT][UI_STRING_MAX];
static char s_str_prev[UI_SRC_COUNT][UI_STRING_MAX];
static bool s_initialized;

const ui_source_info_t *ui_source_info(uint8_t source)
{
    if (source >= UI_SRC_COUNT || s_sources[source].name == NULL) {
        return NULL;
    }
    return &s_sources[source];
}

lv_subject_t *ui_data_subject(uint8_t source)
{
    return ui_source_info(source) ? &s_subjects[source] : NULL;
}

void ui_data_init(void)
{
    if (s_initialized) {
        return;
    }
    for (int i = 0; i < UI_SRC_COUNT; i++) {
        if (s_sources[i].name == NULL) {
            continue;
        }
        if (s_sources[i].type == UI_VALUE_STRING) {
            lv_subject_init_string(&s_subjects[i], s_str_buf[i], s_str_prev[i], UI_STRING_MAX, "");
        } else {
            lv_subject_init_int(&s_subjects[i], UI_VALUE_EMPTY);
        }
    }
    s_initialized = true;
}

void ui_data_set_number(uint8_t source, double value)
{
    const ui_source_info_t *info = ui_source_info(source);
    if (!info || info->type != UI_VALUE_NUMBER || isnan(value)) {
        return;
    }
    double scaled = value * info->scale;
    if (scaled > INT32_MAX) scaled = INT32_MAX;
    if (scaled < INT32_MIN + 1) scaled = INT32_MIN + 1;
    lv_subject_set_int(&s_subjects[source], (int32_t)lround(scaled));
}

void ui_data_set_string(uint8_t source, const char *value)
{
    const ui_source_info_t *info = ui_source_info(source);
    if (!info || info->type != UI_VALUE_STRING) {
        return;
    }
    lv_subject_copy_string(&s_subjects[source], value ? value : "");
}

void ui_data_clear(uint8_t source)
{
    const ui_source_info_t *info = ui_source_info(source);
    if (!info) {
        return;
    }
    if (info->type == UI_VALUE_STRING) {
        lv_subject_copy_string(&s_subjects[source], "");
    } else {
        lv_subject_set_int(&s_subjects[source], UI_VALUE_EMPTY);
    }
}

void ui_data_clear_host_sources(void)
{
    for (int i = UI_SRC_MAP_TITLE; i <= UI_SRC_GAME_STATE; i++) {
        ui_data_clear((uint8_t)i);
    }
}

bool ui_data_is_empty(uint8_t source)
{
    const ui_source_info_t *info = ui_source_info(source);
    if (!info || source == UI_SRC_NONE) {
        return false;
    }
    if (info->type == UI_VALUE_STRING) {
        return lv_subject_get_string(&s_subjects[source])[0] == '\0';
    }
    return lv_subject_get_int(&s_subjects[source]) == UI_VALUE_EMPTY;
}

static size_t format_grouped(char *out, size_t len, long long v)
{
    char digits[24];
    int n = snprintf(digits, sizeof(digits), "%lld", v < 0 ? -v : v);
    size_t pos = 0;
    if (v < 0 && pos + 1 < len) out[pos++] = '-';
    for (int i = 0; i < n && pos + 1 < len; i++) {
        if (i > 0 && (n - i) % 3 == 0 && pos + 1 < len) {
            out[pos++] = ',';
        }
        out[pos++] = digits[i];
    }
    out[pos] = '\0';
    return pos;
}

void ui_data_format(uint8_t source, uint8_t decimals, char *out, size_t len)
{
    const ui_source_info_t *info = ui_source_info(source);
    if (!info || len == 0) {
        if (len) out[0] = '\0';
        return;
    }
    if (info->type == UI_VALUE_STRING) {
        lv_strlcpy(out, lv_subject_get_string(&s_subjects[source]), len);
        return;
    }

    int32_t raw = lv_subject_get_int(&s_subjects[source]);
    if (raw == UI_VALUE_EMPTY) {
        lv_strlcpy(out, "-", len);
        return;
    }
    int dec = decimals == UI_DECIMALS_DEFAULT ? info->default_decimals : decimals;
    if (dec > 4) dec = 4;
    double real = (double)raw / info->scale;

    switch (info->format) {
    case UI_FMT_GROUPED:
        format_grouped(out, len, llround(real));
        break;
    case UI_FMT_FIXED:
        snprintf(out, len, "%.*f", dec, real);
        break;
    case UI_FMT_PERCENT:
        snprintf(out, len, "%.*f", dec, real * 100.0);
        break;
    case UI_FMT_DURATION: {
        long long total_s = llabs(llround(real)) / 1000;
        long long h = total_s / 3600, m = (total_s / 60) % 60, s = total_s % 60;
        if (h > 0) {
            snprintf(out, len, "%s%lld:%02lld:%02lld", raw < 0 ? "-" : "", h, m, s);
        } else {
            snprintf(out, len, "%s%lld:%02lld", raw < 0 ? "-" : "", (long long)(total_s / 60), s);
        }
        break;
    }
    case UI_FMT_INT:
    case UI_FMT_TEXT:
    default:
        snprintf(out, len, "%lld", (long long)llround(real));
        break;
    }
}
