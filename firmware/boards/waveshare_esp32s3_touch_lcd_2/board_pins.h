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

