#include "module_id.h"

module_id_t module_id_classify(int open_mv, int loaded_mv)
{
    if (open_mv < 0 || loaded_mv < 0 || loaded_mv < MODULE_ID_PRESENT_MIN_MV) {
        return MODULE_ID_NONE;      // floating, or unreadable
    } else if (open_mv < MODULE_ID_MX_MAX_MV) {
        return MODULE_ID_MX;        // 0.30 V
    } else if (open_mv < MODULE_ID_HE_MAX_MV) {
        return MODULE_ID_HE;        // 1.06 V
    } else if (open_mv < MODULE_ID_HE_V3_MAX_MV) {
        return MODULE_ID_HE_V3;     // 1.65 V
    } else if (open_mv < MODULE_ID_MX_V3_MAX_MV) {
        return MODULE_ID_MX_V3;     // 2.24 V
    }
    return MODULE_ID_NONE;          // no module sits up here: ID shorted to 3V3
}

bool module_id_is_v3(module_id_t m)
{
    return m == MODULE_ID_HE_V3 || m == MODULE_ID_MX_V3;
}
