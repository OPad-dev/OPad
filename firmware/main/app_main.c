#include <stdio.h>
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "tinyusb.h"
#include "tinyusb_default_config.h"

#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "config/device_config.h"
#include "input/keypad.h"
#include "input/touch_retry.h"
#include "input/latency_stats.h"
#include "usb/usb_descriptors.h"
#include "usb/usb_hid.h"
#include "usb/usb_cdc.h"
#include "counters/counters.h"
#include "ui/ui.h"
#include "runtime/runtime.h"
#include "soc/rtc_cntl_reg.h"
#include "esp_ota_ops.h"
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
        // TinyUSB task, core 0: no NVS here, the runtime task writes
        counters_request_checkpoint();
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
    ESP_LOGI(TAG, "  OPad ESP32-S3 Firmware v1.0.0         ");
    ESP_LOGI(TAG, "  Ultra Low-Latency Rhythm Gaming Keypad");
    ESP_LOGI(TAG, "========================================");

    // 1. Load device configuration from NVS (or fall back to defaults) (NON-FATAL)
    device_config_init();
    device_config_data_t dev_cfg;
    device_config_get(&dev_cfg);

    // 1b. Which input module is on the carrier's connector (ID divider on GPIO8).
    // Hand-wired pads read as none and keep their configured pins.
    board_module_type_t module = board_detect_module();
    if (module == BOARD_MODULE_MX &&
        dev_cfg.key1_gpio == DEVICE_CONFIG_DEFAULT_KEY1_GPIO &&
        dev_cfg.key2_gpio == DEVICE_CONFIG_DEFAULT_KEY2_GPIO) {
        // The carrier routes the MX keys to GPIO10/GPIO7; the defaults are the
        // hand-wired pins. Pins someone chose themselves are left alone.
        ESP_LOGI(TAG, "MX module on the default hand-wired pins: moving keys to GPIO%d/GPIO%d",
                 BOARD_MX_KEY1_GPIO, BOARD_MX_KEY2_GPIO);
        dev_cfg.key1_gpio = BOARD_MX_KEY1_GPIO;
        dev_cfg.key2_gpio = BOARD_MX_KEY2_GPIO;
        device_config_set(&dev_cfg);
    } else if (module == BOARD_MODULE_HE) {
        ESP_LOGW(TAG, "Hall Effect module: needs firmware v2, key input disabled");
    }

    keypad_config_t k_cfg = {
        .keycode1 = (uint8_t)dev_cfg.key1_usage,
        .keycode2 = (uint8_t)dev_cfg.key2_usage,
        .debounce_us = dev_cfg.debounce_us,
        .key1_gpio = (uint8_t)dev_cfg.key1_gpio,
        .key2_gpio = (uint8_t)dev_cfg.key2_gpio,
    };

    // 2. Initialize Keypad: switch GPIOs, usages and debouncing (FATAL if fails)
    if (module == BOARD_MODULE_HE) {
        // Before the ISR is armed: Hall sensors drive analog levels on the key pins
        keypad_set_input_enabled(false);
    }
    ESP_ERROR_CHECK(keypad_init(&k_cfg));
    keypad_config_t armed;
    keypad_get_config(&armed);
    if (armed.key1_gpio != k_cfg.key1_gpio || armed.key2_gpio != k_cfg.key2_gpio) {
        // keypad_init fell back to the default pins: the config the host is
        // shown must name the pins the keys are really on
        dev_cfg.key1_gpio = armed.key1_gpio;
        dev_cfg.key2_gpio = armed.key2_gpio;
        device_config_set(&dev_cfg);
    }

    // 4. Initialize USB HID Subsystem and install TinyUSB stack (FATAL if fails)
    ESP_ERROR_CHECK(usb_hid_init());

    ESP_LOGI(TAG, "Configuring TinyUSB Composite Stack (HID 1000Hz + CDC-ACM)...");
    usb_descriptors_init();
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

    // The keypad works, so this image is good: cancel a pending OTA rollback
    esp_ota_img_states_t ota_state;
    if (esp_ota_get_state_partition(esp_ota_get_running_partition(), &ota_state) == ESP_OK &&
        ota_state == ESP_OTA_IMG_PENDING_VERIFY) {
        esp_ota_mark_app_valid_cancel_rollback();
        ESP_LOGI(TAG, "OTA image marked valid");
    }

    // Step ii complete: HID is now fully operational! Everything below is NON-FATAL.

#if defined(CONFIG_OSUPAD_BENCH_HID_ONLY) || defined(OSUPAD_BENCH_HID_ONLY)
    ESP_LOGW(TAG, "========================================");
    ESP_LOGW(TAG, "  BENCHMARK STAGE A: HID-ONLY MODE      ");
    ESP_LOGW(TAG, "  CDC, Runtime, and Display Disabled    ");
    ESP_LOGW(TAG, "========================================");

    // No CDC in this build and TinyUSB owns the USB PHY, so these lines only reach UART0
    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10000));
        latency_stats_t stats;
        latency_stats_get(&stats);
        ESP_LOGI("bench", "STAGE A STATS: samples=%lu, p50=%lu us, p99=%lu us, p99.9=%lu us, max=%lu us, deferred=%lu",
                 (unsigned long)stats.samples,
                 (unsigned long)stats.p50_us,
                 (unsigned long)stats.p99_us,
                 (unsigned long)stats.p999_us,
                 (unsigned long)stats.max_us,
                 (unsigned long)stats.deferred_reports);
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
    } else if (module == BOARD_MODULE_HE) {
        ui_show_notice("Hall Effect module needs firmware v2.\nKeys are disabled.");
    }

    // Apply brightness and sleep timeout to display & UI. Read again: the host
    // may have sent a config since dev_cfg was taken, and the keypad already
    // has whatever is current
    device_config_get(&dev_cfg);
    board_backlight_set((uint8_t)dev_cfg.brightness);
    ui_set_brightness((uint8_t)dev_cfg.brightness);
    ui_set_sleep_timeout(dev_cfg.sleep_s);

    // 8. Capacitive Touchscreen Quick Retry (core 1, NON-FATAL)
    err = touch_retry_init();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "touch_retry_init failed: %s (continuing without touch)", esp_err_to_name(err));
    }
#else
    ESP_LOGW(TAG, "BENCHMARK STAGE B: Display UI and Backlight Disabled");
#endif

    ESP_LOGI(TAG, "OPad initialized and ready");
#endif
}
