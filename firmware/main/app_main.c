#include <stdio.h>
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "tinyusb.h"
#include "tinyusb_default_config.h"

#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "config/device_config.h"
#include "input/keypad.h"
#include "usb/usb_descriptors.h"
#include "usb/usb_hid.h"
#include "usb/usb_cdc.h"
#include "counters/counters.h"
#include "ui/ui.h"
#include "runtime/runtime.h"
#include "soc/rtc_cntl_reg.h"
#include "esp_system.h"
#include "diag/diag.h"

static const char *TAG = "app_main";

static void usb_event_handler(tinyusb_event_t *event, void *arg)
{
    (void)arg;
    switch (event->id) {
    case TINYUSB_EVENT_ATTACHED:
        diag_record(DIAG_EVENT_HID_MOUNTED, 1 /* INFO */, 0, 0);
        break;
    case TINYUSB_EVENT_DETACHED:
        diag_record(DIAG_EVENT_HID_UNMOUNTED, 1 /* INFO */, 0, 0);
        break;
#ifdef CONFIG_TINYUSB_SUSPEND_CALLBACK
    case TINYUSB_EVENT_SUSPENDED:
        diag_record(DIAG_EVENT_HID_SUSPENDED, 1 /* INFO */, 0, 0);
        break;
#endif
    default:
        break;
    }
}

void app_main(void)
{
    // Clear any previous software bootloader download flag
    REG_WRITE(RTC_CNTL_OPTION1_REG, 0);

    diag_init();
    esp_reset_reason_t rst_reason = esp_reset_reason();
    diag_record(DIAG_EVENT_BOOT, 1 /* INFO */, (uint32_t)rst_reason, 0);

    ESP_LOGI(TAG, "========================================");
    ESP_LOGI(TAG, "  osu!pad ESP32-S3 Firmware v1.0.0      ");
    ESP_LOGI(TAG, "  Ultra Low-Latency 2-Key osu! Keypad   ");
    ESP_LOGI(TAG, "========================================");

    // 1. Initialize Board Peripherals: Switch GPIOs only (FATAL if fails)
    ESP_ERROR_CHECK(board_init());

    // 2. Load device configuration from NVS (or fall back to defaults) (NON-FATAL)
    device_config_init();
    device_config_data_t dev_cfg;
    device_config_get(&dev_cfg);

    keypad_config_t k_cfg = {
        .keycode1 = (uint8_t)dev_cfg.key1_usage,
        .keycode2 = (uint8_t)dev_cfg.key2_usage,
        .debounce_us = dev_cfg.debounce_us,
    };

    // 3. Initialize Keypad with configured usages and debouncing (FATAL if fails)
    ESP_ERROR_CHECK(keypad_init(&k_cfg));

    // 4. Initialize USB HID Subsystem and install TinyUSB stack (FATAL if fails)
    ESP_ERROR_CHECK(usb_hid_init());
    usb_hid_set_keycodes(k_cfg.keycode1, k_cfg.keycode2);

    ESP_LOGI(TAG, "Configuring TinyUSB Composite Stack (HID 1000Hz + CDC-ACM)...");
    tinyusb_config_t tusb_cfg = TINYUSB_DEFAULT_CONFIG();
    tusb_cfg.descriptor.device = &osupad_usb_device_desc;
    tusb_cfg.descriptor.full_speed_config = osupad_usb_config_desc;
    tusb_cfg.descriptor.string = osupad_usb_string_desc;
    tusb_cfg.descriptor.string_count = 6;
#if (TUD_OPT_HIGH_SPEED)
    tusb_cfg.descriptor.high_speed_config = osupad_usb_config_desc;
#endif
    // Core 0 with the key ISR and keypad task (the only higher priority), so HID
    // submits and USB completions never wait on the display or protocol on core 1
    tusb_cfg.task.xCoreID = 0;
    tusb_cfg.task.priority = configMAX_PRIORITIES - 2;
    tusb_cfg.event_cb = usb_event_handler;

    ESP_ERROR_CHECK(tinyusb_driver_install(&tusb_cfg));
    ESP_LOGI(TAG, "TinyUSB stack installed successfully (HID operational)");

    // Step ii complete: HID is now fully operational! Everything below is NON-FATAL.

#if defined(CONFIG_OSUPAD_BENCH_HID_ONLY) || defined(OSUPAD_BENCH_HID_ONLY)
    ESP_LOGW(TAG, "========================================");
    ESP_LOGW(TAG, "  BENCHMARK STAGE A: HID-ONLY MODE      ");
    ESP_LOGW(TAG, "  CDC, Runtime, and Display Disabled    ");
    ESP_LOGW(TAG, "========================================");

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
        latency_stats_t stats;
        keypad_get_latency_stats(&stats);
        ESP_LOGI("bench", "STAGE A STATS: samples=%lu, p50=%lu us, p99=%lu us, p99.9=%lu us, max=%lu us, deferred=%lu",
                 (unsigned long)stats.sample_count,
                 (unsigned long)stats.p50_us,
                 (unsigned long)stats.p99_us,
                 (unsigned long)stats.p999_us,
                 (unsigned long)stats.max_us,
                 (unsigned long)stats.deferred_count);
    }
#else

    // 4. USB CDC-ACM and protocol task (core 1)
    esp_err_t err = usb_cdc_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "usb_cdc_init failed: %s (continuing)", esp_err_to_name(err));
    } else {
        err = usb_cdc_start_task();
        if (err != ESP_OK) {
            ESP_LOGE(TAG, "usb_cdc_start_task failed: %s (continuing)", esp_err_to_name(err));
        }
    }

    // 5. Persistent NVS Lifetime Counters (NON-FATAL)
    // Keypad starts with RAM counters at 0. counters_init adds NVS values
    // (keypad_add_lifetime_presses) rather than overwriting, so any keypresses
    // that occurred between HID-ready and counters_init are preserved.
    err = counters_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "counters_init failed: %s (continuing with RAM counters)", esp_err_to_name(err));
    }

    // 6. Runtime Supervisor and State Machine (core 1, NON-FATAL)
    err = runtime_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "runtime_init failed: %s (continuing)", esp_err_to_name(err));
    }

#if !defined(CONFIG_OSUPAD_BENCH_NO_DISPLAY) && !defined(OSUPAD_BENCH_NO_DISPLAY)
    // 7. Backlight PWM and Display UI (LVGL, core 1, NON-FATAL)
    err = board_backlight_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "board_backlight_init failed: %s (continuing)", esp_err_to_name(err));
    }
    err = ui_init();
    if (err != ESP_OK) {
        diag_record(DIAG_EVENT_LCD_INIT_FAILED, 3 /* ERROR */, (uint32_t)err, 0);
        ESP_LOGE(TAG, "ui_init failed: %s (continuing in headless mode)", esp_err_to_name(err));
    }

    // Apply brightness and sleep timeout to display & UI
    device_config_apply(&dev_cfg);
#else
    ESP_LOGW(TAG, "BENCHMARK STAGE B: Display UI and Backlight Disabled");
#endif

    ESP_LOGI(TAG, "osu!pad initialized and ready");
#endif
}
