#include "protocol.h"
#include "protocol/nanopb/pb_encode.h"
#include "protocol/nanopb/pb_decode.h"
#include "usb/usb_cdc.h"
#include "usb/usb_hid.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "config/device_config.h"
#include "input/keypad.h"
#include "counters/counters.h"
#include "ui/ui.h"
#include "ui/ui_store.h"
#include "runtime/runtime.h"
#include "input/latency_stats.h"
#include "esp_mac.h"
#include "esp_timer.h"
#include "esp_log.h"
#include "esp_app_desc.h"
#include "esp_ota_ops.h"
#include "diag/diag.h"
#include "frame_parser.h"
#include <string.h>

static const char *TAG = "protocol";

static frame_parser_t s_parser;
// Host frames are written in one go, so a partial frame that sits this long is
// a false start (stray AA 55 + length): drop it rather than wait for bytes
// that swallow the real frames behind it
#define RX_STALE_US 500000
static int64_t s_last_rx_us = 0;
// Framing of the host on the other end, so a host that predates the AA 55
// marker is answered in the framing it parses. Latched once per connection from
// the first frame that decodes to a message we know, then the parser is locked
// to it: a false header of the other framing inside a payload can no longer
// capture the parser or flip our replies. Cleared only by protocol_reset_rx.
// Until the host has sent anything we do not know, and send nothing: an
// unsolicited frame in the wrong framing would desynchronise an old host.
// Protocol state is owned by the CDC task.
static bool s_host_framing_known = false;
static frame_format_t s_host_framing = FRAME_FORMAT_MARKED;
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
    if (!msg || !out_buf || !out_len || max_len <= FRAME_HEADER_SIZE) {
        return ESP_ERR_INVALID_ARG;
    }

    size_t room = max_len - FRAME_HEADER_SIZE;
    if (room > FRAME_MAX_PAYLOAD) {
        room = FRAME_MAX_PAYLOAD;
    }
    pb_ostream_t stream = pb_ostream_from_buffer(out_buf + FRAME_HEADER_SIZE, room);
    if (!pb_encode(&stream, osupad_DeviceToHost_fields, msg)) {
        ESP_LOGE(TAG, "NanoPB encode failed: %s", PB_GET_ERROR(&stream));
        return ESP_FAIL;
    }

    frame_write_header(out_buf, (uint16_t)stream.bytes_written, s_host_framing);
    *out_len = FRAME_HEADER_SIZE + stream.bytes_written;
    return ESP_OK;
}

bool protocol_decode_host_message(const uint8_t *payload, size_t payload_len, osupad_HostToDevice *out_msg)
{
    if (!payload || !out_msg) {
        return false;
    }

    pb_istream_t stream = pb_istream_from_buffer(payload, payload_len);
    if (!pb_decode(&stream, osupad_HostToDevice_fields, out_msg)) {
        diag_record(DIAG_EVENT_DECODE_FAILED, 3 /* ERROR */, (uint32_t)payload_len, 0);
        ESP_LOGE(TAG, "NanoPB decode failed: %s", PB_GET_ERROR(&stream));
        return false;
    }

    return true;
}

static esp_err_t send_envelope(const osupad_DeviceToHost *msg)
{
    if (!s_host_framing_known) {
        return ESP_ERR_INVALID_STATE;
    }

    uint8_t tx_buf[1024];
    size_t tx_len = 0;
    _Static_assert(sizeof(tx_buf) <= CONFIG_TINYUSB_CDC_TX_BUFSIZE,
                   "a whole frame must fit the CDC TX FIFO to be sent atomically");

    esp_err_t err = protocol_encode_device_message(msg, tx_buf, sizeof(tx_buf), &tx_len);
    if (err != ESP_OK) {
        return err;
    }

    size_t written = usb_cdc_write(tx_buf, tx_len);
    if (written < tx_len) {
        diag_record(DIAG_EVENT_CDC_WRITE_DROPPED, 2 /* WARN */, (uint32_t)tx_len, (uint32_t)(tx_len - written));
        ESP_LOGW(TAG, "CDC TX frame dropped (%zu bytes, FIFO full)", tx_len);
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
    const esp_app_desc_t *app_desc = esp_app_get_description();
    msg.which_payload = osupad_DeviceToHost_hello_ack_tag;
    msg.payload.hello_ack.protocol_version = osupad_ProtocolVersion_PROTOCOL_VERSION_V1;
    strncpy(msg.payload.hello_ack.firmware_version, app_desc->version, sizeof(msg.payload.hello_ack.firmware_version) - 1);
    strncpy(msg.payload.hello_ack.board_profile, "waveshare_esp32s3_touch_lcd_2", sizeof(msg.payload.hello_ack.board_profile) - 1);
    strncpy(msg.payload.hello_ack.device_id, dev_id, sizeof(msg.payload.hello_ack.device_id) - 1);
    msg.payload.hello_ack.counter_generation = snap.generation;
    msg.payload.hello_ack.lifetime_key1 = snap.lifetime_key1;
    msg.payload.hello_ack.lifetime_key2 = snap.lifetime_key2;
    // §W3-2: all zero until a host claims the pad, which is what the host
    // reads as "unclaimed" and claims silently on first connect (§W3-3)
    msg.payload.hello_ack.owner_id.size = OWNER_ID_LEN;
    device_config_get_owner(msg.payload.hello_ack.owner_id.bytes);

    // §U-3a: which of the two OTA slots this image booted from. A pad still on
    // the old single-app layout answers "factory"; the host uses this to tell
    // an OTA-capable pad from one that needs a serial reflash first.
    const esp_partition_t *running = esp_ota_get_running_partition();
    if (running) {
        strncpy(msg.payload.hello_ack.running_partition, running->label,
                sizeof(msg.payload.hello_ack.running_partition) - 1);
    }

    device_config_data_t cfg;
    device_config_get(&cfg);
    msg.payload.hello_ack.has_current_config = true;
    msg.payload.hello_ack.current_config.key1_hid_usage = cfg.key1_usage;
    msg.payload.hello_ack.current_config.key2_hid_usage = cfg.key2_usage;
    msg.payload.hello_ack.current_config.debounce_us = cfg.debounce_us;
    msg.payload.hello_ack.current_config.brightness = cfg.brightness;
    msg.payload.hello_ack.current_config.display_sleep_seconds = cfg.sleep_s;
    msg.payload.hello_ack.current_config.gameplay_display_hz = cfg.gameplay_display_hz ? cfg.gameplay_display_hz : 10;
    msg.payload.hello_ack.current_config.press_color_rgb = 0;
    msg.payload.hello_ack.current_config.key1_gpio = cfg.key1_gpio;
    msg.payload.hello_ack.current_config.key2_gpio = cfg.key2_gpio;

    ESP_LOGI(TAG, "Sending HelloAck to host (Firmware: %s, Gen: %lu, Partition: %s, K1: GPIO%lu, K2: GPIO%lu)",
             app_desc->version, (unsigned long)snap.generation,
             msg.payload.hello_ack.running_partition,
             (unsigned long)cfg.key1_gpio, (unsigned long)cfg.key2_gpio);
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
    msg.payload.status.display_ok = ui_is_ok();
    msg.payload.status.nvs_ok = counters_is_nvs_ok();
    msg.payload.status.nvs_writes = counters_get_nvs_writes();

    return send_envelope(&msg);
}

esp_err_t protocol_send_config_ack(uint32_t seq, bool success, const char *text)
{
    device_config_data_t cfg;
    device_config_get(&cfg);

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_config_ack_tag;
    msg.payload.config_ack.success = success;
    if (text) {
        strncpy(msg.payload.config_ack.message, text, sizeof(msg.payload.config_ack.message) - 1);
    }
    msg.payload.config_ack.has_current_config = true;
    msg.payload.config_ack.current_config.key1_hid_usage = cfg.key1_usage;
    msg.payload.config_ack.current_config.key2_hid_usage = cfg.key2_usage;
    msg.payload.config_ack.current_config.debounce_us = cfg.debounce_us;
    msg.payload.config_ack.current_config.brightness = cfg.brightness;
    msg.payload.config_ack.current_config.display_sleep_seconds = cfg.sleep_s;
    msg.payload.config_ack.current_config.gameplay_display_hz = cfg.gameplay_display_hz ? cfg.gameplay_display_hz : 10;
    msg.payload.config_ack.current_config.press_color_rgb = 0;
    msg.payload.config_ack.current_config.key1_gpio = cfg.key1_gpio;
    msg.payload.config_ack.current_config.key2_gpio = cfg.key2_gpio;

    return send_envelope(&msg);
}

esp_err_t protocol_send_counter_sync_resp(uint32_t seq, bool success, const char *msg_text)
{
    counters_snapshot_t snap;
    counters_get(&snap);

    char dev_id[32];
    get_device_mac_string(dev_id, sizeof(dev_id));

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_counter_sync_resp_tag;
    msg.payload.counter_sync_resp.success = success;
    if (msg_text && msg_text[0]) {
        strncpy(msg.payload.counter_sync_resp.message, msg_text, sizeof(msg.payload.counter_sync_resp.message) - 1);
    }
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

esp_err_t protocol_send_detect_pin_resp(uint32_t seq, uint32_t key_id, uint32_t gpio, bool success)
{
    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_detect_pin_resp_tag;
    msg.payload.detect_pin_resp.key_id = key_id;
    msg.payload.detect_pin_resp.gpio = gpio;
    msg.payload.detect_pin_resp.success = success;
    return send_envelope(&msg);
}

esp_err_t protocol_send_log_batch(void)
{
    if (!usb_cdc_is_connected()) {
        return ESP_ERR_INVALID_STATE;
    }

    uint32_t dropped = 0;
    diag_entry_t entries[8];
    size_t count = diag_drain(entries, 8, &dropped);

    // If there was an overflow drop, and we have space in this batch, synthesize an overflow event
    if (dropped > 0 && count < 8) {
        entries[count].timestamp_ms = (uint32_t)(esp_timer_get_time() / 1000);
        entries[count].event_id = DIAG_EVENT_BUFFER_OVERFLOW;
        entries[count].level = 2; // WARN
        entries[count].arg0 = dropped;
        entries[count].arg1 = 0;
        count++;
    }

    if (count == 0) {
        return ESP_OK;
    }

    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_log_batch_tag;
    msg.payload.log_batch.events_count = (pb_size_t)count;

    for (size_t i = 0; i < count; i++) {
        msg.payload.log_batch.events[i].timestamp_ms = entries[i].timestamp_ms;
        msg.payload.log_batch.events[i].level = (osupad_LogLevel)entries[i].level;
        msg.payload.log_batch.events[i].event_id = entries[i].event_id;
        msg.payload.log_batch.events[i].arg0 = entries[i].arg0;
        msg.payload.log_batch.events[i].arg1 = entries[i].arg1;
        msg.payload.log_batch.events[i].tag[0] = '\0';
        msg.payload.log_batch.events[i].message[0] = '\0';
    }

    return send_envelope(&msg);
}

void protocol_drain_diag_logs(void)
{
    if (!usb_cdc_is_connected() || !s_host_framing_known) {
        return;
    }
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return;
    }

    uint32_t outlier_max = 0;
    uint32_t outlier_count = 0;
    if (latency_stats_drain_outlier(&outlier_max, &outlier_count)) {
        diag_record(DIAG_EVENT_LATENCY_OUTLIER, 2 /* WARN */, outlier_max, outlier_count);
    }

    static int64_t s_last_drain_us = 0;
    int64_t now_us = esp_timer_get_time();
    if (now_us - s_last_drain_us < 1000000) { // 1 second rate limit
        return;
    }
    s_last_drain_us = now_us;

    for (int b = 0; b < 8 && diag_available() > 0; b++) {
        if (protocol_send_log_batch() != ESP_OK) {
            break;
        }
    }
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

#define DETECT_PIN_DEFAULT_MS 10000
// The host waits timeout + 2 s; the pad should never scan longer than a person would
#define DETECT_PIN_MAX_MS 30000
// Pin scan awaiting its answer (id 0: none)
static struct {
    uint32_t id;
    uint32_t seq;
    uint32_t key_id;
} s_detect;

static void handle_host_message(const osupad_HostToDevice *msg)
{
    switch (msg->which_payload) {
    case osupad_HostToDevice_hello_tag:
        protocol_send_hello_ack(msg->sequence_number);
        break;

    case osupad_HostToDevice_claim_ownership_tag: {
        // §W3-2. The host prompts the user before sending this, so the
        // firmware records it rather than arbitrating between hosts. It is an
        // NVS write, so it is refused outright while a map is running (P1-3)
        // instead of being deferred: the host claims at connect time, which is
        // already IDLE, and a deferred claim would be a silent one.
        const osupad_ClaimOwnership_owner_id_t *req = &msg->payload.claim_ownership.owner_id;
        if (req->size != OWNER_ID_LEN) {
            ESP_LOGW(TAG, "Ownership claim refused: %u bytes, expected %d",
                     (unsigned)req->size, OWNER_ID_LEN);
            break;
        }
        esp_err_t err = device_config_claim_owner(req->bytes);
        if (err != ESP_OK) {
            ESP_LOGW(TAG, "Ownership claim not applied: %s", esp_err_to_name(err));
        }
        break;
    }

    case osupad_HostToDevice_set_config_tag:
        if (msg->payload.set_config.has_config) {
            const osupad_ConfigPayload *c = &msg->payload.set_config.config;
            device_config_data_t dcfg;
            device_config_get(&dcfg);

            if (c->key1_hid_usage > 0) dcfg.key1_usage = c->key1_hid_usage;
            if (c->key2_hid_usage > 0) dcfg.key2_usage = c->key2_hid_usage;
            if (c->debounce_us > 0) dcfg.debounce_us = c->debounce_us;
            dcfg.brightness = c->brightness;
            dcfg.sleep_s = c->display_sleep_seconds;
            if (c->gameplay_display_hz > 0) dcfg.gameplay_display_hz = c->gameplay_display_hz;
            if (c->key1_gpio > 0) dcfg.key1_gpio = c->key1_gpio;
            if (c->key2_gpio > 0) dcfg.key2_gpio = c->key2_gpio;

            char err_msg[64] = "";
            if (!device_config_validate(&dcfg, err_msg, sizeof(err_msg))) {
                diag_record(DIAG_EVENT_CONFIG_REJECTED, 2 /* WARN */, 1, 0);
                protocol_send_config_ack(msg->sequence_number, false, err_msg);
            } else {
                esp_err_t err = device_config_set(&dcfg);
                if (err != ESP_OK) {
                    diag_record(DIAG_EVENT_CONFIG_REJECTED, 2 /* WARN */, (uint32_t)err, 0);
                    protocol_send_config_ack(msg->sequence_number, false, "Failed to persist configuration");
                } else {
                    protocol_send_config_ack(msg->sequence_number, true, "Configuration applied successfully");
                }
            }
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
            char err_msg[64] = "";
            esp_err_t err = counters_sync_from_host(
                tgt->counter_generation,
                tgt->lifetime_key1,
                tgt->lifetime_key2,
                msg->payload.counter_sync.force_restore,
                err_msg,
                sizeof(err_msg)
            );
            if (err != ESP_OK) {
                diag_record(DIAG_EVENT_COUNTER_SYNC_REJECTED, 2 /* WARN */, (uint32_t)err, 0);
            }
            protocol_send_counter_sync_resp(msg->sequence_number, (err == ESP_OK), err_msg);
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
            if (runtime_get_state() != OSUPAD_STATE_IDLE) {
                // Flash writes would stall the key core: apply in RAM now, deferred to IDLE
                ui_store_save((uint8_t)sl->screen, &layout);
                snprintf(err, sizeof(err), "applied, will be saved after gameplay");
            } else if (ui_store_save((uint8_t)sl->screen, &layout) != ESP_OK) {
                snprintf(err, sizeof(err), "applied, but saving to flash failed");
            }
        } else {
            diag_record(DIAG_EVENT_LAYOUT_REJECTED, 2 /* WARN */, sl->screen, 0);
        }
        protocol_send_layout_ack(msg->sequence_number, sl->screen, ok, err);
        break;
    }

    case osupad_HostToDevice_reset_layout_tag: {
        uint32_t screen = msg->payload.reset_layout;
        const ui_layout_t *def = ui_default_layout(screen <= UINT8_MAX ? (uint8_t)screen : UINT8_MAX);
        char err[64] = "";
        bool ok = def && ui_set_layout((uint8_t)screen, def, err, sizeof(err));
        if (ok) {
            if (runtime_get_state() != OSUPAD_STATE_IDLE) {
                ui_store_erase((uint8_t)screen);
                snprintf(err, sizeof(err), "reset applied, will be persisted after gameplay");
            } else if (ui_store_erase((uint8_t)screen) != ESP_OK) {
                snprintf(err, sizeof(err), "reset applied, but erasing from flash failed");
            }
        } else {
            diag_record(DIAG_EVENT_LAYOUT_REJECTED, 2 /* WARN */, screen, 0);
        }
        protocol_send_layout_ack(msg->sequence_number, screen, ok, def ? err : "unknown screen");
        break;
    }

    case osupad_HostToDevice_data_update_tag: {
        const osupad_DataUpdate *du = &msg->payload.data_update;
        if (!ui_lock()) {
            break; // headless: nothing to show it on
        }
        for (pb_size_t i = 0; i < du->values_count; i++) {
            const osupad_DataValue *v = &du->values[i];
            if (v->source == 67) {
                ui_trigger_easter_egg(); // the LVGL lock is recursive
                continue;
            }
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

    case osupad_HostToDevice_request_logs_tag:
        if (msg->payload.request_logs && runtime_get_state() == OSUPAD_STATE_IDLE) {
            for (int b = 0; b < 8 && diag_available() > 0; b++) {
                if (protocol_send_log_batch() != ESP_OK) {
                    break;
                }
            }
        }
        break;

    case osupad_HostToDevice_detect_pin_tag: {
        // The scan runs on the keypad task; protocol_poll_detect_pin answers
        // when it ends. A new request replaces one still scanning, whose
        // host-side wait then times out, as it did when the host gave up.
        const osupad_DetectPinRequest *req = &msg->payload.detect_pin;
        uint32_t timeout = req->timeout_ms > 0 ? req->timeout_ms : DETECT_PIN_DEFAULT_MS;
        if (timeout > DETECT_PIN_MAX_MS) {
            timeout = DETECT_PIN_MAX_MS;
        }
        uint32_t id = keypad_detect_pin_start(timeout, req->exclude_gpio);
        if (id == 0) {
            protocol_send_detect_pin_resp(msg->sequence_number, req->key_id, 0, false);
            break;
        }
        s_detect.id = id;
        s_detect.seq = msg->sequence_number;
        s_detect.key_id = req->key_id;
        break;
    }

    default:
        diag_record(DIAG_EVENT_UNKNOWN_HOST_MSG, 0 /* DEBUG */, msg->which_payload, 0);
        ESP_LOGD(TAG, "Unhandled host message payload tag: %d", msg->which_payload);
        break;
    }
}

static void on_frame_received(const uint8_t *payload, size_t payload_len, void *user_data)
{
    (void)user_data;
    static osupad_HostToDevice msg;
    msg = (osupad_HostToDevice)osupad_HostToDevice_init_zero;
    if (protocol_decode_host_message(payload, payload_len, &msg)) {
        // nanopb accepts most varint garbage with no payload set: only a
        // recognised message is evidence of the host's framing
        if (!s_host_framing_known && msg.which_payload >= osupad_HostToDevice_hello_tag &&
            msg.which_payload <= osupad_HostToDevice_detect_pin_tag) {
            s_host_framing = s_parser.last_format;
            s_host_framing_known = true;
        }
        handle_host_message(&msg);
    }
}

void protocol_feed_cdc_bytes(const uint8_t *data, size_t len)
{
    int64_t now = esp_timer_get_time();
    if (!frame_parser_is_idle(&s_parser) && (now - s_last_rx_us) > RX_STALE_US) {
        ESP_LOGW(TAG, "Dropping stale partial frame (%u bytes)", (unsigned)s_parser.rx_len);
        frame_parser_reset(&s_parser);
    }
    s_last_rx_us = now;

    uint32_t prev_oversized = s_parser.oversized_count;
    frame_accept_t accept = FRAME_ACCEPT_ANY;
    if (s_host_framing_known) {
        accept = s_host_framing == FRAME_FORMAT_LEGACY ? FRAME_ACCEPT_LEGACY : FRAME_ACCEPT_MARKED;
    }
    frame_parser_feed(&s_parser, data, len, accept, on_frame_received, NULL);
    if (s_parser.oversized_count > prev_oversized) {
        diag_record(DIAG_EVENT_FRAME_TOO_LARGE, 2 /* WARN */, 0, 0);
        ESP_LOGW(TAG, "Header with an impossible length skipped (resync)");
    }
}

void protocol_reset_rx(void)
{
    frame_parser_reset(&s_parser);
    // Called when the host closes the port: the next one may frame differently
    s_host_framing_known = false;
}

void protocol_poll_detect_pin(void)
{
    int pin;
    if (s_detect.id == 0 || !keypad_detect_pin_result(s_detect.id, &pin)) {
        return;
    }
    bool ok = pin > 0;
    protocol_send_detect_pin_resp(s_detect.seq, s_detect.key_id, ok ? (uint32_t)pin : 0, ok);
    s_detect.id = 0;
}

bool protocol_rx_idle(void)
{
    if (!frame_parser_is_idle(&s_parser) && (esp_timer_get_time() - s_last_rx_us) > RX_STALE_US) {
        frame_parser_reset(&s_parser);
    }
    return frame_parser_is_idle(&s_parser);
}
