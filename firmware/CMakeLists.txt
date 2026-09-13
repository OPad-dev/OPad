cmake_minimum_required(VERSION 3.16)

# Set firmware project version (Single version source)
set(PROJECT_VER "1.0.0")

# Include ESP-IDF project boilerplate
include($ENV{IDF_PATH}/tools/cmake/project.cmake)

project(osupad-firmware)
