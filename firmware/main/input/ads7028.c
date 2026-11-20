#include "ads7028.h"
#include "ads7028_regs.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_attr.h"
#include "hal/spi_ll.h"
#include <string.h>

#define ADS7028_HOST SPI3_HOST
#define ADS7028_HW   (&GPSPI3)  // SPI3_HOST's registers

static spi_device_handle_t s_dev;
static bool s_bus_open;
static ads7028_pins_t s_pins;
static bool s_crc;  // GENERAL_CFG.CRC_EN as last written

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
    s_crc = false;
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
        s_crc = false;
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

// One 32-bit frame with CRC on: the command, its CRC, and back the SDO bytes
static esp_err_t transfer_crc(uint8_t op, uint8_t addr, uint8_t data, uint8_t rx[4])
{
    if (!s_dev) {
        return ESP_ERR_INVALID_STATE;
    }
    spi_transaction_t t = {
        .length = 32,
        .flags = SPI_TRANS_USE_TXDATA | SPI_TRANS_USE_RXDATA,
    };
    ads7028_frame_crc(op, addr, data, t.tx_data);
    esp_err_t err = spi_device_polling_transmit(s_dev, &t);
    if (err == ESP_OK && rx) {
        memcpy(rx, t.rx_data, 4);
    }
    return err;
}

// One command in the framing the ADC expects now
static esp_err_t command(uint8_t op, uint8_t addr, uint8_t data, uint8_t rx[4])
{
    if (s_crc) {
        return transfer_crc(op, addr, data, rx);
    }
    uint8_t tx[3];
    ads7028_frame(op, addr, data, tx);
    return ads7028_transfer(tx, rx);
}

bool ads7028_crc_on(void)
{
    return s_crc;
}

esp_err_t ads7028_write_reg(uint8_t addr, uint8_t value)
{
    esp_err_t err = command(ADS7028_OP_WRITE, addr, value, NULL);
    // CRC_EN takes effect from the next frame; a reset clears it (8.5.2)
    if (err == ESP_OK && addr == ADS7028_REG_GENERAL_CFG) {
        s_crc = (value & ADS7028_GENERAL_CRC_EN) && !(value & ADS7028_GENERAL_RST);
    }
    return err;
}

esp_err_t ads7028_reset(void)
{
    // A stop (he_spi_source_stop) leaves the ADC with CRC on while this
    // driver starts every open with it off. With CRC on only the 32-bit frame
    // counts (the 24-bit one is a CRC error, which the reset clears); with it
    // off only the 24-bit one does (the 32-bit one did not reset the ADC on
    // the owner's pad)
    esp_err_t err = transfer_crc(ADS7028_OP_WRITE, ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_RST, NULL);
    if (err == ESP_OK) {
        uint8_t tx[3];
        ads7028_frame(ADS7028_OP_WRITE, ADS7028_REG_GENERAL_CFG, ADS7028_GENERAL_RST, tx);
        err = ads7028_transfer(tx, NULL);
    }
    s_crc = false;
    return err;
}

esp_err_t ads7028_read_reg(uint8_t addr, uint8_t *value)
{
    uint8_t rx[4] = {0};
    esp_err_t err = command(ADS7028_OP_READ, addr, 0, NULL);
    if (err == ESP_OK) {
        err = command(ADS7028_OP_NOP, 0, 0, rx);
    }
    if (err == ESP_OK && s_crc && !ads7028_rx_crc_ok(rx, 1)) {
        err = ESP_ERR_INVALID_CRC;
    }
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
    // A no-op with CRC on is four zero bytes: its CRC is 0 too
    spi_transaction_t t = {
        .length = s_crc ? 32 : 16,
        .flags = SPI_TRANS_USE_TXDATA | SPI_TRANS_USE_RXDATA,
    };
    esp_err_t err = spi_device_polling_transmit(s_dev, &t);
    *out = (uint16_t)((t.rx_data[0] << 8) | t.rx_data[1]);
    if (err == ESP_OK && s_crc && !ads7028_rx_crc_ok(t.rx_data, 2)) {
        err = ESP_ERR_INVALID_CRC;
    }
    return err;
}

esp_err_t IRAM_ATTR ads7028_read16_again(uint16_t *out)
{
    // ads7028_read16 left length, clock, mode and CS set: the transfer only
    // needs starting. Full duplex shares the data buffer with RX, so the
    // no-op's zeros go back in first (spi_hal_iram.c does the same steps).
    spi_dev_t *hw = ADS7028_HW;
    spi_ll_clear_int_stat(hw);
    hw->data_buf[0] = 0;
    spi_ll_apply_config(hw);
    spi_ll_user_start(hw);
    while (!spi_ll_usr_is_done(hw)) {
    }
    uint32_t word = hw->data_buf[0];
    uint8_t rx[4];
    memcpy(rx, &word, 4);
    *out = (uint16_t)((rx[0] << 8) | rx[1]);
    if (s_crc && !ads7028_rx_crc_ok(rx, 2)) {
        return ESP_ERR_INVALID_CRC;
    }
    return ESP_OK;
}
