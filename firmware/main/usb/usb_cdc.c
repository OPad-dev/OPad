#include "usb_cdc.h"
#include "protocol/protocol.h"
#include "tusb.h"
#include "esp_log.h"
#include "esp_system.h"
#include "soc/rtc_cntl_reg.h"
#include "esp32s3/rom/usb/chip_usb_dw_wrapper.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "usb_cdc";
static volatile bool s_cdc_connected = false;
static bool s_prev_rts_state = false;
static volatile bool s_need_bootloader_reboot = false;

esp_err_t usb_cdc_init(void)
{
    ESP_LOGI(TAG, "USB CDC telemetry channel initialized");
    return ESP_OK;
}

bool usb_cdc_is_connected(void)
{
    return s_cdc_connected && tud_cdc_n_connected(0);
}

size_t usb_cdc_write(const uint8_t *data, size_t len)
{
    if (!tud_cdc_n_connected(0)) {
        return 0;
    }

    size_t total_written = 0;
    while (total_written < len) {
        uint32_t avail = tud_cdc_n_write_available(0);
        if (avail == 0) {
            tud_cdc_n_write_flush(0);
            break;
        }

        uint32_t chunk = len - total_written;
        if (chunk > avail) {
            chunk = avail;
        }

        uint32_t written = tud_cdc_n_write(0, data + total_written, chunk);
        if (written == 0) {
            break;
        }
        total_written += written;
    }

    tud_cdc_n_write_flush(0);
    return total_written;
}

void usb_cdc_flush(void)
{
    tud_cdc_n_write_flush(0);
}

void usb_cdc_reboot_to_bootloader(void)
{
    s_need_bootloader_reboot = true;
}

void usb_cdc_task_poll(void)
{
    if (s_need_bootloader_reboot) {
        s_need_bootloader_reboot = false;
        ESP_LOGI(TAG, "Rebooting into ROM Download Bootloader...");
        vTaskDelay(pdMS_TO_TICKS(50));
        chip_usb_set_persist_flags(0);
        REG_WRITE(RTC_CNTL_OPTION1_REG, RTC_CNTL_FORCE_DOWNLOAD_BOOT);
        esp_restart();
    }

    if (!tud_cdc_n_available(0)) {
        return;
    }

    uint8_t rx_buf[256];
    uint32_t count = tud_cdc_n_read(0, rx_buf, sizeof(rx_buf));
    if (count > 0) {
        if (memmem(rx_buf, count, "BOOTLOADER", 10) != NULL ||
            memmem(rx_buf, count, "REBOOT", 6) != NULL) {
            ESP_LOGI(TAG, "CDC serial command received: entering bootloader...");
            s_need_bootloader_reboot = true;
        }
        protocol_feed_cdc_bytes(rx_buf, count);
    }
}

// TinyUSB CDC Callbacks
void tud_cdc_rx_cb(uint8_t itf)
{
    (void)itf;
}

void tud_cdc_line_state_cb(uint8_t itf, bool dtr, bool rts)
{
    (void)itf;
    s_cdc_connected = dtr;
    ESP_LOGD(TAG, "CDC line state: DTR=%d, RTS=%d", dtr, rts);

    // Standard Espressif CDC-ACM bootloader reset trigger:
    // When RTS falls from HIGH to LOW while DTR is HIGH (classic esptool pattern)
    if (!rts && s_prev_rts_state && dtr) {
        ESP_LOGI(TAG, "CDC DTR/RTS bootloader trigger detected, scheduling download mode...");
        s_need_bootloader_reboot = true;
    }
    s_prev_rts_state = rts;
}

void tud_cdc_line_coding_cb(uint8_t itf, cdc_line_coding_t const* p_line_coding)
{
    (void)itf;
    if (p_line_coding && p_line_coding->bit_rate == 1200) {
        ESP_LOGI(TAG, "1200 baud touch detected, scheduling download mode...");
        s_need_bootloader_reboot = true;
    }
}
