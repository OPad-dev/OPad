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
#define BOARD_LCD_PIXEL_CLOCK_HZ (40 * 1000 * 1000)

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

