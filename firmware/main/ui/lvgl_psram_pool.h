#pragma once

/*
 * Included by LVGL's builtin allocator when CONFIG_OSUPAD_LVGL_PSRAM is set
 * (LV_MEM_POOL_INCLUDE, firmware/CMakeLists.txt): lv_mem_init then takes its
 * pool from PSRAM through LV_MEM_POOL_ALLOC instead of a static array in
 * internal RAM.
 */
#include <stddef.h>
#include "esp_heap_caps.h"

static inline void *opad_lvgl_pool_alloc(size_t size)
{
    return heap_caps_malloc(size, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
}
