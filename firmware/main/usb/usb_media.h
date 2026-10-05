#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Consumer page usages (HID Usage Tables, 0x0C)
#define USB_MEDIA_VOLUME_UP     0x00E9
#define USB_MEDIA_VOLUME_DOWN   0x00EA
#define USB_MEDIA_MUTE          0x00E2
#define USB_MEDIA_PLAY_PAUSE    0x00CD
#define USB_MEDIA_NEXT_TRACK    0x00B5
#define USB_MEDIA_PREV_TRACK    0x00B6

/*
 * The media HID interface: what touchscreen swipes send. Separate from the
 * keyboard interface and its endpoint, so none of this ever delays a key
 * report. Every call may wait a few ms for the endpoint: touch task only.
 */

/**
 * @brief Press and release one consumer control (volume, media keys).
 */
bool usb_media_consumer_tap(uint16_t usage);

/**
 * @brief Scroll the wheel by @p steps notches (positive = up).
 */
bool usb_media_wheel(int8_t steps);

/**
 * @brief Waits up to @p timeout_ms for the media endpoint to be free, which
 * also means the last report reached the host.
 */
bool usb_media_wait_ready(uint32_t timeout_ms);

#ifdef __cplusplus
}
#endif
