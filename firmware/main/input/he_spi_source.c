#include "he_spi_source.h"
#include "input/ads7028.h"
#include "input/ads7028_regs.h"
#include "input/he_spi_frame.h"
#include "driver/gptimer.h"
#include "esp_attr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "freertos/task.h"

static const char *TAG = "he_spi";

// 10 MHz: a 16-bit read in 1.6 us. The FFC is clean to 40 MHz (board test).
#define STREAM_SCLK_HZ  10000000
// One stale conversion (from the end of the last frame) and one per key
#define FRAME_READS     4
// AUTO_SEQ_CH_SEL: AIN3, AIN4, AIN5 (keys 3, 2, 1)
#define KEY_AINS_MASK   ((1u << 3) | (1u << 4) | (1u << 5))
#define DATA_CFG_CHANNEL_ID 0x10  // APPEND_STATUS = 01b
#define SEQUENCE_AUTO_START 0x11  // SEQ_START, SEQ_MODE = 01b

static TaskHandle_t s_task;
static gptimer_handle_t s_timer;
static SemaphoreHandle_t s_frame_lock;  // held by the sampler for a frame, by stop to wait it out
static StaticSemaphore_t s_frame_lock_buf;
static volatile bool s_active;
static he_spi_frame_cb_t s_cb;
static portMUX_TYPE s_stats_lock = portMUX_INITIALIZER_UNLOCKED;
static he_spi_stats_t s_stats;

static bool IRAM_ATTR on_alarm(gptimer_handle_t timer, const gptimer_alarm_event_data_t *edata, void *ctx)
{
    (void)timer;
    (void)edata;
    (void)ctx;
    BaseType_t woken = pdFALSE;
    vTaskNotifyGiveFromISR(s_task, &woken);
    return woken == pdTRUE;
}

static void sampler_task(void *arg)
{
    (void)arg;
    while (1) {
        ulTaskNotifyTake(pdTRUE, portMAX_DELAY);
        if (!s_active) {
            continue;
        }
        xSemaphoreTake(s_frame_lock, portMAX_DELAY);
        if (!s_active) {
            xSemaphoreGive(s_frame_lock);
            continue;
        }
        int64_t t0 = esp_timer_get_time();
        uint16_t words[FRAME_READS];
        uint32_t n = 0;
        uint32_t errors = 0;
        for (int i = 0; i < FRAME_READS; i++) {
            uint16_t w = 0;
            if (ads7028_read16(&w) != ESP_OK) {
                errors++;
            } else if (i > 0) {
                words[n++] = w;  // the first was converted a frame ago
            }
        }
        xSemaphoreGive(s_frame_lock);

        int64_t now = esp_timer_get_time();
        he_frame_t f;
        int32_t raw[HE_SPI_KEYS];
        he_spi_frame_decode(words, n, &f, raw);
        s_cb(&f, raw, now);

        uint32_t took = (uint32_t)(now - t0);
        portENTER_CRITICAL(&s_stats_lock);
        s_stats.frames++;
        s_stats.spi_errors += errors;
        s_stats.foreign += f.foreign;
        if (took > s_stats.max_frame_us) {
            s_stats.max_frame_us = took;
        }
        portEXIT_CRITICAL(&s_stats_lock);
    }
}

static esp_err_t write_checked(uint8_t addr, uint8_t value)
{
    uint8_t back = 0;
    esp_err_t err = ads7028_write_reg(addr, value);
    if (err == ESP_OK) {
        err = ads7028_read_reg(addr, &back);
    }
    if (err == ESP_OK && back != value) {
        ESP_LOGE(TAG, "Register 0x%02x reads 0x%02x, not 0x%02x", addr, back, value);
        err = ESP_ERR_INVALID_RESPONSE;
    }
    return err;
}

// Reset the ADC and start its auto-sequence with channel IDs
static esp_err_t configure_adc(void)
{
    esp_err_t err = ads7028_write_reg(ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_RST);
    if (err != ESP_OK) {
        return err;
    }
    vTaskDelay(pdMS_TO_TICKS(ADS7028_RESET_MS + 1));
    uint8_t status = 0;
    if ((err = ads7028_read_reg(ADS7028_REG_SYSTEM_STATUS, &status)) != ESP_OK) {
        return err;
    }
    if (status != ADS7028_STATUS_AFTER_RESET) {
        ESP_LOGE(TAG, "ADC status 0x%02x after reset, not 0x81: no ADC answers", status);
        return ESP_ERR_NOT_FOUND;
    }
    ads7028_write_reg(ADS7028_REG_SYSTEM_STATUS, ADS7028_STATUS_BOR);
    ads7028_write_reg(ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_CAL);
    for (int i = 0; i < 20; i++) {
        uint8_t cfg = 0;
        vTaskDelay(1);
        if (ads7028_read_reg(ADS7028_REG_GENERAL_CFG, &cfg) == ESP_OK && !(cfg & ADS7028_GENERAL_CAL)) {
            break;
        }
    }
    if ((err = write_checked(ADS7028_REG_DATA_CFG, DATA_CFG_CHANNEL_ID)) != ESP_OK ||
        (err = write_checked(ADS7028_REG_AUTO_SEQ_CH_SEL, KEY_AINS_MASK)) != ESP_OK ||
        (err = write_checked(ADS7028_REG_SEQUENCE_CFG, SEQUENCE_AUTO_START)) != ESP_OK) {
        return err;
    }
    return ESP_OK;
}

esp_err_t he_spi_source_start(bool cable_reversed, he_spi_frame_cb_t cb)
{
    if (s_active) {
        return ESP_ERR_INVALID_STATE;
    }
    s_cb = cb;
    if (s_frame_lock == NULL) {
        s_frame_lock = xSemaphoreCreateMutexStatic(&s_frame_lock_buf);
    }
    if (s_task == NULL) {
        // Core 0 at the key path's priority, like he_task and the MX keypad task
        if (xTaskCreatePinnedToCore(sampler_task, "he_spi", 3072, NULL, configMAX_PRIORITIES - 1,
                                    &s_task, 0) != pdPASS) {
            return ESP_ERR_NO_MEM;
        }
    }
    if (s_timer == NULL) {
        gptimer_config_t tcfg = {
            .clk_src = GPTIMER_CLK_SRC_DEFAULT,
            .direction = GPTIMER_COUNT_UP,
            .resolution_hz = 1000000,
        };
        esp_err_t err = gptimer_new_timer(&tcfg, &s_timer);
        if (err != ESP_OK) {
            return err;
        }
        gptimer_alarm_config_t alarm = {
            .alarm_count = HE_SPI_FRAME_US,
            .reload_count = 0,
            .flags.auto_reload_on_alarm = true,
        };
        gptimer_event_callbacks_t cbs = {.on_alarm = on_alarm};
        if ((err = gptimer_set_alarm_action(s_timer, &alarm)) != ESP_OK ||
            (err = gptimer_register_event_callbacks(s_timer, &cbs, NULL)) != ESP_OK ||
            (err = gptimer_enable(s_timer)) != ESP_OK) {
            return err;
        }
    }

    ads7028_pins_t pins;
    ads7028_carrier_pins(cable_reversed, &pins);
    esp_err_t err = ads7028_open(&pins, STREAM_SCLK_HZ);
    if (err != ESP_OK) {
        return err;
    }
    err = configure_adc();
    if (err == ESP_OK) {
        err = ads7028_acquire_bus();
    }
    if (err != ESP_OK) {
        ads7028_close();
        return err;
    }
    s_active = true;
    err = gptimer_start(s_timer);
    if (err != ESP_OK) {
        s_active = false;
        ads7028_release_bus();
        ads7028_close();
        return err;
    }
    ESP_LOGI(TAG, "ADS7028 streaming: %d us frames, %d MHz SCLK, %s cable", HE_SPI_FRAME_US,
             STREAM_SCLK_HZ / 1000000, cable_reversed ? "turned-over" : "straight");
    return ESP_OK;
}

void he_spi_source_stop(void)
{
    if (!s_active) {
        return;
    }
    s_active = false;
    gptimer_stop(s_timer);
    // A frame in flight finishes before the bus goes
    xSemaphoreTake(s_frame_lock, portMAX_DELAY);
    ads7028_release_bus();
    ads7028_close();
    xSemaphoreGive(s_frame_lock);
}

void he_spi_source_get_stats(he_spi_stats_t *out)
{
    portENTER_CRITICAL(&s_stats_lock);
    *out = s_stats;
    portEXIT_CRITICAL(&s_stats_lock);
}
