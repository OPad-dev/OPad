#include "touch_retry.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "driver/i2c.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include <stdbool.h>

static const char *TAG = "touch_retry";

#define CST816_I2C_ADDR         0x15
#define CST816_I2C_PORT         I2C_NUM_0
#define CST816_I2C_FREQ_HZ      400000

static volatile bool s_touch_pressed = false;
static TaskHandle_t s_touch_task = NULL;

static void touch_retry_task(void *arg)
{
    (void)arg;
    ESP_LOGI(TAG, "Touch Retry task running on core %d (100 Hz)", xPortGetCoreID());

    bool last_state = false;
    uint8_t release_debounce = 0;

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10)); // 10ms = 100 Hz

        uint8_t reg = 0x01;
        uint8_t buf[6] = {0};
        esp_err_t err = i2c_master_write_read_device(
            CST816_I2C_PORT,
            CST816_I2C_ADDR,
            &reg,
            1,
            buf,
            sizeof(buf),
            pdMS_TO_TICKS(10)
        );

        bool raw_down = false;
        if (err == ESP_OK) {
            // buf[0] is reg 0x01 (finger count), buf[1] is reg 0x02 (event/x_high)
            uint8_t finger_count = buf[0] & 0x0F;
            uint8_t event = (buf[1] >> 6) & 0x03;

            if (finger_count > 0 || event == 0 /* Press Down */ || event == 2 /* Contact */) {
                raw_down = true;
            }
        }

        if (raw_down) {
            release_debounce = 0;
            if (!last_state) {
                last_state = true;
                s_touch_pressed = true;
                usb_hid_set_touch_retry(true);
                ui_notify_activity();
                ESP_LOGD(TAG, "Touch down -> Quick Retry pressed");
            }
        } else {
            if (last_state) {
                // 2 consecutive released reads (~20ms) to ensure clean release without jitter
                if (++release_debounce >= 2) {
                    last_state = false;
                    s_touch_pressed = false;
                    usb_hid_set_touch_retry(false);
                    ESP_LOGD(TAG, "Touch up -> Quick Retry released");
                }
            }
        }
    }
}

esp_err_t touch_retry_init(void)
{
    i2c_config_t conf = {
        .mode = I2C_MODE_MASTER,
        .sda_io_num = BOARD_I2C_SDA_GPIO,
        .scl_io_num = BOARD_I2C_SCL_GPIO,
        .sda_pullup_en = GPIO_PULLUP_ENABLE,
        .scl_pullup_en = GPIO_PULLUP_ENABLE,
        .master.clk_speed = CST816_I2C_FREQ_HZ,
    };

    esp_err_t err = i2c_param_config(CST816_I2C_PORT, &conf);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_param_config failed: %s", esp_err_to_name(err));
        return err;
    }

    err = i2c_driver_install(CST816_I2C_PORT, conf.mode, 0, 0, 0);
    if (err != ESP_OK && err != ESP_ERR_INVALID_STATE) {
        ESP_LOGE(TAG, "i2c_driver_install failed: %s", esp_err_to_name(err));
        return err;
    }

    // Pinned to Core 1 so it never interferes with Core 0 keypad ISR / TinyUSB
    BaseType_t res = xTaskCreatePinnedToCore(
        touch_retry_task,
        "touch_retry",
        3072,
        NULL,
        2, // Low priority
        &s_touch_task,
        1  // Core 1
    );

    if (res != pdPASS) {
        ESP_LOGE(TAG, "Failed to create touch_retry task");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "Touch Retry initialized (I2C SDA:%d, SCL:%d, Address:0x%02X)",
             BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO, CST816_I2C_ADDR);
    return ESP_OK;
}

bool touch_retry_is_pressed(void)
{
    return s_touch_pressed;
}
