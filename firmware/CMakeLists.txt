cmake_minimum_required(VERSION 3.16)

# Set firmware project version (Single version source)
set(PROJECT_VER "1.0.0-rc") # x-release-please-version

# Include ESP-IDF project boilerplate
include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(opad-firmware)

# LVGL's pool from PSRAM (CONFIG_OSUPAD_LVGL_PSRAM): lv_mem_init allocates it
# through LV_MEM_POOL_ALLOC instead of a static array in internal RAM
if(CONFIG_OSUPAD_LVGL_PSRAM)
    idf_component_get_property(lvgl_lib lvgl__lvgl COMPONENT_LIB)
    # CMake takes no function-like macros here, so the pool comes from a function
    target_compile_definitions(${lvgl_lib} PRIVATE
        "LV_MEM_POOL_INCLUDE=<${CMAKE_CURRENT_SOURCE_DIR}/main/ui/lvgl_psram_pool.h>"
        "LV_MEM_POOL_ALLOC=opad_lvgl_pool_alloc")
endif()
