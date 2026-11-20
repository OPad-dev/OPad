#pragma once

#include "driver/gpio.h"

// Keypad switch pins are configurable at runtime (device config, defaults GPIO14 / GPIO9
// on header P2 pins 11 / 12). Allowed pins: config/config_validate.c

// -----------------------------------------------------------------------------
// Waveshare 2.0" ST7789 LCD (SPI) & Backlight (PWM)
// -----------------------------------------------------------------------------
#define BOARD_LCD_SPI_HOST      SPI2_HOST
#define BOARD_LCD_SCLK_GPIO     GPIO_NUM_39
#define BOARD_LCD_MOSI_GPIO     GPIO_NUM_38
#define BOARD_LCD_DC_GPIO       GPIO_NUM_42
#define BOARD_LCD_CS_GPIO       GPIO_NUM_45
#define BOARD_LCD_RST_GPIO      GPIO_NUM_NC   // Handled via power or software reset
#define BOARD_LCD_BL_GPIO       GPIO_NUM_1    // LEDC PWM Backlight

#define BOARD_LCD_H_RES         320
#define BOARD_LCD_V_RES         240
// 80 MHz: a full frame goes out in ~15 ms instead of ~31 ms, so redraws make
// LVGL's 33 ms period with room to spare. Above the ST7789's rated 62.5 MHz
// write clock; checked on the pad with no artifacts (2026-10-20, UI test
// build D). ESP-IDF allows 80 MHz on the S3 through the GPIO matrix.
#define BOARD_LCD_PIXEL_CLOCK_HZ (80 * 1000 * 1000)

// -----------------------------------------------------------------------------
// Onboard I2C (Touch CST816 & IMU QMI8658)
// -----------------------------------------------------------------------------
#define BOARD_I2C_SDA_GPIO      GPIO_NUM_48
#define BOARD_I2C_SCL_GPIO      GPIO_NUM_47
#define BOARD_TOUCH_INT_GPIO    GPIO_NUM_46

// -----------------------------------------------------------------------------
// Module ID Detection (ADC1_CH7 on connector pin 5)
// -----------------------------------------------------------------------------
#define BOARD_MODULE_ID_GPIO    GPIO_NUM_8    // ADC1_CH7, connector pin 5 (ID)
// Where the carrier routes an MX module's keys (connector pins 3/4)
#define BOARD_MX_KEY1_GPIO      10
#define BOARD_MX_KEY2_GPIO      7
// Spare module lines (connector pins 6, 7, 8): not used by the MX module
#define BOARD_MODULE_IO6_GPIO   6
#define BOARD_MODULE_IO4_GPIO   4
#define BOARD_MODULE_IO2_GPIO   2
// Connector pin 8: where a reversed cable puts the module's 3V3
#define BOARD_REVERSE_PROBE_GPIO GPIO_NUM_2

// -----------------------------------------------------------------------------
// V3 carrier: 14-way FFC (hardware/pcb/V3/README.md, Cables, on hw/redesign).
// The carrier's FFC contacts: 4 GPIO8, 5 GPIO6, 7 GPIO2, 8 GPIO4, 10 GPIO10,
// 11 GPIO7. A turned-over cable joins module contact k to carrier contact
// 15 - k, so the ID arrives on GPIO7 (ADC1_CH6) and every line moves.
// -----------------------------------------------------------------------------
#define BOARD_MODULE_ID_ALT_GPIO GPIO_NUM_7   // ADC1_CH6: ID on a turned-over V3 cable

// HE V3: the module's ADS7028 (module contacts ID 4, MISO 5, SCLK 7, MOSI 8, CS 10)
#define BOARD_HE3_MISO_GPIO      6
#define BOARD_HE3_SCLK_GPIO      2
#define BOARD_HE3_MOSI_GPIO      4
#define BOARD_HE3_CS_GPIO        10
// The same lines with the cable turned over
#define BOARD_HE3_REV_MISO_GPIO  10
#define BOARD_HE3_REV_SCLK_GPIO  4
#define BOARD_HE3_REV_MOSI_GPIO  2
#define BOARD_HE3_REV_CS_GPIO    6

