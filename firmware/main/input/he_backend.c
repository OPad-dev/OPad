/*
 * Hall Effect key input. docs/specs/v2-hall-effect-decisions.md D1-D4, D6.
 *
 * ADC1 continuous mode alternates key 1 (CH9) and key 2 (CH6) at
 * HE_SAMPLE_HZ, HE_FRAME_CONVS conversions per DMA frame (250 us). The frame
 * interrupt (on_frame, core 0, IRAM, also while flash is written: Kconfig
 * selects ADC_CONTINUOUS_ISR_IRAM_SAFE) takes each key's newest valid
 * conversion and steps its decision engine. Only when a key changes does it
 * wake he_task, which reports it like the MX keypad task does: no task runs
 * per frame.
 *
 * he_task also watches health every HE_CHECK_TICKS: a key without a valid
 * reading for HE_STALL_US is released and ignored until readings return; no
 * frames at all for HE_STALL_US releases both keys and restarts the ADC, with
 * a growing back-off. A key released this way must be seen up before it can
 * press again (he_rt_fault), so recovery never presses a key. Calibration is
 * never touched by any of this.
 */
#include "he_backend.h"

#ifdef CONFIG_OSUPAD_HE_BACKEND

#include "input/he_frame.h"
#include "input/he_params.h"
#include "input/key_event.h"
#include "esp_adc/adc_continuous.h"
#include "esp_attr.h"
#include "esp_cpu.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "he";

#define HE_SAMPLE_HZ    64000  // both keys together
#define HE_FRAME_CONVS  16     // 8 per key, one frame every 250 us
#define HE_FRAME_US     (1000000 * HE_FRAME_CONVS / HE_SAMPLE_HZ)
// The driver also copies each frame into a ringbuffer nobody reads here; the
// smallest it takes keeps that cheap
#define HE_STORE_FRAMES 2

// PROVISIONAL (OPEN G): to be validated under display, USB and flash load
#define HE_STALL_US             5000
#define HE_CHECK_TICKS          (pdMS_TO_TICKS(2) ? pdMS_TO_TICKS(2) : 1)
#define HE_RESTART_BACKOFF_MIN_US 10000
#define HE_RESTART_BACKOFF_MAX_US 1000000

/*
 * PROVISIONAL factory thresholds, so an uncalibrated pad types for testing:
 * plain mode, press at half the span, release 10 % above it. Rest and span
 * are the V1.1 bench means (keys 2084/2088 at rest, +447/+508 normal press,
 * docs/specs/v2-hall-effect-design.md §10b). The noise floors stand in for
 * OPEN A/D and are not validated. None of these is a product default yet
 * (OPEN C); the calibration manager replaces them.
 */
static const he_calibration_t HE_FACTORY_CAL = {.rest = 2086, .full = 2086 + 450, .valid = false};
static const he_key_settings_t HE_FACTORY_SETTINGS = {.actuation = 500, .release_hyst = 100};
static const he_limits_t HE_PROVISIONAL_LIMITS = {.min_movement = 32, .min_top_reset = 32};

// Thresholds that never press: `release` below any reading, so a key never arms
static const he_rt_params_t HE_PARAMS_NEVER = {
    .actuation = INT32_MAX, .release = -1, .top_reset = -1,
    .rt_press = 1, .rt_release = 1, .peak_cap = INT32_MAX,
};

static adc_continuous_handle_t s_adc;
static TaskHandle_t s_task;
static volatile bool s_running;

// Shared by on_frame and he_task (and he_backend_set_params). Only copies and
// he_rt calls happen under it.
static portMUX_TYPE s_lock = portMUX_INITIALIZER_UNLOCKED;
static he_rt_key_t s_keys[2];
static bool s_down[2];
static int64_t s_edge_us[2];         // frame time of the last change
static bool s_key_faulted[2];
static int64_t s_last_frame_us;
static int64_t s_last_valid_us[2];
static he_backend_stats_t s_stats = {.raw = {-1, -1}};

static bool IRAM_ATTR on_frame(adc_continuous_handle_t handle, const adc_continuous_evt_data_t *edata,
                               void *user_data)
{
    (void)handle;
    (void)user_data;
    uint32_t c0 = esp_cpu_get_cycle_count();
    int64_t now = esp_timer_get_time();
    he_frame_t f;
    he_frame_parse((const uint32_t *)edata->conv_frame_buffer, edata->size / SOC_ADC_DIGI_RESULT_BYTES, &f);

    bool changed = false;
    portENTER_CRITICAL_ISR(&s_lock);
    if (s_stats.frames > 0) {
        int64_t gap = now - s_last_frame_us;
        if (gap > 2 * HE_FRAME_US) {
            s_stats.late_frames++;
        }
        if (gap > (int64_t)s_stats.max_gap_us) {
            s_stats.max_gap_us = (uint32_t)gap;
        }
    }
    s_last_frame_us = now;
    s_stats.frames++;
    s_stats.foreign_words += f.foreign;
    s_stats.railed_samples += f.railed;
    for (int k = 0; k < 2; k++) {
        if (f.newest[k] < 0) {
            continue;
        }
        s_last_valid_us[k] = now;
        s_stats.raw[k] = f.newest[k];
        if (s_key_faulted[k]) {
            continue;
        }
        he_rt_event_t ev = he_rt_step(&s_keys[k], f.newest[k]);
        if (ev != HE_RT_NONE) {
            s_down[k] = ev == HE_RT_DOWN;
            s_edge_us[k] = now;
            changed = true;
            if (ev == HE_RT_DOWN) {
                key_event_press_accepted((uint8_t)k, now);
            }
        }
    }
    portEXIT_CRITICAL_ISR(&s_lock);

    BaseType_t woken = pdFALSE;
    if (changed && s_task != NULL) {
        vTaskNotifyGiveFromISR(s_task, &woken);
    }
    uint32_t cycles = esp_cpu_get_cycle_count() - c0;
    if (cycles > s_stats.isr_max_cycles) {
        s_stats.isr_max_cycles = cycles;  // written here only
    }
    return woken == pdTRUE;
}

static esp_err_t adc_setup(void)
{
    adc_continuous_handle_cfg_t hcfg = {
        .max_store_buf_size = HE_FRAME_CONVS * SOC_ADC_DIGI_RESULT_BYTES * HE_STORE_FRAMES,
        .conv_frame_size = HE_FRAME_CONVS * SOC_ADC_DIGI_RESULT_BYTES,
    };
    // On this task, so the DMA interrupt is allocated on core 0
    esp_err_t err = adc_continuous_new_handle(&hcfg, &s_adc);
    if (err != ESP_OK) {
        s_adc = NULL;
        return err;
    }
    adc_digi_pattern_config_t pattern[2] = {
        {.atten = ADC_ATTEN_DB_12, .channel = HE_FRAME_CH_KEY1, .unit = ADC_UNIT_1, .bit_width = 12},
        {.atten = ADC_ATTEN_DB_12, .channel = HE_FRAME_CH_KEY2, .unit = ADC_UNIT_1, .bit_width = 12},
    };
    adc_continuous_config_t cfg = {
        .pattern_num = 2,
        .adc_pattern = pattern,
        .sample_freq_hz = HE_SAMPLE_HZ,
        .conv_mode = ADC_CONV_SINGLE_UNIT_1,
        .format = ADC_DIGI_OUTPUT_FORMAT_TYPE2,
    };
    adc_continuous_evt_cbs_t cbs = {.on_conv_done = on_frame};
    err = adc_continuous_config(s_adc, &cfg);
    if (err == ESP_OK) {
        err = adc_continuous_register_event_callbacks(s_adc, &cbs, NULL);
    }
    if (err == ESP_OK) {
        err = adc_continuous_start(s_adc);
    }
    if (err != ESP_OK) {
        adc_continuous_deinit(s_adc);
        s_adc = NULL;
    }
    return err;
}

// Fresh readings from now on: no stall or key fault counted for the time before
static void resync_health(int64_t now)
{
    portENTER_CRITICAL(&s_lock);
    s_last_frame_us = now;
    for (int k = 0; k < 2; k++) {
        s_last_valid_us[k] = now;
        s_key_faulted[k] = false;
    }
    portEXIT_CRITICAL(&s_lock);
}

static void fault_key_locked(int k, int64_t now)
{
    s_key_faulted[k] = true;
    if (he_rt_fault(&s_keys[k]) == HE_RT_UP) {
        s_down[k] = false;
        s_edge_us[k] = now;
    }
}

// IRAM like the MX keypad task: from flash, a cache miss while core 1 is busy
// cost the MX path ~170 us (linker.lf)
static void IRAM_ATTR publish(bool reported[2])
{
    for (int k = 0; k < 2; k++) {
        portENTER_CRITICAL(&s_lock);
        bool down = s_down[k];
        int64_t edge_us = s_edge_us[k];
        portEXIT_CRITICAL(&s_lock);
        if (down != reported[k]) {
            reported[k] = down;
            key_event_report((uint8_t)k, down, edge_us);
        }
    }
}

// True if the ADC has stopped delivering frames and needs a restart
static bool check_health(int64_t now)
{
    bool stalled = false;
    portENTER_CRITICAL(&s_lock);
    if (now - s_last_frame_us > HE_STALL_US) {
        stalled = true;
        s_stats.stalls++;
        for (int k = 0; k < 2; k++) {
            fault_key_locked(k, now);
        }
    } else {
        for (int k = 0; k < 2; k++) {
            bool stale = now - s_last_valid_us[k] > HE_STALL_US;
            if (stale && !s_key_faulted[k]) {
                s_stats.key_faults++;
                fault_key_locked(k, now);
            } else if (!stale && s_key_faulted[k]) {
                s_key_faulted[k] = false;  // still has to be seen up (he_rt_fault)
            }
        }
    }
    portEXIT_CRITICAL(&s_lock);
    return stalled;
}

static void IRAM_ATTR he_task(void *arg)
{
    (void)arg;
    bool reported[2] = {false, false};
    int64_t backoff_us = HE_RESTART_BACKOFF_MIN_US;
    int64_t retry_at_us = 0;
    int64_t healthy_since_us = 0;
    bool adc_ok = false;

    esp_err_t err = adc_setup();
    if (err == ESP_OK) {
        adc_ok = true;
        ESP_LOGI(TAG, "ADC1 continuous: %d conv/s, %d per frame, core %d", HE_SAMPLE_HZ, HE_FRAME_CONVS,
                 xPortGetCoreID());
    } else {
        ESP_LOGE(TAG, "ADC setup failed: %s, retrying", esp_err_to_name(err));
        retry_at_us = esp_timer_get_time() + backoff_us;
    }
    healthy_since_us = esp_timer_get_time();
    resync_health(healthy_since_us);

    while (1) {
        ulTaskNotifyTake(pdTRUE, HE_CHECK_TICKS);
        publish(reported);  // key changes first

        int64_t now = esp_timer_get_time();
        if (adc_ok && check_health(now)) {
            publish(reported);
            adc_ok = false;
            retry_at_us = now + backoff_us;
        }
        if (!adc_ok && now >= retry_at_us) {
            if (s_adc != NULL) {
                adc_continuous_stop(s_adc);  // may already be stopped
                err = adc_continuous_start(s_adc);
            } else {
                err = adc_setup();
            }
            now = esp_timer_get_time();
            if (err == ESP_OK) {
                adc_ok = true;
                healthy_since_us = now;
                portENTER_CRITICAL(&s_lock);
                s_stats.restarts++;
                portEXIT_CRITICAL(&s_lock);
                resync_health(now);
            } else {
                retry_at_us = now + backoff_us;
            }
            backoff_us = backoff_us * 2 > HE_RESTART_BACKOFF_MAX_US ? HE_RESTART_BACKOFF_MAX_US : backoff_us * 2;
        } else if (adc_ok && backoff_us != HE_RESTART_BACKOFF_MIN_US &&
                   now - healthy_since_us > HE_RESTART_BACKOFF_MAX_US) {
            backoff_us = HE_RESTART_BACKOFF_MIN_US;  // a second without trouble
        }
    }
}

esp_err_t he_backend_start(void)
{
    if (s_running) {
        return ESP_ERR_INVALID_STATE;
    }
    for (int k = 0; k < 2; k++) {
        he_rt_params_t p;
        uint32_t r = he_params_build(&HE_FACTORY_SETTINGS, &HE_FACTORY_CAL, &HE_PROVISIONAL_LIMITS, &p);
        if (HE_PARAMS_FAILED(r)) {
            ESP_LOGE(TAG, "Factory thresholds rejected (0x%lx): key %d stays up", (unsigned long)r, k + 1);
            p = HE_PARAMS_NEVER;
        }
        he_rt_init(&s_keys[k], &p);
    }
    s_running = true;
    // Core 0 at the keypad task's priority, above TinyUSB, as the MX key path
    BaseType_t res = xTaskCreatePinnedToCore(he_task, "he_task", 4096, NULL, configMAX_PRIORITIES - 1,
                                             &s_task, 0);
    if (res != pdPASS) {
        s_running = false;
        return ESP_ERR_NO_MEM;
    }
    return ESP_OK;
}

bool he_backend_running(void)
{
    return s_running;
}

void he_backend_set_params(uint8_t key_index, const he_rt_params_t *p)
{
    if (key_index >= 2 || p == NULL) {
        return;
    }
    portENTER_CRITICAL(&s_lock);
    // No reading yet: treat the key as deep, so it must be seen up first
    int32_t raw = s_stats.raw[key_index] >= 0 ? s_stats.raw[key_index] : INT32_MAX;
    he_rt_set_params(&s_keys[key_index], p, raw);
    portEXIT_CRITICAL(&s_lock);
}

void he_backend_get_stats(he_backend_stats_t *out)
{
    if (out == NULL) {
        return;
    }
    portENTER_CRITICAL(&s_lock);
    *out = s_stats;
    portEXIT_CRITICAL(&s_lock);
}

#endif  // CONFIG_OSUPAD_HE_BACKEND
