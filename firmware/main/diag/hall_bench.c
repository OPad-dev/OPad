/*
 * Hall bench (CONFIG_OSUPAD_HALL_BENCH): raw readings of the HE module's two
 * analog sensors, for docs/specs/v2-rapid-trigger.md phase V2-0. Not a
 * product build: it streams text over the CDC port, which only works with
 * opad-daemon stopped, and it does so during key presses.
 *
 * ADC1 continuous (DMA) mode on IN1/IN2 (GPIO10 = ADC1_CH9, GPIO7 =
 * ADC1_CH6), BENCH_SAMPLE_HZ conversions per second alternating between the
 * two. Every millisecond it averages what each channel got and writes
 *   H,<ms>,<key1>,<key2>
 * (raw 12-bit, averaged), and every second, per channel, the spread of the
 * individual conversions:
 *   S,<key>,<n>,<min>,<max>,<mean>,<stddev x100>,<overflows>
 * scripts/hall_bench.py records and summarises them. Build it with
 *   idf.py -B build-hb -D SDKCONFIG=build-hb/sdkconfig \
 *     -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.hallbench" build
 */
#include "hall_bench.h"
#include "usb/usb_cdc.h"
#include "esp_adc/adc_continuous.h"
#include "driver/gpio.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <math.h>
#include <stdio.h>
#include <string.h>

static const char *TAG = "hall_bench";

#define BENCH_SAMPLE_HZ   64000      // both channels together
#define BENCH_FRAME_CONVS 64         // 1 ms of conversions per DMA frame
#define BENCH_CH_KEY1     ADC_CHANNEL_9
#define BENCH_CH_KEY2     ADC_CHANNEL_6
#define BENCH_CH_ID       ADC_CHANNEL_7  // module ID divider, a known ~1.06 V on HE
#define BENCH_CH_REF      ADC_CHANNEL_5  // GPIO6, a spare probe pad: pulled up, reads 3V3

static adc_continuous_handle_t s_adc;
static volatile uint32_t s_overflows;
static esp_err_t s_start_err = ESP_OK;
static const char *s_start_step = "ok";

static bool IRAM_ATTR on_pool_ovf(adc_continuous_handle_t handle,
                                  const adc_continuous_evt_data_t *edata, void *user_data)
{
    (void)handle;
    (void)edata;
    (void)user_data;
    s_overflows++;
    return false;
}

typedef struct {
    uint32_t n;
    uint32_t min;
    uint32_t max;
    uint64_t sum;
    uint64_t sum_sq;
} spread_t;

static void spread_add(spread_t *s, uint32_t v)
{
    if (s->n == 0 || v < s->min) {
        s->min = v;
    }
    if (s->n == 0 || v > s->max) {
        s->max = v;
    }
    s->n++;
    s->sum += v;
    s->sum_sq += (uint64_t)v * v;
}

static void write_line(const char *line, int len)
{
    if (len > 0) {
        usb_cdc_write((const uint8_t *)line, (size_t)len);
    }
}

// The key driver re-arms the key pins whenever the host pushes a config (the
// daemon does on every connect): put them back to plain analog inputs
// Cycles the internal pulls once a second (floating, pull-down, pull-up,
// announced as a P line): a sensor that drives its output barely notices
// them, a line nothing drives follows them
static void pins_analog(void)
{
    static const gpio_pull_mode_t modes[3] = {GPIO_FLOATING, GPIO_PULLDOWN_ONLY, GPIO_PULLUP_ONLY};
    static uint32_t phase;
    const gpio_num_t pins[2] = {GPIO_NUM_10, GPIO_NUM_7};
    for (int i = 0; i < 2; i++) {
        gpio_intr_disable(pins[i]);
        gpio_set_intr_type(pins[i], GPIO_INTR_DISABLE);
        gpio_set_pull_mode(pins[i], modes[phase % 3]);
    }
    char line[16];
    int len = snprintf(line, sizeof(line), "P,%lu\n", (unsigned long)(phase % 3));
    write_line(line, len);
    phase++;
}

static void hall_bench_task(void *arg)
{
    (void)arg;
    static adc_continuous_data_t parsed[BENCH_FRAME_CONVS * 4];
    spread_t spread[4] = {0};
    int64_t next_stats_us = esp_timer_get_time() + 1000000;
    char line[96];

    uint32_t read_errs = 0;
    esp_err_t last_err = ESP_OK;
    while (1) {
        int64_t t = esp_timer_get_time();
        if (s_start_err != ESP_OK) {
            // Say why there is no stream, instead of staying silent
            int len = snprintf(line, sizeof(line), "E,start,%s,%s\n", s_start_step,
                               esp_err_to_name(s_start_err));
            write_line(line, len);
            vTaskDelay(pdMS_TO_TICKS(1000));
            continue;
        }
        uint32_t n = 0;
        esp_err_t rerr = adc_continuous_read_parse(s_adc, parsed, BENCH_FRAME_CONVS, &n, 100);
        if (rerr != ESP_OK) {
            read_errs++;
            last_err = rerr;
            if (rerr == ESP_ERR_TIMEOUT) {
                // The conversions stopped (something else touched ADC1):
                // restart them, and say so on the next E line
                adc_continuous_stop(s_adc);
                esp_err_t serr = adc_continuous_start(s_adc);
                int len = snprintf(line, sizeof(line), "E,restart,%lu,%s\n", (unsigned long)read_errs,
                                   esp_err_to_name(serr));
                write_line(line, len);
            }
            if (t >= next_stats_us) {
                next_stats_us += 1000000;
                int len = snprintf(line, sizeof(line), "E,read,%lu,%s\n", (unsigned long)read_errs,
                                   esp_err_to_name(last_err));
                write_line(line, len);
            }
            continue;
        }
        uint32_t sum[4] = {0};
        uint32_t cnt[4] = {0};
        for (uint32_t i = 0; i < n; i++) {
            if (!parsed[i].valid) {
                continue;
            }
            int k = parsed[i].channel == BENCH_CH_KEY1 ? 0 : parsed[i].channel == BENCH_CH_KEY2 ? 1
                  : parsed[i].channel == BENCH_CH_ID ? 2
                  : parsed[i].channel == BENCH_CH_REF ? 3 : -1;
            if (k < 0) {
                continue;
            }
            sum[k] += parsed[i].raw_data;
            cnt[k]++;
            spread_add(&spread[k], parsed[i].raw_data);
        }
        int64_t now = esp_timer_get_time();
        if (cnt[0] && cnt[1]) {
            int len = snprintf(line, sizeof(line), "H,%lld,%lu,%lu\n", (long long)(now / 1000),
                               (unsigned long)(sum[0] / cnt[0]), (unsigned long)(sum[1] / cnt[1]));
            write_line(line, len);
        }
        if (now >= next_stats_us) {
            next_stats_us += 1000000;
            pins_analog();
            for (int k = 0; k < 4; k++) {
                spread_t *s = &spread[k];
                if (s->n == 0) {
                    continue;
                }
                double mean = (double)s->sum / s->n;
                double var = (double)s->sum_sq / s->n - mean * mean;
                int len = snprintf(line, sizeof(line), "S,%d,%lu,%lu,%lu,%lu,%lu,%lu\n", k + 1,
                                   (unsigned long)s->n, (unsigned long)s->min, (unsigned long)s->max,
                                   (unsigned long)(mean + 0.5),
                                   (unsigned long)(sqrt(var > 0 ? var : 0) * 100 + 0.5),
                                   (unsigned long)s_overflows);
                write_line(line, len);
                memset(s, 0, sizeof(*s));
            }
        }
    }
}

esp_err_t hall_bench_start(void)
{
    // The key driver armed IN1/IN2 as pulled-up digital inputs with edge
    // interrupts: the pull-up shifts the sensor's output, and a level near
    // mid-rail can fire the interrupt without end
    const gpio_num_t pins[2] = {GPIO_NUM_10, GPIO_NUM_7};
    for (int i = 0; i < 2; i++) {
        gpio_intr_disable(pins[i]);
        gpio_set_intr_type(pins[i], GPIO_INTR_DISABLE);
        gpio_set_pull_mode(pins[i], GPIO_FLOATING);
    }

    adc_continuous_handle_cfg_t hcfg = {
        .max_store_buf_size = BENCH_FRAME_CONVS * SOC_ADC_DIGI_RESULT_BYTES * 8,
        .conv_frame_size = BENCH_FRAME_CONVS * SOC_ADC_DIGI_RESULT_BYTES,
    };
    s_start_step = "new_handle";
    esp_err_t err = adc_continuous_new_handle(&hcfg, &s_adc);
    gpio_set_pull_mode(GPIO_NUM_8, GPIO_FLOATING);
    adc_digi_pattern_config_t pattern[4] = {
        {.atten = ADC_ATTEN_DB_12, .channel = BENCH_CH_KEY1, .unit = ADC_UNIT_1, .bit_width = 12},
        {.atten = ADC_ATTEN_DB_12, .channel = BENCH_CH_KEY2, .unit = ADC_UNIT_1, .bit_width = 12},
        {.atten = ADC_ATTEN_DB_12, .channel = BENCH_CH_ID, .unit = ADC_UNIT_1, .bit_width = 12},
        {.atten = ADC_ATTEN_DB_12, .channel = BENCH_CH_REF, .unit = ADC_UNIT_1, .bit_width = 12},
    };
    adc_continuous_config_t cfg = {
        .pattern_num = 4,
        .adc_pattern = pattern,
        .sample_freq_hz = BENCH_SAMPLE_HZ,
        .conv_mode = ADC_CONV_SINGLE_UNIT_1,
        .format = ADC_DIGI_OUTPUT_FORMAT_TYPE2,
    };
    if (err == ESP_OK) {
        s_start_step = "config";
        err = adc_continuous_config(s_adc, &cfg);
    }
    if (err == ESP_OK) {
        s_start_step = "callbacks";
        adc_continuous_evt_cbs_t cbs = {.on_pool_ovf = on_pool_ovf};
        err = adc_continuous_register_event_callbacks(s_adc, &cbs, NULL);
    }
    if (err == ESP_OK) {
        s_start_step = "start";
        err = adc_continuous_start(s_adc);
    }
    // After the config: setting up the channel clears the pad's pulls
    gpio_set_pull_mode(GPIO_NUM_6, GPIO_PULLUP_ONLY);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "ADC continuous setup failed at %s: %s", s_start_step, esp_err_to_name(err));
        s_start_err = err;  // the task reports it on the stream
    }
    // Core 0 below the keypad task, as the real sampling task would sit
    BaseType_t res = xTaskCreatePinnedToCore(hall_bench_task, "hall_bench", 4096, NULL,
                                             configMAX_PRIORITIES - 3, NULL, 0);
    return res == pdPASS ? ESP_OK : ESP_ERR_NO_MEM;
}
