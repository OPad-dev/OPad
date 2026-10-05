#include "usb_media.h"
#include "usb_descriptors.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "tusb.h"
#include "class/hid/hid_device.h"
#include "esp_log.h"

static const char *TAG = "usb_media";

// A press waits at most this long for the endpoint
#define MEDIA_PRESS_WAIT_MS     30
// A release waits longer: a lost one leaves the host repeating volume up
#define MEDIA_RELEASE_WAIT_MS   250

bool usb_media_wait_ready(uint32_t timeout_ms)
{
    for (uint32_t waited = 0;; waited++) {
        if (!tud_mounted()) {
            return false;
        }
        if (tud_hid_n_ready(HID_INSTANCE_MEDIA)) {
            return true;
        }
        if (waited >= timeout_ms) {
            return false;
        }
        vTaskDelay(pdMS_TO_TICKS(1));
    }
}

bool usb_media_consumer_tap(uint16_t usage)
{
    if (!usb_media_wait_ready(MEDIA_PRESS_WAIT_MS) ||
        !tud_hid_n_report(HID_INSTANCE_MEDIA, MEDIA_REPORT_ID_CONSUMER, &usage, sizeof(usage))) {
        return false;
    }
    const uint16_t none = 0;
    if (!usb_media_wait_ready(MEDIA_RELEASE_WAIT_MS) ||
        !tud_hid_n_report(HID_INSTANCE_MEDIA, MEDIA_REPORT_ID_CONSUMER, &none, sizeof(none))) {
        ESP_LOGW(TAG, "Consumer control 0x%03X release not sent", usage);
        return false;
    }
    return true;
}

bool usb_media_wheel(int8_t steps)
{
    // A wheel report is relative: nothing to release afterwards
    return usb_media_wait_ready(MEDIA_PRESS_WAIT_MS) &&
           tud_hid_n_mouse_report(HID_INSTANCE_MEDIA, MEDIA_REPORT_ID_MOUSE, 0, 0, 0, steps, 0);
}
