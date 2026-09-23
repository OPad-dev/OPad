#include "usb_hid.h"
#include "input/keypad.h"
#include "input/latency_stats.h"
#include "diag/diag.h"
#include "esp_timer.h"
#include "tusb.h"
#include "class/hid/hid_device.h"
#include "esp_log.h"
#include <stdatomic.h>

static const char *TAG = "usb_hid";

static uint8_t s_key1_code = 0x1D; // 'z'
static uint8_t s_key2_code = 0x1B; // 'x'
static uint8_t s_key3_code = 0x35; // '`' / '~' (osu! Quick Retry)

/*
 * Who touches what:
 * - s_key1/2_pressed: written by the keypad task (core 0), s_key3_pressed by
 *   the touch task (core 1). Each has that one writer; every submitter reads all
 *   three.
 * - s_change_seq: bumped by both writers after they change a key, so a submit
 *   that loaded it before reading the keys carries at least that change.
 * - s_sent_seq: the newest s_change_seq a submitted report carried, raised by
 *   whichever submit succeeds (keypad task, touch task or the TinyUSB task's
 *   completion). A change is pending while s_sent_seq is behind s_change_seq.
 *   Unlike a pending flag, one submitter cannot clear another's change.
 * - s_pending_edge_us: armed only by the keypad task, taken by whoever delivers.
 */
static volatile bool s_key1_pressed = false;
static volatile bool s_key2_pressed = false;
static volatile bool s_key3_pressed = false;
static atomic_uint s_change_seq = 0;
static atomic_uint s_sent_seq = 0;
// Edge time of the oldest key change not yet delivered, for latency stats (0 = none)
static atomic_llong s_pending_edge_us = 0;

// a is at or past b, with wrap-around
static inline bool seq_reached(unsigned a, unsigned b)
{
    return (int)(a - b) >= 0;
}

static void record_pending_latency(void)
{
    int64_t edge = atomic_exchange(&s_pending_edge_us, 0);
    if (edge) {
        latency_stats_record((uint32_t)(esp_timer_get_time() - edge));
    }
}

esp_err_t usb_hid_init(void)
{
    keypad_config_t cfg;
    keypad_get_config(&cfg);
    s_key1_code = cfg.keycode1;
    s_key2_code = cfg.keycode2;
    s_key3_code = 0x35; // Default: '`' (Grave accent / Tilde for osu! Quick Retry)

    keypad_set_state_callback(usb_hid_handle_key_event);
    ESP_LOGI(TAG, "USB HID keyboard initialized (Key1: 0x%02X, Key2: 0x%02X, TouchRetry: 0x%02X)",
             s_key1_code, s_key2_code, s_key3_code);
    return ESP_OK;
}

bool usb_hid_is_ready(void)
{
    return tud_hid_ready();
}

void usb_hid_set_keycodes(uint8_t key1_code, uint8_t key2_code)
{
    s_key1_code = key1_code;
    s_key2_code = key2_code;
}

void usb_hid_set_key3_code(uint8_t key3_code)
{
    s_key3_code = key3_code;
}

static bool submit_current_state(void)
{
    uint8_t keycodes[6] = {0};
    uint8_t count = 0;

    if (s_key1_pressed && count < 6) {
        keycodes[count++] = s_key1_code;
    }
    if (s_key2_pressed && count < 6) {
        keycodes[count++] = s_key2_code;
    }
    if (s_key3_pressed && count < 6) {
        keycodes[count++] = s_key3_code;
    }

    return tud_hid_ready() && tud_hid_keyboard_report(0, 0, keycodes);
}

// Submits the current state. On success marks every change up to the one
// loaded here as delivered; on failure they stay pending and the completion of
// the transfer in flight resends them.
static bool try_submit(void)
{
    unsigned seq = atomic_load(&s_change_seq);
    if (!submit_current_state()) {
        return false;
    }
    unsigned sent = atomic_load(&s_sent_seq);
    while (!seq_reached(sent, seq) && !atomic_compare_exchange_weak(&s_sent_seq, &sent, seq)) {
    }
    return true;
}

static bool change_pending(void)
{
    return !seq_reached(atomic_load(&s_sent_seq), atomic_load(&s_change_seq));
}

void usb_hid_set_touch_retry(bool pressed)
{
    if (s_key3_pressed == pressed) {
        return;
    }
    s_key3_pressed = pressed;
    atomic_fetch_add(&s_change_seq, 1);
    try_submit();
}

bool IRAM_ATTR usb_hid_handle_key_event(uint8_t key_index, bool pressed, int64_t edge_us)
{
    if (key_index == 0) {
        s_key1_pressed = pressed;
    } else if (key_index == 1) {
        s_key2_pressed = pressed;
    }

    // Counted before trying, so a completion racing with this submit still resends
    unsigned seq = atomic_fetch_add(&s_change_seq, 1) + 1;
    if (try_submit()) {
        // This report also carries any earlier change that was waiting
        record_pending_latency();
        return true;
    }
    int64_t none = 0;
    if (atomic_compare_exchange_strong(&s_pending_edge_us, &none, edge_us) &&
        seq_reached(atomic_load(&s_sent_seq), seq)) {
        // A completion delivered this change between the failed submit and
        // arming the edge: take it back now rather than let it age into the
        // next report's latency. Whoever clears it records it, once.
        int64_t armed = edge_us;
        if (atomic_compare_exchange_strong(&s_pending_edge_us, &armed, 0)) {
            latency_stats_record((uint32_t)(esp_timer_get_time() - edge_us));
        }
    }
    return false;
}

// Runs in the TinyUSB task when the previous report reached the host. A change that
// arrived while the endpoint was busy is sent now instead of being lost (stuck key).
void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint16_t len)
{
    (void)instance;
    (void)report;
    (void)len;
    // On failure another transfer is in flight, and its completion retries
    if (change_pending() && try_submit()) {
        record_pending_latency();
    }
}
