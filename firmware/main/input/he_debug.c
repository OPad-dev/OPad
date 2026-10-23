#include "input/he_debug.h"

#ifdef CONFIG_OSUPAD_HE_DEBUG

#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "input/he_backend.h"
#include "input/he_cal.h"
#include "runtime/runtime.h"
#include "ui/core/ui_anim.h"
#include "input/touch_retry.h"
#include "driver/temperature_sensor.h"
#include "esp_attr.h"
#include "esp_heap_caps.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "he_debug";

#define EDGE_RING  32   // presses/releases waiting to be formatted
#define HIST       16   // frames of history per key (>= HE_DEBUG_PRE)
#define LINE_RING  512  // formatted lines waiting for the host (PSRAM)

typedef struct {
    int64_t at_us;
    int32_t raw;
    int32_t extreme;
    int16_t pre[HE_DEBUG_PRE];  // oldest first; -1 = no reading in that frame
    uint8_t key, ev, from, to;
} edge_t;

// Interrupt side: internal RAM, guarded by s_isr_lock
static portMUX_TYPE s_isr_lock = portMUX_INITIALIZER_UNLOCKED;
static DRAM_ATTR int16_t s_hist[2][HIST];
static DRAM_ATTR uint8_t s_hist_at;
static DRAM_ATTR edge_t s_edges[EDGE_RING];
static DRAM_ATTR uint8_t s_edge_head, s_edge_tail;
static DRAM_ATTR uint32_t s_edges_lost;
static DRAM_ATTR int32_t s_min[2] = {INT32_MAX, INT32_MAX}, s_max[2] = {INT32_MIN, INT32_MIN};
static DRAM_ATTR int64_t s_sum[2];
static DRAM_ATTR uint32_t s_n[2];

// Task side: formatted lines, guarded by s_line_lock
static portMUX_TYPE s_line_lock = portMUX_INITIALIZER_UNLOCKED;
static he_debug_line_t *s_lines;
static size_t s_line_cap, s_line_head, s_line_count;
static uint32_t s_lines_lost;

void IRAM_ATTR he_debug_isr_frame(int32_t raw0, int32_t raw1)
{
    const int32_t raw[2] = {raw0, raw1};
    portENTER_CRITICAL_ISR(&s_isr_lock);
    for (int k = 0; k < 2; k++) {
        s_hist[k][s_hist_at] = (int16_t)raw[k];
        if (raw[k] < 0) {
            continue;
        }
        if (raw[k] < s_min[k]) s_min[k] = raw[k];
        if (raw[k] > s_max[k]) s_max[k] = raw[k];
        s_sum[k] += raw[k];
        s_n[k]++;
    }
    s_hist_at = (uint8_t)((s_hist_at + 1) % HIST);
    portEXIT_CRITICAL_ISR(&s_isr_lock);
}

void IRAM_ATTR he_debug_isr_edge(uint8_t key, he_rt_event_t ev, he_rt_state_t from, he_rt_state_t to,
                                 int32_t extreme, int32_t raw, int64_t now_us)
{
    portENTER_CRITICAL_ISR(&s_isr_lock);
    uint8_t next = (uint8_t)((s_edge_head + 1) % EDGE_RING);
    if (next == s_edge_tail) {
        s_edges_lost++;
    } else {
        edge_t *e = &s_edges[s_edge_head];
        e->at_us = now_us;
        e->raw = raw;
        e->extreme = extreme;
        e->key = key;
        e->ev = (uint8_t)ev;
        e->from = (uint8_t)from;
        e->to = (uint8_t)to;
        // The newest HE_DEBUG_PRE frames, oldest first; the current frame was
        // already recorded by he_debug_isr_frame
        for (int i = 0; i < HE_DEBUG_PRE; i++) {
            e->pre[i] = s_hist[key & 1][(s_hist_at + HIST - HE_DEBUG_PRE + i) % HIST];
        }
        s_edge_head = next;
    }
    portEXIT_CRITICAL_ISR(&s_isr_lock);
}

// `ms`: when it happened on the pad's clock (a press: the frame it was made in)
static void push_line_at(const char *tag, uint8_t level, const char *msg, uint32_t ms)
{
    portENTER_CRITICAL(&s_line_lock);
    if (s_lines == NULL || s_line_count == s_line_cap) {
        s_lines_lost++;
        portEXIT_CRITICAL(&s_line_lock);
        return;
    }
    he_debug_line_t *l = &s_lines[(s_line_head + s_line_count) % s_line_cap];
    strncpy(l->tag, tag, sizeof(l->tag) - 1);
    l->tag[sizeof(l->tag) - 1] = '\0';
    strncpy(l->msg, msg, sizeof(l->msg) - 1);
    l->msg[sizeof(l->msg) - 1] = '\0';
    l->ms = ms;
    l->level = level;
    s_line_count++;
    portEXIT_CRITICAL(&s_line_lock);
}

void he_debug_line(const char *tag, uint8_t level, const char *fmt, ...)
{
    char msg[64];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof(msg), fmt, ap);
    va_end(ap);
    push_line_at(tag, level, msg, (uint32_t)(esp_timer_get_time() / 1000));
}

void he_debug_line_at(const char *tag, uint8_t level, uint32_t ms, const char *fmt, ...)
{
    char msg[64];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(msg, sizeof(msg), fmt, ap);
    va_end(ap);
    push_line_at(tag, level, msg, ms);
}

size_t he_debug_take(he_debug_line_t *out, size_t max)
{
    portENTER_CRITICAL(&s_line_lock);
    size_t n = s_line_count < max ? s_line_count : max;
    for (size_t i = 0; i < n; i++) {
        out[i] = s_lines[(s_line_head + i) % s_line_cap];
    }
    s_line_head = s_line_cap ? (s_line_head + n) % s_line_cap : 0;
    s_line_count -= n;
    portEXIT_CRITICAL(&s_line_lock);
    return n;
}

bool he_debug_pending(void)
{
    portENTER_CRITICAL(&s_line_lock);
    bool pending = s_line_count > 0;
    portEXIT_CRITICAL(&s_line_lock);
    return pending;
}

static int32_t rest_of(uint8_t key)
{
    he_calibration_t cal;
    he_cal_get(key, &cal);
    return cal.rest;
}

static int clamp999(int32_t v)
{
    return v > 999 ? 999 : v < -999 ? -999 : (int)v;
}

static char state_letter(uint8_t s)
{
    return s == HE_RT_ARMED ? 'A' : s == HE_RT_PRESSED ? 'P' : s == HE_RT_RT_RELEASED ? 'R' : '?';
}

void he_debug_params(uint8_t key, const he_rt_params_t *p)
{
    int32_t r = rest_of(key);
    he_debug_line("he.cfg", 1, "K%u act%+d rel%+d top%+d rtp%d rtr%d %s cap%s", key + 1,
                  clamp999(p->actuation - r), clamp999(p->release - r), clamp999(p->top_reset - r),
                  clamp999(p->rt_press), clamp999(p->rt_release),
                  !p->rapid_trigger ? "plain" : p->continuous ? "rt-cont" : "rt",
                  p->peak_cap == INT32_MAX ? "-" : "set");
}

static void format_edge(const edge_t *e)
{
    int32_t r = rest_of(e->key);
    he_rt_params_t p;
    he_backend_debug_params(e->key, &p);
    // Stamped with the frame's own time, not when this task got to it (every
    // 20 ms), so presses can be lined up against touches and each other
    uint32_t at_ms = (uint32_t)(e->at_us / 1000);
    char edge[64];
    snprintf(edge, sizeof(edge), "K%u %s %c>%c raw%+d act%+d rel%+d top%+d pk%+d", e->key + 1,
             e->ev == HE_RT_DOWN ? "DOWN" : "UP", state_letter(e->from), state_letter(e->to),
             clamp999(e->raw - r), clamp999(p.actuation - r), clamp999(p.release - r), clamp999(p.top_reset - r),
             clamp999(e->extreme - r));
    push_line_at("he.edge", 1, edge, at_ms);
    char pre[64];
    int n = snprintf(pre, sizeof(pre), "K%u", e->key + 1);
    for (int i = 0; i < HE_DEBUG_PRE && n < (int)sizeof(pre); i++) {
        n += e->pre[i] < 0 ? snprintf(pre + n, sizeof(pre) - n, " x")
                           : snprintf(pre + n, sizeof(pre) - n, " %+d", clamp999(e->pre[i] - r));
    }
    push_line_at("he.pre", 1, pre, at_ms);
}

static const char *state_name(osupad_state_t s)
{
    switch (s) {
    case OSUPAD_STATE_IDLE: return "idle";
    case OSUPAD_STATE_PLAYING: return "playing";
    case OSUPAD_STATE_COOLDOWN: return "cooldown";
    case OSUPAD_STATE_SYNC: return "sync";
    default: return "?";
    }
}

static void debug_task(void *arg)
{
    (void)arg;
    temperature_sensor_handle_t tsens = NULL;
    temperature_sensor_config_t tcfg = TEMPERATURE_SENSOR_CONFIG_DEFAULT(-10, 80);
    if (temperature_sensor_install(&tcfg, &tsens) != ESP_OK || temperature_sensor_enable(tsens) != ESP_OK) {
        tsens = NULL;
    }
    he_backend_stats_t prev = {0};
    he_backend_get_stats(&prev);
    int64_t next_sec = esp_timer_get_time() + 1000000;
    osupad_state_t last_state = runtime_get_state();
    bool last_anim = ui_anim_active();
    bool last_touch = touch_retry_is_pressed();
    uint32_t touch_ticks = 0;  // 20 ms ticks with a finger on the screen, this second
    uint32_t prev_reads = touch_retry_debug_reads();
    he_debug_line("he.touch", 1, "controller %s", touch_retry_debug_int_wakeup() ? "INT wake" : "polled at 100 Hz");
    he_debug_line("he.ctx", 1, "debug log started, state %s", state_name(last_state));
    for (uint8_t k = 0; k < 2; k++) {
        he_rt_params_t p;
        he_backend_debug_params(k, &p);
        he_debug_params(k, &p);  // the thresholds it booted with
    }

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(20));

        edge_t e;
        while (true) {
            bool have = false;
            portENTER_CRITICAL(&s_isr_lock);
            if (s_edge_tail != s_edge_head) {
                e = s_edges[s_edge_tail];
                s_edge_tail = (uint8_t)((s_edge_tail + 1) % EDGE_RING);
                have = true;
            }
            portEXIT_CRITICAL(&s_isr_lock);
            if (!have) break;
            format_edge(&e);
        }

        // Context changes are logged when they happen, not only each second
        osupad_state_t st = runtime_get_state();
        bool anim = ui_anim_active();
        if (st != last_state || anim != last_anim) {
            he_debug_line("he.ctx", 1, "state %s anim %d", state_name(st), anim);
            last_state = st;
            last_anim = anim;
        }
        // A finger on the touchscreen: the touch controller is polled over I2C
        // while it stays down, a suspect for interference on the key readings
        bool touch = touch_retry_is_pressed();
        if (touch != last_touch) {
            he_debug_line("he.touch", 1, touch ? "finger down" : "finger up");
            last_touch = touch;
        }
        touch_ticks += touch;

        int64_t now = esp_timer_get_time();
        if (now < next_sec) {
            continue;
        }
        next_sec += 1000000;

        int32_t mn[2], mx[2], avg[2];
        uint32_t lost_edges;
        portENTER_CRITICAL(&s_isr_lock);
        for (int k = 0; k < 2; k++) {
            mn[k] = s_min[k];
            mx[k] = s_max[k];
            avg[k] = s_n[k] ? (int32_t)(s_sum[k] / s_n[k]) : INT32_MIN;
            s_min[k] = INT32_MAX;
            s_max[k] = INT32_MIN;
            s_sum[k] = 0;
            s_n[k] = 0;
        }
        lost_edges = s_edges_lost;
        portEXIT_CRITICAL(&s_isr_lock);
        int32_t r0 = rest_of(0), r1 = rest_of(1);
        if (avg[0] != INT32_MIN && avg[1] != INT32_MIN) {
            he_debug_line("he.sec", 1, "K1 %+d..%+d avg%+d K2 %+d..%+d avg%+d", clamp999(mn[0] - r0),
                          clamp999(mx[0] - r0), clamp999(avg[0] - r0), clamp999(mn[1] - r1),
                          clamp999(mx[1] - r1), clamp999(avg[1] - r1));
        } else {
            he_debug_line("he.sec", 2, "no readings this second");
        }

        he_backend_stats_t s;
        he_backend_get_stats(&s);
        he_debug_line("he.adc", (s.key_faults != prev.key_faults || s.stalls != prev.stalls) ? 2 : 1,
                      "fr%lu late%lu gap%lu forg%lu rail%lu flt%lu stl%lu isr%lu",
                      (unsigned long)(s.frames - prev.frames), (unsigned long)(s.late_frames - prev.late_frames),
                      (unsigned long)s.max_gap_us, (unsigned long)(s.foreign_words - prev.foreign_words),
                      (unsigned long)(s.railed_samples - prev.railed_samples),
                      (unsigned long)(s.key_faults - prev.key_faults), (unsigned long)(s.stalls - prev.stalls),
                      (unsigned long)s.isr_max_cycles);
        prev = s;

        float temp = 0;
        bool have_temp = tsens && temperature_sensor_get_celsius(tsens, &temp) == ESP_OK;
        portENTER_CRITICAL(&s_line_lock);
        uint32_t lost_lines = s_lines_lost;
        portEXIT_CRITICAL(&s_line_lock);
        char temp_s[8] = "?";
        if (have_temp) {
            snprintf(temp_s, sizeof(temp_s), "%d", (int)(temp + (temp < 0 ? -0.5f : 0.5f)));
        }
        uint32_t reads = touch_retry_debug_reads();
        he_debug_line("he.ctx", 1, "state %s anim %d touch %lums i2c %lu temp %sC lost %lu/%lu", state_name(st),
                      anim, (unsigned long)(touch_ticks * 20), (unsigned long)(reads - prev_reads), temp_s,
                      (unsigned long)lost_edges, (unsigned long)lost_lines);
        prev_reads = reads;
        touch_ticks = 0;
    }
}

bool he_debug_log_open(void)
{
    if (s_lines != NULL) {
        return true;
    }
    he_debug_line_t *lines = heap_caps_calloc(LINE_RING, sizeof(he_debug_line_t), MALLOC_CAP_SPIRAM);
    size_t cap = LINE_RING;
    if (lines == NULL) {
        cap = 32;
        lines = heap_caps_calloc(cap, sizeof(he_debug_line_t), MALLOC_CAP_INTERNAL);
    }
    if (lines == NULL) {
        ESP_LOGE(TAG, "No memory for the debug log");
        return false;
    }
    portENTER_CRITICAL(&s_line_lock);
    s_line_cap = cap;
    s_lines = lines;
    portEXIT_CRITICAL(&s_line_lock);
    return true;
}

void he_debug_start(void)
{
    if (!he_debug_log_open()) {
        return;
    }
    if (xTaskCreatePinnedToCore(debug_task, "he_debug", 4096, NULL, tskIDLE_PRIORITY + 2, NULL, 1) != pdPASS) {
        ESP_LOGE(TAG, "Could not start the debug log task");
        return;
    }
    ESP_LOGW(TAG, "Hall Effect DEBUG firmware: extra logging, latency figures do not count");
}

#endif
