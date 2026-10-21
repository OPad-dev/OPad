#include "protocol.h"
#include "protocol/nanopb/pb_encode.h"
#include "protocol/nanopb/pb_decode.h"
#include "usb/usb_cdc.h"
#include "usb/usb_hid.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "config/device_config.h"
#include "input/touch_retry.h"
#include "input/keypad.h"
#include "input/key_event.h"
#include "input/he_cal.h"
#include "input/he_backend.h"
#include "ui/he_cal_ui.h"
#include "counters/counters.h"
#include "ui/ui.h"
#include "ui/ui_store.h"
#include "ui/core/ui_anim.h"
#include "runtime/runtime.h"
#include "input/latency_stats.h"
#include "input/press_log.h"
#include "esp_mac.h"
#include "esp_timer.h"
#include "esp_log.h"
#include "esp_app_desc.h"
#include "esp_ota_ops.h"
#include "diag/diag.h"
#include "diag/board_test.h"
#include "frame_parser.h"
#include "freertos/FreeRTOS.h"
#include "freertos/portmacro.h"
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

// Live key view (he_live_view): streamed until this time, 0 = off. Written and
// read only by the CDC protocol task.
#define HE_LIVE_KEEPALIVE_US 2000000
#define HE_LIVE_PERIOD_US    20000
static int64_t s_live_until_us;
#ifdef CONFIG_OSUPAD_HE_BACKEND
static int64_t s_live_sent_us;
#endif

// UI producers publish the latest progress; only the CDC task frames/writes it.
// This keeps framing state, output sequence and whole-frame TX single-owner.
static portMUX_TYPE s_progress_lock = portMUX_INITIALIZER_UNLOCKED;
static osupad_HeCalibrationProgress s_progress;
static bool s_progress_pending;

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

static void fill_config_payload(osupad_ConfigPayload *out, const device_config_data_t *cfg)
{
    out->key1_hid_usage = cfg->key1_usage;
    out->key2_hid_usage = cfg->key2_usage;
    out->debounce_us = cfg->debounce_us;
    out->brightness = cfg->brightness;
    out->display_sleep_seconds = cfg->sleep_s;
    out->gameplay_display_hz = cfg->gameplay_display_hz ? cfg->gameplay_display_hz : 10;
    out->press_color_rgb = 0;
    out->key1_gpio = cfg->key1_gpio;
    out->key2_gpio = cfg->key2_gpio;
    out->swipe_left_action = (osupad_SwipeAction)cfg->swipe_left_action;
    out->swipe_right_action = (osupad_SwipeAction)cfg->swipe_right_action;
    out->swipe_left_key = cfg->swipe_left_key;
    out->swipe_right_key = cfg->swipe_right_key;
    out->swipe_up_action = (osupad_SwipeAction)cfg->swipe_up_action;
    out->swipe_down_action = (osupad_SwipeAction)cfg->swipe_down_action;
    out->swipe_up_key = cfg->swipe_up_key;
    out->swipe_down_key = cfg->swipe_down_key;
    out->swipe_up_modifiers = cfg->swipe_mods[0];
    out->swipe_down_modifiers = cfg->swipe_mods[1];
    out->swipe_left_modifiers = cfg->swipe_mods[2];
    out->swipe_right_modifiers = cfg->swipe_mods[3];

    he_key_settings_t he_keys[2];
    device_config_get_he(he_keys);

    out->has_key1_he = true;
    out->key1_he.actuation = he_keys[0].actuation;
    out->key1_he.release_hyst = he_keys[0].release_hyst;
    out->key1_he.top_reset = he_keys[0].top_reset;
    out->key1_he.rt_press = he_keys[0].rt_press;
    out->key1_he.rt_release = he_keys[0].rt_release;
    out->key1_he.has_bottom_dead_zone = true;
    out->key1_he.bottom_dead_zone = he_keys[0].bottom_dz;
    out->key1_he.rt_mode = !he_keys[0].rapid_trigger ? osupad_HeRtMode_HE_RT_MODE_OFF :
        he_keys[0].continuous ? osupad_HeRtMode_HE_RT_MODE_CONTINUOUS :
        osupad_HeRtMode_HE_RT_MODE_NORMAL;

    out->has_key2_he = true;
    out->key2_he.actuation = he_keys[1].actuation;
    out->key2_he.release_hyst = he_keys[1].release_hyst;
    out->key2_he.top_reset = he_keys[1].top_reset;
    out->key2_he.rt_press = he_keys[1].rt_press;
    out->key2_he.rt_release = he_keys[1].rt_release;
    out->key2_he.has_bottom_dead_zone = true;
    out->key2_he.bottom_dead_zone = he_keys[1].bottom_dz;
    out->key2_he.rt_mode = !he_keys[1].rapid_trigger ? osupad_HeRtMode_HE_RT_MODE_OFF :
        he_keys[1].continuous ? osupad_HeRtMode_HE_RT_MODE_CONTINUOUS :
        osupad_HeRtMode_HE_RT_MODE_NORMAL;
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
    fill_config_payload(&msg.payload.hello_ack.current_config, &cfg);
    // KeyPressBatch is sent while a map is played (issue #2)
    msg.payload.hello_ack.key_press_times = true;
    // Answers run_board_test
    msg.payload.hello_ack.board_test = true;
    // Plays PlayAnimation (issue #3)
    msg.payload.hello_ack.builtin_animations = UI_ANIM_BUILTIN_COUNT;
    msg.payload.hello_ack.restart = true;
#ifdef CONFIG_OSUPAD_HE_BACKEND
    msg.payload.hello_ack.he_input = true;
    msg.payload.hello_ack.he_calibration = he_backend_running();
    msg.payload.hello_ack.he_live = he_backend_running();
#else
    msg.payload.hello_ack.he_input = false;
    msg.payload.hello_ack.he_calibration = false;
#endif

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
    key_event_get_map_presses(&msg.payload.status.map_key1, &msg.payload.status.map_key2);

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

    if (he_backend_running()) {
        he_calibration_t cal0 = {0};
        he_calibration_t cal1 = {0};
        he_cal_get(0, &cal0);
        he_cal_get(1, &cal1);
        msg.payload.status.has_he_calibration = true;
        msg.payload.status.he_calibration.key1_calibrated = cal0.valid;
        msg.payload.status.he_calibration.key1_rest = cal0.rest >= 0 ? (uint32_t)cal0.rest : 0;
        msg.payload.status.he_calibration.key1_full = cal0.full >= 0 ? (uint32_t)cal0.full : 0;
        msg.payload.status.he_calibration.key2_calibrated = cal1.valid;
        msg.payload.status.he_calibration.key2_rest = cal1.rest >= 0 ? (uint32_t)cal1.rest : 0;
        msg.payload.status.he_calibration.key2_full = cal1.full >= 0 ? (uint32_t)cal1.full : 0;
        msg.payload.status.he_calibration.is_locked = he_backend_is_locked();
        msg.payload.status.he_calibration.is_calibrating = he_backend_is_calibrating();
    }

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
    fill_config_payload(&msg.payload.config_ack.current_config, &cfg);

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

static osupad_InputModule input_module_to_proto(board_module_type_t m)
{
    switch (m) {
    case BOARD_MODULE_MX: return osupad_InputModule_INPUT_MODULE_MX;
    case BOARD_MODULE_HE: return osupad_InputModule_INPUT_MODULE_HALL_EFFECT;
    default:              return osupad_InputModule_INPUT_MODULE_NONE;
    }
}

esp_err_t protocol_send_board_test(uint32_t seq, const char *refusal)
{
    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = seq ? seq : s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_board_test_tag;
    osupad_BoardTestResult *out = &msg.payload.board_test;

    board_test_result_t r;
    esp_err_t err = ESP_OK;
    if (refusal) {
        strncpy(out->message, refusal, sizeof(out->message) - 1);
    } else if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        // It reconfigures pins and ignores key edges for a moment: never in a map
        strncpy(out->message, "not while a map is played", sizeof(out->message) - 1);
    } else if ((err = board_test_run(&r)) != ESP_OK) {
        snprintf(out->message, sizeof(out->message), "test failed to run: %s", esp_err_to_name(err));
    } else {
        out->ran = true;
        out->boot_module = input_module_to_proto(r.boot_module);
        out->module = input_module_to_proto(r.module);
        out->id_mv = r.id_mv;
        out->id_loaded_mv = r.id_loaded_mv;
        out->reverse_probe_mv = r.reverse_probe_mv;
        out->key1_gpio = r.key1_gpio;
        out->key2_gpio = r.key2_gpio;
        out->keys_enabled = r.keys_enabled;
        out->tested_gpios = r.tested_gpios;
        out->high_with_pullup = r.high_with_pullup;
        out->high_with_pulldown = r.high_with_pulldown;
        out->bridged_gpios = r.bridged_gpios;
    }
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

void protocol_send_key_presses(void)
{
    // Only a played map's presses are wanted; outside one, or with nobody to
    // send to, they are dropped so the log is empty when the next map starts
    if (!usb_cdc_is_connected() || !s_host_framing_known ||
        runtime_get_state() != OSUPAD_STATE_PLAYING) {
        press_log_clear();
        return;
    }
    static uint32_t s_dropped;
    while (press_log_pending() > 0) {
        static press_log_entry_t entries[32];  // protocol task only
        size_t n = press_log_drain(entries, 32, &s_dropped);
        osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
        msg.sequence_number = s_out_sequence++;
        msg.which_payload = osupad_DeviceToHost_key_presses_tag;
        osupad_KeyPressBatch *batch = &msg.payload.key_presses;
        batch->now_us = (uint64_t)esp_timer_get_time();
        batch->presses_count = (pb_size_t)n;
        for (size_t i = 0; i < n; i++) {
            batch->presses[i].key = entries[i].key;
            batch->presses[i].t_us = (uint64_t)entries[i].t_us;
        }
        batch->dropped = s_dropped;
        if (send_envelope(&msg) != ESP_OK) {
            // The CDC FIFO is full: these presses are gone, and the host is told
            s_dropped += (uint32_t)n;
            return;
        }
        s_dropped = 0;
    }
}

// False if a widget holds a value that has no place in ui_layout_t: those are
// clamped into ones the validator rejects where it checks the field, and fail
// here where it does not
static bool layout_from_proto(const osupad_SetLayout *in, ui_layout_t *out, char *err, size_t err_len)
{
    memset(out, 0, sizeof(*out));
    out->background = in->background;
    out->count = in->widgets_count > UI_MAX_WIDGETS ? UI_MAX_WIDGETS : in->widgets_count;
    for (int i = 0; i < out->count; i++) {
        const osupad_UiWidget *src = &in->widgets[i];
        ui_widget_t *w = &out->widgets[i];
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
        if (src->flags > UINT8_MAX) {
            snprintf(err, err_len, "widget %d: unknown flags 0x%lx", i, (unsigned long)src->flags);
            return false;
        }
        w->flags = (uint8_t)src->flags;
        strncpy(w->label, src->label, UI_LABEL_MAX - 1);
        strncpy(w->suffix, src->suffix, UI_SUFFIX_MAX - 1);
    }
    return true;
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
        he_cal_ui_set_app_capable(msg->payload.hello.he_calibration);
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
            he_key_settings_t he_keys[2];
            device_config_get_he(he_keys);

            if (c->key1_hid_usage > 0) dcfg.key1_usage = c->key1_hid_usage;
            if (c->key2_hid_usage > 0) dcfg.key2_usage = c->key2_hid_usage;
            if (c->debounce_us > 0) dcfg.debounce_us = c->debounce_us;
            dcfg.brightness = c->brightness;
            dcfg.sleep_s = c->display_sleep_seconds;
            if (c->gameplay_display_hz > 0) dcfg.gameplay_display_hz = c->gameplay_display_hz;
            if (c->key1_gpio > 0) dcfg.key1_gpio = c->key1_gpio;
            if (c->key2_gpio > 0) dcfg.key2_gpio = c->key2_gpio;
            // Out-of-range values are left for validation to reject, not truncated
            if (c->swipe_left_action > 0) dcfg.swipe_left_action = c->swipe_left_action <= UINT8_MAX ? (uint8_t)c->swipe_left_action : 0;
            if (c->swipe_right_action > 0) dcfg.swipe_right_action = c->swipe_right_action <= UINT8_MAX ? (uint8_t)c->swipe_right_action : 0;
            if (c->swipe_left_key > 0) dcfg.swipe_left_key = c->swipe_left_key <= UINT8_MAX ? (uint8_t)c->swipe_left_key : 1;
            if (c->swipe_right_key > 0) dcfg.swipe_right_key = c->swipe_right_key <= UINT8_MAX ? (uint8_t)c->swipe_right_key : 1;
            if (c->swipe_up_action > 0) dcfg.swipe_up_action = c->swipe_up_action <= UINT8_MAX ? (uint8_t)c->swipe_up_action : 0;
            if (c->swipe_down_action > 0) dcfg.swipe_down_action = c->swipe_down_action <= UINT8_MAX ? (uint8_t)c->swipe_down_action : 0;
            if (c->swipe_up_key > 0) dcfg.swipe_up_key = c->swipe_up_key <= UINT8_MAX ? (uint8_t)c->swipe_up_key : 1;
            if (c->swipe_down_key > 0) dcfg.swipe_down_key = c->swipe_down_key <= UINT8_MAX ? (uint8_t)c->swipe_down_key : 1;
            // No "keep current" for modifiers: none is a value. An old host
            // that does not know them sends 0 along with plain keys only.
            const uint32_t mods[4] = {c->swipe_up_modifiers, c->swipe_down_modifiers,
                                      c->swipe_left_modifiers, c->swipe_right_modifiers};
            for (int i = 0; i < 4; i++) {
                dcfg.swipe_mods[i] = (uint8_t)mods[i]; // HID modifiers are 8 bits
            }

            if (c->has_key1_he) {
                if (c->key1_he.actuation > 0) he_keys[0].actuation = (uint16_t)c->key1_he.actuation;
                if (c->key1_he.release_hyst > 0) he_keys[0].release_hyst = (uint16_t)c->key1_he.release_hyst;
                if (c->key1_he.top_reset > 0) he_keys[0].top_reset = (uint16_t)c->key1_he.top_reset;
                if (c->key1_he.rt_press > 0) he_keys[0].rt_press = (uint16_t)c->key1_he.rt_press;
                if (c->key1_he.rt_release > 0) he_keys[0].rt_release = (uint16_t)c->key1_he.rt_release;
                if (c->key1_he.has_bottom_dead_zone) he_keys[0].bottom_dz = (uint16_t)c->key1_he.bottom_dead_zone;
                if (c->key1_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_OFF) {
                    he_keys[0].rapid_trigger = false;
                    he_keys[0].continuous = false;
                } else if (c->key1_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_NORMAL) {
                    he_keys[0].rapid_trigger = true;
                    he_keys[0].continuous = false;
                } else if (c->key1_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_CONTINUOUS) {
                    he_keys[0].rapid_trigger = true;
                    he_keys[0].continuous = true;
                }
            }
            if (c->has_key2_he) {
                if (c->key2_he.actuation > 0) he_keys[1].actuation = (uint16_t)c->key2_he.actuation;
                if (c->key2_he.release_hyst > 0) he_keys[1].release_hyst = (uint16_t)c->key2_he.release_hyst;
                if (c->key2_he.top_reset > 0) he_keys[1].top_reset = (uint16_t)c->key2_he.top_reset;
                if (c->key2_he.rt_press > 0) he_keys[1].rt_press = (uint16_t)c->key2_he.rt_press;
                if (c->key2_he.rt_release > 0) he_keys[1].rt_release = (uint16_t)c->key2_he.rt_release;
                if (c->key2_he.has_bottom_dead_zone) he_keys[1].bottom_dz = (uint16_t)c->key2_he.bottom_dead_zone;
                if (c->key2_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_OFF) {
                    he_keys[1].rapid_trigger = false;
                    he_keys[1].continuous = false;
                } else if (c->key2_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_NORMAL) {
                    he_keys[1].rapid_trigger = true;
                    he_keys[1].continuous = false;
                } else if (c->key2_he.rt_mode == osupad_HeRtMode_HE_RT_MODE_CONTINUOUS) {
                    he_keys[1].rapid_trigger = true;
                    he_keys[1].continuous = true;
                }
            }

            char err_msg[64] = "";
            if (!device_config_validate(&dcfg, err_msg, sizeof(err_msg)) ||
                !device_config_validate_he(he_keys, err_msg, sizeof(err_msg))) {
                diag_record(DIAG_EVENT_CONFIG_REJECTED, 2 /* WARN */, 1, 0);
                protocol_send_config_ack(msg->sequence_number, false, err_msg);
            } else {
                // Two NVS records: both are applied, and the ack says which one did not save
                esp_err_t err = device_config_set(&dcfg);
                esp_err_t he_err = ESP_OK;
                if (c->has_key1_he || c->has_key2_he) {
                    he_err = device_config_set_he(he_keys);
                }
                if (err != ESP_OK || he_err != ESP_OK) {
                    diag_record(DIAG_EVENT_CONFIG_REJECTED, 2 /* WARN */,
                                (uint32_t)(err != ESP_OK ? err : he_err), 0);
                    protocol_send_config_ack(msg->sequence_number, false,
                                             err != ESP_OK && he_err != ESP_OK
                                                 ? "Failed to persist configuration"
                                             : err != ESP_OK ? "Failed to persist configuration (HE settings saved)"
                                                             : "Failed to persist HE settings (rest saved)");
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
            key_event_reset_map_presses();
        }
        ui_set_tosu_connected(hs->tosu_connected);
        touch_retry_set_host_status(hs->tosu_connected, hs->osu_active);
        runtime_notify_gameplay(hs->playing);
        break;
    }

    case osupad_HostToDevice_set_layout_tag: {
        static ui_layout_t layout;  // ~2 KB, protocol task only
        const osupad_SetLayout *sl = &msg->payload.set_layout;
        char err[64] = "";
        bool ok = layout_from_proto(sl, &layout, err, sizeof(err)) &&
                  ui_set_layout((uint8_t)sl->screen, &layout, err, sizeof(err));
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

    case osupad_HostToDevice_run_board_test_tag:
        if (msg->payload.run_board_test) {
            // The pin scan holds GPIO2/4/6 as pulled-up inputs, which the test reconfigures
            protocol_send_board_test(msg->sequence_number,
                                     s_detect.id != 0 ? "pin detection is running" : NULL);
        }
        break;

    case osupad_HostToDevice_play_animation_tag: {
        // The host sends it on the results screen; a map that has started since wins
        const osupad_PlayAnimation *pa = &msg->payload.play_animation;
        if (runtime_get_state() != OSUPAD_STATE_PLAYING) {
            ui_play_animation(pa->animation <= UINT8_MAX ? (uint8_t)pa->animation : UI_ANIM_POP,
                              pa->grade);
        }
        break;
    }

    case osupad_HostToDevice_restart_tag:
        if (msg->payload.restart) {
            runtime_request_restart();  // once idle, counters saved first
        }
        break;

    case osupad_HostToDevice_start_he_calibration_tag:
        if (msg->payload.start_he_calibration) {
            he_cal_ui_start_calibration(true);
        }
        break;

    case osupad_HostToDevice_cancel_he_calibration_tag:
        if (msg->payload.cancel_he_calibration) {
            he_cal_ui_cancel_calibration();
        }
        break;

    case osupad_HostToDevice_he_live_view_tag:
        // Same task as protocol_poll_he_live, so no lock
        s_live_until_us = msg->payload.he_live_view && he_backend_running()
                              ? esp_timer_get_time() + HE_LIVE_KEEPALIVE_US
                              : 0;
        break;

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
            msg.which_payload <= osupad_HostToDevice_cancel_he_calibration_tag) {
            s_host_framing = s_parser.last_format;
            s_host_framing_known = true;
        }
        handle_host_message(&msg);
    }
}

// A partial frame the host stopped sending half a second ago will never complete
static void expire_stale_partial(int64_t now)
{
    if (!frame_parser_is_idle(&s_parser) && (now - s_last_rx_us) > RX_STALE_US) {
        ESP_LOGW(TAG, "Dropping stale partial frame (%u bytes)", (unsigned)s_parser.rx_len);
        diag_record(DIAG_EVENT_FRAME_TOO_LARGE, 2 /* WARN */, (uint32_t)s_parser.rx_len,
                    DIAG_FRAME_DROP_STALE);
        frame_parser_reset(&s_parser);
    }
}

void protocol_feed_cdc_bytes(const uint8_t *data, size_t len)
{
    int64_t now = esp_timer_get_time();
    expire_stale_partial(now);
    s_last_rx_us = now;

    uint32_t prev_resync = s_parser.resync_bytes;
    frame_accept_t accept = FRAME_ACCEPT_ANY;
    if (s_host_framing_known) {
        accept = s_host_framing == FRAME_FORMAT_LEGACY ? FRAME_ACCEPT_LEGACY : FRAME_ACCEPT_MARKED;
    }
    frame_parser_feed(&s_parser, data, len, accept, on_frame_received, NULL);
    uint32_t skipped = s_parser.resync_bytes - prev_resync;
    if (skipped > 0) {
        diag_record(DIAG_EVENT_FRAME_TOO_LARGE, 2 /* WARN */, skipped, DIAG_FRAME_DROP_RESYNC);
        ESP_LOGW(TAG, "Skipped %lu bytes looking for a frame (resync)", (unsigned long)skipped);
    }
}

void protocol_reset_rx(void)
{
    frame_parser_reset(&s_parser);
    // Called when the host closes the port: the next one may frame differently
    s_host_framing_known = false;
    he_cal_ui_on_host_disconnect();
    s_live_until_us = 0;
    portENTER_CRITICAL(&s_progress_lock);
    s_progress_pending = false;
    portEXIT_CRITICAL(&s_progress_lock);
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
    expire_stale_partial(esp_timer_get_time());
    return frame_parser_is_idle(&s_parser);
}

esp_err_t protocol_send_he_calibration_progress(const osupad_HeCalibrationProgress *prog)
{
    if (!prog) {
        return ESP_ERR_INVALID_ARG;
    }
    portENTER_CRITICAL(&s_progress_lock);
    s_progress = *prog;
    s_progress_pending = true;
    portEXIT_CRITICAL(&s_progress_lock);
    return ESP_OK;
}

void protocol_poll_he_calibration(void)
{
    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    portENTER_CRITICAL(&s_progress_lock);
    bool pending = s_progress_pending;
    msg.payload.he_calibration_progress = s_progress;
    s_progress_pending = false;
    portEXIT_CRITICAL(&s_progress_lock);
    if (!pending) return;
    msg.sequence_number = s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_he_calibration_progress_tag;
    if (send_envelope(&msg) != ESP_OK) {
        portENTER_CRITICAL(&s_progress_lock);
        if (!s_progress_pending) {
            s_progress = msg.payload.he_calibration_progress;
            s_progress_pending = true;
        }
        portEXIT_CRITICAL(&s_progress_lock);
    }
}

bool protocol_he_live_active(void)
{
    return s_live_until_us != 0;
}

void protocol_poll_he_live(void)
{
#ifdef CONFIG_OSUPAD_HE_BACKEND
    if (s_live_until_us == 0) {
        return;
    }
    int64_t now = esp_timer_get_time();
    if (now >= s_live_until_us) {
        s_live_until_us = 0;  // the host stopped asking
        return;
    }
    // Never during a map or its cooldown; resumes after if the host still asks
    if (runtime_get_state() != OSUPAD_STATE_IDLE || now - s_live_sent_us < HE_LIVE_PERIOD_US) {
        return;
    }
    s_live_sent_us = now;
    he_backend_stats_t st;
    he_backend_get_stats(&st);
    osupad_DeviceToHost msg = osupad_DeviceToHost_init_zero;
    msg.sequence_number = s_out_sequence++;
    msg.which_payload = osupad_DeviceToHost_he_live_sample_tag;
    msg.payload.he_live_sample.key1_raw = st.raw[0] > 0 ? (uint32_t)st.raw[0] : 0;
    msg.payload.he_live_sample.key2_raw = st.raw[1] > 0 ? (uint32_t)st.raw[1] : 0;
    msg.payload.he_live_sample.key1_down = key_event_is_down(KEY_ID_1);
    msg.payload.he_live_sample.key2_down = key_event_is_down(KEY_ID_2);
    (void)send_envelope(&msg);  // a lost sample is replaced by the next one
#endif
}
