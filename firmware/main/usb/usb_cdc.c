#include "usb_cdc.h"
#include "protocol/protocol.h"
#include "diag/diag.h"
#include "ui/ui.h"
#include "tusb.h"
#include "tinyusb.h"
#include "esp_log.h"
#include "esp_system.h"
#include "soc/rtc_cntl_reg.h"
#include "hal/usb_serial_jtag_ll.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include <string.h>

static const char *TAG = "usb_cdc";
static volatile bool s_cdc_connected = false;
static volatile bool s_download_mode_armed = false;
static volatile bool s_need_bootloader_reboot = false;
static TaskHandle_t s_cdc_task = NULL;
// Set by the TinyUSB task (core 0) on DTR drop; the framing buffer is only touched by the protocol task
static volatile bool s_rx_reset_requested = false;

static void schedule_bootloader_reboot(void)
{
    s_need_bootloader_reboot = true;
    if (s_cdc_task) {
        xTaskNotifyGive(s_cdc_task);
    }
}

esp_err_t usb_cdc_init(void)
{
    ESP_LOGI(TAG, "USB CDC telemetry channel initialized");
    return ESP_OK;
}

bool usb_cdc_is_connected(void)
{
    return s_cdc_connected && tud_cdc_n_connected(0);
}

// How long a sender (the protocol task on core 1) may wait for FIFO room
#define CDC_TX_WAIT_MS 20

/*
 * All-or-nothing: a frame is queued only once the TX FIFO has room for the whole
 * of it. A partial write would leave the host holding a header whose payload
 * never arrives, and every frame after it would be misparsed.
 */
size_t usb_cdc_write(const uint8_t *data, size_t len)
{
    if (!tud_ready() || len == 0 || len > CONFIG_TINYUSB_CDC_TX_BUFSIZE) {
        return 0;
    }

    TickType_t deadline = xTaskGetTickCount() + pdMS_TO_TICKS(CDC_TX_WAIT_MS);
    while (tud_cdc_n_write_available(0) < len) {
        tud_cdc_n_write_flush(0);
        if (!tud_cdc_n_connected(0) || (int32_t)(xTaskGetTickCount() - deadline) >= 0) {
            return 0;
        }
        vTaskDelay(1);
    }

    uint32_t written = tud_cdc_n_write(0, data, len);
    tud_cdc_n_write_flush(0);
    return written;
}

void usb_cdc_flush(void)
{
    tud_cdc_n_write_flush(0);
}

void usb_cdc_reboot_to_bootloader(void)
{
    schedule_bootloader_reboot();
}

/*
 * Hand the internal USB PHY back to the USB-Serial-JTAG (USJ) controller and
 * restart into the ROM download bootloader, so the board enumerates as
 * 303a:1001 and can be flashed without touching BOOT/RST.
 *
 * The USJ controller is a fixed-function device that enumerates in hardware,
 * so it does not need the CPU. But it was already enumerated at power-on
 * before TinyUSB took the PHY, so it still holds a stale bus address. The
 * host must see a real disconnect + reconnect and send a bus reset, otherwise
 * it never re-enumerates the port (seen as error -71 and the app coming back).
 * Same sequence as arduino-esp32's usb_switch_to_cdc_jtag().
 */
static void reboot_to_rom_download(void)
{
    ESP_LOGI(TAG, "Rebooting into ROM Download Bootloader...");
    tud_cdc_n_write_flush(0);
    vTaskDelay(pdMS_TO_TICKS(50));

    // Soft-detach USB-OTG. Deliberately not tinyusb_driver_uninstall(): it frees
    // TinyUSB mutexes/FIFOs while the HID and protocol tasks may still call tud_*,
    // and a crash here reboots straight back into the app.
    tud_disconnect();
    vTaskDelay(pdMS_TO_TICKS(10));

    // Route the internal FSLS PHY back to USJ
    CLEAR_PERI_REG_MASK(RTC_CNTL_USB_CONF_REG,
                        RTC_CNTL_SW_HW_USB_PHY_SEL | RTC_CNTL_SW_USB_PHY_SEL | RTC_CNTL_USB_PAD_ENABLE);
    usb_serial_jtag_ll_phy_enable_external(false);
    usb_serial_jtag_ll_phy_enable_pad(true);

    // Hold SE0 (no pull-up, both lines pulled down) long enough for the hub to report a detach
    const usb_serial_jtag_pull_override_vals_t detach = {
        .dp_pu = false, .dm_pu = false, .dp_pd = true, .dm_pd = true,
    };
    usb_serial_jtag_ll_phy_enable_pull_override(&detach);
    vTaskDelay(pdMS_TO_TICKS(300));

    // Re-attach with the D+ pull-up and wait for the host's bus reset. dp_pullup
    // must be restored to 1 before dropping the override: the hardware keeps
    // using that bit, and leaving it 0 means the host never sees the device.
    usb_serial_jtag_ll_clr_intsts_mask(USB_SERIAL_JTAG_INTR_BUS_RESET);
    const usb_serial_jtag_pull_override_vals_t attach = {
        .dp_pu = true, .dm_pu = false, .dp_pd = false, .dm_pd = false,
    };
    usb_serial_jtag_ll_phy_enable_pull_override(&attach);
    usb_serial_jtag_ll_phy_disable_pull_override();
    bool got_reset = false;
    for (int i = 0; i < 150 && !got_reset; i++) {
        vTaskDelay(pdMS_TO_TICKS(10));
        got_reset = (usb_serial_jtag_ll_get_intraw_mask() & USB_SERIAL_JTAG_INTR_BUS_RESET) != 0;
    }
    if (!got_reset) {
        ESP_LOGW(TAG, "No USJ bus reset seen from host, restarting anyway");
    }

    // ROM reads this after the CPU reset done by esp_restart() and stays in download mode
    REG_WRITE(RTC_CNTL_OPTION1_REG, RTC_CNTL_FORCE_DOWNLOAD_BOOT);
    esp_restart();
}

/*
 * The plain-text "BOOTLOADER" command is only honoured when it is the entire
 * read ("BOOTLOADER", optionally followed by \r/\n) and arrives between protocol
 * frames. Searching the stream for the text instead would reboot the pad whenever
 * a protobuf payload (e.g. a song title) happened to contain it. At a frame
 * boundary those bytes would decode as an oversized length prefix, so a valid
 * frame can never be mistaken for the command.
 */
/*
 * Plain-text commands ("BOOTLOADER", "FREAKY67") are only honoured when the
 * command is the entire read, optionally followed by \r/\n, and arrives between
 * protocol frames. Searching the stream for the text instead would fire whenever
 * a protobuf payload (e.g. a song title) happened to contain it. At a frame
 * boundary neither framing's header can read as ASCII text (a marked frame
 * starts 0xAA 0x55, a legacy one has two zero bytes in its length), so a
 * valid frame can never be mistaken for a command.
 */
static bool is_text_command(const uint8_t *buf, size_t len, const char *cmd)
{
    const size_t cmd_len = strlen(cmd);

    if (len < cmd_len || memcmp(buf, cmd, cmd_len) != 0) {
        return false;
    }
    for (size_t i = cmd_len; i < len; i++) {
        if (buf[i] != '\r' && buf[i] != '\n') {
            return false;
        }
    }
    return true;
}

static void usb_cdc_task_poll(void)
{
    if (s_rx_reset_requested) {
        s_rx_reset_requested = false;
        protocol_reset_rx();
    }

    if (s_need_bootloader_reboot) {
        s_need_bootloader_reboot = false;
        reboot_to_rom_download();
    }

    uint8_t rx_buf[256];
    uint32_t count;
    while (tud_cdc_n_available(0) && (count = tud_cdc_n_read(0, rx_buf, sizeof(rx_buf))) > 0) {
        if (protocol_rx_idle() && is_text_command(rx_buf, count, "FREAKY67")) {
            ESP_LOGI(TAG, "CDC serial command received: triggering easter egg!");
            ui_trigger_easter_egg();
            continue;
        }
        if (protocol_rx_idle() && is_text_command(rx_buf, count, "BOOTLOADER")) {
            ESP_LOGI(TAG, "CDC serial command received: entering bootloader...");
            schedule_bootloader_reboot();
            return;
        }
        protocol_feed_cdc_bytes(rx_buf, count);
    }

    protocol_drain_diag_logs();
}

// Protocol/CDC handling lives on core 1, away from the key ISR and USB stack on core 0.
// Woken by tud_cdc_rx_cb instead of polling; the timeout is only a safety net.
static void usb_cdc_task(void *arg)
{
    (void)arg;
    while (1) {
        ulTaskNotifyTake(pdTRUE, pdMS_TO_TICKS(200));
        usb_cdc_task_poll();
    }
}

esp_err_t usb_cdc_start_task(void)
{
    BaseType_t res = xTaskCreatePinnedToCore(usb_cdc_task, "cdc_proto", 6144, NULL,
                                             tskIDLE_PRIORITY + 3, &s_cdc_task, 1);
    return res == pdPASS ? ESP_OK : ESP_FAIL;
}

// TinyUSB CDC Callbacks
void tud_cdc_rx_cb(uint8_t itf)
{
    (void)itf;
    if (s_cdc_task) {
        xTaskNotifyGive(s_cdc_task);
    }
}

void tud_cdc_line_state_cb(uint8_t itf, bool dtr, bool rts)
{
    (void)itf;
    if (dtr != s_cdc_connected) {
        if (dtr) {
            diag_record(DIAG_EVENT_CDC_OPENED, 1 /* INFO */, 0, 0);
        } else {
            diag_record(DIAG_EVENT_CDC_CLOSED, 1 /* INFO */, 0, 0);
        }
    }
    s_cdc_connected = dtr;
    ESP_LOGD(TAG, "CDC line state: DTR=%d, RTS=%d", dtr, rts);

    if (!dtr) {
        s_rx_reset_requested = true;
        if (s_cdc_task) {
            xTaskNotifyGive(s_cdc_task);
        }
    }

    // 1. Magic 1200 baud touch armed: clearing DTR (host closing port) triggers download mode
    if (s_download_mode_armed && !dtr) {
        ESP_LOGI(TAG, "Port closed after 1200 baud touch, entering bootloader...");
        schedule_bootloader_reboot();
    }

    // No reset on the esptool RTS/DTR pattern: ModemManager and generic serial
    // probes toggle those lines when they open any tty, and each rebooted the
    // pad into download mode. The command and the 1200-baud touch remain.
}

void tud_cdc_line_coding_cb(uint8_t itf, cdc_line_coding_t const* p_line_coding)
{
    if (p_line_coding && p_line_coding->bit_rate == 1200) {
        ESP_LOGI(TAG, "1200 baud touch detected, arming download mode reset");
        s_download_mode_armed = true;
        // Fire immediately if host already has DTR low
        if (!s_cdc_connected) {
            schedule_bootloader_reboot();
        }
        return;
    }
    s_download_mode_armed = false;
}
