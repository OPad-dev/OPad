#pragma once

/*
 * Which input module an ID reading names. Pure C, host-tested
 * (firmware/test/host/test_module_id.c); board.c reads the voltages.
 *
 * Every module puts a divider from 3V3 to GND on its ID contact:
 *
 *   module   divider     ID       carrier pin it reaches
 *   MX V1    100k/10k    0.30 V   GPIO8 (JST cable, one way only)
 *   HE V1    100k/47k    1.06 V   GPIO8
 *   HE V3    100k/100k   1.65 V   GPIO8, or GPIO7 on a turned-over FFC
 *   MX V3    47k/100k    2.24 V   GPIO8, or GPIO7 on a turned-over FFC
 *
 * Cut-offs between them: 0.68 / 1.35 / 1.95 / 2.6 V
 * (hardware/pcb/V3/README.md, Cables, on branch hw/redesign).
 *
 * A V3 FFC turned over connects contact k to 15 - k, so the ID lands on GPIO7
 * instead of GPIO8. GPIO7 is only read when GPIO8 shows nothing, and only the
 * V3 classes count there: on a V1 carrier GPIO7 is MX key 2 or a hand-wired
 * key, which never reads like a divider with the pull-down on.
 */

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Values match board_module_type_t (board.h) and must stay in step with it
typedef enum {
    MODULE_ID_NONE = 0,
    MODULE_ID_MX = 1,
    MODULE_ID_HE = 2,
    MODULE_ID_HE_V3 = 3,
    MODULE_ID_MX_V3 = 4,
} module_id_t;

// A floating ID with the ~45k pull-down reads ~0 V; every divider holds it well
// above. An MX V1 pad measured 140-180 mV with dips to 90 mV, so the line sits at 60 mV.
#define MODULE_ID_PRESENT_MIN_MV 60

#define MODULE_ID_MX_MAX_MV      680
#define MODULE_ID_HE_MAX_MV      1350
#define MODULE_ID_HE_V3_MAX_MV   1950
#define MODULE_ID_MX_V3_MAX_MV   2600

/**
 * @brief The module an ID reading names. open_mv: pull-down off; loaded_mv:
 * pull-down on. Either below 0 means the read failed: none.
 */
module_id_t module_id_classify(int open_mv, int loaded_mv);

/** @brief True for the modules on the V3 FFC (HE V3, MX V3). */
bool module_id_is_v3(module_id_t m);

#ifdef __cplusplus
}
#endif
