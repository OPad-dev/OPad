#include "input/mx_debug.h"

#ifdef CONFIG_OSUPAD_HE_DEBUG

#include <limits.h>
#include <stdio.h>
#include "input/he_debug.h"
#include "input/key_event.h"
#include "input/keypad.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "esp_attr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "mx_debug";

#define TAP_RING     32
#define MISS_RING    16
#define SAMPLE_US    1000
// A pin pressed this long without a press reported (or up this long with one)
// is logged. Real taps are tens of ms; bounces within a lockout are well under
#define MISMATCH_US  3000

typedef enum { MISS_NONE, MISS_MISS, MISS_LATE, MISS_STUCK } miss_kind_t;

typedef struct {
    int64_t down_us, up_us, prev_down_us;
    uint16_t sup;
    uint8_t key;
    bool rs_down, rs_up;
} tap_t;

typedef struct {
    int64_t at_us;
    uint32_t dur_us;
    uint8_t key, kind;
} miss_t;

// Key ISR, keypad task and sampler, guarded by s_lock
static portMUX_TYPE s_lock = portMUX_INITIALIZER_UNLOCKED;
static DRAM_ATTR tap_t s_open[2];  // the tap each key is in (down_us 0: none)
static DRAM_ATTR int64_t s_last_down[2];
static DRAM_ATTR tap_t s_taps[TAP_RING];
static DRAM_ATTR uint8_t s_tap_head, s_tap_tail;
static DRAM_ATTR miss_t s_misses[MISS_RING];
static DRAM_ATTR uint8_t s_miss_head, s_miss_tail;
static DRAM_ATTR uint32_t s_lost;
static DRAM_ATTR uint32_t s_presses[2], s_sup[2], s_miss_count[2];
static int s_id_min = INT_MAX, s_id_max = INT_MIN;
static int64_t s_off_start;           // keys off since (0: on), for he.mxmod
static int64_t s_off_since, s_off_us;  // the same, for this second's he.mxsec
static volatile bool s_running;

void IRAM_ATTR mx_debug_transition(uint8_t key, bool pressed, int64_t now_us, bool resample)
{
    if (!s_running || key > 1) {
        return;
    }
    // Callers hold the keypad spinlock; s_lock nests inside it
    portENTER_CRITICAL_SAFE(&s_lock);
    tap_t *t = &s_open[key];
    if (pressed) {
        *t = (tap_t){.down_us = now_us, .prev_down_us = s_last_down[key], .key = key, .rs_down = resample};
        s_last_down[key] = now_us;
        s_presses[key]++;
    } else if (t->down_us != 0) {
        t->up_us = now_us;
        t->rs_up = resample;
        uint8_t next = (uint8_t)((s_tap_head + 1) % TAP_RING);
        if (next == s_tap_tail) {
            s_lost++;
        } else {
            s_taps[s_tap_head] = *t;
            s_tap_head = next;
        }
        t->down_us = 0;
    }
    portEXIT_CRITICAL_SAFE(&s_lock);
}

void IRAM_ATTR mx_debug_suppressed(uint8_t key)
{
    if (!s_running || key > 1) {
        return;
    }
    portENTER_CRITICAL_SAFE(&s_lock);
    s_sup[key]++;
    if (s_open[key].down_us != 0 && s_open[key].sup < UINT16_MAX) {
        s_open[key].sup++;
    }
    portEXIT_CRITICAL_SAFE(&s_lock);
}

void mx_debug_module(bool keys_on, int id_mv)
{
    if (!s_running) {
        return;
    }
    int64_t now = esp_timer_get_time();
    int64_t off_for = 0;
    portENTER_CRITICAL(&s_lock);
    if (!keys_on) {
        s_off_start = s_off_since = now;
    } else {
        off_for = s_off_start ? now - s_off_start : 0;
        s_off_us += s_off_since ? now - s_off_since : 0;
        s_off_start = s_off_since = 0;
    }
    portEXIT_CRITICAL(&s_lock);
    if (keys_on) {
        he_debug_line("he.mxmod", 2, "keys ON after %lums, ID %dmV", (unsigned long)(off_for / 1000), id_mv);
    } else {
        // id_mv -1: the fast check, which only says "nothing there"
        he_debug_line("he.mxmod", 2, "keys OFF, ID %dmV", id_mv);
    }
}

void mx_debug_id_mv(int id_mv)
{
    if (!s_running || id_mv < 0) {
        return;
    }
    portENTER_CRITICAL(&s_lock);
    if (id_mv < s_id_min) s_id_min = id_mv;
    if (id_mv > s_id_max) s_id_max = id_mv;
    portEXIT_CRITICAL(&s_lock);
}

static void push_miss(uint8_t key, miss_kind_t kind, int64_t since_us, int64_t now_us)
{
    portENTER_CRITICAL(&s_lock);
    uint8_t next = (uint8_t)((s_miss_head + 1) % MISS_RING);
    if (next == s_miss_tail) {
        s_lost++;
    } else {
        s_misses[s_miss_head] = (miss_t){
            .at_us = since_us, .dur_us = (uint32_t)(now_us - since_us), .key = key, .kind = (uint8_t)kind};
        s_miss_head = next;
    }
    if (kind == MISS_MISS) {
        s_miss_count[key]++;
    }
    portEXIT_CRITICAL(&s_lock);
}

/*
 * The pins as they are against what the host was told, every millisecond.
 * A pin down with no press reported ends either with the press (LATE) or
 * with the pin up again (MISS: the tap never reached the host); a press
 * reported with the pin up is STUCK until the release comes.
 */
static void sample_cb(void *arg)
{
    (void)arg;
    static int64_t s_down_since[2], s_up_since[2];
    int64_t now = esp_timer_get_time();
    bool enabled = keypad_input_enabled();
    for (uint8_t k = 0; k < 2; k++) {
        if (!enabled) {
            s_down_since[k] = s_up_since[k] = 0;  // pins parked analog: nothing to read
            continue;
        }
        bool pin = k == 0 ? board_key1_read() : board_key2_read();
        bool rep = key_event_is_down((keypad_key_id_t)k);

        if (pin && !rep) {
            if (s_down_since[k] == 0) s_down_since[k] = now;
        } else if (s_down_since[k] != 0) {
            if (now - s_down_since[k] >= MISMATCH_US) {
                push_miss(k, pin ? MISS_LATE : MISS_MISS, s_down_since[k], now);
            }
            s_down_since[k] = 0;
        }

        if (!pin && rep) {
            if (s_up_since[k] == 0) s_up_since[k] = now;
        } else if (s_up_since[k] != 0) {
            if (now - s_up_since[k] >= MISMATCH_US) {
                push_miss(k, MISS_STUCK, s_up_since[k], now);
            }
            s_up_since[k] = 0;
        }
    }
}

static void format_tap(const tap_t *t)
{
    uint32_t held_x10 = (uint32_t)((t->up_us - t->down_us) / 100);
    uint32_t prev_ms = t->prev_down_us ? (uint32_t)((t->down_us - t->prev_down_us) / 1000) : 0;
    he_debug_line_at("he.mxtap", 1, (uint32_t)(t->down_us / 1000), "K%u held %lu.%lums prev %lums sup%u%s%s",
                     t->key + 1, (unsigned long)(held_x10 / 10), (unsigned long)(held_x10 % 10),
                     (unsigned long)prev_ms, t->sup, t->rs_down ? " rsdn" : "", t->rs_up ? " rsup" : "");
}

static void format_miss(const miss_t *m)
{
    uint32_t ms = m->dur_us / 1000;
    uint32_t at = (uint32_t)(m->at_us / 1000);
    switch (m->kind) {
    case MISS_MISS:
        he_debug_line_at("he.mxmiss", 2, at, "K%u MISS pin down %lums, no press sent", m->key + 1,
                         (unsigned long)ms);
        break;
    case MISS_LATE:
        he_debug_line_at("he.mxmiss", 1, at, "K%u LATE press %lums after the pin", m->key + 1, (unsigned long)ms);
        break;
    default:
        he_debug_line_at("he.mxmiss", 2, at, "K%u STUCK reported down, pin up %lums", m->key + 1,
                         (unsigned long)ms);
        break;
    }
}

static void debug_task(void *arg)
{
    (void)arg;
    keypad_config_t cfg;
    keypad_get_config(&cfg);
    he_debug_line("he.mxsec", 1, "MX debug log started, K1 GPIO%u K2 GPIO%u debounce %luus", cfg.key1_gpio,
                  cfg.key2_gpio, (unsigned long)cfg.debounce_us);
    int64_t next_sec = esp_timer_get_time() + 1000000;

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(20));

        while (true) {
            tap_t t;
            miss_t m;
            bool have_tap = false, have_miss = false;
            portENTER_CRITICAL(&s_lock);
            if (s_tap_tail != s_tap_head) {
                t = s_taps[s_tap_tail];
                s_tap_tail = (uint8_t)((s_tap_tail + 1) % TAP_RING);
                have_tap = true;
            }
            if (s_miss_tail != s_miss_head) {
                m = s_misses[s_miss_tail];
                s_miss_tail = (uint8_t)((s_miss_tail + 1) % MISS_RING);
                have_miss = true;
            }
            portEXIT_CRITICAL(&s_lock);
            if (!have_tap && !have_miss) break;
            if (have_tap) format_tap(&t);
            if (have_miss) format_miss(&m);
        }

        int64_t now = esp_timer_get_time();
        if (now < next_sec) {
            continue;
        }
        next_sec += 1000000;

        uint32_t dn[2], sup[2], miss[2], lost;
        int id_min, id_max;
        int64_t off_us;
        portENTER_CRITICAL(&s_lock);
        for (int k = 0; k < 2; k++) {
            dn[k] = s_presses[k];
            sup[k] = s_sup[k];
            miss[k] = s_miss_count[k];
            s_presses[k] = s_sup[k] = s_miss_count[k] = 0;
        }
        lost = s_lost;
        id_min = s_id_min;
        id_max = s_id_max;
        s_id_min = INT_MAX;
        s_id_max = INT_MIN;
        off_us = s_off_us + (s_off_since ? now - s_off_since : 0);
        s_off_us = 0;
        if (s_off_since) s_off_since = now;
        portEXIT_CRITICAL(&s_lock);

        // Quiet seconds stay out of the log
        if (dn[0] + dn[1] + sup[0] + sup[1] + miss[0] + miss[1] == 0 && off_us == 0) {
            continue;
        }
        char id[24] = "id ?";
        if (id_min != INT_MAX) {
            snprintf(id, sizeof(id), "id %d..%dmV", id_min, id_max);
        }
        he_debug_line("he.mxsec", (miss[0] || miss[1] || off_us) ? 2 : 1,
                      "K1 dn%lu sup%lu miss%lu K2 dn%lu sup%lu miss%lu %s off%lums lost%lu", (unsigned long)dn[0],
                      (unsigned long)sup[0], (unsigned long)miss[0], (unsigned long)dn[1], (unsigned long)sup[1],
                      (unsigned long)miss[1], id, (unsigned long)(off_us / 1000), (unsigned long)lost);
    }
}

void mx_debug_start(void)
{
    if (!he_debug_log_open()) {
        return;
    }
    const esp_timer_create_args_t args = {
        .callback = sample_cb,
        .dispatch_method = ESP_TIMER_TASK,
        .name = "mx_sample",
    };
    esp_timer_handle_t timer;
    if (esp_timer_create(&args, &timer) != ESP_OK || esp_timer_start_periodic(timer, SAMPLE_US) != ESP_OK) {
        ESP_LOGE(TAG, "Could not start the pin sampler");
        return;
    }
    if (xTaskCreatePinnedToCore(debug_task, "mx_debug", 4096, NULL, tskIDLE_PRIORITY + 2, NULL, 1) != pdPASS) {
        ESP_LOGE(TAG, "Could not start the debug log task");
        return;
    }
    s_running = true;
    ESP_LOGW(TAG, "MX DEBUG firmware: extra logging, latency figures do not count");
}

#endif
