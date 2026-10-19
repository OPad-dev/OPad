/*
 * UI performance test (CONFIG_OSUPAD_UI_PERF_TEST): an evaluation build to
 * compare LVGL's memory in internal RAM vs PSRAM (CONFIG_OSUPAD_LVGL_PSRAM),
 * the LCD at 40 vs 80 MHz (CONFIG_OSUPAD_LCD_PCLK_80MHZ) and the refresh
 * period (CONFIG_LV_DEF_REFR_PERIOD). Not a product build.
 *
 * Loops /sdcard/test.gif from the TF card, read whole into PSRAM first so
 * playing it never touches the card (which shares the LCD's SPI bus), or the
 * built-in easter-egg GIF without a card. LVGL's own monitors (sysmon,
 * enabled by sdkconfig.uitest) show FPS, CPU and memory in the corners; a
 * label at the top says which configuration this is and where the GIF came
 * from. Keys and touch keep working.
 */
#include "ui_perf_test.h"
#include "sdkconfig.h"

#ifdef CONFIG_OSUPAD_UI_PERF_TEST

#include "ui/ui.h"
#include "ui/easter_egg_gif.h"
#include "board_pins.h"
#include "lvgl.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_vfs_fat.h"
#include "driver/sdspi_host.h"
#include "sdmmc_cmd.h"
#include <stdio.h>

static const char *TAG = "ui_perf";

#define SD_MOUNT "/sdcard"
#define SD_GIF   SD_MOUNT "/test.gif"
#define SD_FREQ_KHZ 20000

static lv_image_dsc_t s_sd_gif;

// The whole file into PSRAM; false with the reason in `why`
static bool load_sd_gif(char *why, size_t why_len)
{
    sdmmc_host_t host = SDSPI_HOST_DEFAULT();
    host.slot = BOARD_LCD_SPI_HOST;  // the bus ui_init already set up
    host.max_freq_khz = SD_FREQ_KHZ;
    sdspi_device_config_t slot = SDSPI_DEVICE_CONFIG_DEFAULT();
    slot.gpio_cs = BOARD_SD_CS_GPIO;
    slot.host_id = host.slot;
    esp_vfs_fat_sdmmc_mount_config_t mount = {
        .format_if_mount_failed = false,
        .max_files = 2,
        .allocation_unit_size = 16 * 1024,
    };
    sdmmc_card_t *card = NULL;
    esp_err_t err = esp_vfs_fat_sdspi_mount(SD_MOUNT, &host, &slot, &mount, &card);
    if (err != ESP_OK) {
        snprintf(why, why_len, "no TF card (%s)", esp_err_to_name(err));
        return false;
    }
    FILE *f = fopen(SD_GIF, "rb");
    if (!f) {
        snprintf(why, why_len, "card has no test.gif");
        return false;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    uint8_t *buf = size > 0 ? heap_caps_malloc((size_t)size, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT) : NULL;
    if (!buf || fread(buf, 1, (size_t)size, f) != (size_t)size) {
        fclose(f);
        free(buf);
        snprintf(why, why_len, "test.gif unreadable (%ld bytes)", size);
        return false;
    }
    fclose(f);
    s_sd_gif = (lv_image_dsc_t){
        .header = {.magic = LV_IMAGE_HEADER_MAGIC, .cf = LV_COLOR_FORMAT_RAW},
        .data_size = (uint32_t)size,
        .data = buf,
    };
    snprintf(why, why_len, "test.gif from TF, %ld KB", size / 1024);
    return true;
}

// The display would sleep after its timeout and hide the test
static void keep_awake(lv_timer_t *t)
{
    (void)t;
    ui_notify_activity();
}

void ui_perf_test_start(void)
{
    char source[64];
    bool from_sd = load_sd_gif(source, sizeof(source));
    ESP_LOGW(TAG, "UI perf test: %s", source);

    if (!ui_lock()) {
        return;  // headless
    }
    lv_obj_t *gif = lv_gif_create(lv_layer_top());
    // A user GIF is opaque: RGB565 halves its canvas and skips blending.
    // The easter egg has transparency, as in the product.
    lv_gif_set_color_format(gif, from_sd ? LV_COLOR_FORMAT_RGB565 : LV_COLOR_FORMAT_ARGB8888);
    lv_gif_set_src(gif, from_sd ? (const void *)&s_sd_gif : (const void *)&s_easter_egg_gif_dsc);
    lv_gif_set_loop_count(gif, 0);
    lv_obj_center(gif);

    lv_obj_t *label = lv_label_create(lv_layer_top());
    lv_obj_set_style_text_color(label, lv_color_hex(0xFFFFFF), 0);
    lv_obj_set_style_bg_color(label, lv_color_hex(0x000000), 0);
    lv_obj_set_style_bg_opa(label, LV_OPA_60, 0);
    lv_label_set_text_fmt(label, "LVGL %s %d KB | LCD %d MHz | %d ms\n%s%s",
#ifdef CONFIG_OSUPAD_LVGL_PSRAM
                          "PSRAM",
#else
                          "internal",
#endif
                          CONFIG_LV_MEM_SIZE_KILOBYTES, BOARD_LCD_PIXEL_CLOCK_HZ / 1000000,
                          CONFIG_LV_DEF_REFR_PERIOD, source,
                          lv_gif_is_loaded(gif) ? "" : " - FAILED TO LOAD (out of LVGL memory?)");
    lv_obj_align(label, LV_ALIGN_TOP_MID, 0, 2);
    lv_timer_create(keep_awake, 1000, NULL);
    ui_unlock();
}

#endif  // CONFIG_OSUPAD_UI_PERF_TEST
