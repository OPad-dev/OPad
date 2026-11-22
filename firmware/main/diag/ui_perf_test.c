/*
 * UI performance test (CONFIG_OPAD_UI_PERF_TEST): an evaluation build to
 * compare LVGL's memory in internal RAM vs PSRAM (CONFIG_OPAD_LVGL_PSRAM)
 * and the refresh period (CONFIG_LV_DEF_REFR_PERIOD). Not a product build.
 *
 * Loops a fake osu! session on the real screens, driven the way the host
 * drives them: the runtime state (runtime_notify_gameplay) picks the idle or
 * playing screen in ui_port.c, and tosu-like values arrive every
 * DATA_PERIOD_MS, the default gameplay display rate. One cycle:
 *   IDLE_MS on the idle screen, a SONG_MS map on the playing screen, then the
 *   SS result animation over the idle screen, cycling pop / drop / burst.
 * LVGL's monitors (sdkconfig.uitest) show FPS/CPU and memory in the corners.
 * At the end of each phase the pad also sends DIAG_EVENT_UI_PERF (opadctl
 * monitor): the frames drawn, and the average and longest time a frame took
 * from the start of LVGL's refresh to its end.
 * Keys and touch keep working; the keycards show real presses.
 */
#include "ui_perf_test.h"
#include "sdkconfig.h"

#ifdef CONFIG_OPAD_UI_PERF_TEST

#include "ui/ui.h"
#include "ui/core/ui_anim.h"
#include "runtime/runtime.h"
#include "board_pins.h"
#include "lvgl.h"
#include "diag/diag.h"
#include "esp_log.h"
#include "esp_timer.h"

static const char *TAG = "ui_perf";

#define DATA_PERIOD_MS 100      // DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ = 10
#define IDLE_MS        8000
#define SONG_MS        25000
#define RESULT_MS      (UI_ANIM_DURATION_MS + 1400)
#define CYCLE_MS       (IDLE_MS + SONG_MS + RESULT_MS)

#define SONG_OBJECTS   420
#define SONG_FINAL_PP  712.0    // not ending in 67: that would start the easter egg

static uint32_t s_t_ms;         // position in the cycle

// Frame times of the current phase, from LVGL's display events (LVGL task)
static int64_t s_refr_start_us;
static bool s_rendered;         // this refresh drew something
static uint32_t s_frames;
static uint64_t s_frame_sum_us;
static uint32_t s_frame_max_us;

static void on_display_event(lv_event_t *e)
{
    switch (lv_event_get_code(e)) {
    case LV_EVENT_REFR_START:
        s_refr_start_us = esp_timer_get_time();
        s_rendered = false;
        break;
    case LV_EVENT_RENDER_START:
        s_rendered = true;
        break;
    case LV_EVENT_REFR_READY:
        if (s_rendered) {
            uint32_t us = (uint32_t)(esp_timer_get_time() - s_refr_start_us);
            s_frames++;
            s_frame_sum_us += us;
            if (us > s_frame_max_us) s_frame_max_us = us;
        }
        break;
    default:
        break;
    }
}

// One DIAG_EVENT_UI_PERF for the phase that ended, then a fresh count
static void report_phase(uint8_t phase)
{
    static bool s_counting;  // the first call only starts the count
    if (!s_counting) {
        s_counting = true;
        s_frames = 0;
        s_frame_sum_us = 0;
        s_frame_max_us = 0;
        return;
    }
    uint32_t avg_us = s_frames ? (uint32_t)(s_frame_sum_us / s_frames) : 0;
    uint32_t frames = s_frames > 0xFFFF ? 0xFFFF : s_frames;
    uint32_t avg_100us = avg_us / 100 > 0xFFFF ? 0xFFFF : avg_us / 100;
    uint32_t max_100us = s_frame_max_us / 100 > 0xFFFF ? 0xFFFF : s_frame_max_us / 100;
    diag_record(DIAG_EVENT_UI_PERF, 1 /* INFO */, ((uint32_t)phase << 16) | frames, (max_100us << 16) | avg_100us);
    ESP_LOGI(TAG, "phase %u: %lu frames, avg %lu us, max %lu us", phase, (unsigned long)s_frames,
             (unsigned long)avg_us, (unsigned long)s_frame_max_us);
    s_frames = 0;
    s_frame_sum_us = 0;
    s_frame_max_us = 0;
}
static uint8_t s_anim = UI_ANIM_POP;

static void set_idle_data(void)
{
    ui_data_set_number(UI_SRC_STATUS_TOSU, 1);
    ui_data_set_number(UI_SRC_STATUS_OSU, 1);
    ui_data_set_string(UI_SRC_PROFILE_NAME, "OPad tester");
    ui_data_set_number(UI_SRC_PROFILE_RANK, 12345);
    ui_data_set_number(UI_SRC_PROFILE_PP, 7021);
    ui_data_set_string(UI_SRC_GAME_STATE, "Song select");
}

static void set_song_start(void)
{
    ui_data_set_string(UI_SRC_MAP_TITLE, "Freedom Dive");
    ui_data_set_string(UI_SRC_MAP_ARTIST, "xi");
    ui_data_set_string(UI_SRC_MAP_DIFFICULTY, "FOUR DIMENSIONS");
    ui_data_set_number(UI_SRC_MAP_STARS, 7.21);
    ui_data_set_number(UI_SRC_MAP_LENGTH, SONG_MS);
    ui_data_set_number(UI_SRC_MAP_MAX_COMBO, SONG_OBJECTS);
    ui_data_set_string(UI_SRC_PLAY_MODS, "HDDT");
    ui_data_set_string(UI_SRC_PLAY_GRADE, "SS");
    ui_data_set_string(UI_SRC_GAME_STATE, "Playing");
}

// One 10 Hz update during the song, at t_ms into it
static void set_song_data(uint32_t t_ms)
{
    double p = (double)t_ms / SONG_MS;
    int32_t combo = (int32_t)(p * SONG_OBJECTS);
    ui_data_set_number(UI_SRC_MAP_TIME_ELAPSED, t_ms);
    ui_data_set_number(UI_SRC_MAP_TIME_REMAINING, SONG_MS - t_ms);
    ui_data_set_number(UI_SRC_MAP_PROGRESS, p);
    ui_data_set_number(UI_SRC_PLAY_PP, p * SONG_FINAL_PP);
    ui_data_set_number(UI_SRC_PLAY_PP_FC, SONG_FINAL_PP);
    ui_data_set_number(UI_SRC_PLAY_ACCURACY, 100.0);
    ui_data_set_number(UI_SRC_PLAY_COMBO, combo);
    ui_data_set_number(UI_SRC_PLAY_MAX_COMBO, combo);
    ui_data_set_number(UI_SRC_PLAY_HITS_300, combo);
    ui_data_set_number(UI_SRC_PLAY_SCORE, (double)combo * 23456);
    ui_data_set_number(UI_SRC_PLAY_HEALTH, 1.0);
    // A stream with some swing, so the numbers keep changing
    int32_t ppm = 250 + (int32_t)((t_ms / 100) % 40);
    ui_data_set_number(UI_SRC_PLAY_PPM, ppm);
    ui_data_set_number(UI_SRC_PLAY_PPM_PEAK, 289);
    ui_data_set_number(UI_SRC_PLAY_PPM_SONG, t_ms >= 1000 ? 262 : 0);
}

static void tick(lv_timer_t *timer)
{
    (void)timer;
    uint32_t t = s_t_ms;
    if (t == 0) {
        report_phase(DIAG_UI_PERF_RESULT);
        set_idle_data();
    } else if (t == IDLE_MS) {
        report_phase(DIAG_UI_PERF_IDLE);
    } else if (t == IDLE_MS + SONG_MS) {
        report_phase(DIAG_UI_PERF_SONG);
    }
    if (t < IDLE_MS) {
        // idle screen: nothing but the clock and the real key counters
    } else if (t < IDLE_MS + SONG_MS) {
        if (t == IDLE_MS) {
            set_song_start();
        }
        runtime_notify_gameplay(true);  // within the 3 s it times out after
        set_song_data(t - IDLE_MS);
    } else if (t == IDLE_MS + SONG_MS) {
        set_song_data(SONG_MS);
        runtime_notify_gameplay(false);  // COOLDOWN: ui_port.c loads the idle screen
        // As the host does on osu!'s results screen (PlayAnimation)
        ui_anim_play(s_anim, "SS");
        ESP_LOGI(TAG, "Result animation %u", s_anim);
        s_anim = s_anim % UI_ANIM_BUILTIN_COUNT + 1;
    }
    ui_notify_activity();  // the screen would sleep after its timeout
    s_t_ms = (t + DATA_PERIOD_MS) % CYCLE_MS;
}

void ui_perf_test_start(void)
{
    ESP_LOGW(TAG, "UI perf test: idle %d s, song %d s, SS result", IDLE_MS / 1000, SONG_MS / 1000);
    if (!ui_lock()) {
        return;  // headless
    }
    lv_obj_t *label = lv_label_create(lv_layer_sys());
    lv_obj_set_style_text_color(label, lv_color_hex(0xFFFFFF), 0);
    lv_obj_set_style_text_font(label, &lv_font_montserrat_12, 0);
    lv_obj_set_style_bg_color(label, lv_color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(label, LV_OPA_60, 0);
    lv_label_set_text_fmt(label, "LVGL %s %d KB | %d lines | LCD %d MHz | %d ms",
#ifdef CONFIG_OPAD_LVGL_PSRAM
                          "PSRAM",
#else
                          "internal",
#endif
                          CONFIG_LV_MEM_SIZE_KILOBYTES, UI_DRAW_BUF_LINES, BOARD_LCD_PIXEL_CLOCK_HZ / 1000000,
                          CONFIG_LV_DEF_REFR_PERIOD);
    lv_obj_align(label, LV_ALIGN_TOP_MID, 0, 0);
    lv_display_t *disp = lv_display_get_default();
    lv_display_add_event_cb(disp, on_display_event, LV_EVENT_REFR_START, NULL);
    lv_display_add_event_cb(disp, on_display_event, LV_EVENT_RENDER_START, NULL);
    lv_display_add_event_cb(disp, on_display_event, LV_EVENT_REFR_READY, NULL);
    lv_timer_create(tick, DATA_PERIOD_MS, NULL);
    ui_unlock();
}

#endif  // CONFIG_OPAD_UI_PERF_TEST
