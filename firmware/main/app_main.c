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

static const char *TAG = "app_main";

void app_main(void)
{
    // Clear any previous software bootloader download flag
    REG_WRITE(RTC_CNTL_OPTION1_REG, 0);

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

    ESP_ERROR_CHECK(tinyusb_driver_install(&tusb_cfg));
    ESP_LOGI(TAG, "TinyUSB stack installed successfully (HID operational)");

    // Step ii complete: HID is now fully operational! Everything below is NON-FATAL.

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

    // 7. Backlight PWM and Display UI (LVGL, core 1, NON-FATAL)
    err = board_backlight_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "board_backlight_init failed: %s (continuing)", esp_err_to_name(err));
    }
    err = ui_init();
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "ui_init failed: %s (continuing in headless mode)", esp_err_to_name(err));
    }

    // Apply brightness and sleep timeout to display & UI
    device_config_apply(&dev_cfg);

    ESP_LOGI(TAG, "osu!pad initialized and ready");
}
