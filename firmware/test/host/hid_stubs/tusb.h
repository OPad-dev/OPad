#pragma once
// Host-test stand-in for TinyUSB: just what usb_hid.c calls (test_usb_hid.c)
#include <stdbool.h>
#include <stdint.h>
typedef struct { uint8_t unused; } tusb_desc_device_t;
bool tud_ready(void);
bool tud_hid_n_ready(uint8_t instance);
bool tud_hid_n_keyboard_report(uint8_t instance, uint8_t report_id, uint8_t modifier, const uint8_t keycode[6]);
void tud_hid_report_complete_cb(uint8_t instance, uint8_t const *report, uint16_t len);
