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

/** 0xRRGGBB a grade is drawn in, from tosu's rank ("SS", "SH", "A", ...) */
uint32_t ui_grade_color(const char *grade);

/** The letters osu! shows for a rank: X/XH as SS, SH as S */
const char *ui_grade_shown(const char *grade);

#ifdef __cplusplus
}
#endif
