// usb_hid.c against a TinyUSB stand-in: a change made while the host reads
// nothing (unconfigured or suspended) is resent when it comes back, and its
// wait is not taken for key latency.
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "usb/usb_hid.h"
#include "usb/usb_descriptors.h"
#include "input/keypad.h"

static bool host_ready;              // configured and not suspended
static bool busy[2];                 // a report in flight per HID instance
static uint8_t last_keys[2][6];      // last report sent per instance
static int reports;
static int64_t now_us;
static int samples;
static uint32_t last_sample;

bool tud_ready(void) { return host_ready; }
bool tud_hid_n_ready(uint8_t instance) { return host_ready && !busy[instance]; }
bool tud_hid_n_keyboard_report(uint8_t instance, uint8_t report_id, uint8_t modifier, const uint8_t keycode[6])
{
    (void)report_id;
    (void)modifier;
    busy[instance] = true;
    memcpy(last_keys[instance], keycode, 6);
    reports++;
    return true;
}
int64_t esp_timer_get_time(void) { return now_us; }
void latency_stats_record(uint32_t latency_us) { samples++; last_sample = latency_us; }
void keypad_get_config(keypad_config_t *out) { memset(out, 0, sizeof(*out)); out->keycode1 = 0x1d; out->keycode2 = 0x1b; }

// The host takes the transfer in flight on an instance
static void host_polls(uint8_t instance)
{
    if (busy[instance]) {
        busy[instance] = false;
        tud_hid_report_complete_cb(instance, NULL, 0);
    }
}

static void reset(bool ready)
{
    host_ready = ready;
    busy[0] = busy[1] = false;
    memset(last_keys, 0, sizeof(last_keys));
    reports = samples = 0;
}

int main(void)
{
    usb_hid_init();
    const uint8_t K1 = 0x1d; // keycode1 from keypad_get_config

    // 1. Pressed before the host configured the pad: sent at mount, no sample
    reset(false);
    now_us = 1000;
    assert(!usb_hid_handle_key_event(0, true, now_us));
    assert(reports == 0);
    now_us += 350000;
    host_ready = true;
    usb_hid_resync();
    assert(last_keys[0][0] == K1);
    assert(samples == 0);
    host_polls(0);
    host_polls(1);
    // The next change goes out at once (key_event.c measures that one) and
    // carries no stale wait from before the mount
    now_us += 1000;
    assert(usb_hid_handle_key_event(0, false, now_us));
    assert(last_keys[0][0] == 0 && samples == 0);
    host_polls(0);

    // 2. Released while suspended: the host learns at resume (no stuck key)
    reset(true);
    assert(usb_hid_handle_key_event(0, true, now_us));
    assert(last_keys[0][0] == K1);
    host_polls(0);
    host_ready = false; // suspended
    int before = reports;
    assert(!usb_hid_handle_key_event(0, false, now_us));
    assert(reports == before);
    now_us += 5000000;
    host_ready = true; // resumed
    int s = samples;
    usb_hid_resync();
    assert(last_keys[0][0] == 0);
    assert(samples == s);
    host_polls(0);
    host_polls(1);

    // 3. Endpoint busy while the host reads: resent on completion and measured
    reset(true);
    now_us = 10000000;
    assert(usb_hid_handle_key_event(1, true, now_us));
    host_polls(1);
    assert(usb_hid_handle_key_event(1, false, now_us)); // in flight now
    assert(!usb_hid_handle_key_event(1, true, now_us));  // waits for it
    now_us += 800;
    host_polls(1);
    assert(last_keys[1][0] == 0x1b);
    assert(samples == 1 && last_sample == 800);

    printf("✓ usb_hid resync and latency tests passed\n");
    return 0;
}
