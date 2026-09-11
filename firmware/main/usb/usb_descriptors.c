#include "usb_descriptors.h"
#include "esp_log.h"
#include <string.h>

#define CONFIG_TOTAL_LEN (TUD_CONFIG_DESC_LEN + TUD_HID_DESC_LEN + TUD_CDC_DESC_LEN)

const tusb_desc_device_t osupad_usb_device_desc = {
    .bLength            = sizeof(tusb_desc_device_t),
    .bDescriptorType    = TUSB_DESC_DEVICE,
    .bcdUSB             = 0x0200,
    .bDeviceClass       = TUSB_CLASS_MISC,
    .bDeviceSubClass    = MISC_SUBCLASS_COMMON,
    .bDeviceProtocol    = MISC_PROTOCOL_IAD,
    .bMaxPacketSize0    = CFG_TUD_ENDPOINT0_SIZE,
    .idVendor           = 0x303A,   // Espressif VID
    .idProduct          = 0x4001,   // Custom PID for osu!pad
    .bcdDevice          = 0x0100,   // Version 1.0.0
    .iManufacturer      = STRID_MANUFACTURER,
    .iProduct           = STRID_PRODUCT,
    .iSerialNumber      = STRID_SERIAL,
    .bNumConfigurations = 0x01
};

const uint8_t osupad_hid_report_desc[] = {
    TUD_HID_REPORT_DESC_KEYBOARD()
};

const uint8_t osupad_usb_config_desc[] = {
    // Config number, interface count, string index, total length, attribute, power in mA
    TUD_CONFIG_DESCRIPTOR(1, ITF_NUM_TOTAL, 0, CONFIG_TOTAL_LEN, 0, 500),

    // Interface 0: HID Keyboard (polling rate = 1ms -> 1000 Hz USB polling)
    TUD_HID_DESCRIPTOR(ITF_NUM_HID, STRID_HID, HID_ITF_PROTOCOL_KEYBOARD,
                       sizeof(osupad_hid_report_desc), 0x81, 8, 1),

    // Interface 1 & 2: CDC-ACM (Notification EP: 0x82, Data OUT: 0x03, Data IN: 0x83)
    TUD_CDC_DESCRIPTOR(ITF_NUM_CDC, STRID_CDC, 0x82, 8, 0x03, 0x83, 64),
};

const char *osupad_usb_string_desc[] = {
    (const char[]) { 0x09, 0x04 },  // 0: Supported language (English 0x0409)
    "GFerreiroS",                   // 1: Manufacturer
    "osu!pad ESP32-S3",             // 2: Product
    "OSUPAD-S3-0001",               // 3: Serial
    "osu!pad HID Keyboard",         // 4: HID Interface
    "osu!pad CDC Telemetry",        // 5: CDC Interface
};


uint8_t const *tud_hid_descriptor_report_cb(uint8_t instance)
{
    (void)instance;
    return osupad_hid_report_desc;
}

uint16_t tud_hid_get_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type, uint8_t *buffer, uint16_t reqlen)
{
    (void)instance;
    (void)report_id;
    (void)report_type;
    (void)buffer;
    (void)reqlen;
    return 0;
}

void tud_hid_set_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type, uint8_t const *buffer, uint16_t bufsize)
{
    (void)instance;
    (void)report_id;
    (void)report_type;
    (void)buffer;
    (void)bufsize;
}
