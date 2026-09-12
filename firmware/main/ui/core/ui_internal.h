#pragma once

// Internal helpers shared by the ui core translation units.

#include "ui_core.h"

#ifdef __cplusplus
extern "C" {
#endif

// Numeric subjects hold this while the source has no value
#define UI_VALUE_EMPTY INT32_MIN

lv_subject_t *ui_data_subject(uint8_t source);
bool ui_data_is_empty(uint8_t source);

/** Format the current value (no prefix/suffix). Empty numeric values render as "-". */
void ui_data_format(uint8_t source, uint8_t decimals, char *out, size_t len);

#ifdef __cplusplus
}
#endif
