#include "board.h"
#include "esp_log.h"
#include "driver/gpio.h"
#include "driver/ledc.h"

static const char *TAG = "board";
static uint8_t s_backlight_percent = 100;
static bool s_gpio_isr_service_installed = false;

esp_err_t board_init(void)
{
    ESP_LOGI(TAG, "Initializing Waveshare ESP32-S3-Touch-LCD-2 key GPIOs...");

    // Configure Switch GPIOs (Active LOW, internal pull-up)
    gpio_config_t key_cfg = {
        .pin_bit_mask = (1ULL << BOARD_KEY1_GPIO) | (1ULL << BOARD_KEY2_GPIO),
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_ANYEDGE,
    };
    esp_err_t err = gpio_config(&key_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure key GPIOs: %s", esp_err_to_name(err));
        return err;
    }

    ESP_LOGI(TAG, "Board key GPIOs initialized successfully (Key1: GPIO%d, Key2: GPIO%d)",
             BOARD_KEY1_GPIO, BOARD_KEY2_GPIO);
    return ESP_OK;
}

esp_err_t board_backlight_init(void)
{
    ESP_LOGI(TAG, "Initializing Backlight PWM (LEDC)...");

    // Configure Backlight PWM (LEDC)
    ledc_timer_config_t ledc_timer = {
        .speed_mode       = LEDC_LOW_SPEED_MODE,
        .timer_num        = LEDC_TIMER_0,
        .duty_resolution  = LEDC_TIMER_8_BIT,
        .freq_hz          = 5000,
        .clk_cfg          = LEDC_AUTO_CLK,
    };
    esp_err_t err = ledc_timer_config(&ledc_timer);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure LEDC timer: %s", esp_err_to_name(err));
        return err;
    }

    ledc_channel_config_t ledc_channel = {
        .speed_mode     = LEDC_LOW_SPEED_MODE,
        .channel        = LEDC_CHANNEL_0,
        .timer_sel      = LEDC_TIMER_0,
        .intr_type      = LEDC_INTR_DISABLE,
        .gpio_num       = BOARD_LCD_BL_GPIO,
        .duty           = (s_backlight_percent * 255) / 100,
        .hpoint         = 0,
    };
    err = ledc_channel_config(&ledc_channel);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure LEDC channel: %s", esp_err_to_name(err));
        return err;
    }

    ESP_LOGI(TAG, "Backlight initialized successfully (BL: GPIO%d, duty: %u%%)",
             BOARD_LCD_BL_GPIO, s_backlight_percent);
    return ESP_OK;
}

esp_err_t board_keys_register_isr(gpio_isr_t isr_handler, void *arg)
{
    if (!s_gpio_isr_service_installed) {
        esp_err_t err = gpio_install_isr_service(ESP_INTR_FLAG_IRAM);
        if (err == ESP_OK || err == ESP_ERR_INVALID_STATE) {
            s_gpio_isr_service_installed = true;
        } else {
            ESP_LOGE(TAG, "Failed to install GPIO ISR service: %s", esp_err_to_name(err));
            return err;
        }
    }

    esp_err_t err1 = gpio_isr_handler_add(BOARD_KEY1_GPIO, isr_handler, (void *)(intptr_t)1);
    esp_err_t err2 = gpio_isr_handler_add(BOARD_KEY2_GPIO, isr_handler, (void *)(intptr_t)2);

    if (err1 != ESP_OK || err2 != ESP_OK) {
        ESP_LOGE(TAG, "Failed to add ISR handlers (key1=%s, key2=%s)",
                 esp_err_to_name(err1), esp_err_to_name(err2));
        return (err1 != ESP_OK) ? err1 : err2;
    }

    return ESP_OK;
}

bool board_key1_read(void)
{
    return gpio_get_level(BOARD_KEY1_GPIO) == 0;
}

bool board_key2_read(void)
{
    return gpio_get_level(BOARD_KEY2_GPIO) == 0;
}

void board_backlight_set(uint8_t percent)
{
    if (percent > 100) {
        percent = 100;
    }
    s_backlight_percent = percent;
    uint32_t duty = (percent * 255) / 100;
    ledc_set_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_0, duty);
    ledc_update_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_0);
}

uint8_t board_backlight_get(void)
{
    return s_backlight_percent;
}
