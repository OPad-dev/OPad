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
    // Core 0 with the key ISR and keypad task (the only higher priority), so HID
    // submits and USB completions never wait on the display or protocol on core 1
    tusb_cfg.task.xCoreID = 0;
    tusb_cfg.task.priority = configMAX_PRIORITIES - 2;

    ESP_ERROR_CHECK(tinyusb_driver_install(&tusb_cfg));
    ESP_LOGI(TAG, "TinyUSB stack installed successfully");

    // 6. Host protocol (core 1)
    ESP_ERROR_CHECK(usb_cdc_start_task());

    // 7. Display UI (LVGL, core 1)
    ESP_ERROR_CHECK(ui_init());

    ESP_LOGI(TAG, "osu!pad initialized and ready");
}
