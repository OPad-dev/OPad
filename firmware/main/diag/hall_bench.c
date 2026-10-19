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
 * scripts/hall_bench.py records and summarises them. The LCD shows the same
 * readings live (hall_bench_show_ui): rest is what each sensor reads in the
 * first BENCH_REST_MS, so the keys must be up while the pad starts. Build it with
 *   idf.py -B build-hb -D SDKCONFIG=build-hb/sdkconfig \
 *     -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.hallbench" build
 */
#include "hall_bench.h"
#include "usb/usb_cdc.h"
#include "ui/ui.h"
#include "lvgl.h"
#include "esp_adc/adc_cali.h"
#include "esp_adc/adc_cali_scheme.h"
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

#define BENCH_REST_MS     500        // averaged at start for each key's rest

static adc_continuous_handle_t s_adc;
static adc_cali_handle_t s_cali;
static volatile uint32_t s_overflows;
// Latest 1 ms averages and the rest readings, raw counts, for the LCD view
static volatile uint32_t s_live[2];
static volatile uint32_t s_rest[2];
static volatile bool s_rest_ready;
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
    uint64_t rest_sum[2] = {0};
    uint32_t rest_n = 0;
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
            s_live[0] = sum[0] / cnt[0];
            s_live[1] = sum[1] / cnt[1];
            if (!s_rest_ready) {
                rest_sum[0] += s_live[0];
                rest_sum[1] += s_live[1];
                if (++rest_n >= BENCH_REST_MS) {
                    s_rest[0] = rest_sum[0] / rest_n;
                    s_rest[1] = rest_sum[1] / rest_n;
                    s_rest_ready = true;
                }
            }
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
    // Volts for the LCD view only; the stream stays in raw counts
    adc_cali_curve_fitting_config_t cali_cfg = {
        .unit_id = ADC_UNIT_1,
        .chan = BENCH_CH_KEY1,
        .atten = ADC_ATTEN_DB_12,
        .bitwidth = ADC_BITWIDTH_12,
    };
    if (adc_cali_create_scheme_curve_fitting(&cali_cfg, &s_cali) != ESP_OK) {
        s_cali = NULL;
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "ADC continuous setup failed at %s: %s", s_start_step, esp_err_to_name(err));
        s_start_err = err;  // the task reports it on the stream
    }
    // Core 0 below the keypad task, as the real sampling task would sit
    BaseType_t res = xTaskCreatePinnedToCore(hall_bench_task, "hall_bench", 4096, NULL,
                                             configMAX_PRIORITIES - 3, NULL, 0);
    return res == pdPASS ? ESP_OK : ESP_ERR_NO_MEM;
}

// ---- LCD view (LVGL task, core 1) -----------------------------------------------------

#define VIEW_PERIOD_MS     33
#define VIEW_MIN_SPAN_MV   200   // the bar's full scale until a deeper press is seen
#define VIEW_ACTIVE_MV     50    // a change this big keeps the screen awake

typedef struct {
    lv_obj_t *bar;
    lv_obj_t *volts;
    lv_obj_t *delta;
    lv_obj_t *rest;
    lv_obj_t *status;
    int max_mv;  // deepest change from rest seen so far
} key_view_t;

static key_view_t s_view[2];

static int raw_to_mv(uint32_t raw)
{
    int mv;
    if (s_cali && adc_cali_raw_to_voltage(s_cali, (int)raw, &mv) == ESP_OK) {
        return mv;
    }
    return (int)(raw * 875 / 1000);  // ~0.875 mV per count at 12 dB, uncalibrated
}

static lv_obj_t *view_label(lv_obj_t *parent, const lv_font_t *font, uint32_t color)
{
    lv_obj_t *l = lv_label_create(parent);
    lv_obj_set_style_text_font(l, font, 0);
    lv_obj_set_style_text_color(l, lv_color_hex(color), 0);
    lv_label_set_text(l, "");
    return l;
}

static void view_timer_cb(lv_timer_t *timer)
{
    (void)timer;
    if (!s_rest_ready) {
        return;
    }
    bool active = false;
    for (int k = 0; k < 2; k++) {
        key_view_t *v = &s_view[k];
        int mv = raw_to_mv(s_live[k]);
        int rest_mv = raw_to_mv(s_rest[k]);
        int d = mv - rest_mv;
        int ad = d < 0 ? -d : d;
        if (ad > v->max_mv) {
            v->max_mv = ad;
        }
        int span = v->max_mv > VIEW_MIN_SPAN_MV ? v->max_mv : VIEW_MIN_SPAN_MV;
        lv_bar_set_value(v->bar, ad * 100 / span, LV_ANIM_OFF);
        lv_label_set_text_fmt(v->volts, "%d.%02d V", mv / 1000, (mv % 1000) / 10);
        lv_label_set_text_fmt(v->delta, "%c%d mV", d < 0 ? '-' : '+', ad);
        lv_label_set_text_fmt(v->rest, "rest %d.%02d V\ndeepest %d mV", rest_mv / 1000,
                              (rest_mv % 1000) / 10, v->max_mv);

        // DRV5055 at 3.3 V: about 1.65 V with no field, the switch magnet at
        // rest adds a little; a sensor that is missing, reversed or unpowered
        // sits at a rail (V1.0 read 2.9-3.1 V)
        const char *text = "sensor OK";
        uint32_t color = 0x4ADE80;
        if (rest_mv > 2700) {
            text = "stuck high";
            color = 0xF87171;
        } else if (rest_mv < 300) {
            text = "no signal";
            color = 0xF87171;
        } else if (rest_mv < 1450 || rest_mv > 2100) {
            text = "rest unusual";
            color = 0xFACC15;
        }
        lv_label_set_text(v->status, text);
        lv_obj_set_style_text_color(v->status, lv_color_hex(color), 0);
        if (ad > VIEW_ACTIVE_MV) {
            active = true;
        }
    }
    if (active) {
        ui_notify_activity();
    }
}

void hall_bench_show_ui(void)
{
    if (!ui_lock()) {
        return;  // headless
    }
    lv_obj_t *root = lv_obj_create(lv_layer_top());
    lv_obj_set_size(root, UI_SCREEN_W, UI_SCREEN_H);
    lv_obj_set_pos(root, 0, 0);
    lv_obj_set_style_bg_color(root, lv_color_hex(0x11111B), 0);
    lv_obj_set_style_bg_opa(root, LV_OPA_COVER, 0);
    lv_obj_set_style_border_width(root, 0, 0);
    lv_obj_set_style_radius(root, 0, 0);
    lv_obj_set_style_pad_all(root, 0, 0);
    lv_obj_remove_flag(root, LV_OBJ_FLAG_SCROLLABLE | LV_OBJ_FLAG_CLICKABLE);

    lv_obj_t *title = view_label(root, &lv_font_montserrat_16, 0xFFFFFF);
    lv_label_set_text(title, "HE sensor test");
    lv_obj_align(title, LV_ALIGN_TOP_MID, 0, 6);

    for (int k = 0; k < 2; k++) {
        key_view_t *v = &s_view[k];
        int x = 8 + k * 160;

        lv_obj_t *name = view_label(root, &lv_font_montserrat_14, 0xA6ADC8);
        lv_label_set_text_fmt(name, "Key %d", k + 1);
        lv_obj_set_pos(name, x, 32);

        v->bar = lv_bar_create(root);
        lv_obj_set_size(v->bar, 28, 150);
        lv_obj_set_pos(v->bar, x, 54);
        lv_bar_set_orientation(v->bar, LV_BAR_ORIENTATION_VERTICAL);
        lv_bar_set_range(v->bar, 0, 100);
        lv_obj_set_style_bg_color(v->bar, lv_color_hex(0x313244), LV_PART_MAIN);
        lv_obj_set_style_bg_color(v->bar, lv_color_hex(0xF5C2E7), LV_PART_INDICATOR);

        v->volts = view_label(root, &lv_font_montserrat_24, 0xFFFFFF);
        lv_obj_set_pos(v->volts, x + 38, 54);
        v->delta = view_label(root, &lv_font_montserrat_20, 0xF5C2E7);
        lv_obj_set_pos(v->delta, x + 38, 86);
        v->rest = view_label(root, &lv_font_montserrat_12, 0xA6ADC8);
        lv_obj_set_pos(v->rest, x + 38, 116);
        v->status = view_label(root, &lv_font_montserrat_16, 0xA6ADC8);
        lv_label_set_text(v->status, "measuring rest");
        lv_obj_set_pos(v->status, x + 38, 156);
    }

    lv_obj_t *hint = view_label(root, &lv_font_montserrat_12, 0x7F849C);
    lv_label_set_text(hint, "Rest is read at power-on: keep the keys up then.");
    lv_obj_align(hint, LV_ALIGN_BOTTOM_MID, 0, -8);

    lv_timer_create(view_timer_cb, VIEW_PERIOD_MS, NULL);
    ui_unlock();
}
