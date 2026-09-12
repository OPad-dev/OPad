#include "usb_cdc.h"
#include "protocol/protocol.h"
#include "tusb.h"
#include "esp_log.h"
#include "esp_system.h"
#include "soc/rtc_cntl_reg.h"

#include "esp_timer.h"

static const char *TAG = "usb_cdc";
static volatile bool s_cdc_connected = false;
static bool s_prev_rts_state = false;

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

void usb_cdc_task_poll(void)
{
    if (!tud_cdc_n_available(0)) {
        return;
    }

    uint8_t rx_buf[256];
    uint32_t count = tud_cdc_n_read(0, rx_buf, sizeof(rx_buf));
    if (count > 0) {
        protocol_feed_cdc_bytes(rx_buf, count);
    }
}

static void deferred_bootloader_reboot_cb(void *arg)
{
    (void)arg;
    ESP_LOGI(TAG, "Rebooting into ESP32-S3 ROM Download Bootloader...");
    REG_WRITE(RTC_CNTL_OPTION1_REG, RTC_CNTL_FORCE_DOWNLOAD_BOOT);
    esp_restart();
}

void usb_cdc_reboot_to_bootloader(void)
{
    static esp_timer_handle_t s_reboot_timer = NULL;
    if (!s_reboot_timer) {
        const esp_timer_create_args_t timer_args = {
            .callback = &deferred_bootloader_reboot_cb,
            .name = "cdc_reboot_timer",
        };
        esp_timer_create(&timer_args, &s_reboot_timer);
    }
    // Give TinyUSB 50ms to acknowledge the USB control request cleanly before rebooting
    esp_timer_start_once(s_reboot_timer, 50 * 1000);
}

// TinyUSB CDC Callbacks
void tud_cdc_rx_cb(uint8_t itf)
{
    (void)itf;
    usb_cdc_task_poll();
}

void tud_cdc_line_state_cb(uint8_t itf, bool dtr, bool rts)
{
    (void)itf;
    s_cdc_connected = dtr;
    ESP_LOGD(TAG, "CDC line state: DTR=%d, RTS=%d", dtr, rts);

    // Standard Espressif CDC-ACM bootloader reset trigger:
    // When RTS falls from HIGH to LOW while DTR is HIGH (classic esptool pattern)
    if (!rts && s_prev_rts_state && dtr) {
        ESP_LOGI(TAG, "CDC DTR/RTS bootloader trigger detected, entering download mode...");
        usb_cdc_reboot_to_bootloader();
    }
    s_prev_rts_state = rts;
}

void tud_cdc_line_coding_cb(uint8_t itf, cdc_line_coding_t const* p_line_coding)
{
    (void)itf;
    if (p_line_coding && p_line_coding->bit_rate == 1200) {
        ESP_LOGI(TAG, "1200 baud touch detected, entering bootloader...");
        usb_cdc_reboot_to_bootloader();
    }
}
