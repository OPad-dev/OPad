#include "easter_egg.h"
#include "easter_egg_gif.h"
#include "lvgl.h"
#include "esp_log.h"
#include "esp_heap_caps.h"

static const char *TAG = "easter_egg";
static lv_obj_t *s_gif_obj = NULL;
static bool s_is_active = false;

// Base resolution is 180x93.
// 70% of 320px screen width is 224px -> scale = (224 * 256) / 180 = 318.
// 100% (or ~290px wide on 320px screen) -> scale = (290 * 256) / 180 = 412.
#define SCALE_70_PCT  318
#define SCALE_100_PCT 412

void easter_egg_init(void)
{
    // Try to allocate PSRAM pool for LVGL if PSRAM hardware is present
    void *psram_pool = heap_caps_malloc(512 * 1024, MALLOC_CAP_SPIRAM);
    if (psram_pool) {
        lv_mem_add_pool(psram_pool, 512 * 1024);
        ESP_LOGI(TAG, "Allocated 512KB PSRAM pool for LVGL GIF decoding");
    } else {
        ESP_LOGI(TAG, "PSRAM not available or internal; using LVGL internal pool");
    }
}

static void easter_egg_anim_cb(void *var, int32_t val)
{
    lv_obj_t *obj = (lv_obj_t *)var;
    if (!obj || !s_is_active) {
        return;
    }

    // val ranges from 0 to 1000
    // Phase 1 (val 0 -> 200, ~700ms):
    //   Scale: SCALE_70_PCT (318) -> SCALE_100_PCT (412)
    //   Opacity: 0 -> 255
    // Phase 2 (val 200 -> 800, ~2100ms):
    //   Scale: SCALE_100_PCT (412)
    //   Opacity: 255
    // Phase 3 (val 800 -> 1000, ~700ms):
    //   Scale: SCALE_100_PCT (412) -> SCALE_70_PCT (318)
    //   Opacity: 255 -> 0

    int32_t scale;
    int32_t opa;

    if (val < 200) {
        scale = SCALE_70_PCT + ((SCALE_100_PCT - SCALE_70_PCT) * val) / 200;
        opa = (255 * val) / 200;
    } else if (val > 800) {
        int32_t progress = val - 800; // 0 to 200
        scale = SCALE_100_PCT - ((SCALE_100_PCT - SCALE_70_PCT) * progress) / 200;
        opa = 255 - (255 * progress) / 200;
    } else {
        scale = SCALE_100_PCT;
        opa = 255;
    }

    if (scale < SCALE_70_PCT) scale = SCALE_70_PCT;
    if (scale > SCALE_100_PCT) scale = SCALE_100_PCT;
    if (opa < 0) opa = 0;
    if (opa > 255) opa = 255;

    lv_image_set_scale(obj, (uint32_t)scale);
    lv_obj_set_style_opa(obj, (lv_opa_t)opa, 0);
}

static void easter_egg_anim_completed_cb(lv_anim_t *a)
{
    (void)a;
    ESP_LOGI(TAG, "Easter egg animation complete, deleting GIF");
    if (s_gif_obj) {
        lv_obj_delete(s_gif_obj);
        s_gif_obj = NULL;
    }
    s_is_active = false;
}

void easter_egg_trigger(void)
{
    if (s_is_active) {
        ESP_LOGI(TAG, "Easter egg already playing, ignoring trigger");
        return;
    }

    ESP_LOGI(TAG, "Triggering freaky 67 easter egg animation!");
    lv_obj_t *top_layer = lv_layer_top();
    s_gif_obj = lv_gif_create(top_layer);
    if (!s_gif_obj) {
        ESP_LOGE(TAG, "Failed to create GIF object");
        return;
    }

    lv_gif_set_color_format(s_gif_obj, LV_COLOR_FORMAT_ARGB8888);
    lv_gif_set_src(s_gif_obj, &s_easter_egg_gif_dsc);

    if (!lv_gif_is_loaded(s_gif_obj)) {
        ESP_LOGE(TAG, "GIF resource failed to load");
        lv_obj_delete(s_gif_obj);
        s_gif_obj = NULL;
        return;
    }

    lv_obj_align(s_gif_obj, LV_ALIGN_CENTER, 0, 0);
    // 180x93 center pivot
    lv_image_set_pivot(s_gif_obj, 90, 46);

    // Start at 70% scale and completely transparent
    lv_image_set_scale(s_gif_obj, SCALE_70_PCT);
    lv_obj_set_style_opa(s_gif_obj, 0, 0);

    lv_anim_t a;
    lv_anim_init(&a);
    lv_anim_set_var(&a, s_gif_obj);
    lv_anim_set_values(&a, 0, 1000);
    lv_anim_set_duration(&a, 3500); // 3.5 seconds total
    lv_anim_set_exec_cb(&a, easter_egg_anim_cb);
    lv_anim_set_completed_cb(&a, easter_egg_anim_completed_cb);
    lv_anim_start(&a);

    s_is_active = true;
}
