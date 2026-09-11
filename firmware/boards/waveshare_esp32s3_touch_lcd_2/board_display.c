#include "board.h"
#include "esp_log.h"
#include "driver/spi_master.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_vendor.h"
#include "esp_lcd_panel_ops.h"

static const char *TAG = "board_display";

esp_err_t board_display_init(esp_lcd_panel_handle_t *out_panel)
{
    if (!out_panel) {
        return ESP_ERR_INVALID_ARG;
    }

    ESP_LOGI(TAG, "Initializing ST7789 display (SCLK=%d, MOSI=%d, CS=%d, DC=%d)...",
             BOARD_LCD_SCLK_GPIO, BOARD_LCD_MOSI_GPIO, BOARD_LCD_CS_GPIO, BOARD_LCD_DC_GPIO);

    // 1. Initialize SPI bus
    spi_bus_config_t buscfg = {
        .sclk_io_num = BOARD_LCD_SCLK_GPIO,
        .mosi_io_num = BOARD_LCD_MOSI_GPIO,
        .miso_io_num = -1,
        .quadwp_io_num = -1,
        .quadhd_io_num = -1,
        .max_transfer_sz = BOARD_LCD_H_RES * 40 * sizeof(uint16_t),
    };
    esp_err_t ret = spi_bus_initialize(BOARD_LCD_SPI_HOST, &buscfg, SPI_DMA_CH_AUTO);
    if (ret != ESP_OK && ret != ESP_ERR_INVALID_STATE) {
        ESP_LOGE(TAG, "Failed to initialize SPI bus: %s", esp_err_to_name(ret));
        return ret;
    }

    // 2. Attach LCD to the SPI bus
    esp_lcd_panel_io_handle_t io_handle = NULL;
    esp_lcd_panel_io_spi_config_t io_config = {
        .dc_gpio_num = BOARD_LCD_DC_GPIO,
        .cs_gpio_num = BOARD_LCD_CS_GPIO,
        .pclk_hz = BOARD_LCD_PIXEL_CLOCK_HZ,
        .lcd_cmd_bits = 8,
        .lcd_param_bits = 8,
        .spi_mode = 0,
        .trans_queue_depth = 10,
    };
    ret = esp_lcd_new_panel_io_spi((esp_lcd_spi_bus_handle_t)BOARD_LCD_SPI_HOST, &io_config, &io_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to create LCD panel IO: %s", esp_err_to_name(ret));
        return ret;
    }

    // 3. Configure ST7789 panel
    esp_lcd_panel_handle_t panel_handle = NULL;
    esp_lcd_panel_dev_config_t panel_config = {
        .reset_gpio_num = BOARD_LCD_RST_GPIO,
        .rgb_endian = LCD_RGB_ENDIAN_BGR,
        .bits_per_pixel = 16,
    };
    ret = esp_lcd_new_panel_st7789(io_handle, &panel_config, &panel_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to create ST7789 panel: %s", esp_err_to_name(ret));
        return ret;
    }

    ret = esp_lcd_panel_reset(panel_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to reset panel: %s", esp_err_to_name(ret));
        return ret;
    }

    ret = esp_lcd_panel_init(panel_handle);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "Failed to init panel: %s", esp_err_to_name(ret));
        return ret;
    }

    // Waveshare ST7789 configuration: invert colors, normal mirror
    esp_lcd_panel_invert_color(panel_handle, true);
    esp_lcd_panel_mirror(panel_handle, false, false);
    esp_lcd_panel_disp_on_off(panel_handle, true);

    *out_panel = panel_handle;
    ESP_LOGI(TAG, "ST7789 display initialized successfully (%dx%d)", BOARD_LCD_H_RES, BOARD_LCD_V_RES);
    return ESP_OK;
}
