#include "usb_hid.h"
#include "input/keypad.h"
#include "tusb.h"
#include "class/hid/hid_device.h"
#include "esp_log.h"
#include <stdatomic.h>

static const char *TAG = "usb_hid";

static uint8_t s_key1_code = 0x1D; // 'z'
static uint8_t s_key2_code = 0x1B; // 'x'

static volatile bool s_key1_pressed = false;
static volatile bool s_key2_pressed = false;
// Set when a state change could not be submitted (endpoint busy); resent on completion
static atomic_bool s_report_pending = false;

esp_err_t usb_hid_init(void)
{
    keypad_config_t cfg;
    keypad_get_config(&cfg);
    s_key1_code = cfg.keycode1;
    s_key2_code = cfg.keycode2;

    keypad_set_state_callback(usb_hid_handle_key_event);
    ESP_LOGI(TAG, "USB HID keyboard initialized (Key1: 0x%02X, Key2: 0x%02X)", s_key1_code, s_key2_code);
    return ESP_OK;
}

bool usb_hid_is_ready(void)
{
    return tud_hid_ready();
}

bool usb_hid_send_keyboard_report(uint8_t modifier, const uint8_t keycodes[6])
{
    if (!tud_hid_ready()) {
        return false;
    }
    return tud_hid_keyboard_report(0, modifier, (uint8_t *)keycodes);
}

void usb_hid_set_keycodes(uint8_t key1_code, uint8_t key2_code)
{
    s_key1_code = key1_code;
    s_key2_code = key2_code;
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

    return tud_hid_ready() && tud_hid_keyboard_report(0, 0, keycodes);
}

bool IRAM_ATTR usb_hid_handle_key_event(uint8_t key_index, bool pressed)
{
    if (key_index == 0) {
        s_key1_pressed = pressed;
    } else if (key_index == 1) {
        s_key2_pressed = pressed;
    }

    // Mark pending before trying, so a completion racing with this submit still resends
    atomic_store(&s_report_pending, true);
    if (submit_current_state()) {
        atomic_store(&s_report_pending, false);
        return true;
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
    if (atomic_exchange(&s_report_pending, false) && !submit_current_state()) {
        // Still busy: another transfer is in flight, its completion retries
        atomic_store(&s_report_pending, true);
    }
}
