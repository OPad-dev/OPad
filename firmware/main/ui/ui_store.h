#pragma once

// Custom screen layouts persisted in NVS. Only ever written while not PLAYING:
// flash writes pause both cores, including the key/USB core.

#include <stdbool.h>
#include "esp_err.h"
#include "ui/core/ui_core.h"

#ifdef __cplusplus
extern "C" {
#endif

/** Load a stored layout. Returns false if none is stored or it is invalid/outdated. */
bool ui_store_load(uint8_t screen, ui_layout_t *out);

/** Store a layout (skips the flash write when identical to what is stored). */
esp_err_t ui_store_save(uint8_t screen, const ui_layout_t *layout);

/** Forget a stored layout (the built-in default is used again). */
esp_err_t ui_store_erase(uint8_t screen);

/** Flush any dirty pending layouts/erases to NVS if in IDLE state. */
esp_err_t ui_store_flush_dirty(void);

#ifdef __cplusplus
}
#endif
