#include "protocol.h"
#include "protocol/nanopb/pb_encode.h"
#include "protocol/nanopb/pb_decode.h"
#include "usb/usb_cdc.h"
#include "usb/usb_hid.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "input/keypad.h"
#include "counters/counters.h"
#include "display/display.h"
#include "runtime/runtime.h"
#include "esp_mac.h"
#include "esp_log.h"
#include <string.h>

static const char *TAG = "protocol";

static uint8_t s_rx_frame_buf[PROTOCOL_MAX_FRAME_SIZE];
static size_t s_rx_frame_len = 0;
static uint32_t s_out_sequence = 1;

static void get_device_mac_string(char *buf, size_t max_len)
{
    uint8_t mac[6] = {0};
    esp_read_mac(mac, ESP_MAC_WIFI_STA);
    snprintf(buf, max_len, "OSUPAD-%02X%02X%02X%02X%02X%02X",
             mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

esp_err_t protocol_encode_device_message(const osupad_DeviceToHost *msg, uint8_t *out_buf, size_t max_len, size_t *out_len)
{
    if (!msg || !out_buf || !out_len || max_len < 5) {
        return ESP_ERR_INVALID_ARG;
    }

    pb_ostream_t stream = pb_ostream_from_buffer(out_buf + 4, max_len - 4);
    if (!pb_encode(&stream, osupad_DeviceToHost_fields, msg)) {
        ESP_LOGE(TAG, "NanoPB encode failed: %s", PB_GET_ERROR(&stream));
        return ESP_FAIL;
    }

    uint32_t payload_len = (uint32_t)stream.bytes_written;
    out_buf[0] = (uint8_t)(payload_len & 0xFF);
    out_buf[1] = (uint8_t)((payload_len >> 8) & 0xFF);
    out_buf[2] = (uint8_t)((payload_len >> 16) & 0xFF);
    out_buf[3] = (uint8_t)((payload_len >> 24) & 0xFF);

    *out_len = 4 + payload_len;
    return ESP_OK;
}

bool protocol_decode_host_message(const uint8_t *payload, size_t payload_len, osupad_HostToDevice *out_msg)
{
    if (!payload || !out_msg) {
        return false;
    }

    pb_istream_t stream = pb_istream_from_buffer(payload, payload_len);
    if (!pb_decode(&stream, osupad_HostToDevice_fields, out_msg)) {
        ESP_LOGE(TAG, "NanoPB decode failed: %s", PB_GET_ERROR(&stream));
        return false;
    }

    return true;
}

static esp_err_t send_envelope(const osupad_DeviceToHost *msg)
{
    uint8_t tx_buf[512];
    size_t tx_len = 0;

    esp_err_t err = protocol_encode_device_message(msg, tx_buf, sizeof(tx_buf), &tx_len);
    if (err != ESP_OK) {
        return err;
    }

    size_t written = usb_cdc_write(tx_buf, tx_len);
    if (written < tx_len) {
        ESP_LOGW(TAG, "CDC TX dropped bytes (%zu / %zu)", written, tx_len);
        return ESP_FAIL;
    }

    return ESP_OK;
}

esp_err_t protocol_send_hello_ack(uint32_t seq)
{
    counters_snapshot_t snap;
    counters_get(&snap);

    char dev_id[32];
    get_device_mac_string(dev_id, sizeof(dev_id));

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_hello_ack_tag;
    msg.payload.hello_ack.protocol_version = osupad_ProtocolVersion_PROTOCOL_VERSION_V1;
    strncpy(msg.payload.hello_ack.firmware_version, "1.0.0", sizeof(msg.payload.hello_ack.firmware_version) - 1);
    strncpy(msg.payload.hello_ack.board_profile, "waveshare_esp32s3_touch_lcd_2", sizeof(msg.payload.hello_ack.board_profile) - 1);
    strncpy(msg.payload.hello_ack.device_id, dev_id, sizeof(msg.payload.hello_ack.device_id) - 1);
    msg.payload.hello_ack.counter_generation = snap.generation;
    msg.payload.hello_ack.lifetime_key1 = snap.lifetime_key1;
    msg.payload.hello_ack.lifetime_key2 = snap.lifetime_key2;

    ESP_LOGI(TAG, "Sending HelloAck to host (Firmware: 1.0.0, Gen: %lu)", (unsigned long)snap.generation);
    return send_envelope(&msg);
}

esp_err_t protocol_send_status(void)
{
    counters_snapshot_t snap;
    counters_get(&snap);

    keypad_config_t cfg;
    keypad_get_config(&cfg);

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_status_tag;
    msg.payload.status.uptime_seconds = runtime_get_uptime_seconds();
    msg.payload.status.state = (osupad_DeviceState)runtime_get_state();
    msg.payload.status.brightness = board_backlight_get();
    msg.payload.status.display_asleep = display_is_asleep();
    msg.payload.status.lifetime_key1 = snap.lifetime_key1;
    msg.payload.status.lifetime_key2 = snap.lifetime_key2;
    msg.payload.status.map_key1 = cfg.keycode1;
    msg.payload.status.map_key2 = cfg.keycode2;

    return send_envelope(&msg);
}

esp_err_t protocol_send_config_ack(uint32_t seq, bool success, const char *text)
{
    keypad_config_t cfg;
    keypad_get_config(&cfg);

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_config_ack_tag;
    msg.payload.config_ack.success = success;
    if (text) {
        strncpy(msg.payload.config_ack.message, text, sizeof(msg.payload.config_ack.message) - 1);
    }
    msg.payload.config_ack.has_current_config = true;
    msg.payload.config_ack.current_config.key1_hid_usage = cfg.keycode1;
    msg.payload.config_ack.current_config.key2_hid_usage = cfg.keycode2;
    msg.payload.config_ack.current_config.debounce_us = (uint32_t)cfg.debounce_ms * 1000;
    msg.payload.config_ack.current_config.brightness = board_backlight_get();

    return send_envelope(&msg);
}

esp_err_t protocol_send_counter_sync_resp(uint32_t seq, bool success)
{
    counters_snapshot_t snap;
    counters_get(&snap);

    char dev_id[32];
    get_device_mac_string(dev_id, sizeof(dev_id));

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_counter_sync_resp_tag;
    msg.payload.counter_sync_resp.success = success;
    msg.payload.counter_sync_resp.has_synchronized_state = true;
    strncpy(msg.payload.counter_sync_resp.synchronized_state.device_id, dev_id, sizeof(msg.payload.counter_sync_resp.synchronized_state.device_id) - 1);
    msg.payload.counter_sync_resp.synchronized_state.counter_generation = snap.generation;
    msg.payload.counter_sync_resp.synchronized_state.lifetime_key1 = snap.lifetime_key1;
    msg.payload.counter_sync_resp.synchronized_state.lifetime_key2 = snap.lifetime_key2;

    return send_envelope(&msg);
}

static void handle_host_message(const osupad_HostToDevice *msg)
{
    switch (msg->which_payload) {
    case osupad_HostToDevice_hello_tag:
        protocol_send_hello_ack(msg->sequence_number);
        break;

    case osupad_HostToDevice_set_config_tag:
        if (msg->payload.set_config.has_config) {
            const osupad_ConfigPayload *c = &msg->payload.set_config.config;
            keypad_config_t cfg;
            keypad_get_config(&cfg);

            if (c->key1_hid_usage > 0) cfg.keycode1 = (uint8_t)c->key1_hid_usage;
            if (c->key2_hid_usage > 0) cfg.keycode2 = (uint8_t)c->key2_hid_usage;
            if (c->debounce_us > 0) cfg.debounce_ms = (uint16_t)(c->debounce_us / 1000);

            keypad_set_config(&cfg);
            usb_hid_set_keycodes(cfg.keycode1, cfg.keycode2);

            if (c->brightness > 0 && c->brightness <= 100) {
                display_set_brightness((uint8_t)c->brightness);
            }
            if (c->display_sleep_seconds > 0) {
                display_set_sleep_timeout(c->display_sleep_seconds);
            }

            protocol_send_config_ack(msg->sequence_number, true, "Configuration applied successfully");
        }
        break;

    case osupad_HostToDevice_time_sync_tag:
        display_set_time(
            msg->payload.time_sync.year,
            msg->payload.time_sync.month,
            msg->payload.time_sync.day,
            msg->payload.time_sync.hour,
            msg->payload.time_sync.minute,
            msg->payload.time_sync.second
        );
        break;

    case osupad_HostToDevice_gameplay_state_tag:
        display_update_gameplay_state(&msg->payload.gameplay_state);
        runtime_notify_gameplay(true);
        break;

    case osupad_HostToDevice_counter_sync_tag:
        if (msg->payload.counter_sync.has_target_state) {
            const osupad_CounterState *tgt = &msg->payload.counter_sync.target_state;
            esp_err_t err = counters_sync_from_host(
                tgt->counter_generation,
                tgt->lifetime_key1,
                tgt->lifetime_key2,
                msg->payload.counter_sync.force_restore
            );
            protocol_send_counter_sync_resp(msg->sequence_number, (err == ESP_OK));
        }
        break;

    case osupad_HostToDevice_request_status_tag:
        protocol_send_status();
        break;

    default:
        ESP_LOGD(TAG, "Unhandled host message payload tag: %d", msg->which_payload);
        break;
    }
}

void protocol_feed_cdc_bytes(const uint8_t *data, size_t len)
{
    if (!data || len == 0) return;

    if (s_rx_frame_len + len > sizeof(s_rx_frame_buf)) {
        ESP_LOGW(TAG, "RX buffer overflow, resetting framing state");
        s_rx_frame_len = 0;
        return;
    }

    memcpy(s_rx_frame_buf + s_rx_frame_len, data, len);
    s_rx_frame_len += len;

    // Process all complete frames in buffer
    while (s_rx_frame_len >= 4) {
        uint32_t expected_len = (uint32_t)s_rx_frame_buf[0] |
                                ((uint32_t)s_rx_frame_buf[1] << 8) |
                                ((uint32_t)s_rx_frame_buf[2] << 16) |
                                ((uint32_t)s_rx_frame_buf[3] << 24);

        if (expected_len > PROTOCOL_MAX_FRAME_SIZE - 4) {
            ESP_LOGE(TAG, "Frame length exceeds max allowed (%lu bytes), dropping buffer", (unsigned long)expected_len);
            s_rx_frame_len = 0;
            return;
        }

        if (s_rx_frame_len < 4 + expected_len) {
            // Wait for rest of frame
            break;
        }

        osupad_HostToDevice msg = osupad_HostToDevice_init_zero;
        if (protocol_decode_host_message(s_rx_frame_buf + 4, expected_len, &msg)) {
            handle_host_message(&msg);
        }

        size_t consumed = 4 + expected_len;
        size_t remaining = s_rx_frame_len - consumed;
        if (remaining > 0) {
            memmove(s_rx_frame_buf, s_rx_frame_buf + consumed, remaining);
        }
        s_rx_frame_len = remaining;
    }
}
