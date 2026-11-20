#include "board.h"
#include "esp_log.h"
#include "driver/gpio.h"
#include "driver/ledc.h"
#include "esp_rom_sys.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"
#include "esp_adc/adc_oneshot.h"
#include "esp_adc/adc_cali.h"
#include "esp_adc/adc_cali_scheme.h"
#include "input/module_id.h"

static const char *TAG = "board";
static uint8_t s_backlight_percent = 100;
static bool s_gpio_isr_service_installed = false;

// Read from the key ISR; only changed by board_keys_set_gpio with that pin's ISR removed
static volatile gpio_num_t s_key1_gpio = GPIO_NUM_NC;
static volatile gpio_num_t s_key2_gpio = GPIO_NUM_NC;
// MX V3's key 3: fixed by the module, never moved (board_key3_set_gpio)
static volatile gpio_num_t s_key3_gpio = GPIO_NUM_NC;
static volatile bool s_key3_armed = false;
static gpio_isr_t s_key_isr = NULL;
// Set before keypad_init with an HE module: the key pins carry analog outputs
static bool s_keys_analog = false;
// Set before keypad_init with an HE V3 module: the key pins are its SPI lines,
// driven by the SPI peripheral, so nothing here may touch them
static bool s_keys_detached = false;

// No pulls, no digital input, no interrupt: a sensor output resting near
// mid-rail would otherwise fire an any-edge interrupt without end on core 0
static void key_pin_park_analog(gpio_num_t pin)
{
    gpio_intr_disable(pin);
    gpio_set_intr_type(pin, GPIO_INTR_DISABLE);
    gpio_set_pull_mode(pin, GPIO_FLOATING);
    gpio_set_direction(pin, GPIO_MODE_DISABLE);
}

void board_keys_set_analog(bool analog)
{
    s_keys_analog = analog;
}

void board_keys_set_detached(bool detached)
{
    s_keys_detached = detached;
}

esp_err_t board_keys_set_gpio(int key1_gpio, int key2_gpio)
{
    if (key1_gpio == key2_gpio || !GPIO_IS_VALID_GPIO(key1_gpio) || !GPIO_IS_VALID_GPIO(key2_gpio)) {
        return ESP_ERR_INVALID_ARG;
    }
    if (s_key3_gpio != GPIO_NUM_NC && (key1_gpio == s_key3_gpio || key2_gpio == s_key3_gpio)) {
        return ESP_ERR_INVALID_ARG;  // the module's key 3 is on it
    }
    if (s_keys_detached) {
        // Record only: GPIO10 is the HE V3 module's CS (or MISO, cable turned over)
        s_key1_gpio = (gpio_num_t)key1_gpio;
        s_key2_gpio = (gpio_num_t)key2_gpio;
        return ESP_OK;
    }
    if (s_keys_analog) {
        // Every config the host pushes comes through here, on each connect
        // too: record the pins, but never arm them
        gpio_num_t pins[4] = {s_key1_gpio, s_key2_gpio, key1_gpio, key2_gpio};
        for (int i = 0; i < 4; i++) {
            if (pins[i] != GPIO_NUM_NC) {
                key_pin_park_analog(pins[i]);
            }
        }
        s_key1_gpio = (gpio_num_t)key1_gpio;
        s_key2_gpio = (gpio_num_t)key2_gpio;
        return ESP_OK;
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

// 8-bit LEDC duty for a brightness. 100 % is 256 (always on), not 255: with
// 255 the pin still switches every 200 us, and GPIO1 is an ADC1 pin next to
// the HE key channels, where those edges show up as spikes on the readings.
static uint32_t backlight_duty(uint8_t percent)
{
    return percent >= 100 ? 256 : (percent * 255u) / 100u;
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
        .duty           = backlight_duty(s_backlight_percent),
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
    if (s_keys_analog || s_keys_detached) {
        s_key_isr = isr_handler; // kept, never attached to analog or SPI pins
        return ESP_OK;
    }

    esp_err_t err1 = gpio_isr_handler_add(s_key1_gpio, isr_handler, (void *)(intptr_t)1);
    esp_err_t err2 = gpio_isr_handler_add(s_key2_gpio, isr_handler, (void *)(intptr_t)2);

    if (err1 != ESP_OK || err2 != ESP_OK) {
        ESP_LOGE(TAG, "Failed to add ISR handlers (key1=%s, key2=%s)",
                 esp_err_to_name(err1), esp_err_to_name(err2));
        return (err1 != ESP_OK) ? err1 : err2;
    }
    s_key_isr = isr_handler;
    if (s_key3_armed) {
        esp_err_t err3 = gpio_isr_handler_add(s_key3_gpio, isr_handler, (void *)(intptr_t)3);
        if (err3 != ESP_OK) {
            ESP_LOGE(TAG, "Failed to add the key 3 ISR handler: %s", esp_err_to_name(err3));
            return err3;
        }
    }

    return ESP_OK;
}

esp_err_t board_key3_set_gpio(int gpio)
{
    if (!GPIO_IS_VALID_GPIO(gpio) || s_key3_gpio != GPIO_NUM_NC) {
        return ESP_ERR_INVALID_ARG;
    }
    s_key3_gpio = (gpio_num_t)gpio;
    return ESP_OK;
}

esp_err_t board_key3_arm(bool armed)
{
    gpio_num_t pin = s_key3_gpio;
    if (pin == GPIO_NUM_NC || armed == s_key3_armed) {
        return ESP_OK;
    }
    if (!armed) {
        s_key3_armed = false;
        if (s_key_isr) {
            gpio_isr_handler_remove(pin);
        }
        key_pin_park_analog(pin);
        return ESP_OK;
    }
    // As keys 1 and 2: active LOW, internal pull-up beside the module's 10k (R3)
    gpio_config_t cfg = {
        .pin_bit_mask = 1ULL << pin,
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_ANYEDGE,
    };
    esp_err_t err = gpio_config(&cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure key 3 GPIO%d: %s", pin, esp_err_to_name(err));
        return err;
    }
    esp_rom_delay_us(200);
    if (s_key_isr) {
        err = gpio_isr_handler_add(pin, s_key_isr, (void *)(intptr_t)3);
        if (err != ESP_OK) {
            ESP_LOGE(TAG, "Failed to add the key 3 ISR handler: %s", esp_err_to_name(err));
            return err;
        }
    }
    s_key3_armed = true;
    ESP_LOGI(TAG, "Key 3 GPIO configured (GPIO%d)", pin);
    return ESP_OK;
}

bool IRAM_ATTR board_key3_read(void)
{
    gpio_num_t gpio = s_key3_gpio;
    return s_key3_armed && gpio_get_level(gpio) == 0;
}

int board_get_key3_gpio(void)
{
    return s_key3_gpio == GPIO_NUM_NC ? -1 : s_key3_gpio;
}

// IRAM: called from the key ISR, which also runs while the flash cache is off
bool IRAM_ATTR board_key1_read(void)
{
    gpio_num_t gpio = s_key1_gpio;
    return gpio != GPIO_NUM_NC && gpio_get_level(gpio) == 0;
}

bool IRAM_ATTR board_key2_read(void)
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
    ledc_set_duty(LEDC_LOW_SPEED_MODE, LEDC_CHANNEL_0, backlight_duty(percent));
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

// The ID pin: GPIO8 (ADC1_CH7), or GPIO7 (ADC1_CH6) once board_detect_module
// found a V3 module's ID there (FFC turned over)
static gpio_num_t s_id_gpio = BOARD_MODULE_ID_GPIO;
static bool s_boot_cable_reversed = false;

static adc_channel_t id_channel(gpio_num_t gpio)
{
    return gpio == BOARD_MODULE_ID_ALT_GPIO ? ADC_CHANNEL_6 : ADC_CHANNEL_7;
}

int board_module_id_gpio(void)
{
    return s_id_gpio;
}

bool board_boot_cable_reversed(void)
{
    return s_boot_cable_reversed;
}

// Average of 16 conversions in mV (calibrated when the eFuse calibration is
// present, otherwise a linear estimate), -1 on error
static int read_module_id_mv(adc_oneshot_unit_handle_t adc, adc_cali_handle_t cali, adc_channel_t chan)
{
    int32_t sum = 0;
    for (int i = 0; i < 16; i++) {
        int raw = 0;
        if (adc_oneshot_read(adc, chan, &raw) != ESP_OK) {
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

typedef struct {
    adc_oneshot_unit_handle_t adc;
    adc_cali_handle_t cali;
} module_id_adc_t;

// ADC1 oneshot takes one user at a time: the module monitor and the board
// test both read ID. Held from open to close. The unit stays open between
// reads (the monitor checks every 5 ms) until board_module_id_release().
static SemaphoreHandle_t s_module_id_lock;
static StaticSemaphore_t s_module_id_lock_buf;
static portMUX_TYPE s_module_id_lock_init = portMUX_INITIALIZER_UNLOCKED;

static void module_id_lock(void)
{
    portENTER_CRITICAL(&s_module_id_lock_init);
    if (s_module_id_lock == NULL) {
        s_module_id_lock = xSemaphoreCreateMutexStatic(&s_module_id_lock_buf);
    }
    portEXIT_CRITICAL(&s_module_id_lock_init);
    xSemaphoreTake(s_module_id_lock, portMAX_DELAY);
}

static module_id_adc_t s_module_id_adc;
static bool s_module_id_adc_open;
// ID pins whose ADC channel is configured (bit = GPIO number), under the lock
static uint64_t s_module_id_chan_pins;

static esp_err_t module_id_adc_open(module_id_adc_t *m)
{
    module_id_lock();
    if (s_module_id_adc_open) {
        *m = s_module_id_adc;
        return ESP_OK;
    }
    *m = (module_id_adc_t){0};
    adc_oneshot_unit_init_cfg_t unit_cfg = {
        .unit_id = ADC_UNIT_1,
    };
    esp_err_t err = adc_oneshot_new_unit(&unit_cfg, &m->adc);
    if (err != ESP_OK) {
        xSemaphoreGive(s_module_id_lock);
        return err;
    }
    // Channels are configured per pin when first read (module_id_chan_ready):
    // configuring one turns its pad analog, and GPIO7 is MX key 2

    // The S3's curve fitting is per unit and attenuation, not per channel
    adc_cali_curve_fitting_config_t cali_cfg = {
        .unit_id = ADC_UNIT_1,
        .chan = ADC_CHANNEL_7,
        .atten = ADC_ATTEN_DB_12,
        .bitwidth = ADC_BITWIDTH_12,
    };
    if (adc_cali_create_scheme_curve_fitting(&cali_cfg, &m->cali) != ESP_OK) {
        m->cali = NULL;
    }
    s_module_id_adc = *m;
    s_module_id_adc_open = true;
    s_module_id_chan_pins = 0;
    return ESP_OK;
}

// A pin a key reads digitally now. Detached (HE V3) and analog (HE V1) pins
// are only recorded: an HE V3 on a turned-over cable has its ID on GPIO7.
static bool is_key_pin(gpio_num_t gpio)
{
    if (s_key3_armed && gpio == s_key3_gpio) {
        return true;
    }
    return !s_keys_detached && !s_keys_analog && (gpio == s_key1_gpio || gpio == s_key2_gpio);
}

/*
 * Configure the ADC channel of an ID pin, once per open unit. That makes the
 * pad analog (gpio_config_as_analog: digital input and pulls off), so it is
 * refused on a pin a key is armed on: GPIO7, MX key 2, then read low, as a
 * key held down until the next boot. Called with the lock held.
 */
static esp_err_t module_id_chan_ready(const module_id_adc_t *m, gpio_num_t gpio)
{
    if (s_module_id_chan_pins & (1ULL << gpio)) {
        return ESP_OK;
    }
    if (is_key_pin(gpio)) {
        return ESP_ERR_INVALID_STATE;
    }
    adc_oneshot_chan_cfg_t chan_cfg = {
        .atten = ADC_ATTEN_DB_12,
        .bitwidth = ADC_BITWIDTH_12,
    };
    esp_err_t err = adc_oneshot_config_channel(m->adc, id_channel(gpio), &chan_cfg);
    if (err == ESP_OK) {
        s_module_id_chan_pins |= 1ULL << gpio;
    }
    return err;
}

// Done with ID for now: the unit stays open, the pin keeps its pull-down (a
// floating pin then reads ~0 V, ready for board_module_present_fast)
static void module_id_adc_close(module_id_adc_t *m, gpio_num_t gpio)
{
    (void)m;
    gpio_pullup_dis(gpio);
    gpio_pulldown_en(gpio);
    xSemaphoreGive(s_module_id_lock);
}

void board_module_id_release(void)
{
    module_id_lock();
    if (s_module_id_adc_open) {
        if (s_module_id_adc.cali) {
            adc_cali_delete_scheme_curve_fitting(s_module_id_adc.cali);
        }
        adc_oneshot_del_unit(s_module_id_adc.adc);
        gpio_reset_pin(s_id_gpio);
        s_module_id_adc_open = false;
    }
    xSemaphoreGive(s_module_id_lock);
}

esp_err_t board_module_present_fast(bool *out_present, int *out_mv)
{
    module_id_adc_t m;
    esp_err_t err = module_id_adc_open(&m);
    if (err != ESP_OK) {
        return err;
    }
    gpio_num_t gpio = s_id_gpio;
    err = module_id_chan_ready(&m, gpio);
    if (err != ESP_OK) {
        xSemaphoreGive(s_module_id_lock);
        return err;
    }
    gpio_pullup_dis(gpio);
    gpio_pulldown_en(gpio);  // already on after a read; settles in microseconds
    esp_rom_delay_us(20);
    int raw = 0;
    err = adc_oneshot_read(m.adc, id_channel(gpio), &raw);
    module_id_adc_close(&m, gpio);
    if (err == ESP_OK) {
        int mv = raw * 3100 / 4095;  // linear is plenty here
        *out_present = mv >= MODULE_ID_PRESENT_MIN_MV;
        if (out_mv) {
            *out_mv = mv;
        }
    }
    return err;
}

/*
 * The module on the connector identifies itself with a divider from 3V3 to
 * GND on ID (input/module_id.h has the table). With nothing plugged in, ID
 * floats.
 *
 * Two reads: with the internal pull-down on, a floating pin sits at ~0 V while
 * every divider still holds it well above; that only tells "something is
 * there". The voltage that identifies the module is read with the pull-down
 * off, since its ~45k would load the 32k-source HE divider down to where it
 * looks like MX.
 */
esp_err_t board_read_module_id_on(int gpio_num, int *out_open_mv, int *out_loaded_mv)
{
    gpio_num_t gpio = (gpio_num_t)gpio_num;
    if (gpio != BOARD_MODULE_ID_GPIO && gpio != BOARD_MODULE_ID_ALT_GPIO) {
        return ESP_ERR_INVALID_ARG;
    }
    module_id_adc_t m;
    esp_err_t err = module_id_adc_open(&m);
    if (err != ESP_OK) {
        return err;
    }
    err = module_id_chan_ready(&m, gpio);
    if (err != ESP_OK) {
        xSemaphoreGive(s_module_id_lock);
        return err;
    }
    adc_channel_t chan = id_channel(gpio);
    gpio_pulldown_en(gpio);
    gpio_pullup_dis(gpio);
    vTaskDelay(pdMS_TO_TICKS(2) ? pdMS_TO_TICKS(2) : 1);
    int loaded_mv = read_module_id_mv(m.adc, m.cali, chan);

    gpio_pulldown_dis(gpio);
    vTaskDelay(pdMS_TO_TICKS(2) ? pdMS_TO_TICKS(2) : 1);
    int open_mv = read_module_id_mv(m.adc, m.cali, chan);
    module_id_adc_close(&m, gpio);

    *out_open_mv = open_mv;
    *out_loaded_mv = loaded_mv;
    return (open_mv < 0 || loaded_mv < 0) ? ESP_FAIL : ESP_OK;
}

esp_err_t board_read_module_id(int *out_open_mv, int *out_loaded_mv)
{
    return board_read_module_id_on(s_id_gpio, out_open_mv, out_loaded_mv);
}

/*
 * A V1 cable that swaps pin 1 <-> 8 puts the module's 3V3 on GPIO2 and its IN2
 * line (10k pull-up to that 3V3, on an MX module) on ID. Unpowered, the module
 * then reads as nothing at all. Pulling GPIO2 up powers that pull-up through
 * ~45k, so ID climbs to ~1.5 V against its pull-down, while with the cable the
 * right way round GPIO2 goes nowhere and ID stays at its divider (or ~0 V).
 * Pulls only: nothing is driven, whatever is wired to these pins.
 */
esp_err_t board_probe_reversed_cable(int *out_mv)
{
    module_id_adc_t m;
    esp_err_t err = module_id_adc_open(&m);
    if (err != ESP_OK) {
        return err;
    }
    gpio_num_t gpio = s_id_gpio;
    gpio_set_direction(BOARD_REVERSE_PROBE_GPIO, GPIO_MODE_INPUT);
    gpio_set_pull_mode(BOARD_REVERSE_PROBE_GPIO, GPIO_PULLUP_ONLY);
    gpio_pulldown_en(gpio);
    gpio_pullup_dis(gpio);
    esp_rom_delay_us(5000);
    int mv = read_module_id_mv(m.adc, m.cali, id_channel(gpio));
    module_id_adc_close(&m, gpio);
    gpio_reset_pin(BOARD_REVERSE_PROBE_GPIO);

    *out_mv = mv;
    return mv < 0 ? ESP_FAIL : ESP_OK;
}

board_module_type_t board_module_from_id(int open_mv, int loaded_mv)
{
    return (board_module_type_t)module_id_classify(open_mv, loaded_mv);
}

static const char *module_name(board_module_type_t m)
{
    switch (m) {
    case BOARD_MODULE_MX:    return "MX";
    case BOARD_MODULE_HE:    return "Hall Effect";
    case BOARD_MODULE_HE_V3: return "Hall Effect V3";
    case BOARD_MODULE_MX_V3: return "MX V3";
    default:                 return "none";
    }
}

static board_module_type_t s_boot_module = BOARD_MODULE_NONE;

board_module_type_t board_detect_module(void)
{
    int open_mv = -1;
    int loaded_mv = -1;
    esp_err_t err = board_read_module_id_on(BOARD_MODULE_ID_GPIO, &open_mv, &loaded_mv);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Module ID: read failed (%s), assuming no module", esp_err_to_name(err));
    }
    board_module_type_t result = board_module_from_id(open_mv, loaded_mv);

    ESP_LOGI(TAG, "Module ID: %d mV (%d mV with pull-down) -> %s", open_mv, loaded_mv,
             module_name(result));

    if (err == ESP_OK && result == BOARD_MODULE_NONE && loaded_mv < MODULE_ID_PRESENT_MIN_MV) {
        // Nothing on GPIO8: a V3 module on a turned-over FFC has its ID on GPIO7
        int alt_open = -1;
        int alt_loaded = -1;
        if (board_read_module_id_on(BOARD_MODULE_ID_ALT_GPIO, &alt_open, &alt_loaded) == ESP_OK) {
            board_module_type_t alt = board_module_from_id(alt_open, alt_loaded);
            if (module_id_is_v3((module_id_t)alt)) {
                ESP_LOGW(TAG, "Module ID on GPIO7: %d mV (%d mV with pull-down) -> %s, "
                         "cable turned over", alt_open, alt_loaded, module_name(alt));
                gpio_reset_pin(BOARD_MODULE_ID_GPIO);
                s_id_gpio = BOARD_MODULE_ID_ALT_GPIO;
                s_boot_cable_reversed = true;
                result = alt;
            }
        }
        if (!s_boot_cable_reversed) {
            // GPIO7 is a key pin on V1 and hand-wired pads: hand it back
            gpio_reset_pin(BOARD_MODULE_ID_ALT_GPIO);
        }
    }
    s_boot_module = result;
    return result;
}

board_module_type_t board_boot_module(void)
{
    return s_boot_module;
}
