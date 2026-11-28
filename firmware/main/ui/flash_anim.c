#include "ui/flash_anim.h"

#include <string.h>
#include "assets/asset_pack.h"
#include "ui/ui.h"
#include "board.h"
#include "libs/tjpgd/tjpgd.h"
#include "display/lv_display_private.h"
#include "esp_attr.h"
#include "esp_heap_caps.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "esp_system.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"

static const char *TAG = "flash_anim";

/*
 * Clips go to the panel without LVGL. TJpgDec decodes a whole frame into
 * s_frame (PSRAM) first, and only then is it sent, PUSH_LINES at a time
 * through a DMA buffer in internal RAM, in about 20 ms: the panel refreshes
 * every ~16 ms, so it shows at most one tear line. (Sending each row of JPEG
 * blocks as it was decoded spread a frame over three or four refreshes, and
 * the tearing showed.) After each piece a NOP command waits for it: the panel
 * IO finishes queued pixels before it sends a command. LVGL's own "pixels
 * sent" callback still sees those transfers; with LVGL not drawing, it has
 * nothing to do. One clip plays at a time, always on core 1.
 */
#define SCREEN_W 320
#define SCREEN_H 240
#define PUSH_LINES 40
#define JPEG_WORK_SIZE 4096  // TJpgDec's pool, as LVGL's own decoder uses
#define LCD_CMD_NOP 0x00
// The update loop plays this many rounds, then keeps its last frame up
#define LOOP_ROUNDS 2
#define TASK_STACK 4096
#define BOOT_PRIO 2          // as LVGL's task, which is idle meanwhile
// Below the protocol task, which receives and writes an update's pieces:
// above it, the loop took most of core 1 and 1 MB took 40 s instead of 17
#define LOOP_PRIO 2
#define RESUME_MAGIC 0x55AA0E7Au
#define BOOT_WAIT_MS 6000    // longest an update waits for the boot clip to end

typedef struct {
    const asset_pack_entry_t *e;  // NULL: not in the pack
    const uint8_t *data;
} clip_t;

typedef struct {
    const uint8_t *p;
    size_t left;
} jpeg_src_t;

// Set by flash_anim_stop(true), kept through esp_restart()
RTC_NOINIT_ATTR static uint32_t s_resume_magic;

static bool s_prepared;
static asset_pack_t s_pack;
static clip_t s_updating;
static clip_t s_boot;
static esp_lcd_panel_handle_t s_panel;
static esp_lcd_panel_io_handle_t s_io;
static uint16_t *s_frame;  // SCREEN_W x SCREEN_H, big-endian RGB565, PSRAM
static uint16_t *s_push;   // SCREEN_W x PUSH_LINES, internal DMA RAM
static uint8_t *s_work;
static bool s_send_failed;

static TaskHandle_t s_task;        // the update loop
static bool s_started;             // flash_anim_start drew frame 0
static volatile bool s_stop;
static SemaphoreHandle_t s_stopped;
static SemaphoreHandle_t s_boot_done;  // given when the boot clip has handed over to LVGL

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

static bool draw_frame(const clip_t *c, uint32_t i)
{
    const uint32_t *ends = (const uint32_t *)c->data;  // 4-byte aligned by the pack builder
    uint32_t start = i ? ends[i - 1] : 4 * c->e->frame_count;
    uint32_t end = ends[i];
    if (end < start || end > c->e->size) return false;

    jpeg_src_t src = {c->data + start, end - start};
    JDEC jd;
    JRESULT rc = jd_prepare(&jd, jpeg_in, s_work, JPEG_WORK_SIZE, &src);
    if (rc == JDR_OK && (jd.width != SCREEN_W || jd.height != SCREEN_H)) {
        rc = JDR_FMT3;
    }
    if (rc == JDR_OK) rc = jd_decomp(&jd, jpeg_out, 0);
    if (rc != JDR_OK) {
        ESP_LOGW(TAG, "%s frame %lu: TJpgDec error %d", c->e->name, (unsigned long)i, rc);
        return false;
    }
    push_frame();
    return true;
}

static clip_t find_clip(const char *name)
{
    for (uint16_t i = 0; i < s_pack.entry_count; i++) {
        const asset_pack_entry_t *e = &s_pack.entries[i];
        if (e->kind == ASSET_KIND_MJPEG && strcmp(e->name, name) == 0 && e->frame_us > 0) {
            return (clip_t){.e = e, .data = asset_pack_data(&s_pack, i)};
        }
    }
    ESP_LOGI(TAG, "No \"%s\" animation in the assets partition", name);
    return (clip_t){0};
}

// Map the pack and get the buffers, once. False without a pack or memory.
static bool prepare(void)
{
    if (s_prepared) return true;
    if (asset_pack_open(ASSET_PART_OPAD, &s_pack) != ESP_OK) return false;
    s_panel = board_display_get_panel_handle();
    s_io = board_display_get_io_handle();
    s_frame = heap_caps_malloc(SCREEN_W * SCREEN_H * 2, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
    s_push = heap_caps_malloc(SCREEN_W * PUSH_LINES * 2, MALLOC_CAP_DMA | MALLOC_CAP_INTERNAL);
    s_work = heap_caps_malloc(JPEG_WORK_SIZE, MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT);
    if (!s_panel || !s_io || !s_frame || !s_push || !s_work) {
        ESP_LOGE(TAG, "No display or no memory for the animations");
        heap_caps_free(s_frame);
        heap_caps_free(s_push);
        heap_caps_free(s_work);
        s_frame = s_push = NULL;
        s_work = NULL;
        return false;
    }
    s_updating = find_clip(FLASH_ANIM_ASSET);
    s_boot = find_clip(BOOT_ANIM_ASSET);
    s_prepared = true;
    return true;
}

// Frames of c from first on (rounds times round the clip, 1 = to its end),
// each when its time comes, until max_us or *stop; returns the last one
// drawn. A frame that is late is skipped, so a slow stretch shows as fewer
// frames, not as slow motion.
static uint32_t play(const clip_t *c, uint32_t first, uint32_t rounds, int64_t max_us, volatile bool *stop)
{
    const uint32_t n = c->e->frame_count;
    int64_t t0 = esp_timer_get_time();
    int64_t shown = -1;
    uint32_t last = first;
    uint32_t drawn = 0;
    s_send_failed = false;
    while (!(stop && *stop) && !s_send_failed) {
        int64_t elapsed = esp_timer_get_time() - t0;
        if (elapsed >= max_us) break;
        int64_t due = elapsed / c->e->frame_us;
        if (first + due >= (int64_t)n * rounds) break;
        if (due == shown) {
            vTaskDelay(pdMS_TO_TICKS(5));
            continue;
        }
        uint32_t i = (uint32_t)((first + due) % n);
        if (draw_frame(c, i)) {
            drawn++;
            last = i;
        }
        shown = due;
        ui_notify_activity();  // no display sleep while it plays
    }
    ESP_LOGI(TAG, "%s: %lu frames in %lld ms", c->e->name, (unsigned long)drawn,
             (long long)((esp_timer_get_time() - t0) / 1000));
    return last;
}

// LVGL draws again, everything at once
static void hand_back_to_lvgl(void)
{
    if (ui_lock()) {
        lv_display_enable_invalidation(NULL, true);
        lv_obj_invalidate(lv_screen_active());
        lv_obj_invalidate(lv_layer_top());
        ui_unlock();
    }
}

// ---- ROM bootloader flash -----------------------------------------------------------

void flash_anim_play(uint32_t max_ms)
{
    if (!ui_is_ok() || !prepare() || !s_updating.e) return;
    if (s_boot_done) xSemaphoreTake(s_boot_done, pdMS_TO_TICKS(BOOT_WAIT_MS));
    // LVGL stops drawing for good: the pad restarts next
    ui_lock();
    lv_refr_now(NULL);  // whatever it had pending goes out before the panel is ours
    board_display_wake();
    board_backlight_hold_on();
    play(&s_updating, 0, 1, (int64_t)max_ms * 1000, NULL);
}

// ---- update over the protocol -------------------------------------------------------

static void loop_task(void *arg)
{
    (void)arg;
    play(&s_updating, 1, LOOP_ROUNDS, INT64_MAX, &s_stop);  // frame 0 is up already
    // Then its last frame stays up until the update is over
    while (!s_stop) {
        ui_notify_activity();
        vTaskDelay(pdMS_TO_TICKS(50));
    }
    xSemaphoreGive(s_stopped);
    vTaskDelete(NULL);
}

bool flash_anim_start(void)
{
    if (s_started || !ui_is_ok() || !prepare() || !s_updating.e) return false;
    if (!s_stopped) s_stopped = xSemaphoreCreateBinary();
    if (!s_stopped) return false;
    // An update asked for right after a restart waits for the boot clip
    if (s_boot_done && xSemaphoreTake(s_boot_done, pdMS_TO_TICKS(BOOT_WAIT_MS)) == pdTRUE) {
        xSemaphoreGive(s_boot_done);
    }

    // LVGL keeps running but draws nothing until flash_anim_stop(false)
    ui_lock();
    lv_refr_now(NULL);
    lv_display_enable_invalidation(NULL, false);
    ui_unlock();
    board_display_wake();
    board_backlight_hold_on();
    s_send_failed = false;
    draw_frame(&s_updating, 0);
    s_started = true;
    return true;
}

void flash_anim_run(void)
{
    if (!s_started || s_task) return;
    s_stop = false;
    if (xTaskCreatePinnedToCore(loop_task, "flash_anim", TASK_STACK, NULL, LOOP_PRIO, &s_task, 1) != pdPASS) {
        s_task = NULL;  // frame 0 stays up until the end
    }
}

void flash_anim_stop(bool for_restart)
{
    if (s_task) {
        s_stop = true;
        xSemaphoreTake(s_stopped, portMAX_DELAY);  // its frame is finished
        s_task = NULL;
    }
    s_started = false;
    if (for_restart) {
        // The frame stays on the panel, and the next boot leaves it there
        // until the boot clip starts
        s_resume_magic = RESUME_MAGIC;
        return;  // the latch holds the backlight through esp_restart()
    }
    board_backlight_release();
    hand_back_to_lvgl();
}

// ---- boot ---------------------------------------------------------------------------

bool flash_anim_resuming(void)
{
    static int s_resuming = -1;
    if (s_resuming < 0) {
        s_resuming = s_resume_magic == RESUME_MAGIC && esp_reset_reason() == ESP_RST_SW;
        s_resume_magic = 0;
    }
    return s_resuming == 1;
}

static void boot_task(void *arg)
{
    (void)arg;
    play(&s_boot, 0, 1, INT64_MAX, NULL);
    hand_back_to_lvgl();
    xSemaphoreGive(s_boot_done);
    vTaskDelete(NULL);
}

void flash_anim_boot(lv_display_t *disp)
{
    if (!prepare() || !s_boot.e) return;
    s_boot_done = xSemaphoreCreateBinary();
    if (!s_boot_done) return;
    // Nothing LVGL has queued so far is drawn, and nothing until the clip ends
    lv_display_enable_invalidation(disp, false);
    disp->inv_p = 0;
    // A panel just reset shows whatever its memory held at power-on: black
    // until the first frame. One left up by an update stays as it is.
    if (!flash_anim_resuming()) {
        memset(s_frame, 0, SCREEN_W * SCREEN_H * 2);
        s_send_failed = false;
        push_frame();
    }
    if (xTaskCreatePinnedToCore(boot_task, "boot_anim", TASK_STACK, NULL, BOOT_PRIO, NULL, 1) != pdPASS) {
        lv_display_enable_invalidation(disp, true);
        lv_obj_invalidate(lv_screen_active());
        xSemaphoreGive(s_boot_done);
    }
}
