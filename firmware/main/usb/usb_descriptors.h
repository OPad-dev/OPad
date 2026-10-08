#pragma once

#include <stdint.h>
#include "tusb.h"
#include "class/hid/hid_device.h"
#include "class/cdc/cdc_device.h"

#ifdef __cplusplus
extern "C" {
#endif

enum {
    ITF_NUM_HID = 0,
    ITF_NUM_CDC,
    ITF_NUM_CDC_DATA,
    // After CDC, so the keyboard and COM port keep the interface numbers they
    // had before it existed
    ITF_NUM_MEDIA,
    ITF_NUM_TOTAL
};

enum {
    STRID_LANGID = 0,
    STRID_MANUFACTURER,
    STRID_PRODUCT,
    STRID_SERIAL,
    STRID_HID,
    STRID_CDC,
    STRID_MEDIA,
    STRID_COUNT
};

// TinyUSB numbers HID instances in interface order
enum {
    HID_INSTANCE_KEYBOARD = 0,
    HID_INSTANCE_MEDIA = 1,
};

// Report ids on the media interface. Key 2 has its own keyboard report here,
// so each key has an endpoint of its own (see usb_hid.c)
enum {
    MEDIA_REPORT_ID_CONSUMER = 1,
    MEDIA_REPORT_ID_MOUSE = 2,
    MEDIA_REPORT_ID_KEYBOARD = 3,
};

extern const tusb_desc_device_t osupad_usb_device_desc;
extern const uint8_t osupad_usb_config_desc[];
extern const uint8_t osupad_hid_report_desc[];
extern const uint8_t osupad_media_report_desc[];
extern const char *osupad_usb_string_desc[];
extern const uint8_t osupad_usb_bos_desc[];
extern const uint8_t osupad_usb_ms_os_20_desc[];

void usb_descriptors_init(void);

#ifdef __cplusplus
}
#endif
