#include "protocol.h"
#include "protocol/nanopb/pb_encode.h"
#include "protocol/nanopb/pb_decode.h"
#include "usb/usb_cdc.h"
#include "usb/usb_hid.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "input/keypad.h"
#include "counters/counters.h"
#include "ui/ui.h"
#include "ui/ui_store.h"
#include "runtime/runtime.h"
#include "input/latency_stats.h"
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

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_status_tag;
    msg.payload.status.uptime_seconds = runtime_get_uptime_seconds();
    msg.payload.status.state = (osupad_DeviceState)runtime_get_state();
    msg.payload.status.brightness = board_backlight_get();
    msg.payload.status.display_asleep = ui_is_asleep();
    msg.payload.status.lifetime_key1 = snap.lifetime_key1;
    msg.payload.status.lifetime_key2 = snap.lifetime_key2;
    keypad_get_map_presses(&msg.payload.status.map_key1, &msg.payload.status.map_key2);

    latency_stats_t lat;
    latency_stats_get(&lat);
    msg.payload.status.latency_samples = lat.samples;
    msg.payload.status.latency_p50_us = lat.p50_us;
    msg.payload.status.latency_p99_us = lat.p99_us;
    msg.payload.status.latency_p999_us = lat.p999_us;
    msg.payload.status.latency_max_us = lat.max_us;
    msg.payload.status.hid_deferred_reports = lat.deferred_reports;

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

esp_err_t protocol_send_layout_ack(uint32_t seq, uint32_t screen, bool success, const char *text)
{
    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_layout_ack_tag;
    msg.payload.layout_ack.screen = screen;
    msg.payload.layout_ack.success = success;
    if (text) {
        strncpy(msg.payload.layout_ack.message, text, sizeof(msg.payload.layout_ack.message) - 1);
    }
    return send_envelope(&msg);
}

static void layout_from_proto(const osupad_SetLayout *in, ui_layout_t *out)
{
    memset(out, 0, sizeof(*out));
    out->background = in->background;
    out->count = in->widgets_count > UI_MAX_WIDGETS ? UI_MAX_WIDGETS : in->widgets_count;
    for (int i = 0; i < out->count; i++) {
        const osupad_UiWidget *src = &in->widgets[i];
        ui_widget_t *w = &out->widgets[i];
        // Out-of-range values are clamped into ones the validator rejects
        w->kind = src->kind > UINT8_MAX ? UINT8_MAX : src->kind;
        w->source = src->source > UINT8_MAX ? UINT8_MAX : src->source;
        w->font = src->font > UINT8_MAX ? UINT8_MAX : src->font;
        w->align = src->align > UINT8_MAX ? UINT8_MAX : src->align;
        w->x = (int16_t)src->x;
        w->y = (int16_t)src->y;
        w->w = src->w > INT16_MAX ? 0 : (int16_t)src->w;
        w->h = src->h > INT16_MAX ? 0 : (int16_t)src->h;
        w->fg = src->fg;
        w->bg = src->bg;
        w->accent = src->accent;
        w->radius = src->radius > UINT8_MAX ? UINT8_MAX : src->radius;
        w->decimals = src->decimals > UINT8_MAX ? UINT8_MAX : src->decimals;
        w->flags = src->flags > UINT8_MAX ? 0 : src->flags;
        strncpy(w->label, src->label, UI_LABEL_MAX - 1);
        strncpy(w->suffix, src->suffix, UI_SUFFIX_MAX - 1);
    }
}

// Last attempt id seen from the host; a new id means a new attempt
static uint32_t s_play_id = 0;

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
                ui_set_brightness((uint8_t)c->brightness);
            }
            if (c->display_sleep_seconds > 0) {
                ui_set_sleep_timeout(c->display_sleep_seconds);
            }

            protocol_send_config_ack(msg->sequence_number, true, "Configuration applied successfully");
        }
        break;

    case osupad_HostToDevice_time_sync_tag:
        ui_set_time(
            msg->payload.time_sync.year,
            msg->payload.time_sync.month,
            msg->payload.time_sync.day,
            msg->payload.time_sync.hour,
            msg->payload.time_sync.minute,
            msg->payload.time_sync.second
        );
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

    case osupad_HostToDevice_reset_latency_stats_tag:
        latency_stats_reset();
        break;

    case osupad_HostToDevice_host_status_tag: {
        const osupad_HostStatus *hs = &msg->payload.host_status;
        if (hs->play_id != s_play_id) {
            s_play_id = hs->play_id;
            keypad_reset_map_presses();
        }
        ui_set_tosu_connected(hs->tosu_connected);
        runtime_notify_gameplay(hs->playing);
        break;
    }

    case osupad_HostToDevice_set_layout_tag: {
        static ui_layout_t layout;  // ~2 KB, protocol task only
        const osupad_SetLayout *sl = &msg->payload.set_layout;
        char err[64] = "";
        layout_from_proto(sl, &layout);
        bool ok = ui_set_layout((uint8_t)sl->screen, &layout, err, sizeof(err));
        if (ok) {
            if (runtime_get_state() == OSUPAD_STATE_PLAYING) {
                // Flash writes would stall the key core: apply now, the host resends later
                snprintf(err, sizeof(err), "applied, not saved while playing");
            } else if (ui_store_save((uint8_t)sl->screen, &layout) != ESP_OK) {
                snprintf(err, sizeof(err), "applied, but saving to flash failed");
            }
        }
        protocol_send_layout_ack(msg->sequence_number, sl->screen, ok, err);
        break;
    }

    case osupad_HostToDevice_reset_layout_tag: {
        uint32_t screen = msg->payload.reset_layout;
        const ui_layout_t *def = ui_default_layout(screen <= UINT8_MAX ? (uint8_t)screen : UINT8_MAX);
        char err[64] = "";
        bool ok = def && ui_set_layout((uint8_t)screen, def, err, sizeof(err));
        if (ok && runtime_get_state() != OSUPAD_STATE_PLAYING) {
            ui_store_erase((uint8_t)screen);
        }
        protocol_send_layout_ack(msg->sequence_number, screen, ok, def ? err : "unknown screen");
        break;
    }

    case osupad_HostToDevice_data_update_tag: {
        const osupad_DataUpdate *du = &msg->payload.data_update;
        ui_lock();
        for (pb_size_t i = 0; i < du->values_count; i++) {
            const osupad_DataValue *v = &du->values[i];
            if (v->source > UINT8_MAX) {
                continue;
            }
            switch (v->which_value) {
            case osupad_DataValue_number_tag: ui_data_set_number((uint8_t)v->source, v->value.number); break;
            case osupad_DataValue_text_tag:   ui_data_set_string((uint8_t)v->source, v->value.text); break;
            default:                          ui_data_clear((uint8_t)v->source); break;
            }
        }
        ui_unlock();
        break;
    }

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

        // Static: a DataUpdate makes the decoded message several KB (single protocol task)
        static osupad_HostToDevice msg;
        msg = (osupad_HostToDevice)osupad_HostToDevice_init_zero;
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

void protocol_reset_rx(void)
{
    s_rx_frame_len = 0;
}

bool protocol_rx_idle(void)
{
    return s_rx_frame_len == 0;
}
