#pragma once

// Portable OPad UI core on top of LVGL: layout model, data subjects and screen builder.
// Depends only on LVGL, so the PC designer compiles the same code for its preview.
// All functions must be called with the LVGL lock held (lvgl_port_lock on the device).

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "lvgl.h"
#include "ui_ids.h"

#ifdef __cplusplus
extern "C" {
#endif

#define UI_SCREEN_W 320
#define UI_SCREEN_H 240
#define UI_MAX_WIDGETS 32
#define UI_LABEL_MAX 32   // including NUL
#define UI_SUFFIX_MAX 12  // including NUL
#define UI_STRING_MAX 64  // string source values, including NUL

typedef struct {
    uint8_t kind;      // ui_widget_kind_t
    uint8_t source;    // ui_source_t
    uint8_t font;      // ui_font_id_t
    uint8_t align;     // ui_align_t
    int16_t x, y, w, h;
    uint32_t fg;       // 0xRRGGBB
    uint32_t bg;
    uint32_t accent;
    uint8_t radius;
    uint8_t decimals;  // 0xFF = source default
    uint8_t flags;     // UI_FLAG_*
    uint8_t reserved;
    char label[UI_LABEL_MAX];   // static text, or prefix before a bound value
    char suffix[UI_SUFFIX_MAX];
} ui_widget_t;

typedef struct {
    uint32_t background;  // 0xRRGGBB
    uint8_t count;
    ui_widget_t widgets[UI_MAX_WIDGETS];
} ui_layout_t;

#define UI_DECIMALS_DEFAULT 0xFF

// ---- Data sources ----------------------------------------------------------------

typedef enum {
    UI_VALUE_STRING,
    UI_VALUE_NUMBER,   // scaled int32, see ui_source_info_t.scale
} ui_value_type_t;

typedef enum {
    UI_FMT_TEXT,       // string as is
    UI_FMT_INT,        // plain integer
    UI_FMT_GROUPED,    // integer with thousands separators
    UI_FMT_FIXED,      // value / scale with decimals
    UI_FMT_PERCENT,    // value / scale * 100 with decimals (for 0..1 sources)
    UI_FMT_DURATION,   // milliseconds as m:ss or h:mm:ss
} ui_format_t;

typedef struct {
    const char *name;        // stable identifier, e.g. "play.pp"
    ui_value_type_t type;
    ui_format_t format;
    int32_t scale;           // stored = real value * scale
    uint8_t default_decimals;
} ui_source_info_t;

/** Metadata for a source, or NULL for unknown/reserved ids. */
const ui_source_info_t *ui_source_info(uint8_t source);

/** Create the subjects. Call once after lv_init(). */
void ui_data_init(void);

/** Update a numeric source with a real (unscaled) value. Observers run only if it changed. */
void ui_data_set_number(uint8_t source, double value);

/** Update a string source. Observers run only if it changed. */
void ui_data_set_string(uint8_t source, const char *value);

/** Mark a source as having no value (widgets with UI_FLAG_HIDE_WHEN_EMPTY disappear). */
void ui_data_clear(uint8_t source);

/** Clear all tosu-derived sources (map, play, profile, session), e.g. when tosu disconnects. */
void ui_data_clear_host_sources(void);

// ---- Layouts & screens -----------------------------------------------------------

const ui_layout_t *ui_default_layout(uint8_t screen);

/** Returns true if the layout is safe to build; otherwise writes a reason into err. */
bool ui_layout_validate(const ui_layout_t *layout, char *err, size_t err_len);

/** Build a new LVGL screen object (not loaded) for a validated layout. */
lv_obj_t *ui_screen_create(const ui_layout_t *layout);

/** Delete a screen from ui_screen_create (or any other screen) and free its layout copy. */
void ui_screen_delete(lv_obj_t *scr);

#ifdef __cplusplus
}
#endif
