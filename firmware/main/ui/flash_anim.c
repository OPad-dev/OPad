#include "ui/flash_anim.h"

#include <string.h>
#include "assets/asset_pack.h"
#include "ui/ui.h"
#include "board.h"
#include "libs/tjpgd/tjpgd.h"
#include "esp_attr.h"
#include "esp_heap_caps.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "esp_lvgl_port.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"

static const char *TAG = "flash_anim";

/*
 * Frames go to the panel without LVGL. TJpgDec decodes a whole frame into
 * s_frame (PSRAM) first, and only then is it sent, PUSH_LINES at a time
 * through a DMA buffer in internal RAM, in about 20 ms: the panel refreshes
 * every ~16 ms, so it shows at most one tear line. (Sending each row of JPEG
 * blocks as it was decoded spread a frame over three or four refreshes, and
 * the tearing showed.) After each piece a NOP command waits for it: the panel
 * IO finishes queued pixels before it sends a command. LVGL's own "pixels
 * sent" callback still sees those transfers; with LVGL not drawing, it has
 * nothing to do.
 */
#define SCREEN_W 320
#define SCREEN_H 240
#define PUSH_LINES 40
#define JPEG_WORK_SIZE 4096  // TJpgDec's pool, as LVGL's own decoder uses
#define LCD_CMD_NOP 0x00
#define TASK_STACK 4096
#define TASK_PRIO 2          // as LVGL's task, which is idle meanwhile; below the protocol task
#define RESUME_MAGIC 0x55AA0E7Au

typedef struct {
    const uint8_t *p;
    size_t left;
} jpeg_src_t;

// Where flash_anim_stop(true) left the clip, kept through esp_restart()
RTC_NOINIT_ATTR static uint32_t s_resume_magic;
RTC_NOINIT_ATTR static uint32_t s_resume_frame;

static asset_pack_t s_pack;
static const asset_pack_entry_t *s_entry;
static const uint8_t *s_data;
static esp_lcd_panel_handle_t s_panel;
static esp_lcd_panel_io_handle_t s_io;
static uint16_t *s_frame;  // SCREEN_W x SCREEN_H, big-endian RGB565, PSRAM
static uint16_t *s_push;   // SCREEN_W x PUSH_LINES, internal DMA RAM
static uint8_t *s_work;
static bool s_send_failed;

static TaskHandle_t s_task;
static volatile bool s_stop;
static SemaphoreHandle_t s_stopped;
static volatile uint32_t s_last_frame;

static size_t jpeg_in(JDEC *jd, uint8_t *buf, size_t n)
{
    jpeg_src_t *src = jd->device;
    if (n > src->left) n = src->left;
    if (buf) memcpy(buf, src->p, n);
    src->p += n;
    src->left -= n;
    return n;
}

static void push_frame(void)
{
    for (int y = 0; y < SCREEN_H && !s_send_failed; y += PUSH_LINES) {
        int lines = SCREEN_H - y < PUSH_LINES ? SCREEN_H - y : PUSH_LINES;
        memcpy(s_push, s_frame + y * SCREEN_W, (size_t)SCREEN_W * lines * 2);
        if (esp_lcd_panel_draw_bitmap(s_panel, 0, y, SCREEN_W, y + lines, s_push) != ESP_OK ||
            esp_lcd_panel_io_tx_param(s_io, LCD_CMD_NOP, NULL, 0) != ESP_OK) {
            s_send_failed = true;
        }
    }
}

// B, G, R bytes per pixel (LVGL's RGB888 order) into big-endian RGB565, as the
// panel takes it over SPI
static int jpeg_out(JDEC *jd, void *bitmap, JRECT *r)
{
    (void)jd;
    const uint8_t *px = bitmap;
    for (unsigned y = r->top; y <= r->bottom; y++) {
        for (unsigned x = r->left; x <= r->right; x++, px += 3) {
            if (x < SCREEN_W && y < SCREEN_H) {
                uint16_t v = (uint16_t)(((px[2] & 0xF8) << 8) | ((px[1] & 0xFC) << 3) | (px[0] >> 3));
                s_frame[y * SCREEN_W + x] = (uint16_t)(v >> 8 | v << 8);
            }
        }
    }
    return 1;
}

static bool draw_frame(uint32_t i)
{
    const uint32_t *ends = (const uint32_t *)s_data;  // 4-byte aligned by the pack builder
    uint32_t start = i ? ends[i - 1] : 4 * s_entry->frame_count;
    uint32_t end = ends[i];
    if (end < start || end > s_entry->size) return false;

    jpeg_src_t src = {s_data + start, end - start};
    JDEC jd;
    JRESULT rc = jd_prepare(&jd, jpeg_in, s_work, JPEG_WORK_SIZE, &src);
    if (rc == JDR_OK && (jd.width != SCREEN_W || jd.height != SCREEN_H)) {
        rc = JDR_FMT3;
    }
    if (rc == JDR_OK) rc = jd_decomp(&jd, jpeg_out, 0);
    if (rc != JDR_OK) {
        ESP_LOGW(TAG, "frame %lu: TJpgDec error %d", (unsigned long)i, rc);
        return false;
    }
    push_frame();
    s_last_frame = i;
    return true;
}

// Find the clip and get the buffers, once. False without the asset or memory.
static bool prepare(void)
{
    if (s_entry) return true;
    if (asset_pack_open(ASSET_PART_OPAD, &s_pack) != ESP_OK) return false;
    for (uint16_t i = 0; i < s_pack.entry_count; i++) {
        const asset_pack_entry_t *e = &s_pack.entries[i];
        if (e->kind == ASSET_KIND_MJPEG && strcmp(e->name, FLASH_ANIM_ASSET) == 0 && e->frame_us > 0) {
            s_data = asset_pack_data(&s_pack, i);
            s_entry = e;
            break;
        }
    }
    if (!s_entry) {
        ESP_LOGI(TAG, "No \"%s\" animation in the assets partition", FLASH_ANIM_ASSET);
        return false;
    }
    s_panel = board_display_get_panel_handle();
    s_io = board_display_get_io_handle();
    if (!s_frame) s_frame = heap_caps_malloc(SCREEN_W * SCREEN_H * 2, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    if (!s_push) s_push = heap_caps_malloc(SCREEN_W * PUSH_LINES * 2, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    if (!s_work) s_work = heap_caps_malloc(JPEG_WORK_SIZE, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
    if (!s_panel || !s_io || !s_frame || !s_push || !s_work) {
        ESP_LOGE(TAG, "No display or no memory for the animation");
        s_entry = NULL;
        return false;
    }
    return true;
}

// Frames first..end of the clip (looping when loop), each when its time
// comes, until max_us or *stop. A frame that is late is skipped, so a slow
// stretch shows as fewer frames, not as slow motion.
static uint32_t play(uint32_t first, bool loop, int64_t max_us, volatile bool *stop)
{
    const uint32_t n = s_entry->frame_count;
    int64_t t0 = esp_timer_get_time();
    int64_t shown = -1;
    uint32_t drawn = 0;
    s_send_failed = false;
    while (!(stop && *stop) && !s_send_failed) {
        int64_t elapsed = esp_timer_get_time() - t0;
        if (elapsed >= max_us) break;
        int64_t due = elapsed / s_entry->frame_us;
        if (!loop && first + due >= n) break;
        if (due == shown) {
            vTaskDelay(pdMS_TO_TICKS(5));
            continue;
        }
        if (draw_frame((uint32_t)((first + due) % n))) drawn++;
        shown = due;
        ui_notify_activity();  // no display sleep while it plays
    }
    return drawn;
}

// ---- ROM bootloader flash -----------------------------------------------------------

void flash_anim_play(uint32_t max_ms)
{
    if (!ui_is_ok() || !prepare()) return;
    // LVGL stops drawing for good: the pad restarts next
    ui_lock();
    lv_refr_now(NULL);  // whatever it had pending goes out before the panel is ours
    board_display_wake();
    board_backlight_hold_on();
    int64_t t0 = esp_timer_get_time();
    uint32_t drawn = play(0, false, (int64_t)max_ms * 1000, NULL);
    ESP_LOGI(TAG, "%lu frames in %lld ms; the last one stays up", (unsigned long)drawn,
             (long long)((esp_timer_get_time() - t0) / 1000));
}

// ---- update over the protocol -------------------------------------------------------

static void loop_task(void *arg)
{
    (void)arg;
    uint32_t drawn = play(0, true, INT64_MAX, &s_stop);
    ESP_LOGI(TAG, "loop stopped after %lu frames", (unsigned long)drawn);
    xSemaphoreGive(s_stopped);
    vTaskDelete(NULL);
}

bool flash_anim_start(void)
{
    if (s_task || !ui_is_ok() || !prepare()) return false;
    if (!s_stopped) s_stopped = xSemaphoreCreateBinary();
    if (!s_stopped) return false;

    // LVGL keeps running but draws nothing until flash_anim_stop(false)
    ui_lock();
    lv_refr_now(NULL);
    lv_display_enable_invalidation(NULL, false);
    ui_unlock();
    board_display_wake();
    board_backlight_hold_on();

    s_stop = false;
    if (xTaskCreatePinnedToCore(loop_task, "flash_anim", TASK_STACK, NULL, TASK_PRIO, &s_task, 1) != pdPASS) {
        s_task = NULL;
        flash_anim_stop(false);
        return false;
    }
    return true;
}

void flash_anim_stop(bool for_restart)
{
    if (s_task) {
        s_stop = true;
        xSemaphoreTake(s_stopped, portMAX_DELAY);  // its frame is finished
        s_task = NULL;
    }
    if (for_restart) {
        if (s_entry) {
            s_resume_frame = (s_last_frame + 1) % s_entry->frame_count;
            s_resume_magic = RESUME_MAGIC;
        }
        return;  // the latch holds the backlight through esp_restart()
    }
    board_backlight_release();
    if (ui_lock()) {
        lv_display_enable_invalidation(NULL, true);
        lv_obj_invalidate(lv_screen_active());
        lv_obj_invalidate(lv_layer_top());
        ui_unlock();
    }
}

// ---- next boot ----------------------------------------------------------------------

bool flash_anim_resuming(void)
{
    static int s_resuming = -1;
    if (s_resuming < 0) {
        s_resuming = s_resume_magic == RESUME_MAGIC && esp_reset_reason() == ESP_RST_SW;
        s_resume_magic = 0;
    }
    return s_resuming == 1;
}

void flash_anim_resume(uint32_t max_ms)
{
    if (!flash_anim_resuming() || !prepare()) return;
    if (s_resume_frame >= s_entry->frame_count) return;
    int64_t t0 = esp_timer_get_time();
    uint32_t drawn = play(s_resume_frame, false, (int64_t)max_ms * 1000, NULL);
    ESP_LOGI(TAG, "carried on from frame %lu: %lu frames in %lld ms", (unsigned long)s_resume_frame,
             (unsigned long)drawn, (long long)((esp_timer_get_time() - t0) / 1000));
}
