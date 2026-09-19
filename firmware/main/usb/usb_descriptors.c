#include "usb_descriptors.h"
#include "esp_log.h"
#include "esp_mac.h"
#include <stdio.h>
#include <string.h>

static char s_serial_str[32] = "OSUPAD-000000000000";

void usb_descriptors_init(void)
{
    uint8_t mac[6] = {0};
    esp_read_mac(mac, ESP_MAC_WIFI_STA);
    snprintf(s_serial_str, sizeof(s_serial_str), "OSUPAD-%02X%02X%02X%02X%02X%02X",
             mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

#define CONFIG_TOTAL_LEN (TUD_CONFIG_DESC_LEN + TUD_HID_DESC_LEN + TUD_CDC_DESC_LEN)

const tusb_desc_device_t osupad_usb_device_desc = {
    .bLength            = sizeof(tusb_desc_device_t),
    .bDescriptorType    = TUSB_DESC_DEVICE,
    .bcdUSB             = 0x0210,   // USB 2.1 (required for BOS & MS OS 2.0 descriptors)
    .bDeviceClass       = TUSB_CLASS_MISC,
    .bDeviceSubClass    = MISC_SUBCLASS_COMMON,
    .bDeviceProtocol    = MISC_PROTOCOL_IAD,
    .bMaxPacketSize0    = CFG_TUD_ENDPOINT0_SIZE,
    .idVendor           = 0x303A,   // Espressif VID
    .idProduct          = 0x4001,   // Custom PID for OPad
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
    "OPad ESP32-S3",                // 2: Product
    s_serial_str,                   // 3: Serial (runtime MAC-derived)
    "OPad HID Keyboard",            // 4: HID Interface
    "OPad CDC Telemetry",           // 5: CDC Interface
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

#define VENDOR_REQUEST_MICROSOFT 0x01

// Binary Device Object Store (BOS) Descriptor exposing MS OS 2.0 platform capability
const uint8_t osupad_usb_bos_desc[] = {
    // BOS descriptor header: bLength (5), bDescriptorType (0x0F = TUSB_DESC_BOS), wTotalLength (33 bytes = 0x0021), bNumDeviceCaps (1)
    0x05, TUSB_DESC_BOS, 0x21, 0x00, 0x01,

    // Microsoft OS 2.0 Platform Capability Descriptor (28 bytes)
    0x1C, TUSB_DESC_DEVICE_CAPABILITY, 0x05, 0x00,
    // MS OS 2.0 Platform Capability UUID: {D8DD60DF-4589-4CC7-9CD2-659D9E648A9F}
    0xDF, 0x60, 0xDD, 0xD8, 0x89, 0x45, 0xC7, 0x4C, 0x9C, 0xD2, 0x65, 0x9D, 0x9E, 0x64, 0x8A, 0x9F,
    // dwWindowsVersion: 0x06030000 (Windows 8.1+)
    0x00, 0x00, 0x03, 0x06,
    // wMSOSDescriptorSetTotalLength: 72 bytes (0x0048)
    0x48, 0x00,
    // bMS_VendorCode
    VENDOR_REQUEST_MICROSOFT,
    // bAltEnumCode
    0x00
};

// Microsoft OS 2.0 Descriptor Set (Total 72 bytes)
// Assigns FriendlyName = "OPad" to Interface 1 (CDC-ACM) so Windows Device Manager
// displays "OPad (COMx)" instead of the generic "USB Serial Device (COMx)".
const uint8_t osupad_usb_ms_os_20_desc[] = {
    // Set Header: wLength (10), wDescriptorType (0x0000), dwWindowsVersion (0x06030000), wTotalLength (72 = 0x0048)
    0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x06, 0x48, 0x00,

    // Configuration Subset Header: wLength (8), wDescriptorType (0x0001), bConfigurationValue (0), bReserved (0), wTotalLength (62 = 0x003E)
    0x08, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3E, 0x00,

    // Function Subset Header: wLength (8), wDescriptorType (0x0002), bFirstInterface (1 = ITF_NUM_CDC), bReserved (0), wSubsetLength (54 = 0x0036)
    0x08, 0x00, 0x02, 0x00, ITF_NUM_CDC, 0x00, 0x36, 0x00,

    // Registry Property Feature Descriptor: wLength (46), wDescriptorType (0x0004), wPropertyDataType (0x0001 = REG_SZ)
    0x2E, 0x00, 0x04, 0x00, 0x01, 0x00,
    // wPropertyNameLength (26 bytes = 13 UTF-16LE characters including null terminator)
    0x1A, 0x00,
    // PropertyName: "FriendlyName" in UTF-16LE
    'F', 0x00, 'r', 0x00, 'i', 0x00, 'e', 0x00, 'n', 0x00, 'd', 0x00,
    'l', 0x00, 'y', 0x00, 'N', 0x00, 'a', 0x00, 'm', 0x00, 'e', 0x00, 0x00, 0x00,
    // wPropertyDataLength (10 bytes = 5 UTF-16LE characters including null terminator)
    0x0A, 0x00,
    // PropertyData: "OPad" in UTF-16LE
    'O', 0x00, 'P', 0x00, 'a', 0x00, 'd', 0x00, 0x00, 0x00
};

uint8_t const *tud_descriptor_bos_cb(void)
{
    return osupad_usb_bos_desc;
}

bool tud_vendor_control_xfer_cb(uint8_t rhport, uint8_t stage, tusb_control_request_t const *request)
{
    if (stage != CONTROL_STAGE_SETUP) {
        return true;
    }

    if (request->bmRequestType_bit.type == TUSB_REQ_TYPE_VENDOR &&
        request->bRequest == VENDOR_REQUEST_MICROSOFT &&
        request->wIndex == 7) {
        return tud_control_xfer(rhport, request, (void *)(uintptr_t)osupad_usb_ms_os_20_desc, sizeof(osupad_usb_ms_os_20_desc));
    }

    return false;
}

