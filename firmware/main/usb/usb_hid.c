#include "usb_hid.h"
#include "usb_descriptors.h"
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
static uint8_t s_key3_code = 0x06; // 'c' (Hall Effect V3's third key)
static uint8_t s_retry_code = 0x35; // '`' / '~' (osu! Quick Retry, the touchscreen)

/*
 * Two keyboard reports, each on its own endpoint: key 1 and the touchscreen
 * key on the keyboard interface, key 2 on the media interface. Key 3 (HE V3)
 * rides on the endpoint the other game key does not use (usb_hid_set_key3_link):
 * playing Z/C it goes with key 2, X/C with key 1, so two game keys never share
 * an endpoint. A report can
 * only be submitted once the host has collected the one before it on the same
 * endpoint (up to 1 ms); with one endpoint, K2 changing within 1 ms of K1 in a
 * stream waited for that. Now a key only waits on its own previous report
 * (at least one debounce lockout old) or, for key 2, a swipe's media report.
 *
 * Who touches what:
 * - s_key1/2_pressed: written by the keypad task (core 0), s_touch_code and
 *   s_touch_mods by the touch task (core 1). Each has that one writer; every
 *   submitter reads them all. During a map the touch slot only ever holds
 *   Quick Retry; Alt and swipe keys are sent outside gameplay only.
 * Per report (hid_link_t):
 * - change_seq: bumped by its writers after they change a key, so a submit
 *   that loaded it before reading the keys carries at least that change.
 * - sent_seq: the newest change_seq a submitted report carried, raised by
 *   whichever submit succeeds (keypad task, touch task or the TinyUSB task's
 *   completion). A change is pending while sent_seq is behind change_seq.
 *   Unlike a pending flag, one submitter cannot clear another's change.
 * - pending_edge_us: armed only by the keypad task, taken by whoever delivers.
 */
static volatile bool s_key1_pressed = false;
static volatile bool s_key2_pressed = false;
static volatile bool s_key3_pressed = false;
// Key the touchscreen holds down (0 = none), and the modifiers with it
static volatile uint8_t s_touch_code = 0;
static volatile uint8_t s_touch_mods = 0;

typedef struct {
    uint8_t instance;
    uint8_t report_id;
    atomic_uint change_seq;
    atomic_uint sent_seq;
    // Edge time of the oldest key change not yet delivered, for latency stats (0 = none)
    atomic_llong pending_edge_us;
} hid_link_t;

enum { LINK_KEY1_TOUCH = 0, LINK_KEY2, LINK_COUNT };

static hid_link_t s_links[LINK_COUNT] = {
    [LINK_KEY1_TOUCH] = {.instance = HID_INSTANCE_KEYBOARD, .report_id = 0},
    [LINK_KEY2] = {.instance = HID_INSTANCE_MEDIA, .report_id = MEDIA_REPORT_ID_KEYBOARD},
};
// Which report carries key 3 (written by the protocol task, read by submitters)
static volatile uint8_t s_key3_link = LINK_KEY2;

// a is at or past b, with wrap-around
static inline bool seq_reached(unsigned a, unsigned b)
{
    return (int)(a - b) >= 0;
}

static void record_pending_latency(hid_link_t *link)
{
    int64_t edge = atomic_exchange(&link->pending_edge_us, 0);
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

    ESP_LOGI(TAG, "USB HID keyboard initialized (Key1: 0x%02X, Key2: 0x%02X, Key3: 0x%02X, TouchRetry: 0x%02X)",
             s_key1_code, s_key2_code, s_key3_code, s_retry_code);
    return ESP_OK;
}

bool usb_hid_is_ready(void)
{
    return tud_hid_n_ready(HID_INSTANCE_KEYBOARD);
}

static bool try_submit(hid_link_t *link);

void usb_hid_set_keycodes(uint8_t key1_code, uint8_t key2_code)
{
    s_key1_code = key1_code;
    s_key2_code = key2_code;
}

void usb_hid_set_key3(uint8_t key3_code, uint8_t game_keys)
{
    // Key 3 rides with key 2 unless key 2 is a game key and key 1 is not
    uint8_t link = (game_keys & 0x2) && !(game_keys & 0x1) ? LINK_KEY1_TOUCH : LINK_KEY2;
    if (key3_code == s_key3_code && link == s_key3_link) {
        return;
    }
    uint8_t old = s_key3_link;
    s_key3_code = key3_code;
    s_key3_link = link;
    if (s_key3_pressed) {
        // Held across the change: both reports get resent with it moved
        for (uint8_t i = 0; i < LINK_COUNT; i++) {
            if (i == old || i == link) {
                atomic_fetch_add(&s_links[i].change_seq, 1);
                try_submit(&s_links[i]);
            }
        }
    }
}

void usb_hid_set_touch_retry_code(uint8_t code)
{
    s_retry_code = code;
}

static bool submit_current_state(hid_link_t *link)
{
    uint8_t keycodes[6] = {0};
    uint8_t mods = 0;

    uint8_t count = 0;
    bool key3_here = s_key3_pressed && s_key3_code && link == &s_links[s_key3_link];
    if (link == &s_links[LINK_KEY2]) {
        if (s_key2_pressed) {
            keycodes[count++] = s_key2_code;
        }
        if (key3_here) {
            keycodes[count++] = s_key3_code;
        }
    } else {
        if (s_key1_pressed) {
            keycodes[count++] = s_key1_code;
        }
        if (key3_here) {
            keycodes[count++] = s_key3_code;
        }
        uint8_t touch_code = s_touch_code;
        if (touch_code) {
            keycodes[count++] = touch_code;
        }
        mods = s_touch_mods;
    }

    return tud_hid_n_ready(link->instance) &&
           tud_hid_n_keyboard_report(link->instance, link->report_id, mods, keycodes);
}

// Submits the link's current state. On success marks every change up to the
// one loaded here as delivered; on failure they stay pending and the
// completion of the transfer in flight on that endpoint resends them.
static bool try_submit(hid_link_t *link)
{
    unsigned seq = atomic_load(&link->change_seq);
    if (!submit_current_state(link)) {
        return false;
    }
    unsigned sent = atomic_load(&link->sent_seq);
    while (!seq_reached(sent, seq) && !atomic_compare_exchange_weak(&link->sent_seq, &sent, seq)) {
    }
    return true;
}

static bool change_pending(hid_link_t *link)
{
    return !seq_reached(atomic_load(&link->sent_seq), atomic_load(&link->change_seq));
}

void usb_hid_set_touch_key(uint8_t keycode, uint8_t modifiers)
{
    if (s_touch_code == keycode && s_touch_mods == modifiers) {
        return;
    }
    s_touch_code = keycode;
    s_touch_mods = modifiers;
    hid_link_t *link = &s_links[LINK_KEY1_TOUCH];
    atomic_fetch_add(&link->change_seq, 1);
    try_submit(link);
}

void usb_hid_set_touch_retry(bool pressed)
{
    usb_hid_set_touch_key(pressed ? s_retry_code : 0, 0);
}

bool usb_hid_touch_key_delivered(void)
{
    hid_link_t *link = &s_links[LINK_KEY1_TOUCH];
    return !change_pending(link) && tud_hid_n_ready(link->instance);
}

bool IRAM_ATTR usb_hid_handle_key_event(uint8_t key_index, bool pressed, int64_t edge_us)
{
    hid_link_t *link;
    if (key_index == 2) {
        s_key3_pressed = pressed;
        link = &s_links[s_key3_link];
    } else if (key_index == 1) {
        s_key2_pressed = pressed;
        link = &s_links[LINK_KEY2];
    } else {
        s_key1_pressed = pressed;
        link = &s_links[LINK_KEY1_TOUCH];
    }

    // Counted before trying, so a completion racing with this submit still resends
    unsigned seq = atomic_fetch_add(&link->change_seq, 1) + 1;
    if (try_submit(link)) {
        // This report also carries any earlier change that was waiting
        record_pending_latency(link);
        return true;
    }
    if (!tud_ready()) {
        // Unconfigured or suspended: the wait is the host's, not the pad's, so
        // no latency sample. usb_hid_resync sends it once the host is back.
        return false;
    }
    int64_t none = 0;
    if (atomic_compare_exchange_strong(&link->pending_edge_us, &none, edge_us) &&
        seq_reached(atomic_load(&link->sent_seq), seq)) {
        // A completion delivered this change between the failed submit and
        // arming the edge: take it back now rather than let it age into the
        // next report's latency. Whoever clears it records it, once.
        int64_t armed = edge_us;
        if (atomic_compare_exchange_strong(&link->pending_edge_us, &armed, 0)) {
            latency_stats_record((uint32_t)(esp_timer_get_time() - edge_us));
        }
    }
    return false;
}

// Runs in the TinyUSB task when the host configures the device or resumes it.
// A change made while it read nothing had no transfer in flight whose
// completion would resend it, so it waited for that key's next event: a key
// released then stayed down on the host. Every link resends its state now.
void usb_hid_resync(void)
{
    for (int i = 0; i < LINK_COUNT; i++) {
        hid_link_t *link = &s_links[i];
        // Armed before the host left, for a change it never read: not a latency
        atomic_store(&link->pending_edge_us, 0);
        atomic_fetch_add(&link->change_seq, 1);
        try_submit(link);
    }
}

// Runs in the TinyUSB task when the previous report reached the host. A change that
// arrived while the endpoint was busy is sent now instead of being lost (stuck key).
// On the media interface the report done may be a swipe's, which frees the
// endpoint for key 2 just the same.
void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint16_t len)
{
    (void)report;
    (void)len;
    for (int i = 0; i < LINK_COUNT; i++) {
        hid_link_t *link = &s_links[i];
        // On failure another transfer is in flight, and its completion retries
        if (link->instance == instance && change_pending(link) && try_submit(link)) {
            record_pending_latency(link);
        }
    }
}
