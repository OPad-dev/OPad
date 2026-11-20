#include "ads7028.h"
#include "ads7028_regs.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_attr.h"

#define ADS7028_HOST SPI3_HOST

static spi_device_handle_t s_dev;
static bool s_bus_open;
static ads7028_pins_t s_pins;

void ads7028_carrier_pins(bool cable_reversed, ads7028_pins_t *out)
{
    if (cable_reversed) {
        *out = (ads7028_pins_t){
            .miso = BOARD_HE3_REV_MISO_GPIO,
            .mosi = BOARD_HE3_REV_MOSI_GPIO,
            .sclk = BOARD_HE3_REV_SCLK_GPIO,
            .cs = BOARD_HE3_REV_CS_GPIO,
        };
    } else {
        *out = (ads7028_pins_t){
            .miso = BOARD_HE3_MISO_GPIO,
            .mosi = BOARD_HE3_MOSI_GPIO,
            .sclk = BOARD_HE3_SCLK_GPIO,
            .cs = BOARD_HE3_CS_GPIO,
        };
    }
}

static esp_err_t add_device(uint32_t sclk_hz)
{
    spi_device_interface_config_t dev = {
        .mode = 0,                    // SPI-00 after power-up and reset (8.3.14.1)
        .clock_speed_hz = (int)sclk_hz,
        .spics_io_num = s_pins.cs,
        .queue_size = 1,
        .cs_ena_pretrans = 1,
        .cs_ena_posttrans = 1,
    };
    return spi_bus_add_device(ADS7028_HOST, &dev, &s_dev);
}

esp_err_t ads7028_open(const ads7028_pins_t *pins, uint32_t sclk_hz)
{
    if (s_bus_open) {
        return ESP_ERR_INVALID_STATE;
    }
    s_pins = *pins;
    spi_bus_config_t bus = {
        .miso_io_num = pins->miso,
        .mosi_io_num = pins->mosi,
        .sclk_io_num = pins->sclk,
        .quadwp_io_num = -1,
        .quadhd_io_num = -1,
        .max_transfer_sz = 4,
    };
    esp_err_t err = spi_bus_initialize(ADS7028_HOST, &bus, SPI_DMA_DISABLED);
    if (err != ESP_OK) {
        return err;
    }
    s_bus_open = true;
    err = add_device(sclk_hz);
    if (err != ESP_OK) {
        ads7028_close();
    }
    return err;
}

esp_err_t ads7028_set_clock(uint32_t sclk_hz)
{
    if (!s_bus_open) {
        return ESP_ERR_INVALID_STATE;
    }
    if (s_dev) {
        spi_bus_remove_device(s_dev);
        s_dev = NULL;
    }
    return add_device(sclk_hz);
}

void ads7028_close(void)
{
    if (s_dev) {
        spi_bus_remove_device(s_dev);
        s_dev = NULL;
    }
    if (s_bus_open) {
        spi_bus_free(ADS7028_HOST);
        s_bus_open = false;
        int pins[] = {s_pins.miso, s_pins.mosi, s_pins.sclk, s_pins.cs};
        for (size_t i = 0; i < sizeof(pins) / sizeof(pins[0]); i++) {
            gpio_reset_pin((gpio_num_t)pins[i]);
        }
    }
}

esp_err_t ads7028_transfer(const uint8_t tx[3], uint8_t rx[3])
{
    if (!s_dev) {
        return ESP_ERR_INVALID_STATE;
    }
    spi_transaction_t t = {
        .length = 24,
        .flags = SPI_TRANS_USE_TXDATA | SPI_TRANS_USE_RXDATA,
    };
    t.tx_data[0] = tx[0];
    t.tx_data[1] = tx[1];
    t.tx_data[2] = tx[2];
    esp_err_t err = spi_device_polling_transmit(s_dev, &t);
    if (err == ESP_OK && rx) {
        rx[0] = t.rx_data[0];
        rx[1] = t.rx_data[1];
        rx[2] = t.rx_data[2];
    }
    return err;
}

esp_err_t ads7028_write_reg(uint8_t addr, uint8_t value)
{
    uint8_t tx[3];
    ads7028_frame(ADS7028_OP_WRITE, addr, value, tx);
    return ads7028_transfer(tx, NULL);
}

esp_err_t ads7028_read_reg(uint8_t addr, uint8_t *value)
{
    uint8_t tx[3];
    uint8_t rx[3];
    ads7028_frame(ADS7028_OP_READ, addr, 0, tx);
    esp_err_t err = ads7028_transfer(tx, NULL);
    if (err != ESP_OK) {
        return err;
    }
    ads7028_frame(ADS7028_OP_NOP, 0, 0, tx);
    err = ads7028_transfer(tx, rx);
    if (err == ESP_OK) {
        *value = rx[0];
    }
    return err;
}

esp_err_t ads7028_acquire_bus(void)
{
    return s_dev ? spi_device_acquire_bus(s_dev, portMAX_DELAY) : ESP_ERR_INVALID_STATE;
}

void ads7028_release_bus(void)
{
    if (s_dev) {
        spi_device_release_bus(s_dev);
    }
}

esp_err_t IRAM_ATTR ads7028_read16(uint16_t *out)
{
    spi_transaction_t t = {
        .length = 16,
        .flags = SPI_TRANS_USE_TXDATA | SPI_TRANS_USE_RXDATA,
    };
    esp_err_t err = spi_device_polling_transmit(s_dev, &t);
    *out = (uint16_t)((t.rx_data[0] << 8) | t.rx_data[1]);
    return err;
}
