#include "touch_retry.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "driver/i2c.h"
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include <stdbool.h>

static const char *TAG = "touch_retry";

#define CST816_I2C_ADDR         0x15
#define CST816_I2C_PORT         I2C_NUM_0
#define CST816_I2C_FREQ_HZ      100000 // 100 kHz standard clock speed

#define CST816_REG_TOUCH_NUM    0x02
#define CST816_REG_TOUCH_XH     0x03
#define CST816_REG_CHIP_ID      0xA7
#define CST816_REG_AUTO_SLEEP   0xFE

static volatile bool s_touch_pressed = false;
static TaskHandle_t s_touch_task = NULL;
static bool s_auto_sleep_disabled = false;

static esp_err_t cst816_read_reg(uint8_t reg, uint8_t *buf, size_t len)
{
    // Step 1: Write register pointer with STOP condition (matching Waveshare driver)
    i2c_cmd_handle_t cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (CST816_I2C_ADDR << 1) | I2C_MASTER_WRITE, true);
    i2c_master_write_byte(cmd, reg, true);
    i2c_master_stop(cmd);
    esp_err_t err = i2c_master_cmd_begin(CST816_I2C_PORT, cmd, pdMS_TO_TICKS(20));
    i2c_cmd_link_delete(cmd);
    if (err != ESP_OK) {
        // Fallback to repeated-start in case controller expects Sr
        return i2c_master_write_read_device(CST816_I2C_PORT, CST816_I2C_ADDR, &reg, 1, buf, len, pdMS_TO_TICKS(20));
    }

    // Step 2: Read data starting with fresh START condition
    cmd = i2c_cmd_link_create();
    i2c_master_start(cmd);
    i2c_master_write_byte(cmd, (CST816_I2C_ADDR << 1) | I2C_MASTER_READ, true);
    if (len > 1) {
        i2c_master_read(cmd, buf, len - 1, I2C_MASTER_ACK);
    }
    i2c_master_read_byte(cmd, buf + len - 1, I2C_MASTER_NACK);
    i2c_master_stop(cmd);
    err = i2c_master_cmd_begin(CST816_I2C_PORT, cmd, pdMS_TO_TICKS(20));
    i2c_cmd_link_delete(cmd);
    return err;
}

static esp_err_t cst816_write_reg(uint8_t reg, uint8_t val)
{
    uint8_t buf[2] = {reg, val};
    return i2c_master_write_to_device(
        CST816_I2C_PORT,
        CST816_I2C_ADDR,
        buf,
        sizeof(buf),
        pdMS_TO_TICKS(20)
    );
}

static void cst816_disable_auto_sleep(void)
{
    esp_err_t err = cst816_write_reg(CST816_REG_AUTO_SLEEP, 0xFF);
    if (err == ESP_OK) {
        s_auto_sleep_disabled = true;
        ESP_LOGI(TAG, "CST816 auto-sleep disabled (dynamic active mode)");
    }
}

static void touch_retry_task(void *arg)
{
    (void)arg;
    ESP_LOGI(TAG, "Touch Retry task running on core %d (100 Hz, INT: GPIO%d)",
             xPortGetCoreID(), BOARD_TOUCH_INT_GPIO);

    bool last_state = false;
    uint8_t release_debounce = 0;

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(10)); // 10ms = 100 Hz

        // Check active-low INT line
        bool int_active = (gpio_get_level(BOARD_TOUCH_INT_GPIO) == 0);

        uint8_t touch_num = 0;
        esp_err_t err = cst816_read_reg(CST816_REG_TOUCH_NUM, &touch_num, 1);

        bool raw_down = false;
        if (int_active || (err == ESP_OK && touch_num > 0)) {
            raw_down = true;
            if (!s_auto_sleep_disabled) {
                cst816_disable_auto_sleep();
            }
        }

        if (raw_down) {
            release_debounce = 0;
            if (!last_state) {
                last_state = true;
                s_touch_pressed = true;
                usb_hid_set_touch_retry(true);
                ui_notify_activity();
                ESP_LOGI(TAG, "Touch down -> Quick Retry pressed ('`')");
            }
        } else {
            if (last_state) {
                // 2 consecutive released reads (~20ms) to ensure clean release without jitter
                if (++release_debounce >= 2) {
                    last_state = false;
                    s_touch_pressed = false;
                    usb_hid_set_touch_retry(false);
                    ESP_LOGI(TAG, "Touch up -> Quick Retry released");
                }
            }
        }
    }
}

esp_err_t touch_retry_init(void)
{
    // 1. Configure INT pin as input with pull-up
    gpio_config_t int_cfg = {
        .pin_bit_mask = (1ULL << BOARD_TOUCH_INT_GPIO),
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    esp_err_t err = gpio_config(&int_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure INT GPIO%d: %s", BOARD_TOUCH_INT_GPIO, esp_err_to_name(err));
    }

    // 2. Configure I2C bus
    i2c_config_t conf = {
        .mode = I2C_MODE_MASTER,
        .sda_io_num = BOARD_I2C_SDA_GPIO,
        .scl_io_num = BOARD_I2C_SCL_GPIO,
        .sda_pullup_en = GPIO_PULLUP_ENABLE,
        .scl_pullup_en = GPIO_PULLUP_ENABLE,
        .master.clk_speed = CST816_I2C_FREQ_HZ,
    };

    err = i2c_param_config(CST816_I2C_PORT, &conf);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_param_config failed: %s", esp_err_to_name(err));
        return err;
    }

    err = i2c_driver_install(CST816_I2C_PORT, conf.mode, 0, 0, 0);
    if (err != ESP_OK && err != ESP_ERR_INVALID_STATE) {
        ESP_LOGE(TAG, "i2c_driver_install failed: %s", esp_err_to_name(err));
        return err;
    }

    // 3. Scan I2C bus to discover devices
    ESP_LOGI(TAG, "Scanning I2C bus (SDA: GPIO%d, SCL: GPIO%d)...", BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO);
    int devices_found = 0;
    for (uint8_t addr = 1; addr < 127; addr++) {
        i2c_cmd_handle_t cmd = i2c_cmd_link_create();
        i2c_master_start(cmd);
        i2c_master_write_byte(cmd, (addr << 1) | I2C_MASTER_WRITE, true);
        i2c_master_stop(cmd);
        esp_err_t ret = i2c_master_cmd_begin(CST816_I2C_PORT, cmd, pdMS_TO_TICKS(10));
        i2c_cmd_link_delete(cmd);
        if (ret == ESP_OK) {
            ESP_LOGI(TAG, " - Found I2C device at 0x%02X", addr);
            devices_found++;
        }
    }
    ESP_LOGI(TAG, "I2C scan complete (%d devices found)", devices_found);

    // 4. Try reading CST816 Chip ID
    uint8_t chip_id = 0;
    esp_err_t id_err = cst816_read_reg(CST816_REG_CHIP_ID, &chip_id, 1);
    if (id_err == ESP_OK) {
        ESP_LOGI(TAG, "CST816 Touch Controller detected! Chip ID: 0x%02X", chip_id);
    } else {
        ESP_LOGI(TAG, "CST816 touch controller standby/idle on boot (INT=GPIO%d, INT level=%d)",
                 BOARD_TOUCH_INT_GPIO, gpio_get_level(BOARD_TOUCH_INT_GPIO));
    }

    // Try disabling auto-sleep upfront
    cst816_disable_auto_sleep();

    // 5. Pinned to Core 1 so it never interferes with Core 0 keypad ISR / TinyUSB
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

    ESP_LOGI(TAG, "Touch Retry initialized (I2C SDA:%d, SCL:%d, INT:%d, Address:0x%02X)",
             BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO, BOARD_TOUCH_INT_GPIO, CST816_I2C_ADDR);
    return ESP_OK;
}

bool touch_retry_is_pressed(void)
{
    return s_touch_pressed;
}
