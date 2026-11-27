#include "ui/flash_anim.h"

#include <string.h>
#include "assets/asset_pack.h"
#include "ui/ui.h"
#include "board.h"
#include "libs/tjpgd/tjpgd.h"
#include "esp_heap_caps.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"

static const char *TAG = "flash_anim";

/*
 * Frames go to the panel without LVGL: TJpgDec hands over one MCU (8 or 16
 * px square) at a time, left to right; a band of BAND_LINES lines collects one
 * row of MCUs and is sent as soon as the row is complete, so no frame buffer
 * is needed. Each band is sent and waited for before the next is decoded into
 * the same buffer.
 */
#define SCREEN_W 320
#define SCREEN_H 240
#define BAND_LINES 16
#define JPEG_WORK_SIZE 4096  // TJpgDec's pool, as LVGL's own decoder uses
#define BAND_TIMEOUT_MS 100

typedef struct {
    const uint8_t *p;
    size_t left;
} jpeg_src_t;

static esp_lcd_panel_handle_t s_panel;
static uint16_t *s_band;
static int s_band_top;
static SemaphoreHandle_t s_sent;
static bool s_send_failed;

static bool on_color_done(esp_lcd_panel_io_handle_t io, esp_lcd_panel_io_event_data_t *edata, void *ctx)
{
    (void)io;
    (void)edata;
    (void)ctx;
    BaseType_t woken = pdFALSE;
    xSemaphoreGiveFromISR(s_sent, &woken);
    return woken == pdTRUE;
}

static size_t jpeg_in(JDEC *jd, uint8_t *buf, size_t n)
{
    jpeg_src_t *src = jd->device;
    if (n > src->left) n = src->left;
    if (buf) memcpy(buf, src->p, n);
    src->p += n;
    src->left -= n;
    return n;
}

static void send_band(int lines)
{
    if (esp_lcd_panel_draw_bitmap(s_panel, 0, s_band_top, SCREEN_W, s_band_top + lines, s_band) != ESP_OK ||
        xSemaphoreTake(s_sent, pdMS_TO_TICKS(BAND_TIMEOUT_MS)) != pdTRUE) {
        s_send_failed = true;
    }
}

// B, G, R bytes per pixel (LVGL's RGB888 order) into big-endian RGB565, as the
// panel takes it over SPI
static int jpeg_out(JDEC *jd, void *bitmap, JRECT *r)
{
    (void)jd;
    if (r->top != s_band_top) {
        s_band_top = r->top;
    }
    const uint8_t *px = bitmap;
    for (unsigned y = r->top; y <= r->bottom; y++) {
        for (unsigned x = r->left; x <= r->right; x++, px += 3) {
            if (x < SCREEN_W && y - s_band_top < BAND_LINES) {
                uint16_t v = (uint16_t)(((px[2] & 0xF8) << 8) | ((px[1] & 0xFC) << 3) | (px[0] >> 3));
                s_band[(y - s_band_top) * SCREEN_W + x] = (uint16_t)(v >> 8 | v << 8);
            }
        }
    }
    if (r->right + 1 >= SCREEN_W) {
        int bottom = r->bottom < SCREEN_H ? r->bottom : SCREEN_H - 1;
        send_band(bottom - s_band_top + 1);
    }
    return s_send_failed ? 0 : 1;
}

static bool draw_frame(const asset_pack_entry_t *e, const uint8_t *data, uint32_t i, uint8_t *work)
{
    const uint32_t *ends = (const uint32_t *)data;  // 4-byte aligned by the pack builder
    uint32_t start = i ? ends[i - 1] : 4 * e->frame_count;
    uint32_t end = ends[i];
    if (end < start || end > e->size) return false;

    jpeg_src_t src = {data + start, end - start};
    JDEC jd;
    JRESULT rc = jd_prepare(&jd, jpeg_in, work, JPEG_WORK_SIZE, &src);
    // Bands are MCU rows: taller MCUs, or another size, would not fit them
    if (rc == JDR_OK && (jd.width != SCREEN_W || jd.height != SCREEN_H || jd.msy * 8 > BAND_LINES)) {
        rc = JDR_FMT3;
    }
    if (rc == JDR_OK) rc = jd_decomp(&jd, jpeg_out, 0);
    if (rc != JDR_OK) {
        ESP_LOGW(TAG, "frame %lu: TJpgDec error %d", (unsigned long)i, rc);
        return false;
    }
    return true;
}

static const asset_pack_entry_t *find_asset(const asset_pack_t *pack, uint16_t *index)
{
    for (uint16_t i = 0; i < pack->entry_count; i++) {
        const asset_pack_entry_t *e = &pack->entries[i];
        if (e->kind == ASSET_KIND_MJPEG && strcmp(e->name, FLASH_ANIM_ASSET) == 0 && e->frame_us > 0) {
            *index = i;
            return e;
        }
    }
    return NULL;
}

void flash_anim_play(uint32_t max_ms)
{
    if (!ui_is_ok()) return;
    asset_pack_t pack;
    uint16_t index;
    const asset_pack_entry_t *e;
    if (asset_pack_open(ASSET_PART_OPAD, &pack) != ESP_OK || !(e = find_asset(&pack, &index))) {
        ESP_LOGI(TAG, "No \"%s\" animation in the assets partition", FLASH_ANIM_ASSET);
        return;
    }
    s_panel = board_display_get_panel_handle();
    esp_lcd_panel_io_handle_t io = board_display_get_io_handle();
    s_band = heap_caps_malloc(SCREEN_W * BAND_LINES * 2, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    uint8_t *work = heap_caps_malloc(JPEG_WORK_SIZE, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
    s_sent = xSemaphoreCreateBinary();
    if (!s_panel || !io || !s_band || !work || !s_sent) {
        ESP_LOGE(TAG, "No memory for the animation");
        return;  // leaks the little it got: the pad restarts next
    }

    // LVGL stops drawing for good, then the panel's "color sent" event is
    // ours: LVGL's would mark one of its flushes done
    ui_lock();
    const esp_lcd_panel_io_callbacks_t cbs = {.on_color_trans_done = on_color_done};
    esp_lcd_panel_io_register_event_callbacks(io, &cbs, NULL);
    // Full brightness from the first frame to the end of the flash: the latch
    // that keeps the light on through the restart cannot dim it
    board_display_wake();
    board_backlight_hold_on();

    const uint8_t *data = asset_pack_data(&pack, index);
    int64_t t0 = esp_timer_get_time();
    int64_t limit_us = (int64_t)max_ms * 1000;
    int64_t clip_us = (int64_t)e->frame_us * e->frame_count;
    if (clip_us < limit_us) limit_us = clip_us;
    int32_t shown = -1;
    uint32_t drawn = 0;
    s_send_failed = false;
    for (;;) {
        int64_t elapsed = esp_timer_get_time() - t0;
        if (elapsed >= limit_us || s_send_failed) break;
        int32_t due = (int32_t)(elapsed / e->frame_us);
        if (due == shown) {
            vTaskDelay(pdMS_TO_TICKS(5));
            continue;
        }
        // Behind the clock: skip to the frame that is due
        if (draw_frame(e, data, (uint32_t)due, work)) drawn++;
        shown = due;
    }
    ESP_LOGI(TAG, "%lu of %ld frames in %lld ms; last frame stays up", (unsigned long)drawn, (long)(shown + 1),
             (long long)((esp_timer_get_time() - t0) / 1000));
}
