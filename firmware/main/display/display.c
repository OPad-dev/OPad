#include "display.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "counters/counters.h"
#include "runtime/runtime.h"
#include "input/keypad.h"
#include "usb/usb_hid.h"
#include "usb/usb_cdc.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_timer.h"
#include "esp_log.h"
#include <string.h>
#include <stdio.h>

static const char *TAG = "display";

static esp_lcd_panel_handle_t s_panel = NULL;
static uint8_t s_brightness = 80;
static uint32_t s_sleep_timeout_sec = 600; // 10 minutes default
static volatile bool s_is_asleep = false;
static int64_t s_last_activity_us = 0;

static bool s_time_synced = false;
static int s_year = 2026, s_month = 1, s_day = 1;
static int s_hour = 0, s_minute = 0, s_second = 0;
static int64_t s_time_sync_us = 0;

static osupad_GameplayDisplayState s_gameplay_state;
static bool s_has_gameplay = false;

// 16-bit RGB565 Colors
#define COLOR_BLACK   0x0000
#define COLOR_WHITE   0xFFFF
#define COLOR_RED     0xF800
#define COLOR_GREEN   0x07E0
#define COLOR_BLUE    0x001F
#define COLOR_YELLOW  0xFFE0
#define COLOR_PINK    0xF81F
#define COLOR_GRAY    0x8410
#define COLOR_DARK_GRAY 0x2104

// Compact 8x16 basic ASCII font bit patterns (standard printable range 32 - 126)
// For embedded efficiency, we implement a clean 8x8 font scaled to 8x16
static const uint8_t font8x8[95][8] = {
    [' ' - 32] = {0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00},
    ['!' - 32] = {0x18, 0x18, 0x18, 0x18, 0x18, 0x00, 0x18, 0x00},
    ['"' - 32] = {0x66, 0x66, 0x66, 0x00, 0x00, 0x00, 0x00, 0x00},
    ['#' - 32] = {0x66, 0x66, 0xFF, 0x66, 0xFF, 0x66, 0x66, 0x00},
    ['$' - 32] = {0x18, 0x7E, 0x18, 0x3C, 0x18, 0x7E, 0x18, 0x00},
    ['%' - 32] = {0x62, 0x64, 0x08, 0x10, 0x20, 0x26, 0x46, 0x00},
    ['&' - 32] = {0x38, 0x6C, 0x38, 0x76, 0xDC, 0xCC, 0x76, 0x00},
    ['\'' - 32] = {0x18, 0x18, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00},
    ['(' - 32] = {0x0C, 0x18, 0x30, 0x30, 0x30, 0x18, 0x0C, 0x00},
    [')' - 32] = {0x30, 0x18, 0x0C, 0x0C, 0x0C, 0x18, 0x30, 0x00},
    ['*' - 32] = {0x00, 0x66, 0x3C, 0xFF, 0x3C, 0x66, 0x00, 0x00},
    ['+' - 32] = {0x00, 0x18, 0x18, 0x7E, 0x18, 0x18, 0x00, 0x00},
    [',' - 32] = {0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x30},
    ['-' - 32] = {0x00, 0x00, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x00},
    ['.' - 32] = {0x00, 0x00, 0x00, 0x00, 0x00, 0x18, 0x18, 0x00},
    ['/' - 32] = {0x02, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x40, 0x00},
    ['0' - 32] = {0x3C, 0x66, 0x6E, 0x76, 0x66, 0x66, 0x3C, 0x00},
    ['1' - 32] = {0x18, 0x38, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00},
    ['2' - 32] = {0x3C, 0x66, 0x06, 0x0C, 0x18, 0x30, 0x7E, 0x00},
    ['3' - 32] = {0x3C, 0x66, 0x06, 0x1C, 0x06, 0x66, 0x3C, 0x00},
    ['4' - 32] = {0x0C, 0x1C, 0x34, 0x64, 0x7E, 0x04, 0x0E, 0x00},
    ['5' - 32] = {0x7E, 0x60, 0x7C, 0x06, 0x06, 0x66, 0x3C, 0x00},
    ['6' - 32] = {0x3C, 0x66, 0x60, 0x7C, 0x66, 0x66, 0x3C, 0x00},
    ['7' - 32] = {0x7E, 0x66, 0x0C, 0x18, 0x18, 0x18, 0x18, 0x00},
    ['8' - 32] = {0x3C, 0x66, 0x66, 0x3C, 0x66, 0x66, 0x3C, 0x00},
    ['9' - 32] = {0x3C, 0x66, 0x66, 0x3E, 0x06, 0x66, 0x3C, 0x00},
    [':' - 32] = {0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x00, 0x00},
    [';' - 32] = {0x00, 0x18, 0x18, 0x00, 0x18, 0x18, 0x30, 0x00},
    ['<' - 32] = {0x06, 0x0C, 0x18, 0x30, 0x18, 0x0C, 0x06, 0x00},
    ['=' - 32] = {0x00, 0x00, 0x7E, 0x00, 0x7E, 0x00, 0x00, 0x00},
    ['>' - 32] = {0x30, 0x18, 0x0C, 0x06, 0x0C, 0x18, 0x30, 0x00},
    ['?' - 32] = {0x3C, 0x66, 0x06, 0x0C, 0x18, 0x00, 0x18, 0x00},
    ['@' - 32] = {0x3C, 0x66, 0x6E, 0x6E, 0x60, 0x62, 0x3C, 0x00},
    ['A' - 32] = {0x18, 0x3C, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x00},
    ['B' - 32] = {0x7C, 0x66, 0x66, 0x7C, 0x66, 0x66, 0x7C, 0x00},
    ['C' - 32] = {0x3C, 0x66, 0x60, 0x60, 0x60, 0x66, 0x3C, 0x00},
    ['D' - 32] = {0x78, 0x6C, 0x66, 0x66, 0x66, 0x6C, 0x78, 0x00},
    ['E' - 32] = {0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x7E, 0x00},
    ['F' - 32] = {0x7E, 0x60, 0x60, 0x7C, 0x60, 0x60, 0x60, 0x00},
    ['G' - 32] = {0x3C, 0x66, 0x60, 0x6E, 0x66, 0x66, 0x3E, 0x00},
    ['H' - 32] = {0x66, 0x66, 0x66, 0x7E, 0x66, 0x66, 0x66, 0x00},
    ['I' - 32] = {0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7E, 0x00},
    ['J' - 32] = {0x1E, 0x0C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38, 0x00},
    ['K' - 32] = {0x66, 0x6C, 0x78, 0x70, 0x78, 0x6C, 0x66, 0x00},
    ['L' - 32] = {0x60, 0x60, 0x60, 0x60, 0x60, 0x60, 0x7E, 0x00},
    ['M' - 32] = {0x63, 0x77, 0x7F, 0x6B, 0x63, 0x63, 0x63, 0x00},
    ['N' - 32] = {0x66, 0x76, 0x7E, 0x7E, 0x6E, 0x66, 0x66, 0x00},
    ['O' - 32] = {0x3C, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00},
    ['P' - 32] = {0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60, 0x60, 0x00},
    ['Q' - 32] = {0x3C, 0x66, 0x66, 0x66, 0x6A, 0x6C, 0x36, 0x00},
    ['R' - 32] = {0x7C, 0x66, 0x66, 0x7C, 0x6C, 0x66, 0x63, 0x00},
    ['S' - 32] = {0x3C, 0x66, 0x60, 0x3C, 0x06, 0x66, 0x3C, 0x00},
    ['T' - 32] = {0x7E, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x00},
    ['U' - 32] = {0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x00},
    ['V' - 32] = {0x66, 0x66, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00},
    ['W' - 32] = {0x63, 0x63, 0x63, 0x6B, 0x7F, 0x77, 0x63, 0x00},
    ['X' - 32] = {0x66, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x66, 0x00},
    ['Y' - 32] = {0x66, 0x66, 0x66, 0x3C, 0x18, 0x18, 0x18, 0x00},
    ['Z' - 32] = {0x7E, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x7E, 0x00},
    ['[' - 32] = {0x3C, 0x30, 0x30, 0x30, 0x30, 0x30, 0x3C, 0x00},
    ['\\' - 32] = {0x40, 0x60, 0x30, 0x18, 0x0C, 0x06, 0x02, 0x00},
    [']' - 32] = {0x3C, 0x0C, 0x0C, 0x0C, 0x0C, 0x0C, 0x3C, 0x00},
    ['^' - 32] = {0x10, 0x38, 0x6C, 0xC6, 0x00, 0x00, 0x00, 0x00},
    ['_' - 32] = {0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00},
    ['`' - 32] = {0x30, 0x18, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00},
    ['a' - 32] = {0x00, 0x00, 0x3C, 0x06, 0x3E, 0x66, 0x3E, 0x00},
    ['b' - 32] = {0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x7C, 0x00},
    ['c' - 32] = {0x00, 0x00, 0x3C, 0x66, 0x60, 0x66, 0x3C, 0x00},
    ['d' - 32] = {0x06, 0x06, 0x3E, 0x66, 0x66, 0x66, 0x3E, 0x00},
    ['e' - 32] = {0x00, 0x00, 0x3C, 0x66, 0x7E, 0x60, 0x3C, 0x00},
    ['f' - 32] = {0x1C, 0x30, 0x78, 0x30, 0x30, 0x30, 0x30, 0x00},
    ['g' - 32] = {0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x3C},
    ['h' - 32] = {0x60, 0x60, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00},
    ['i' - 32] = {0x18, 0x00, 0x38, 0x18, 0x18, 0x18, 0x3C, 0x00},
    ['j' - 32] = {0x0C, 0x00, 0x1C, 0x0C, 0x0C, 0x0C, 0x6C, 0x38},
    ['k' - 32] = {0x60, 0x60, 0x66, 0x6C, 0x78, 0x6C, 0x66, 0x00},
    ['l' - 32] = {0x38, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, 0x00},
    ['m' - 32] = {0x00, 0x00, 0x7E, 0x6B, 0x6B, 0x6B, 0x6B, 0x00},
    ['n' - 32] = {0x00, 0x00, 0x7C, 0x66, 0x66, 0x66, 0x66, 0x00},
    ['o' - 32] = {0x00, 0x00, 0x3C, 0x66, 0x66, 0x66, 0x3C, 0x00},
    ['p' - 32] = {0x00, 0x00, 0x7C, 0x66, 0x66, 0x7C, 0x60, 0x60},
    ['q' - 32] = {0x00, 0x00, 0x3E, 0x66, 0x66, 0x3E, 0x06, 0x07},
    ['r' - 32] = {0x00, 0x00, 0x7C, 0x66, 0x60, 0x60, 0x60, 0x00},
    ['s' - 32] = {0x00, 0x00, 0x3E, 0x60, 0x3C, 0x06, 0x7C, 0x00},
    ['t' - 32] = {0x30, 0x30, 0x78, 0x30, 0x30, 0x34, 0x18, 0x00},
    ['u' - 32] = {0x00, 0x00, 0x66, 0x66, 0x66, 0x66, 0x3E, 0x00},
    ['v' - 32] = {0x00, 0x00, 0x66, 0x66, 0x66, 0x3C, 0x18, 0x00},
    ['w' - 32] = {0x00, 0x00, 0x63, 0x6B, 0x6B, 0x7F, 0x36, 0x00},
    ['x' - 32] = {0x00, 0x00, 0x66, 0x3C, 0x18, 0x3C, 0x66, 0x00},
    ['y' - 32] = {0x00, 0x00, 0x66, 0x66, 0x66, 0x3E, 0x06, 0x3C},
    ['z' - 32] = {0x00, 0x00, 0x7E, 0x0C, 0x18, 0x30, 0x7E, 0x00},
};

// Line buffer for fast drawing without allocating full 153KB framebuffer
static uint16_t s_line_buf[BOARD_LCD_H_RES];

static void fill_rect(int x1, int y1, int w, int h, uint16_t color)
{
    if (x1 < 0) { w += x1; x1 = 0; }
    if (y1 < 0) { h += y1; y1 = 0; }
    if (x1 + w > BOARD_LCD_H_RES) w = BOARD_LCD_H_RES - x1;
    if (y1 + h > BOARD_LCD_V_RES) h = BOARD_LCD_V_RES - y1;
    if (w <= 0 || h <= 0) return;

    for (int i = 0; i < w; i++) {
        s_line_buf[i] = color;
    }

    for (int y = y1; y < y1 + h; y++) {
        esp_lcd_panel_draw_bitmap(s_panel, x1, y, x1 + w, y + 1, s_line_buf);
    }
}

static void draw_char(int x, int y, char c, uint16_t fg, uint16_t bg)
{
    if (c < 32 || c > 126) c = ' ';
    const uint8_t *glyph = font8x8[c - 32];

    for (int row = 0; row < 8; row++) {
        uint8_t bits = glyph[row];
        for (int col = 0; col < 8; col++) {
            s_line_buf[col] = (bits & (0x80 >> col)) ? fg : bg;
        }
        // Double row height for 8x16 look
        int py = y + (row * 2);
        if (py < BOARD_LCD_V_RES && x + 8 <= BOARD_LCD_H_RES) {
            esp_lcd_panel_draw_bitmap(s_panel, x, py, x + 8, py + 1, s_line_buf);
            esp_lcd_panel_draw_bitmap(s_panel, x, py + 1, x + 8, py + 2, s_line_buf);
        }
    }
}

static void draw_text(int x, int y, const char *text, uint16_t fg, uint16_t bg)
{
    int cur_x = x;
    while (*text && cur_x < BOARD_LCD_H_RES - 8) {
        draw_char(cur_x, y, *text++, fg, bg);
        cur_x += 8;
    }
}

static void render_idle_screen(void)
{
    char buf[48];

    // Background & Header
    fill_rect(0, 0, BOARD_LCD_H_RES, 35, COLOR_DARK_GRAY);
    draw_text(16, 10, "=== osu!pad S3 ===", COLOR_YELLOW, COLOR_DARK_GRAY);

    // Clock
    if (s_time_synced) {
        int64_t elapsed_sec = (esp_timer_get_time() - s_time_sync_us) / 1000000;
        int sec = (s_second + elapsed_sec) % 60;
        int min = (s_minute + (s_second + elapsed_sec) / 60) % 60;
        int hr  = (s_hour + (s_minute + (s_second + elapsed_sec) / 60) / 60) % 24;
        snprintf(buf, sizeof(buf), "TIME:  %02d:%02d:%02d", hr, min, sec);
    } else {
        snprintf(buf, sizeof(buf), "TIME:  --:--:--");
    }
    draw_text(20, 50, buf, COLOR_WHITE, COLOR_BLACK);

    // USB & Protocol Status
    bool cdc_ok = usb_cdc_is_connected();
    snprintf(buf, sizeof(buf), "USB HID:  READY (1kHz)");
    draw_text(20, 75, buf, COLOR_GREEN, COLOR_BLACK);

    snprintf(buf, sizeof(buf), "CDC HOST: %s", cdc_ok ? "CONNECTED" : "OFFLINE");
    draw_text(20, 95, buf, cdc_ok ? COLOR_GREEN : COLOR_YELLOW, COLOR_BLACK);

    // Lifetime Counters
    counters_snapshot_t snap;
    counters_get(&snap);

    fill_rect(10, 130, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    draw_text(20, 145, "LIFETIME PRESSES", COLOR_PINK, COLOR_BLACK);
    snprintf(buf, sizeof(buf), "Gen: %lu", (unsigned long)snap.generation);
    draw_text(160, 145, buf, COLOR_GRAY, COLOR_BLACK);

    snprintf(buf, sizeof(buf), "KEY 1 (Z): %llu", (unsigned long long)snap.lifetime_key1);
    draw_text(20, 175, buf, COLOR_WHITE, COLOR_BLACK);

    snprintf(buf, sizeof(buf), "KEY 2 (X): %llu", (unsigned long long)snap.lifetime_key2);
    draw_text(20, 205, buf, COLOR_WHITE, COLOR_BLACK);

    fill_rect(10, 240, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    // Live Switch Indicators
    bool k1 = keypad_is_pressed(KEY_ID_1);
    bool k2 = keypad_is_pressed(KEY_ID_2);

    fill_rect(30, 260, 80, 35, k1 ? COLOR_GREEN : COLOR_DARK_GRAY);
    draw_text(50, 270, "K1", k1 ? COLOR_BLACK : COLOR_WHITE, k1 ? COLOR_GREEN : COLOR_DARK_GRAY);

    fill_rect(130, 260, 80, 35, k2 ? COLOR_GREEN : COLOR_DARK_GRAY);
    draw_text(150, 270, "K2", k2 ? COLOR_BLACK : COLOR_WHITE, k2 ? COLOR_GREEN : COLOR_DARK_GRAY);
}

static void render_gameplay_screen(void)
{
    char buf[48];

    // Banner
    fill_rect(0, 0, BOARD_LCD_H_RES, 35, COLOR_PINK);
    draw_text(24, 10, "** osu! PLAYING **", COLOR_BLACK, COLOR_PINK);

    // Song Title & Artist
    fill_rect(10, 45, BOARD_LCD_H_RES - 20, 40, COLOR_BLACK);
    draw_text(15, 48, s_gameplay_state.title[0] ? s_gameplay_state.title : "Playing Map", COLOR_WHITE, COLOR_BLACK);
    draw_text(15, 68, s_gameplay_state.artist[0] ? s_gameplay_state.artist : "Unknown Artist", COLOR_GRAY, COLOR_BLACK);

    // PP Counter
    fill_rect(10, 95, BOARD_LCD_H_RES - 20, 40, COLOR_DARK_GRAY);
    snprintf(buf, sizeof(buf), "%0.1f pp", s_gameplay_state.current_pp);
    draw_text(80, 108, buf, COLOR_YELLOW, COLOR_DARK_GRAY);

    // Map Progress Bar
    float ratio = s_gameplay_state.progress_ratio;
    if (ratio < 0.0f) ratio = 0.0f;
    if (ratio > 1.0f) ratio = 1.0f;

    draw_text(15, 150, "Progress:", COLOR_WHITE, COLOR_BLACK);
    snprintf(buf, sizeof(buf), "%d%%", (int)(ratio * 100.0f));
    draw_text(180, 150, buf, COLOR_YELLOW, COLOR_BLACK);

    int bar_x = 15, bar_y = 175, bar_w = 210, bar_h = 16;
    fill_rect(bar_x, bar_y, bar_w, bar_h, COLOR_DARK_GRAY);
    int fill_w = (int)(ratio * bar_w);
    if (fill_w > 0) {
        fill_rect(bar_x, bar_y, fill_w, bar_h, COLOR_GREEN);
    }

    // Map Press Counts
    fill_rect(10, 210, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    snprintf(buf, sizeof(buf), "Map K1: %lu", (unsigned long)s_gameplay_state.current_map_presses_k1);
    draw_text(20, 230, buf, COLOR_WHITE, COLOR_BLACK);

    snprintf(buf, sizeof(buf), "Map K2: %lu", (unsigned long)s_gameplay_state.current_map_presses_k2);
    draw_text(20, 255, buf, COLOR_WHITE, COLOR_BLACK);

    // Live Indicators
    bool k1 = keypad_is_pressed(KEY_ID_1);
    bool k2 = keypad_is_pressed(KEY_ID_2);

    fill_rect(30, 280, 80, 25, k1 ? COLOR_PINK : COLOR_DARK_GRAY);
    draw_text(55, 285, "K1", k1 ? COLOR_BLACK : COLOR_WHITE, k1 ? COLOR_PINK : COLOR_DARK_GRAY);

    fill_rect(130, 280, 80, 25, k2 ? COLOR_PINK : COLOR_DARK_GRAY);
    draw_text(155, 285, "K2", k2 ? COLOR_BLACK : COLOR_WHITE, k2 ? COLOR_PINK : COLOR_DARK_GRAY);
}

static void display_task(void *pvParameters)
{
    ESP_LOGI(TAG, "Display render task started on core %d (low-latency priority +2)", xPortGetCoreID());

    // Clear whole screen to black at boot
    fill_rect(0, 0, BOARD_LCD_H_RES, BOARD_LCD_V_RES, COLOR_BLACK);

    s_last_activity_us = esp_timer_get_time();

    while (1) {
        int64_t now = esp_timer_get_time();

        // Sleep management
        if (s_sleep_timeout_sec > 0 && (now - s_last_activity_us) > ((int64_t)s_sleep_timeout_sec * 1000000)) {
            if (!s_is_asleep) {
                s_is_asleep = true;
                board_backlight_set(0);
                esp_lcd_panel_disp_on_off(s_panel, false);
                ESP_LOGI(TAG, "Display entered sleep mode");
            }
            vTaskDelay(pdMS_TO_TICKS(500));
            continue;
        }

        osupad_state_t state = runtime_get_state();
        if (state == OSUPAD_STATE_PLAYING && s_has_gameplay) {
            render_gameplay_screen();
            vTaskDelay(pdMS_TO_TICKS(100)); // 10 Hz refresh during gameplay
        } else {
            render_idle_screen();
            vTaskDelay(pdMS_TO_TICKS(200)); // 5 Hz refresh during idle
        }
    }
}

esp_err_t display_init(void)
{
    esp_err_t err = board_display_init(&s_panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to initialize display panel: %s", esp_err_to_name(err));
        return err;
    }

    board_backlight_set(s_brightness);

    // Create display rendering task pinned to Core 1 at low priority
    BaseType_t res = xTaskCreatePinnedToCore(
        display_task,
        "display_task",
        8192,
        NULL,
        tskIDLE_PRIORITY + 2,
        NULL,
        1
    );
    if (res != pdPASS) {
        ESP_LOGE(TAG, "Failed to create display task");
        return ESP_FAIL;
    }

    return ESP_OK;
}

void display_set_time(uint32_t year, uint32_t month, uint32_t day, uint32_t hour, uint32_t minute, uint32_t second)
{
    s_year = year;
    s_month = month;
    s_day = day;
    s_hour = hour;
    s_minute = minute;
    s_second = second;
    s_time_sync_us = esp_timer_get_time();
    s_time_synced = true;
    display_notify_activity();
}

void display_update_gameplay_state(const osupad_GameplayDisplayState *state)
{
    if (state) {
        s_gameplay_state = *state;
        s_has_gameplay = true;
        display_notify_activity();
    }
}

void display_notify_activity(void)
{
    s_last_activity_us = esp_timer_get_time();
    if (s_is_asleep) {
        s_is_asleep = false;
        esp_lcd_panel_disp_on_off(s_panel, true);
        board_backlight_set(s_brightness);
        ESP_LOGI(TAG, "Display awakened");
    }
}

void display_set_sleep_timeout(uint32_t seconds)
{
    s_sleep_timeout_sec = seconds;
}

void display_set_brightness(uint8_t brightness)
{
    if (brightness > 100) brightness = 100;
    s_brightness = brightness;
    if (!s_is_asleep) {
        board_backlight_set(s_brightness);
    }
}

bool display_is_asleep(void)
{
    return s_is_asleep;
}
