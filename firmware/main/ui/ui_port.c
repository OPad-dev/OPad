#include "ui.h"
#include "ui/ui_store.h"
#include "ui/easter_egg.h"
#include "ui/core/ui_internal.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "input/keypad.h"
#include "runtime/runtime.h"
#include "diag/diag.h"
#include "usb/usb_cdc.h"
#include "esp_lvgl_port.h"
#include "esp_log.h"
#include "esp_timer.h"
#include <stdatomic.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>

static const char *TAG = "ui";

#define PAD_TIMER_PERIOD_MS 20
#define KEY_FLASH_HOLD_US   80000     // keep a tap visible for at least one frame or two
#define KPS_WINDOW          (1000 / PAD_TIMER_PERIOD_MS)

static lv_display_t *s_disp;

static ui_layout_t s_layouts[UI_SCREEN_COUNT];
static lv_obj_t *s_screens[UI_SCREEN_COUNT];
static int s_active_screen = -1;

static atomic_llong s_last_activity_us;
static volatile uint8_t s_brightness = 80;
static volatile uint32_t s_sleep_timeout_s = 600;
static volatile bool s_asleep;

static uint64_t s_kps_ring[KPS_WINDOW];
static int s_kps_pos;
static int s_kps_fill;  // samples in the ring, up to KPS_WINDOW
static time_t s_clock_minute = -1;  // t / 60 of the last formatted pad.clock/pad.date

// ---- helpers (LVGL lock held) -------------------------------------------------------

static uint32_t ui_tick_ms(void)
{
    return (uint32_t)(esp_timer_get_time() / 1000);
}

static void rebuild_screen(uint8_t screen)
{
    lv_obj_t *old = s_screens[screen];
    lv_obj_t *fresh = ui_screen_create(&s_layouts[screen]);
    if (!fresh) {
        ESP_LOGE(TAG, "Failed to build screen %u", screen);
        return;
    }
    s_screens[screen] = fresh;
    if (s_active_screen == screen) {
        lv_screen_load(fresh);
    }
    ui_screen_delete(old);
}

static void update_pad_sources(int64_t now_us)
{
    uint64_t k1_life = 0, k2_life = 0;
    uint32_t k1_map = 0, k2_map = 0;
    keypad_get_lifetime_presses(&k1_life, &k2_life);
    keypad_get_map_presses(&k1_map, &k2_map);

    ui_data_set_number(UI_SRC_PAD_K1_LIFETIME, (double)k1_life);
    ui_data_set_number(UI_SRC_PAD_K2_LIFETIME, (double)k2_life);
    ui_data_set_number(UI_SRC_PAD_TOTAL_LIFETIME, (double)(k1_life + k2_life));
    ui_data_set_number(UI_SRC_PAD_K1_MAP, k1_map);
    ui_data_set_number(UI_SRC_PAD_K2_MAP, k2_map);
    ui_data_set_number(UI_SRC_PAD_TOTAL_MAP, (double)k1_map + k2_map);

    bool k1_down = keypad_is_pressed(KEY_ID_1) || (now_us - keypad_get_last_press_us(KEY_ID_1)) < KEY_FLASH_HOLD_US;
    bool k2_down = keypad_is_pressed(KEY_ID_2) || (now_us - keypad_get_last_press_us(KEY_ID_2)) < KEY_FLASH_HOLD_US;
    ui_data_set_number(UI_SRC_PAD_K1_DOWN, k1_down);
    ui_data_set_number(UI_SRC_PAD_K2_DOWN, k2_down);

    // Taps in the last second, or since the oldest sample while the ring fills.
    // Lifetime counters drop on a forced sync or reset: start the window over.
    uint64_t total = k1_life + k2_life;
    if (s_kps_fill > 0 && total < s_kps_ring[(s_kps_pos + KPS_WINDOW - 1) % KPS_WINDOW]) {
        s_kps_fill = 0;
    }
    uint64_t oldest = s_kps_fill ? s_kps_ring[(s_kps_pos + KPS_WINDOW - s_kps_fill) % KPS_WINDOW] : total;
    int64_t taps = (int64_t)(total - oldest);
    s_kps_ring[s_kps_pos] = total;
    s_kps_pos = (s_kps_pos + 1) % KPS_WINDOW;
    if (s_kps_fill < KPS_WINDOW) {
        s_kps_fill++;
    }
    ui_data_set_number(UI_SRC_PAD_KPS, taps > 0 ? (double)taps : 0);

    ui_data_set_number(UI_SRC_STATUS_PC, usb_cdc_is_connected());
    ui_data_set_number(UI_SRC_PAD_UPTIME, (double)(now_us / 1000000 * 1000));

    // Both strings have minute resolution (no TZ on the device): format once a minute
    time_t t = time(NULL);
    if (t > 1600000000 && t / 60 != s_clock_minute) {  // set by the host
        s_clock_minute = t / 60;
        struct tm tm;
        localtime_r(&t, &tm);
        char buf[24];
        strftime(buf, sizeof(buf), "%H:%M", &tm);
        ui_data_set_string(UI_SRC_PAD_CLOCK, buf);
        strftime(buf, sizeof(buf), "%a %d %b", &tm);
        ui_data_set_string(UI_SRC_PAD_DATE, buf);
    }
}

static void update_sleep(int64_t now_us)
{
    int64_t idle_us = now_us - atomic_load(&s_last_activity_us);
    bool should_sleep = s_sleep_timeout_s > 0 && idle_us > (int64_t)s_sleep_timeout_s * 1000000;

    if (should_sleep && !s_asleep) {
        s_asleep = true;
        lv_display_enable_invalidation(s_disp, false);
        board_display_sleep();
        diag_record(DIAG_EVENT_DISPLAY_SLEEP, 1 /* INFO */, 0, 0);
        ESP_LOGI(TAG, "Display asleep");
    } else if (!should_sleep && s_asleep) {
        s_asleep = false;
        board_display_wake();
        // The brightness may have changed while asleep
        board_display_set_brightness(s_brightness);
        lv_display_enable_invalidation(s_disp, true);
        lv_obj_invalidate(lv_screen_active());
        diag_record(DIAG_EVENT_DISPLAY_WAKE, 1 /* INFO */, 0, 0);
    }
}

// Runs in the LVGL task (core 1) every PAD_TIMER_PERIOD_MS
static void pad_timer_cb(lv_timer_t *timer)
{
    (void)timer;
    int64_t now_us = esp_timer_get_time();

    osupad_state_t st = runtime_get_state();
    static osupad_state_t s_prev_runtime_state = OSUPAD_STATE_IDLE;
    if (s_prev_runtime_state == OSUPAD_STATE_PLAYING && (st == OSUPAD_STATE_COOLDOWN || st == OSUPAD_STATE_IDLE)) {
        lv_subject_t *subj = ui_data_subject(UI_SRC_PLAY_PP);
        if (subj) {
            int32_t raw_pp = lv_subject_get_int(subj);
            if (raw_pp != UI_VALUE_EMPTY && raw_pp > 0) {
                int32_t pp = (raw_pp + 50) / 100;
                ESP_LOGI(TAG, "Song finished with PP: %ld", (long)pp);
                if (pp > 0 && (pp % 100 == 67)) {
                    ESP_LOGI(TAG, "Score PP ends in 67 -> Triggering easter egg!");
                    easter_egg_trigger();
                }
            }
        }
    }
    s_prev_runtime_state = st;

    int want = st == OSUPAD_STATE_PLAYING ? UI_SCREEN_PLAYING : UI_SCREEN_IDLE;
    if (want != s_active_screen && s_screens[want]) {
        s_active_screen = want;
        lv_screen_load(s_screens[want]);
    }

    update_sleep(now_us);
    if (!s_asleep) {
        update_pad_sources(now_us);
    }
}

static bool s_ui_ok = false;

// ---- public API ----------------------------------------------------------------------

bool ui_is_ok(void)
{
    return s_ui_ok;
}

esp_err_t ui_init(void)
{
    s_ui_ok = false;

    esp_lcd_panel_io_handle_t io = NULL;
    esp_lcd_panel_handle_t panel = NULL;
    esp_err_t err = board_display_init(&io, &panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "board_display_init failed: %s", esp_err_to_name(err));
        return err;
    }

    // Core 1, low priority: rendering never competes with the key/USB path on core 0
    lvgl_port_cfg_t port_cfg = ESP_LVGL_PORT_INIT_CONFIG();
    port_cfg.task_affinity = 1;
    port_cfg.task_priority = 2;
    port_cfg.task_max_sleep_ms = 500;
    err = lvgl_port_init(&port_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "lvgl_port_init failed: %s", esp_err_to_name(err));
        return err;
    }

    const lvgl_port_display_cfg_t disp_cfg = {
        .io_handle = io,
        .panel_handle = panel,
        .buffer_size = UI_SCREEN_W * 40,
        .double_buffer = true,
        .hres = UI_SCREEN_W,
        .vres = UI_SCREEN_H,
        .color_format = LV_COLOR_FORMAT_RGB565,
        .rotation = { .swap_xy = true, .mirror_x = true, .mirror_y = false },
        // ST7789 over SPI wants big-endian RGB565; with the panel in RGB order (see
        // board_display.c) this gives the exact design colors
        .flags = { .buff_dma = true, .swap_bytes = true },
    };
    s_disp = lvgl_port_add_disp(&disp_cfg);
    if (!s_disp) {
        ESP_LOGE(TAG, "lvgl_port_add_disp failed");
        return ESP_FAIL;
    }

    atomic_store(&s_last_activity_us, esp_timer_get_time());

    lvgl_port_lock(0);
    // esp_lvgl_port drives lv_tick from a periodic esp_timer, whose task and ISR live on
    // core 0. Stop it and let LVGL read the clock directly instead.
    lvgl_port_stop();
    lv_tick_set_cb(ui_tick_ms);
    lv_timer_enable(true);
    easter_egg_init();

    ui_data_init();
    for (int s = 0; s < UI_SCREEN_COUNT; s++) {
        if (!ui_store_load(s, &s_layouts[s])) {
            s_layouts[s] = *ui_default_layout(s);
        }
        rebuild_screen(s);
    }
    if (s_screens[UI_SCREEN_IDLE] == NULL) {
        // Out of LVGL memory: fall back to headless rather than load a NULL screen
        lvgl_port_unlock();
        ESP_LOGE(TAG, "Idle screen could not be built");
        return ESP_ERR_NO_MEM;
    }
    s_active_screen = UI_SCREEN_IDLE;
    lv_screen_load(s_screens[UI_SCREEN_IDLE]);
    lv_timer_create(pad_timer_cb, PAD_TIMER_PERIOD_MS, NULL);
    lvgl_port_unlock();

    board_display_set_brightness(s_brightness);
    s_ui_ok = true;
    ESP_LOGI(TAG, "LVGL UI started on core 1");
    return ESP_OK;
}

void ui_notify_activity(void)
{
    if (!s_ui_ok) return;
    atomic_store(&s_last_activity_us, esp_timer_get_time());
}

void ui_set_time(uint32_t year, uint32_t month, uint32_t day, uint32_t hour, uint32_t minute, uint32_t second)
{
    // Host local time stored as UTC (no TZ on the device), so localtime() returns it unchanged
    struct tm tm = {
        .tm_year = (int)year - 1900, .tm_mon = (int)month - 1, .tm_mday = (int)day,
        .tm_hour = (int)hour, .tm_min = (int)minute, .tm_sec = (int)second,
    };
    struct timeval tv = { .tv_sec = mktime(&tm) };
    settimeofday(&tv, NULL);
}

// s_brightness is the only record of the setting: applied now if awake, on wake
// otherwise. Under the LVGL lock so it cannot interleave with update_sleep().
void ui_set_brightness(uint8_t percent)
{
    if (!s_ui_ok) {
        s_brightness = percent > 100 ? 100 : percent;  // ui_init applies it
        return;
    }
    lvgl_port_lock(0);
    s_brightness = percent > 100 ? 100 : percent;
    if (!s_asleep) {
        board_display_set_brightness(s_brightness);
    }
    lvgl_port_unlock();
}

void ui_set_sleep_timeout(uint32_t seconds)
{
    s_sleep_timeout_s = seconds;
}

bool ui_is_asleep(void)
{
    if (!s_ui_ok) return false;
    return s_asleep;
}

void ui_set_tosu_connected(bool connected)
{
    if (!s_ui_ok) return;
    lvgl_port_lock(0);
    ui_data_set_number(UI_SRC_STATUS_TOSU, connected);
    if (!connected) {
        ui_data_clear_host_sources();
        ui_data_set_number(UI_SRC_STATUS_OSU, 0);
    }
    lvgl_port_unlock();
}

bool ui_lock(void)
{
    if (!s_ui_ok) return false;
    lvgl_port_lock(0);
    return true;
}

void ui_unlock(void)
{
    if (!s_ui_ok) return;
    lvgl_port_unlock();
}

void ui_set_number(uint8_t source, double value)
{
    if (!s_ui_ok) return;
    lvgl_port_lock(0);
    ui_data_set_number(source, value);
    lvgl_port_unlock();
}

void ui_set_string(uint8_t source, const char *value)
{
    if (!s_ui_ok) return;
    lvgl_port_lock(0);
    ui_data_set_string(source, value);
    lvgl_port_unlock();
}

bool ui_set_layout(uint8_t screen, const ui_layout_t *layout, char *err, size_t err_len)
{
    if (!s_ui_ok) {
        if (err && err_len > 0) {
            snprintf(err, err_len, "UI disabled (LCD init failed)");
        }
        return false;
    }
    if (screen >= UI_SCREEN_COUNT) {
        snprintf(err, err_len, "unknown screen %u", screen);
        return false;
    }
    if (!ui_layout_validate(layout, err, err_len)) {
        return false;
    }
    lvgl_port_lock(0);
    s_layouts[screen] = *layout;
    rebuild_screen(screen);
    lvgl_port_unlock();
    return true;
}

void ui_trigger_easter_egg(void)
{
    if (!s_ui_ok) return;
    lvgl_port_lock(0);
    easter_egg_trigger();
    lvgl_port_unlock();
}

void ui_show_notice(const char *text)
{
    if (!s_ui_ok || !text) return;
    lvgl_port_lock(0);
    lv_obj_t *box = lv_obj_create(lv_layer_top());
    lv_obj_set_size(box, lv_pct(92), LV_SIZE_CONTENT);
    lv_obj_align(box, LV_ALIGN_BOTTOM_MID, 0, -8);
    lv_obj_set_style_bg_color(box, lv_color_hex(0x7A1F1F), 0);
    lv_obj_set_style_bg_opa(box, LV_OPA_90, 0);
    lv_obj_set_style_border_width(box, 0, 0);
    lv_obj_set_style_radius(box, 8, 0);
    lv_obj_set_style_pad_all(box, 8, 0);
    lv_obj_remove_flag(box, LV_OBJ_FLAG_SCROLLABLE);
    lv_obj_t *label = lv_label_create(box);
    lv_obj_set_width(label, lv_pct(100));
    lv_label_set_long_mode(label, LV_LABEL_LONG_WRAP);
    lv_obj_set_style_text_color(label, lv_color_white(), 0);
    lv_obj_set_style_text_font(label, &lv_font_montserrat_14, 0);
    lv_label_set_text(label, text);
    lvgl_port_unlock();
}
