cmake_minimum_required(VERSION 3.16)

# Set firmware project version (Single version source)
set(PROJECT_VER "1.0.0-rc.3") # x-release-please-version

# Include ESP-IDF project boilerplate
include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(opad-firmware)

# LVGL's pool from PSRAM (CONFIG_OPAD_LVGL_PSRAM): lv_mem_init allocates it
# through LV_MEM_POOL_ALLOC instead of a static array in internal RAM
if(CONFIG_OPAD_LVGL_PSRAM)
    idf_component_get_property(lvgl_lib lvgl__lvgl COMPONENT_LIB)
    # CMake takes no function-like macros here, so the pool comes from a function
    target_compile_definitions(${lvgl_lib} PRIVATE
        "LV_MEM_POOL_INCLUDE=<${CMAKE_CURRENT_SOURCE_DIR}/main/ui/lvgl_psram_pool.h>"
        "LV_MEM_POOL_ALLOC=opad_lvgl_pool_alloc")
endif()

# OPad's asset pack for the "assets" partition (main/assets/asset_pack.h),
# built from firmware/assets with every firmware build: build/opad-assets.bin,
# which `opadctl flash --full`, `opadctl recover` and the firmware release
# write with the rest. The clips are 320x240 baseline MJPEG at 12 fps.
idf_build_get_property(python PYTHON)
set(OPAD_ASSETS_DIR ${CMAKE_CURRENT_SOURCE_DIR}/assets)
set(OPAD_ASSET_PACK ${CMAKE_BINARY_DIR}/opad-assets.bin)
add_custom_command(
    OUTPUT ${OPAD_ASSET_PACK}
    COMMAND ${python} ${CMAKE_CURRENT_SOURCE_DIR}/../scripts/build_asset_pack.py
            --partition assets -o ${OPAD_ASSET_PACK} --label opad
            --mjpeg boot ${OPAD_ASSETS_DIR}/boot.mjpeg 12
            --mjpeg updating ${OPAD_ASSETS_DIR}/updating.mjpeg 12
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/../scripts/build_asset_pack.py
            ${CMAKE_CURRENT_SOURCE_DIR}/partitions.csv
            ${OPAD_ASSETS_DIR}/boot.mjpeg
            ${OPAD_ASSETS_DIR}/updating.mjpeg
    COMMENT "Building the asset pack"
    VERBATIM)
add_custom_target(opad_asset_pack ALL DEPENDS ${OPAD_ASSET_PACK})
