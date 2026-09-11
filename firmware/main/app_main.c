#include <stdio.h>
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "tinyusb.h"
#include "tinyusb_default_config.h"

#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "input/keypad.h"
#include "usb/usb_descriptors.h"
#include "usb/usb_hid.h"
#include "usb/usb_cdc.h"
#include "counters/counters.h"
#include "display/display.h"
#include "runtime/runtime.h"

static const char *TAG = "app_main";

void app_main(void)
{
    ESP_LOGI(TAG, "========================================");
    ESP_LOGI(TAG, "  osu!pad ESP32-S3 Firmware v1.0.0      ");
    ESP_LOGI(TAG, "  Ultra Low-Latency 2-Key osu! Keypad   ");
    ESP_LOGI(TAG, "========================================");

    // 1. Initialize Board Peripherals (Switch GPIOs, Backlight PWM)
    ESP_ERROR_CHECK(board_init());

    // 2. Initialize Persistent NVS Lifetime Counters
    ESP_ERROR_CHECK(counters_init());

    // 3. Initialize Runtime Supervisor and State Machine
    ESP_ERROR_CHECK(runtime_init());

    // 4. Initialize Keypad with Eager Debouncing
    ESP_ERROR_CHECK(keypad_init(NULL));

    // 5. Initialize USB Subsystems
    ESP_ERROR_CHECK(usb_hid_init());
    ESP_ERROR_CHECK(usb_cdc_init());

    ESP_LOGI(TAG, "Configuring TinyUSB Composite Stack (HID 1000Hz + CDC-ACM)...");
    tinyusb_config_t tusb_cfg = TINYUSB_DEFAULT_CONFIG();
    tusb_cfg.descriptor.device = &osupad_usb_device_desc;
    tusb_cfg.descriptor.full_speed_config = osupad_usb_config_desc;
    tusb_cfg.descriptor.string = osupad_usb_string_desc;
    tusb_cfg.descriptor.string_count = 6;
#if (TUD_OPT_HIGH_SPEED)
    tusb_cfg.descriptor.high_speed_config = osupad_usb_config_desc;
#endif

    ESP_ERROR_CHECK(tinyusb_driver_install(&tusb_cfg));
    ESP_LOGI(TAG, "TinyUSB stack installed successfully");

    // 6. Initialize ST7789 Display Subsystem (Core 1)
    ESP_ERROR_CHECK(display_init());

    ESP_LOGI(TAG, "osu!pad initialized and ready! Polling USB CDC in background...");

    // Background loop for CDC housekeeping and periodic health logging
    uint32_t loop_counter = 0;
    while (1) {
        usb_cdc_task_poll();
        vTaskDelay(pdMS_TO_TICKS(10));

        loop_counter++;
        if (loop_counter % 500 == 0) { // Every 5 seconds
            counters_snapshot_t snap;
            counters_get(&snap);
            ESP_LOGD(TAG, "Health check: State=%d, Gen=%lu, K1=%llu, K2=%llu, Asleep=%d",
                     runtime_get_state(),
                     (unsigned long)snap.generation,
                     (unsigned long long)snap.lifetime_key1,
                     (unsigned long long)snap.lifetime_key2,
                     display_is_asleep());
        }
    }
}
