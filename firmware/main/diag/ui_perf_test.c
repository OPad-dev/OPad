/*
 * UI performance test (CONFIG_OSUPAD_UI_PERF_TEST): an evaluation build to
 * compare LVGL's memory in internal RAM vs PSRAM (CONFIG_OSUPAD_LVGL_PSRAM),
 * the LCD at 40 vs 80 MHz (CONFIG_OSUPAD_LCD_PCLK_80MHZ) and the refresh
 * period (CONFIG_LV_DEF_REFR_PERIOD). Not a product build.
 *
 * Loops a fake osu! session on the real screens, driven the way the host
 * drives them: the runtime state (runtime_notify_gameplay) picks the idle or
 * playing screen in ui_port.c, and tosu-like values arrive every
 * DATA_PERIOD_MS, the default gameplay display rate. One cycle:
 *   IDLE_MS on the idle screen, a SONG_MS map on the playing screen, then the
 *   SS result animation over the idle screen, cycling pop / drop / burst.
 * LVGL's monitors (sdkconfig.uitest) show FPS/CPU and memory in the corners.
 * Keys and touch keep working; the keycards show real presses.
 */
#include "ui_perf_test.h"
#include "sdkconfig.h"

#ifdef CONFIG_OSUPAD_UI_PERF_TEST

#include "ui/ui.h"
#include "ui/core/ui_anim.h"
#include "runtime/runtime.h"
#include "board_pins.h"
#include "lvgl.h"
#include "esp_log.h"

static const char *TAG = "ui_perf";

#define DATA_PERIOD_MS 100      // DEVICE_CONFIG_DEFAULT_GAMEPLAY_DISPLAY_HZ = 10
#define IDLE_MS        8000
#define SONG_MS        25000
#define RESULT_MS      (UI_ANIM_DURATION_MS + 1400)
#define CYCLE_MS       (IDLE_MS + SONG_MS + RESULT_MS)

#define SONG_OBJECTS   420
#define SONG_FINAL_PP  712.0    // not ending in 67: that would start the easter egg

static uint32_t s_t_ms;         // position in the cycle
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
        set_idle_data();
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
    lv_label_set_text_fmt(label, "LVGL %s %d KB | LCD %d MHz | %d ms",
#ifdef CONFIG_OSUPAD_LVGL_PSRAM
                          "PSRAM",
#else
                          "internal",
#endif
                          CONFIG_LV_MEM_SIZE_KILOBYTES, BOARD_LCD_PIXEL_CLOCK_HZ / 1000000,
                          CONFIG_LV_DEF_REFR_PERIOD);
    lv_obj_align(label, LV_ALIGN_TOP_MID, 0, 0);
    lv_timer_create(tick, DATA_PERIOD_MS, NULL);
    ui_unlock();
}

#endif  // CONFIG_OSUPAD_UI_PERF_TEST
