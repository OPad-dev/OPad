#include "key_event.h"
#include "input/latency_stats.h"
#include "input/press_log.h"
#include "usb/usb_hid.h"
#include "ui/ui.h"
#include "esp_attr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "driver/gpio.h"
#include <stdatomic.h>

static atomic_uint_least64_t s_lifetime_presses[KEY_ID_COUNT];
// Presses in the current osu! attempt; zeroed by the host on every new play
static atomic_uint_least32_t s_map_presses[KEY_ID_COUNT];
static atomic_llong s_last_press_us[KEY_ID_COUNT];
static volatile bool s_reported[KEY_ID_COUNT];

void key_event_init(void)
{
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO) || defined(OSUPAD_BENCH_DEBUG_GPIO)
    gpio_config_t dbg_io_conf = {
        .pin_bit_mask = (1ULL << CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM),
        .mode = GPIO_MODE_OUTPUT,
        .pull_up_en = GPIO_PULLUP_DISABLE,
        .pull_down_en = GPIO_PULLDOWN_ENABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    gpio_config(&dbg_io_conf);
    gpio_set_level(CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM, 0);
    ESP_LOGI("key_event", "Debug GPIO initialized on pin %d", CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM);
#endif
}

void IRAM_ATTR key_event_press_accepted(uint8_t key_index, int64_t t_us)
{
    if (key_index >= KEY_ID_COUNT) {
        return;
    }
    atomic_fetch_add_explicit(&s_lifetime_presses[key_index], 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&s_map_presses[key_index], 1, memory_order_relaxed);
    atomic_store_explicit(&s_last_press_us[key_index], t_us, memory_order_relaxed);
    press_log_push(key_index + 1, t_us);
}

void IRAM_ATTR key_event_report(uint8_t key_index, bool pressed, int64_t edge_us)
{
    if (key_index >= KEY_ID_COUNT) {
        return;
    }
    s_reported[key_index] = pressed;
    // HID report first; activity bookkeeping only after it is submitted
    if (usb_hid_handle_key_event(key_index, pressed, edge_us)) {
        latency_stats_record((uint32_t)(esp_timer_get_time() - edge_us));
    } else {
        // Resent by the HID layer, which records the latency then
        latency_stats_record_deferred();
    }
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO) || defined(OSUPAD_BENCH_DEBUG_GPIO)
    gpio_set_level(CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM, pressed ? 1 : 0);
#endif
    if (pressed) {
        ui_notify_activity();
    }
}

bool key_event_is_down(keypad_key_id_t key_id)
{
    return key_id < KEY_ID_COUNT && s_reported[key_id];
}

void key_event_get_lifetime_presses(uint64_t *key1_presses, uint64_t *key2_presses)
{
    if (key1_presses) {
        *key1_presses = atomic_load_explicit(&s_lifetime_presses[KEY_ID_1], memory_order_relaxed);
    }
    if (key2_presses) {
        *key2_presses = atomic_load_explicit(&s_lifetime_presses[KEY_ID_2], memory_order_relaxed);
    }
}

void key_event_add_lifetime_presses(uint64_t key1_presses, uint64_t key2_presses)
{
    atomic_fetch_add_explicit(&s_lifetime_presses[KEY_ID_1], key1_presses, memory_order_relaxed);
    atomic_fetch_add_explicit(&s_lifetime_presses[KEY_ID_2], key2_presses, memory_order_relaxed);
}

void key_event_get_map_presses(uint32_t *key1_presses, uint32_t *key2_presses)
{
    if (key1_presses) {
        *key1_presses = atomic_load_explicit(&s_map_presses[KEY_ID_1], memory_order_relaxed);
    }
    if (key2_presses) {
        *key2_presses = atomic_load_explicit(&s_map_presses[KEY_ID_2], memory_order_relaxed);
    }
}

void key_event_reset_map_presses(void)
{
    atomic_store_explicit(&s_map_presses[KEY_ID_1], 0, memory_order_relaxed);
    atomic_store_explicit(&s_map_presses[KEY_ID_2], 0, memory_order_relaxed);
}

int64_t key_event_get_last_press_us(keypad_key_id_t key_id)
{
    if (key_id >= KEY_ID_COUNT) {
        return 0;
    }
    return atomic_load_explicit(&s_last_press_us[key_id], memory_order_relaxed);
}
