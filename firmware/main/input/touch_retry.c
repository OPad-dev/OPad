#include "touch_retry.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "driver/i2c_master.h"
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include <stdbool.h>

static const char *TAG = "touch_retry";

#define CST816_I2C_ADDR         0x15
#define CST816_I2C_PORT         I2C_NUM_0
#define CST816_I2C_FREQ_HZ      100000 // 100 kHz standard clock speed
#define CST816_I2C_TIMEOUT_MS   20

#define CST816_REG_TOUCH_NUM    0x02
#define CST816_REG_TOUCH_XH     0x03
#define CST816_REG_CHIP_ID      0xA7
#define CST816_REG_AUTO_SLEEP   0xFE

static volatile bool s_touch_pressed = false;
static TaskHandle_t s_touch_task = NULL;
static bool s_auto_sleep_disabled = false;
static i2c_master_bus_handle_t s_bus = NULL;
static i2c_master_dev_handle_t s_dev = NULL;

static esp_err_t cst816_read_reg(uint8_t reg, uint8_t *buf, size_t len)
{
    // Register pointer write with STOP, then a read from a fresh START (matching
    // the Waveshare driver)
    esp_err_t err = i2c_master_transmit(s_dev, &reg, 1, CST816_I2C_TIMEOUT_MS);
    if (err != ESP_OK) {
        // Fallback to repeated-start in case controller expects Sr
        return i2c_master_transmit_receive(s_dev, &reg, 1, buf, len, CST816_I2C_TIMEOUT_MS);
    }
    return i2c_master_receive(s_dev, buf, len, CST816_I2C_TIMEOUT_MS);
}

static esp_err_t cst816_write_reg(uint8_t reg, uint8_t val)
{
    uint8_t buf[2] = {reg, val};
    return i2c_master_transmit(s_dev, buf, sizeof(buf), CST816_I2C_TIMEOUT_MS);
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
    i2c_master_bus_config_t bus_cfg = {
        .i2c_port = CST816_I2C_PORT,
        .sda_io_num = BOARD_I2C_SDA_GPIO,
        .scl_io_num = BOARD_I2C_SCL_GPIO,
        .clk_source = I2C_CLK_SRC_DEFAULT,
        .glitch_ignore_cnt = 7,
        .flags.enable_internal_pullup = true,
    };
    err = i2c_new_master_bus(&bus_cfg, &s_bus);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_new_master_bus failed: %s", esp_err_to_name(err));
        return err;
    }

    i2c_device_config_t dev_cfg = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7,
        .device_address = CST816_I2C_ADDR,
        .scl_speed_hz = CST816_I2C_FREQ_HZ,
    };
    err = i2c_master_bus_add_device(s_bus, &dev_cfg, &s_dev);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_master_bus_add_device failed: %s", esp_err_to_name(err));
        return err;
    }

#if CONFIG_OSUPAD_DEBUG_I2C_SCAN
    // 3. Scan I2C bus to discover devices (debug only: up to ~1.3 s of boot)
    ESP_LOGI(TAG, "Scanning I2C bus (SDA: GPIO%d, SCL: GPIO%d)...", BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO);
    int devices_found = 0;
    for (uint16_t addr = 1; addr < 127; addr++) {
        if (i2c_master_probe(s_bus, addr, 10) == ESP_OK) {
            ESP_LOGI(TAG, " - Found I2C device at 0x%02X", addr);
            devices_found++;
        }
    }
    ESP_LOGI(TAG, "I2C scan complete (%d devices found)", devices_found);
#endif

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
