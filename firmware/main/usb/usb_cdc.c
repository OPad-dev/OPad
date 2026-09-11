#include "usb_cdc.h"
#include "protocol/protocol.h"
#include "tusb.h"
#include "esp_log.h"

static const char *TAG = "usb_cdc";
static volatile bool s_cdc_connected = false;

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

// TinyUSB CDC Callbacks
void tud_cdc_rx_cb(uint8_t itf)
{
    (void)itf;
    usb_cdc_task_poll();
}

void tud_cdc_line_state_cb(uint8_t itf, bool dtr, bool rts)
{
    (void)itf;
    (void)rts;
    s_cdc_connected = dtr;
    ESP_LOGI(TAG, "CDC DTR line state: %s", dtr ? "CONNECTED" : "DISCONNECTED");
}
