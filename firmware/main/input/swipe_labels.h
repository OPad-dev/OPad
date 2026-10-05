#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// The icon a swipe's feedback shows; the UI turns it into an LVGL symbol
typedef enum {
    SWIPE_ICON_KEYBOARD = 0,
    SWIPE_ICON_SHUFFLE,
    SWIPE_ICON_LIST,
    SWIPE_ICON_BARS,
    SWIPE_ICON_LEFT,
    SWIPE_ICON_RIGHT,
    SWIPE_ICON_UP,
    SWIPE_ICON_DOWN,
    SWIPE_ICON_DIRECTORY,
    SWIPE_ICON_BACKSPACE,
    SWIPE_ICON_OK,
    SWIPE_ICON_CLOSE,
    SWIPE_ICON_ENVELOPE,
    SWIPE_ICON_BELL,
    SWIPE_ICON_SETTINGS,
    SWIPE_ICON_IMAGE,
    SWIPE_ICON_VIDEO,
    SWIPE_ICON_AUDIO,
    SWIPE_ICON_DOWNLOAD,
    SWIPE_ICON_HOME,
    SWIPE_ICON_EYE_OPEN,
    SWIPE_ICON_EYE_CLOSE,
    SWIPE_ICON_TINT,
    SWIPE_ICON_REFRESH,
} swipe_icon_t;

typedef struct {
    uint8_t key;        // HID usage
    uint8_t mods;       // HID modifier bits
    swipe_icon_t icon;
    const char *label;
} swipe_label_t;

/*
 * What a swipe's keyboard key does, for the feedback on the pad: osu!'s
 * default shortcuts by name. Must list every shortcut in the app's
 * OSU_SHORTCUTS (desktop/crates/opad-model/src/swipe.rs); a test there reads
 * this file to check.
 */
extern const swipe_label_t SWIPE_LABELS[];
extern const unsigned SWIPE_LABEL_COUNT;

/**
 * @brief The label for a key and modifiers. A key osu! does not bind gets the
 *        keyboard icon and its name ("F15"), written to @p buf.
 */
swipe_label_t swipe_label_for(uint8_t key, uint8_t mods, char *buf, unsigned buf_len);

#ifdef __cplusplus
}
#endif
