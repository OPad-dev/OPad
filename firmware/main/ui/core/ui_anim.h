#pragma once

// Result animations (issue #3): short overlays on LVGL's top layer, e.g. the
// grade when osu! shows the results screen. Depends only on LVGL, like the rest
// of the ui core, so the PC preview plays the same code. All functions must be
// called with the LVGL lock held.
//
// Ids are the wire values of PlayAnimation.animation (protocol/osupad.proto):
// 0 stops, 1..UI_ANIM_BUILTIN_COUNT are built in, UI_ANIM_CUSTOM_BASE + n is
// reserved for uploaded animations (one slot per trigger, overwritten by the
// next upload). Never renumber a built-in: the PC stores these ids.

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    UI_ANIM_NONE = 0,
    UI_ANIM_POP = 1,    // the grade zooms in with an overshoot
    UI_ANIM_DROP = 2,   // the grade falls in and bounces
    UI_ANIM_BURST = 3,  // the grade pops out of two expanding rings
} ui_anim_id_t;

#define UI_ANIM_BUILTIN_COUNT 3
#define UI_ANIM_CUSTOM_BASE 128

/** Length of every built-in animation */
#define UI_ANIM_DURATION_MS 2600

/**
 * Start an animation over whatever screen is loaded, replacing one already
 * playing. grade is tosu's rank ("SS", "SH", "A", ...). An id the pad does not
 * have (a later built-in, an empty upload slot) plays UI_ANIM_POP. Returns
 * false, and stops, for UI_ANIM_NONE.
 */
bool ui_anim_play(uint8_t id, const char *grade);

/** Remove the playing animation at once (a map started). */
void ui_anim_stop(void);

bool ui_anim_active(void);

/**
 * Freeze the playing animation at t_ms from its start (PC preview). The
 * animation then stays until ui_anim_stop.
 */
void ui_anim_seek(uint32_t t_ms);

#ifdef __cplusplus
}
#endif
