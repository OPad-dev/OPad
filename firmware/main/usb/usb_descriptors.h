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
    ITF_NUM_TOTAL
};

enum {
    STRID_LANGID = 0,
    STRID_MANUFACTURER,
    STRID_PRODUCT,
    STRID_SERIAL,
    STRID_HID,
    STRID_CDC,
};

extern const tusb_desc_device_t osupad_usb_device_desc;
extern const uint8_t osupad_usb_config_desc[];
extern const uint8_t osupad_hid_report_desc[];
extern const char *osupad_usb_string_desc[];

#ifdef __cplusplus
}
#endif
