#include "swipe_labels.h"
#include <stdio.h>

#define MOD_CTRL    0x01
#define MOD_SHIFT   0x02
#define MOD_ALT     0x04

// One row per shortcut: {key, mods, icon, label}. Labels fit the swipe box
// on the 320 px screen at 24 px.
const swipe_label_t SWIPE_LABELS[] = {
    // Song select
    {0x3B, 0, SWIPE_ICON_SHUFFLE, "Random"},                       // F2
    {0x3B, MOD_SHIFT, SWIPE_ICON_SHUFFLE, "Prev random"},          // Shift+F2
    {0x3A, 0, SWIPE_ICON_LIST, "Mods"},                            // F1
    {0x3C, 0, SWIPE_ICON_BARS, "Options"},                         // F3
    {0x50, 0, SWIPE_ICON_LEFT, "Prev map"},                        // Left
    {0x4F, 0, SWIPE_ICON_RIGHT, "Next map"},                       // Right
    {0x52, 0, SWIPE_ICON_UP, "Prev diff"},                         // Up
    {0x51, 0, SWIPE_ICON_DOWN, "Next diff"},                       // Down
    {0x50, MOD_SHIFT, SWIPE_ICON_DIRECTORY, "Prev group"},         // Shift+Left
    {0x4F, MOD_SHIFT, SWIPE_ICON_DIRECTORY, "Next group"},         // Shift+Right
    {0x2A, 0, SWIPE_ICON_BACKSPACE, "Clear mods"},                 // Backspace
    // Anywhere
    {0x28, 0, SWIPE_ICON_OK, "Select"},                            // Enter
    {0x29, 0, SWIPE_ICON_CLOSE, "Back"},                           // Esc
    {0x41, 0, SWIPE_ICON_ENVELOPE, "Chat"},                        // F8
    {0x42, 0, SWIPE_ICON_ENVELOPE, "Social"},                      // F9
    {0x12, MOD_CTRL, SWIPE_ICON_SETTINGS, "Settings"},             // Ctrl+O
    {0x45, 0, SWIPE_ICON_IMAGE, "Screenshot"},                     // F12
    {0x43, 0, SWIPE_ICON_KEYBOARD, "Mouse btns"},                  // F10
    {0x28, MOD_ALT, SWIPE_ICON_VIDEO, "Fullscreen"},               // Alt+Enter
    {0x3F, 0, SWIPE_ICON_AUDIO, "Now playing"},                    // F6
    {0x17, MOD_CTRL, SWIPE_ICON_BARS, "Toolbar"},                  // Ctrl+T
    {0x05, MOD_CTRL, SWIPE_ICON_DOWNLOAD, "Beatmaps"},             // Ctrl+B
    {0x11, MOD_CTRL, SWIPE_ICON_BELL, "Notifications"},            // Ctrl+N
    {0x13, MOD_CTRL, SWIPE_ICON_EYE_OPEN, "Profile"},              // Ctrl+P
    {0x4A, MOD_ALT, SWIPE_ICON_HOME, "Main menu"},                 // Alt+Home
    {0x15, MOD_CTRL | MOD_SHIFT, SWIPE_ICON_SHUFFLE, "Random skin"}, // Ctrl+Shift+R
    {0x08, MOD_CTRL | MOD_SHIFT, SWIPE_ICON_TINT, "Prev skin"},    // Ctrl+Shift+E
    {0x17, MOD_CTRL | MOD_SHIFT, SWIPE_ICON_TINT, "Next skin"},    // Ctrl+Shift+T
    {0x49, 0, SWIPE_ICON_EYE_CLOSE, "Boss key"},                   // Insert
    {0x16, MOD_CTRL | MOD_ALT | MOD_SHIFT, SWIPE_ICON_REFRESH, "Reload skin"}, // Ctrl+Alt+Shift+S
};
const unsigned SWIPE_LABEL_COUNT = sizeof(SWIPE_LABELS) / sizeof(SWIPE_LABELS[0]);

swipe_label_t swipe_label_for(uint8_t key, uint8_t mods, char *buf, unsigned buf_len)
{
    for (unsigned i = 0; i < SWIPE_LABEL_COUNT; i++) {
        if (SWIPE_LABELS[i].key == key && SWIPE_LABELS[i].mods == mods) {
            return SWIPE_LABELS[i];
        }
    }
    // F1-F12 at 0x3A-0x45, F13-F24 at 0x68-0x73 (the free keys); anything
    // else by its usage number
    if (key >= 0x3A && key <= 0x45) {
        snprintf(buf, buf_len, "F%u", (unsigned)(key - 0x3A + 1));
    } else if (key >= 0x68 && key <= 0x73) {
        snprintf(buf, buf_len, "F%u", (unsigned)(key - 0x68 + 13));
    } else {
        snprintf(buf, buf_len, "Key 0x%02X", key);
    }
    swipe_label_t other = {key, mods, SWIPE_ICON_KEYBOARD, buf};
    return other;
}
