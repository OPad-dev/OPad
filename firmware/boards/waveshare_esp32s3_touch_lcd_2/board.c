#include "board.h"
#include "esp_log.h"
#include "driver/gpio.h"
#include "driver/ledc.h"
#include "esp_rom_sys.h"
#include "esp_adc/adc_oneshot.h"
#include "esp_adc/adc_cali.h"
#include "esp_adc/adc_cali_scheme.h"

static const char *TAG = "board";
static uint8_t s_backlight_percent = 100;
static bool s_gpio_isr_service_installed = false;

// Read from the key ISR; only changed by board_keys_set_gpio with that pin's ISR removed
static volatile gpio_num_t s_key1_gpio = GPIO_NUM_NC;
static volatile gpio_num_t s_key2_gpio = GPIO_NUM_NC;
static gpio_isr_t s_key_isr = NULL;

esp_err_t board_keys_set_gpio(int key1_gpio, int key2_gpio)
{
    if (key1_gpio == key2_gpio || !GPIO_IS_VALID_GPIO(key1_gpio) || !GPIO_IS_VALID_GPIO(key2_gpio)) {
        return ESP_ERR_INVALID_ARG;
    }
    if (key1_gpio == s_key1_gpio && key2_gpio == s_key2_gpio) {
        // Same pins: re-arm them anyway. Something else (pin detection) may have
        // reconfigured the pad with interrupts off, and skipping here left the
        // keys dead until the next reboot.
        gpio_num_t pins[2] = {s_key1_gpio, s_key2_gpio};
        for (int i = 0; i < 2; i++) {
            gpio_set_direction(pins[i], GPIO_MODE_INPUT);
            gpio_set_pull_mode(pins[i], GPIO_PULLUP_ONLY);
            gpio_set_intr_type(pins[i], GPIO_INTR_ANYEDGE);
            gpio_intr_enable(pins[i]);
        }
        return ESP_OK;
    }

    // Detach the old pins first so no edge is read from a pin being reconfigured
    gpio_num_t old[2] = {s_key1_gpio, s_key2_gpio};
    s_key1_gpio = GPIO_NUM_NC;
    s_key2_gpio = GPIO_NUM_NC;
    for (int i = 0; i < 2; i++) {
        if (old[i] != GPIO_NUM_NC) {
            if (s_key_isr) {
                gpio_isr_handler_remove(old[i]);
            }
            gpio_reset_pin(old[i]);
        }
    }

    // Active LOW, internal pull-up
    gpio_config_t key_cfg = {
        .pin_bit_mask = (1ULL << key1_gpio) | (1ULL << key2_gpio),
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
    // Let the pull-ups charge the line before the first read
    esp_rom_delay_us(200);

    s_key1_gpio = (gpio_num_t)key1_gpio;
    s_key2_gpio = (gpio_num_t)key2_gpio;

    if (s_key_isr) {
        esp_err_t err1 = gpio_isr_handler_add(s_key1_gpio, s_key_isr, (void *)(intptr_t)1);
        esp_err_t err2 = gpio_isr_handler_add(s_key2_gpio, s_key_isr, (void *)(intptr_t)2);
        if (err1 != ESP_OK || err2 != ESP_OK) {
            ESP_LOGE(TAG, "Failed to add ISR handlers (key1=%s, key2=%s)",
                     esp_err_to_name(err1), esp_err_to_name(err2));
            return (err1 != ESP_OK) ? err1 : err2;
        }
    }

    ESP_LOGI(TAG, "Key GPIOs configured (Key1: GPIO%d, Key2: GPIO%d)", key1_gpio, key2_gpio);
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

esp_err_t board_keys_register_isr(gpio_isr_t isr_handler)
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

    if (s_key1_gpio == GPIO_NUM_NC || s_key2_gpio == GPIO_NUM_NC) {
        return ESP_ERR_INVALID_STATE;
    }

    esp_err_t err1 = gpio_isr_handler_add(s_key1_gpio, isr_handler, (void *)(intptr_t)1);
    esp_err_t err2 = gpio_isr_handler_add(s_key2_gpio, isr_handler, (void *)(intptr_t)2);

    if (err1 != ESP_OK || err2 != ESP_OK) {
        ESP_LOGE(TAG, "Failed to add ISR handlers (key1=%s, key2=%s)",
                 esp_err_to_name(err1), esp_err_to_name(err2));
        return (err1 != ESP_OK) ? err1 : err2;
    }
    s_key_isr = isr_handler;

    return ESP_OK;
}

bool board_key1_read(void)
{
    gpio_num_t gpio = s_key1_gpio;
    return gpio != GPIO_NUM_NC && gpio_get_level(gpio) == 0;
}

bool board_key2_read(void)
{
    gpio_num_t gpio = s_key2_gpio;
    return gpio != GPIO_NUM_NC && gpio_get_level(gpio) == 0;
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

int board_get_key1_gpio(void)
{
    return s_key1_gpio;
}

int board_get_key2_gpio(void)
{
    return s_key2_gpio;
}

// Average of 16 ADC1_CH7 conversions in mV (calibrated when the eFuse
// calibration is present, otherwise a linear estimate), -1 on error
static int read_module_id_mv(adc_oneshot_unit_handle_t adc, adc_cali_handle_t cali)
{
    int32_t sum = 0;
    for (int i = 0; i < 16; i++) {
        int raw = 0;
        if (adc_oneshot_read(adc, ADC_CHANNEL_7, &raw) != ESP_OK) {
            return -1;
        }
        sum += raw;
        esp_rom_delay_us(100);
    }
    int raw_avg = (int)(sum / 16);
    int mv = 0;
    if (cali && adc_cali_raw_to_voltage(cali, raw_avg, &mv) == ESP_OK) {
        return mv;
    }
    return (raw_avg * 3100) / 4095;
}

/*
 * The module on the JST connector identifies itself with a divider from 3V3
 * to GND on ID (GPIO8): 100k/10k = 0.30 V for MX, 100k/47k = 1.06 V for Hall
 * Effect (hardware/pcb/V1/README.md). With nothing plugged in, ID floats.
 *
 * Two reads: with the internal pull-down on, a floating pin sits at ~0 V while
 * either divider still holds it well above (0.25 V / 0.6 V); that only tells
 * "something is there". The voltage that identifies the module is read with
 * the pull-down off, since its ~45k would load the 32k-source HE divider down
 * to where it looks like MX.
 */
board_module_type_t board_detect_module(void)
{
    adc_oneshot_unit_handle_t adc = NULL;
    adc_oneshot_unit_init_cfg_t unit_cfg = {
        .unit_id = ADC_UNIT_1,
    };
    if (adc_oneshot_new_unit(&unit_cfg, &adc) != ESP_OK) {
        ESP_LOGE(TAG, "Module ID: ADC1 unavailable, assuming no module");
        return BOARD_MODULE_NONE;
    }
    adc_oneshot_chan_cfg_t chan_cfg = {
        .atten = ADC_ATTEN_DB_12,
        .bitwidth = ADC_BITWIDTH_12,
    };
    adc_oneshot_config_channel(adc, ADC_CHANNEL_7, &chan_cfg);

    adc_cali_handle_t cali = NULL;
    adc_cali_curve_fitting_config_t cali_cfg = {
        .unit_id = ADC_UNIT_1,
        .chan = ADC_CHANNEL_7,
        .atten = ADC_ATTEN_DB_12,
        .bitwidth = ADC_BITWIDTH_12,
    };
    if (adc_cali_create_scheme_curve_fitting(&cali_cfg, &cali) != ESP_OK) {
        cali = NULL;
    }

    // oneshot_config_channel leaves the pad's pulls alone
    gpio_pulldown_en(BOARD_MODULE_ID_GPIO);
    gpio_pullup_dis(BOARD_MODULE_ID_GPIO);
    esp_rom_delay_us(2000);
    int loaded_mv = read_module_id_mv(adc, cali);

    gpio_pulldown_dis(BOARD_MODULE_ID_GPIO);
    esp_rom_delay_us(2000);
    int open_mv = read_module_id_mv(adc, cali);

    if (cali) {
        adc_cali_delete_scheme_curve_fitting(cali);
    }
    adc_oneshot_del_unit(adc);
    gpio_reset_pin(BOARD_MODULE_ID_GPIO);

    board_module_type_t result;
    if (loaded_mv < 0 || open_mv < 0 || loaded_mv < 120) {
        result = BOARD_MODULE_NONE;          // floating, or unreadable
    } else if (open_mv < 650) {
        result = BOARD_MODULE_MX;            // 0.30 V
    } else if (open_mv < 1600) {
        result = BOARD_MODULE_HE;            // 1.06 V
    } else {
        result = BOARD_MODULE_NONE;          // no known module sits up here
    }

    ESP_LOGI(TAG, "Module ID: %d mV (%d mV with pull-down) -> %s", open_mv, loaded_mv,
             result == BOARD_MODULE_MX ? "MX" :
             result == BOARD_MODULE_HE ? "Hall Effect" : "none");
    return result;
}
