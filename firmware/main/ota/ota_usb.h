#pragma once
// Firmware update over the protocol connection (OtaBegin / OtaChunk / OtaEnd,
// protocol/opad.proto): the new image goes to the app slot not running while
// the pad stays a keyboard, then boots once on trial, as after any flash.
// Protocol task only (core 1).
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "opad.pb.h"

#ifdef __cplusplus
extern "C" {
#endif

// Largest OtaChunk.data accepted (HelloAck.ota_chunk_max); the nanopb limit
#define OTA_USB_CHUNK_MAX 4096

typedef struct {
    opad_OtaStage stage;
    uint32_t written;
    const char *message;  // static text, NULL when there is nothing to say
} ota_usb_reply_t;

ota_usb_reply_t ota_usb_begin(uint32_t image_size, const uint8_t *sha256, size_t sha256_len);
ota_usb_reply_t ota_usb_chunk(uint32_t offset, const uint8_t *data, size_t len);
// DONE: the new slot boots next, and the pad restarts once its state is saved
ota_usb_reply_t ota_usb_end(void);
void ota_usb_abort(void);

// Call often. A transfer the host went quiet on, or that a map started
// during, is given up: true, with the ERROR to send in *out.
bool ota_usb_poll(ota_usb_reply_t *out);

bool ota_usb_active(void);

#ifdef __cplusplus
}
#endif
