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
#include "esp_heap_caps.h"
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

// Full 320x240 RGB565 Framebuffer (150 KB)
static uint16_t *s_fb = NULL;

// Double-buffered DMA row chunks for high-speed SPI transfer without tearing
#define CHUNK_LINES 20
#define CHUNK_BYTES (BOARD_LCD_H_RES * CHUNK_LINES * sizeof(uint16_t))
static uint16_t *s_dma_buf[2] = {NULL, NULL};

static void fb_fill_rect(int x1, int y1, int w, int h, uint16_t color)
{
    if (!s_fb) return;
    if (x1 < 0) { w += x1; x1 = 0; }
    if (y1 < 0) { h += y1; y1 = 0; }
    if (x1 + w > BOARD_LCD_H_RES) w = BOARD_LCD_H_RES - x1;
    if (y1 + h > BOARD_LCD_V_RES) h = BOARD_LCD_V_RES - y1;
    if (w <= 0 || h <= 0) return;

    for (int y = y1; y < y1 + h; y++) {
        uint16_t *row = &s_fb[y * BOARD_LCD_H_RES + x1];
        for (int x = 0; x < w; x++) {
            row[x] = color;
        }
    }
}

static void fb_draw_char(int x, int y, char c, uint16_t fg, uint16_t bg, int scale)
{
    if (!s_fb || c < 32 || c > 126) c = ' ';
    const uint8_t *glyph = font8x8[c - 32];
    if (scale < 1) scale = 1;

    for (int row = 0; row < 8; row++) {
        uint8_t bits = glyph[row];
        for (int dy = 0; dy < (scale * 2); dy++) {
            int py = y + (row * scale * 2) + dy;
            if (py < 0 || py >= BOARD_LCD_V_RES) continue;
            for (int col = 0; col < 8; col++) {
                uint16_t color = (bits & (0x80 >> col)) ? fg : bg;
                for (int dx = 0; dx < scale; dx++) {
                    int px = x + (col * scale) + dx;
                    if (px >= 0 && px < BOARD_LCD_H_RES) {
                        s_fb[py * BOARD_LCD_H_RES + px] = color;
                    }
                }
            }
        }
    }
}

static void fb_draw_text(int x, int y, const char *text, uint16_t fg, uint16_t bg, int scale)
{
    int cur_x = x;
    int char_w = 8 * scale;
    while (*text && cur_x <= BOARD_LCD_H_RES - char_w) {
        fb_draw_char(cur_x, y, *text++, fg, bg, scale);
        cur_x += char_w;
    }
}

static void display_flush_frame(void)
{
    if (!s_panel || !s_fb) return;

    if (s_dma_buf[0] && s_dma_buf[1]) {
        int buf_idx = 0;
        for (int y = 0; y < BOARD_LCD_V_RES; y += CHUNK_LINES) {
            memcpy(s_dma_buf[buf_idx], &s_fb[y * BOARD_LCD_H_RES], CHUNK_BYTES);
            esp_lcd_panel_draw_bitmap(s_panel, 0, y, BOARD_LCD_H_RES, y + CHUNK_LINES, s_dma_buf[buf_idx]);
            buf_idx = 1 - buf_idx;
        }
    } else {
        esp_lcd_panel_draw_bitmap(s_panel, 0, 0, BOARD_LCD_H_RES, BOARD_LCD_V_RES, s_fb);
    }
}

static void render_idle_screen(void)
{
    char buf[48];

    // Clear background to black
    fb_fill_rect(0, 0, BOARD_LCD_H_RES, BOARD_LCD_V_RES, COLOR_BLACK);

    // Header Bar (Full 320 width)
    fb_fill_rect(0, 0, BOARD_LCD_H_RES, 24, COLOR_DARK_GRAY);
    fb_draw_text(60, 4, "=== osu!pad ESP32-S3 ===", COLOR_YELLOW, COLOR_DARK_GRAY, 1);

    // Clock & Connection Status Row
    if (s_time_synced) {
        int64_t elapsed_sec = (esp_timer_get_time() - s_time_sync_us) / 1000000;
        int sec = (s_second + elapsed_sec) % 60;
        int min = (s_minute + (s_second + elapsed_sec) / 60) % 60;
        int hr  = (s_hour + (s_minute + (s_second + elapsed_sec) / 60) / 60) % 24;
        snprintf(buf, sizeof(buf), "TIME: %02d:%02d:%02d", hr, min, sec);
    } else {
        snprintf(buf, sizeof(buf), "TIME: --:--:--");
    }
    fb_draw_text(12, 30, buf, COLOR_WHITE, COLOR_BLACK, 1);

    bool cdc_ok = usb_cdc_is_connected();
    snprintf(buf, sizeof(buf), "CDC: %s", cdc_ok ? "ONLINE" : "OFFLINE");
    fb_draw_text(180, 30, buf, cdc_ok ? COLOR_GREEN : COLOR_YELLOW, COLOR_BLACK, 1);

    fb_draw_text(12, 48, "USB: 1000Hz HID READY", COLOR_GREEN, COLOR_BLACK, 1);

    counters_snapshot_t snap;
    counters_get(&snap);
    snprintf(buf, sizeof(buf), "GEN: #%lu", (unsigned long)snap.generation);
    fb_draw_text(180, 48, buf, COLOR_GRAY, COLOR_BLACK, 1);

    // Divider line
    fb_fill_rect(10, 68, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    // Lifetime Counters Section Header & Cards
    fb_draw_text(12, 74, "LIFETIME COUNTERS", COLOR_PINK, COLOR_BLACK, 1);

    // Key 1 Card: x=12, y=92, w=142, h=52
    fb_fill_rect(12, 92, 142, 52, COLOR_DARK_GRAY);
    fb_draw_text(20, 96, "KEY 1 (Z)", COLOR_PINK, COLOR_DARK_GRAY, 1);
    snprintf(buf, sizeof(buf), "%llu", (unsigned long long)snap.lifetime_key1);
    // Draw count in BIG BOLD scale=2 (16x32 font)
    fb_draw_text(20, 112, buf, COLOR_WHITE, COLOR_DARK_GRAY, 2);

    // Key 2 Card: x=166, y=92, w=142, h=52
    fb_fill_rect(166, 92, 142, 52, COLOR_DARK_GRAY);
    fb_draw_text(174, 96, "KEY 2 (X)", COLOR_PINK, COLOR_DARK_GRAY, 1);
    snprintf(buf, sizeof(buf), "%llu", (unsigned long long)snap.lifetime_key2);
    // Draw count in BIG BOLD scale=2 (16x32 font)
    fb_draw_text(174, 112, buf, COLOR_WHITE, COLOR_DARK_GRAY, 2);

    // Divider line
    fb_fill_rect(10, 150, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    // Live Switch Indicators (Sticky latch: 120ms hold so mechanical taps are easily visible)
    int64_t now = esp_timer_get_time();
    bool k1_pressed = keypad_is_pressed(KEY_ID_1);
    bool k2_pressed = keypad_is_pressed(KEY_ID_2);
    bool k1_visual = k1_pressed || ((now - keypad_get_last_press_us(KEY_ID_1)) < 120000);
    bool k2_visual = k2_pressed || ((now - keypad_get_last_press_us(KEY_ID_2)) < 120000);

    // Key 1 Box
    uint16_t k1_bg = k1_visual ? COLOR_GREEN : COLOR_DARK_GRAY;
    uint16_t k1_fg = k1_visual ? COLOR_BLACK : COLOR_WHITE;
    fb_fill_rect(16, 158, 138, 74, k1_bg);
    fb_draw_text(45, 178, "KEY 1", k1_fg, k1_bg, 2);

    // Key 2 Box
    uint16_t k2_bg = k2_visual ? COLOR_GREEN : COLOR_DARK_GRAY;
    uint16_t k2_fg = k2_visual ? COLOR_BLACK : COLOR_WHITE;
    fb_fill_rect(166, 158, 138, 74, k2_bg);
    fb_draw_text(195, 178, "KEY 2", k2_fg, k2_bg, 2);
}

static void render_gameplay_screen(void)
{
    char buf[48];

    // Clear background
    fb_fill_rect(0, 0, BOARD_LCD_H_RES, BOARD_LCD_V_RES, COLOR_BLACK);

    // Banner & PP Counter
    fb_fill_rect(0, 0, BOARD_LCD_H_RES, 24, COLOR_PINK);
    fb_draw_text(12, 4, "** osu! PLAYING **", COLOR_BLACK, COLOR_PINK, 1);
    snprintf(buf, sizeof(buf), "%0.1f pp", s_gameplay_state.current_pp);
    fb_draw_text(225, 4, buf, COLOR_BLACK, COLOR_PINK, 1);

    // Song Title & Artist
    fb_draw_text(12, 30, s_gameplay_state.title[0] ? s_gameplay_state.title : "Playing Map", COLOR_WHITE, COLOR_BLACK, 1);
    fb_draw_text(12, 48, s_gameplay_state.artist[0] ? s_gameplay_state.artist : "Unknown Artist", COLOR_GRAY, COLOR_BLACK, 1);

    // Map Progress Bar
    float ratio = s_gameplay_state.progress_ratio;
    if (ratio < 0.0f) ratio = 0.0f;
    if (ratio > 1.0f) ratio = 1.0f;

    fb_draw_text(12, 68, "Progress:", COLOR_WHITE, COLOR_BLACK, 1);
    snprintf(buf, sizeof(buf), "%d%%", (int)(ratio * 100.0f));
    fb_draw_text(260, 68, buf, COLOR_YELLOW, COLOR_BLACK, 1);

    int bar_x = 12, bar_y = 86, bar_w = 296, bar_h = 10;
    fb_fill_rect(bar_x, bar_y, bar_w, bar_h, COLOR_DARK_GRAY);
    int fill_w = (int)(ratio * bar_w);
    if (fill_w > 0) {
        fb_fill_rect(bar_x, bar_y, fill_w, bar_h, COLOR_GREEN);
    }

    // Map Press Counts
    snprintf(buf, sizeof(buf), "Map K1: %lu", (unsigned long)s_gameplay_state.current_map_presses_k1);
    fb_draw_text(16, 104, buf, COLOR_WHITE, COLOR_BLACK, 1);

    snprintf(buf, sizeof(buf), "Map K2: %lu", (unsigned long)s_gameplay_state.current_map_presses_k2);
    fb_draw_text(170, 104, buf, COLOR_WHITE, COLOR_BLACK, 1);

    // Divider line
    fb_fill_rect(10, 124, BOARD_LCD_H_RES - 20, 2, COLOR_GRAY);

    // Live Indicators
    int64_t now = esp_timer_get_time();
    bool k1_pressed = keypad_is_pressed(KEY_ID_1);
    bool k2_pressed = keypad_is_pressed(KEY_ID_2);
    bool k1_visual = k1_pressed || ((now - keypad_get_last_press_us(KEY_ID_1)) < 120000);
    bool k2_visual = k2_pressed || ((now - keypad_get_last_press_us(KEY_ID_2)) < 120000);

    uint16_t k1_bg = k1_visual ? COLOR_PINK : COLOR_DARK_GRAY;
    uint16_t k1_fg = k1_visual ? COLOR_BLACK : COLOR_WHITE;
    fb_fill_rect(16, 134, 138, 98, k1_bg);
    fb_draw_text(60, 166, "K1", k1_fg, k1_bg, 2);

    uint16_t k2_bg = k2_visual ? COLOR_PINK : COLOR_DARK_GRAY;
    uint16_t k2_fg = k2_visual ? COLOR_BLACK : COLOR_WHITE;
    fb_fill_rect(166, 134, 138, 98, k2_bg);
    fb_draw_text(210, 166, "K2", k2_fg, k2_bg, 2);
}

static void display_task(void *pvParameters)
{
    ESP_LOGI(TAG, "Display render task started on core %d (smooth ~30 FPS RAM framebuffer)", xPortGetCoreID());

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
        } else {
            render_idle_screen();
        }

        display_flush_frame();
        vTaskDelay(pdMS_TO_TICKS(33)); // ~30 FPS
    }
}

esp_err_t display_init(void)
{
    esp_err_t err = board_display_init(&s_panel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to initialize display panel: %s", esp_err_to_name(err));
        return err;
    }

    // Allocate 320x240 RGB565 Framebuffer (150 KB)
    s_fb = (uint16_t *)heap_caps_malloc(BOARD_LCD_H_RES * BOARD_LCD_V_RES * sizeof(uint16_t), MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    if (!s_fb) {
        s_fb = (uint16_t *)heap_caps_malloc(BOARD_LCD_H_RES * BOARD_LCD_V_RES * sizeof(uint16_t), MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
    }
    if (!s_fb) {
        ESP_LOGE(TAG, "Failed to allocate display framebuffer");
        return ESP_ERR_NO_MEM;
    }

    // Allocate double-buffered DMA row chunks (2x 12.8 KB)
    s_dma_buf[0] = (uint16_t *)heap_caps_malloc(CHUNK_BYTES, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    s_dma_buf[1] = (uint16_t *)heap_caps_malloc(CHUNK_BYTES, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    if (!s_dma_buf[0] || !s_dma_buf[1]) {
        ESP_LOGW(TAG, "DMA chunk allocation failed, falling back to direct flush");
    }

    // Clear screen to black initially
    fb_fill_rect(0, 0, BOARD_LCD_H_RES, BOARD_LCD_V_RES, COLOR_BLACK);
    display_flush_frame();

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
