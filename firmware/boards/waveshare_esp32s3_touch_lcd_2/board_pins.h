#pragma once

#include "driver/gpio.h"

// -----------------------------------------------------------------------------
// Keypad Switch Pins (Header P2, Active LOW with Internal Pull-Up)
// -----------------------------------------------------------------------------
#define BOARD_KEY1_GPIO         GPIO_NUM_14   // Header P2, Pin 11
#define BOARD_KEY2_GPIO         GPIO_NUM_9    // Header P2, Pin 12

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
// Onboard I2C (Touch CST816 & IMU QMI8658) - Unused by keypad core
// -----------------------------------------------------------------------------
#define BOARD_I2C_SDA_GPIO      GPIO_NUM_48
#define BOARD_I2C_SCL_GPIO      GPIO_NUM_47
