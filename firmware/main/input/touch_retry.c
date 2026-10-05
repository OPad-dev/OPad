#include "touch_retry.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board_pins.h"
#include "usb/usb_hid.h"
#include "usb/usb_media.h"
#include "input/gesture.h"
#include "input/swipe_labels.h"
#include "config/device_config.h"
#include "runtime/runtime.h"
#include "ui/ui.h"
#include "ui/core/ui_core.h"
#include "lvgl.h"
#include "esp_timer.h"
#include "driver/i2c_master.h"
#include "driver/gpio.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_log.h"
#include <stdatomic.h>
#include <stdbool.h>

static const char *TAG = "touch_retry";

#define CST816_I2C_ADDR         0x15
#define CST816_I2C_PORT         I2C_NUM_0
#define CST816_I2C_FREQ_HZ      100000 // 100 kHz standard clock speed
#define CST816_I2C_TIMEOUT_MS   20

#define CST816_REG_TOUCH_NUM    0x02
#define CST816_REG_TOUCH_XH     0x03
#define CST816_REG_CHIP_ID      0xA7
#define CST816_REG_AUTO_SLEEP   0xFE

static volatile bool s_touch_pressed = false;
static TaskHandle_t s_touch_task = NULL;
static bool s_auto_sleep_disabled = false;
static i2c_master_bus_handle_t s_bus = NULL;
static i2c_master_dev_handle_t s_dev = NULL;

static esp_err_t cst816_read_reg(uint8_t reg, uint8_t *buf, size_t len)
{
    // Register pointer write with STOP, then a read from a fresh START (matching
    // the Waveshare driver)
    esp_err_t err = i2c_master_transmit(s_dev, &reg, 1, CST816_I2C_TIMEOUT_MS);
    if (err != ESP_OK) {
        // Fallback to repeated-start in case controller expects Sr
        return i2c_master_transmit_receive(s_dev, &reg, 1, buf, len, CST816_I2C_TIMEOUT_MS);
    }
    return i2c_master_receive(s_dev, buf, len, CST816_I2C_TIMEOUT_MS);
}

static esp_err_t cst816_write_reg(uint8_t reg, uint8_t val)
{
    uint8_t buf[2] = {reg, val};
    return i2c_master_transmit(s_dev, buf, sizeof(buf), CST816_I2C_TIMEOUT_MS);
}

static void cst816_disable_auto_sleep(void)
{
    esp_err_t err = cst816_write_reg(CST816_REG_AUTO_SLEEP, 0xFF);
    if (err == ESP_OK) {
        s_auto_sleep_disabled = true;
        ESP_LOGI(TAG, "CST816 auto-sleep disabled (dynamic active mode)");
    }
}

// Poll period while a touch is down (release detection), and the number of
// consecutive "up" reads (~20 ms) that count as a clean release.
#define TOUCH_POLL_MS           10
#define TOUCH_RELEASE_READS     2
// With no finger down the task sleeps on the INT edge; this is only a safety
// net for a miswired or silent INT line, so a held touch is still seen.
#define TOUCH_IDLE_CHECK_MS     1000

// During a map Quick Retry waits until the finger has rested (moved less than
// PLAY_RETRY_STILL_PX) for PLAY_RETRY_STILL_US, so a swipe never sends '`'.
// A finger still moving at PLAY_SWIPE_WINDOW_US without having become a swipe
// is a hold all the same. osu!'s retry is itself a hold, so the wait costs
// nothing noticeable.
#define PLAY_RETRY_STILL_US     60000
#define PLAY_RETRY_STILL_PX     12
#define PLAY_SWIPE_WINDOW_US    250000
// The daemon sends HostStatus every second; this long without one and the pad
// can no longer tell a map from a menu
#define HOST_STATUS_TIMEOUT_US  3000000
// How long a swipe waits for the host to take a keyboard report, and the gap
// that keeps Alt (keyboard) ahead of the wheel (media interface) on the host
#define TOUCH_KEY_WAIT_MS       30
#define TOUCH_ORDER_GAP_MS      4
// How long a swipe's keyboard key stays down
#define TOUCH_KEY_TAP_MS        15

#define HID_MOD_LEFT_ALT        0x04
// Tapped before Alt is let go: Windows reads an Alt press and release with
// no key in between as "open the window menu" (SC_KEYMENU), which can make a
// game drop the next key. Nothing uses F24.
#define HID_KEY_F24             0x73

/*
 * Panel coordinates (portrait, 240 x 320) to the screen as the UI draws it
 * (landscape, 320 x 240; ui_port.c rotates with swap_xy + mirror_x), so "up"
 * is up as the pad is held.
 */
#define TOUCH_SWAP_XY           1
#define TOUCH_MIRROR_X          0
#define TOUCH_MIRROR_Y          1

static bool s_int_wakeup = false; // INT falling edge wakes the task

// Written by the protocol task, read here at touch-down
static atomic_llong s_host_seen_us = 0;     // last HostStatus with tosu connected, 0 = none
static atomic_bool s_osu_active = false;
// Swipe actions (swipe_action_t) and keys from the device config, one byte
// each in gesture_t order: up (bits 0-7), down, left, right
static atomic_uint s_swipe_actions = SWIPE_ACTION_VOLUME_UP | (SWIPE_ACTION_VOLUME_DOWN << 8) |
                                     (SWIPE_ACTION_PREV_TRACK << 16) | (SWIPE_ACTION_NEXT_TRACK << 24);
static atomic_uint s_swipe_keys = 0;
static atomic_uint s_swipe_mods = 0;

typedef enum {
    TOUCH_MODE_RETRY,   // Nothing tells us a map from a menu: Quick Retry only, as always
    TOUCH_MODE_PLAY,    // A map: Quick Retry, plus swipes that use no keyboard report
    TOUCH_MODE_MENU,    // Anything else: swipes, and never Quick Retry
} touch_mode_t;

typedef struct {
    bool down;
    bool has_pos;   // x/y valid (the controller reported a point)
    int16_t x;
    int16_t y;
} touch_sample_t;

// The CST816 pulls INT low on a touch (held or pulsed, depending on mode); the
// edge just wakes the task, which confirms over I2C.
static void IRAM_ATTR touch_int_isr(void *arg)
{
    (void)arg;
    if (s_touch_task) {
        BaseType_t woken = pdFALSE;
        vTaskNotifyGiveFromISR(s_touch_task, &woken);
        portYIELD_FROM_ISR(woken);
    }
}

// One sample: the active-low INT level or a non-zero touch count means down.
static touch_sample_t touch_read(void)
{
    touch_sample_t s = {0};
    bool int_active = (gpio_get_level(BOARD_TOUCH_INT_GPIO) == 0);

    // Touch count, then X and Y (12 bits each, the top nibble of XH/YH)
    uint8_t buf[5] = {0};
    esp_err_t err = cst816_read_reg(CST816_REG_TOUCH_NUM, buf, sizeof(buf));

    s.down = int_active || (err == ESP_OK && buf[0] > 0);
    if (err == ESP_OK && buf[0] > 0) {
        int16_t px = (int16_t)(((buf[1] & 0x0F) << 8) | buf[2]);
        int16_t py = (int16_t)(((buf[3] & 0x0F) << 8) | buf[4]);
#if TOUCH_SWAP_XY
        int16_t t = px;
        px = py;
        py = t;
#endif
#if TOUCH_MIRROR_X
        px = (int16_t)(UI_SCREEN_W - 1 - px);
#endif
#if TOUCH_MIRROR_Y
        py = (int16_t)(UI_SCREEN_H - 1 - py);
#endif
        s.x = px;
        s.y = py;
        s.has_pos = true;
    }
    if (s.down && !s_auto_sleep_disabled) {
        cst816_disable_auto_sleep();
    }
    return s;
}

static touch_mode_t touch_mode_now(void)
{
    int64_t seen = atomic_load(&s_host_seen_us);
    if (seen == 0 || esp_timer_get_time() - seen > HOST_STATUS_TIMEOUT_US) {
        return TOUCH_MODE_RETRY;
    }
    return runtime_get_state() == OSUPAD_STATE_PLAYING ? TOUCH_MODE_PLAY : TOUCH_MODE_MENU;
}

static void wait_touch_key_delivered(void)
{
    for (int i = 0; i < TOUCH_KEY_WAIT_MS && !usb_hid_touch_key_delivered(); i++) {
        vTaskDelay(pdMS_TO_TICKS(1));
    }
}

// Menus only: a keyboard key with its modifiers, pressed and released. The
// modifiers go down first and come up last, as from a keyboard.
static void tap_keyboard_key(uint8_t key, uint8_t mods)
{
    if (mods) {
        usb_hid_set_touch_key(0, mods);
        wait_touch_key_delivered();
    }
    usb_hid_set_touch_key(key, mods);
    wait_touch_key_delivered();
    vTaskDelay(pdMS_TO_TICKS(TOUCH_KEY_TAP_MS));
    if (mods) {
        usb_hid_set_touch_key(0, mods);
        wait_touch_key_delivered();
    }
    usb_hid_set_touch_key(0, 0);
}

/*
 * One volume step. Goes to osu! when the host says it is in front (Windows)
 * or running (Linux, osu!lazer): Alt+wheel in menus, where a plain wheel
 * scrolls song select; the plain wheel during a map, so the keyboard report
 * never carries Alt next to K1/K2. Otherwise the system volume.
 * *alt_held: Alt stays down for the rest of the touch once a menu volume swipe
 * pressed it, so a long drag is one Alt press with several wheel notches.
 */
static const char *step_volume(bool up, bool playing, bool *alt_held)
{
    bool osu = atomic_load(&s_osu_active);
    if (osu && playing) {
        usb_media_wheel(up ? 1 : -1);
    } else if (osu) {
        if (!*alt_held) {
            usb_hid_set_touch_key(0, HID_MOD_LEFT_ALT);
            wait_touch_key_delivered();
            vTaskDelay(pdMS_TO_TICKS(TOUCH_ORDER_GAP_MS));
            *alt_held = true;
        }
        usb_media_wheel(up ? 1 : -1);
    } else {
        usb_media_consumer_tap(up ? USB_MEDIA_VOLUME_UP : USB_MEDIA_VOLUME_DOWN);
    }
    return osu ? (up ? "osu! Vol +" : "osu! Vol -") : (up ? "Vol +" : "Vol -");
}

static const char *icon_symbol(swipe_icon_t icon)
{
    switch (icon) {
    case SWIPE_ICON_SHUFFLE:    return LV_SYMBOL_SHUFFLE;
    case SWIPE_ICON_LIST:       return LV_SYMBOL_LIST;
    case SWIPE_ICON_BARS:       return LV_SYMBOL_BARS;
    case SWIPE_ICON_LEFT:       return LV_SYMBOL_LEFT;
    case SWIPE_ICON_RIGHT:      return LV_SYMBOL_RIGHT;
    case SWIPE_ICON_UP:         return LV_SYMBOL_UP;
    case SWIPE_ICON_DOWN:       return LV_SYMBOL_DOWN;
    case SWIPE_ICON_DIRECTORY:  return LV_SYMBOL_DIRECTORY;
    case SWIPE_ICON_BACKSPACE:  return LV_SYMBOL_BACKSPACE;
    case SWIPE_ICON_OK:         return LV_SYMBOL_OK;
    case SWIPE_ICON_CLOSE:      return LV_SYMBOL_CLOSE;
    case SWIPE_ICON_ENVELOPE:   return LV_SYMBOL_ENVELOPE;
    case SWIPE_ICON_BELL:       return LV_SYMBOL_BELL;
    case SWIPE_ICON_SETTINGS:   return LV_SYMBOL_SETTINGS;
    case SWIPE_ICON_IMAGE:      return LV_SYMBOL_IMAGE;
    case SWIPE_ICON_VIDEO:      return LV_SYMBOL_VIDEO;
    case SWIPE_ICON_AUDIO:      return LV_SYMBOL_AUDIO;
    case SWIPE_ICON_DOWNLOAD:   return LV_SYMBOL_DOWNLOAD;
    case SWIPE_ICON_HOME:       return LV_SYMBOL_HOME;
    case SWIPE_ICON_EYE_OPEN:   return LV_SYMBOL_EYE_OPEN;
    case SWIPE_ICON_EYE_CLOSE:  return LV_SYMBOL_EYE_CLOSE;
    case SWIPE_ICON_TINT:       return LV_SYMBOL_TINT;
    case SWIPE_ICON_REFRESH:    return LV_SYMBOL_REFRESH;
    case SWIPE_ICON_KEYBOARD:
    default:                    return LV_SYMBOL_KEYBOARD;
    }
}

// Runs a swipe's action. Returns the label to show, NULL for nothing done.
static const char *run_swipe_action(uint8_t action, uint8_t key, uint8_t mods, bool playing,
                                    bool *alt_held, const char **symbol)
{
    switch (action) {
    case SWIPE_ACTION_VOLUME_UP:
        *symbol = LV_SYMBOL_VOLUME_MAX;
        return step_volume(true, playing, alt_held);
    case SWIPE_ACTION_VOLUME_DOWN:
        *symbol = LV_SYMBOL_VOLUME_MID;
        return step_volume(false, playing, alt_held);
    case SWIPE_ACTION_PREV_TRACK:
        usb_media_consumer_tap(USB_MEDIA_PREV_TRACK);
        *symbol = LV_SYMBOL_PREV;
        return "Previous";
    case SWIPE_ACTION_NEXT_TRACK:
        usb_media_consumer_tap(USB_MEDIA_NEXT_TRACK);
        *symbol = LV_SYMBOL_NEXT;
        return "Next";
    case SWIPE_ACTION_PLAY_PAUSE:
        usb_media_consumer_tap(USB_MEDIA_PLAY_PAUSE);
        *symbol = LV_SYMBOL_PLAY;
        return "Play/Pause";
    case SWIPE_ACTION_MUTE:
        usb_media_consumer_tap(USB_MEDIA_MUTE);
        *symbol = LV_SYMBOL_MUTE;
        return "Mute";
    case SWIPE_ACTION_KEY:
        // The keyboard report carries only K1, K2 and Quick Retry during a map
        if (playing || key == 0) {
            return NULL;
        }
        tap_keyboard_key(key, mods);
        {
            // Touch task only, and ui_show_swipe copies the text
            static char name[16];
            swipe_label_t l = swipe_label_for(key, mods, name, sizeof(name));
            *symbol = icon_symbol(l.icon);
            return l.label;
        }
    default:
        return NULL;
    }
}

/*
 * One swipe step in direction g. repeat: not the touch's first step (a long
 * vertical drag steps every GESTURE_SWIPE_PX). Volume repeats; every other
 * action fires once per touch.
 */
static void handle_swipe(gesture_t g, touch_mode_t mode, bool repeat, bool *alt_held)
{
    bool playing = (mode == TOUCH_MODE_PLAY);
    unsigned shift = 8u * (unsigned)(g - GESTURE_UP);
    uint8_t action = (uint8_t)(atomic_load(&s_swipe_actions) >> shift);
    uint8_t key = (uint8_t)(atomic_load(&s_swipe_keys) >> shift);
    uint8_t mods = (uint8_t)(atomic_load(&s_swipe_mods) >> shift);
    if (repeat && action != SWIPE_ACTION_VOLUME_UP && action != SWIPE_ACTION_VOLUME_DOWN) {
        return;
    }

    const char *symbol = NULL;
    const char *text = run_swipe_action(action, key, mods, playing, alt_held, &symbol);
    ESP_LOGI(TAG, "Swipe %d -> %s", (int)g, text ? text : "nothing");
    // Gameplay draws nothing extra. The feedback moves the way the finger did.
    if (text && !playing) {
        int dx = g == GESTURE_LEFT ? -1 : g == GESTURE_RIGHT ? 1 : 0;
        int dy = g == GESTURE_UP ? -1 : g == GESTURE_DOWN ? 1 : 0;
        ui_show_swipe(dx, dy, symbol, text);
    }
}

// Lets go of Alt after the wheel notches it was held for have reached the host
static void release_alt(bool *alt_held)
{
    if (*alt_held) {
        usb_media_wait_ready(TOUCH_KEY_WAIT_MS);
        vTaskDelay(pdMS_TO_TICKS(TOUCH_ORDER_GAP_MS));
        usb_hid_set_touch_key(HID_KEY_F24, HID_MOD_LEFT_ALT);
        wait_touch_key_delivered();
        usb_hid_set_touch_key(0, HID_MOD_LEFT_ALT);
        wait_touch_key_delivered();
        usb_hid_set_touch_key(0, 0);
        *alt_held = false;
    }
}

// Quick Retry pressed and released at once: a tap during a map that ended
// before it could count as a hold
static void tap_retry(void)
{
    usb_hid_set_touch_retry(true);
    wait_touch_key_delivered();
    usb_hid_set_touch_retry(false);
}

static void touch_retry_task(void *arg)
{
    (void)arg;
    ESP_LOGI(TAG, "Touch task running on core %d (%s, INT: GPIO%d)",
             xPortGetCoreID(), s_int_wakeup ? "INT wake, 100 Hz while down" : "100 Hz poll",
             BOARD_TOUCH_INT_GPIO);

    bool pressed = false;
    uint8_t release_debounce = 0;
    touch_mode_t mode = TOUCH_MODE_RETRY;
    bool retry_held = false;
    bool retry_pending = false; // a map: Quick Retry once the finger rests
    bool alt_held = false;
    bool tracking = false;      // following the finger for a swipe
    bool tracker_started = false;
    int swipes = 0;             // swipe steps this touch
    int64_t down_us = 0;
    gesture_tracker_t tracker;

    while (1) {
        if (!pressed) {
            // Idle: no I2C traffic and no CPU until INT fires (or the safety
            // interval passes). Without an ISR this is the old 100 Hz poll.
            ulTaskNotifyTake(pdTRUE, pdMS_TO_TICKS(s_int_wakeup ? TOUCH_IDLE_CHECK_MS : TOUCH_POLL_MS));
            touch_sample_t s = touch_read();
            if (!s.down) {
                continue; // spurious edge, or the safety check found nothing
            }
            pressed = true;
            release_debounce = 0;
            s_touch_pressed = true;
            down_us = esp_timer_get_time();
            mode = touch_mode_now();
            swipes = 0;
            // Without a host, every touch is Quick Retry at once, as always
            retry_held = (mode == TOUCH_MODE_RETRY);
            retry_pending = (mode == TOUCH_MODE_PLAY);
            if (retry_held) {
                usb_hid_set_touch_retry(true);
                ESP_LOGI(TAG, "Touch down -> Quick Retry pressed ('`')");
            }
            tracking = (mode != TOUCH_MODE_RETRY);
            tracker_started = tracking && s.has_pos;
            if (tracker_started) {
                gesture_begin(&tracker, s.x, s.y);
            }
            ui_notify_activity();
            continue;
        }

        // Down: poll for the release (INT may only pulse per report, so the
        // touch count is what keeps the key held) and follow the finger.
        vTaskDelay(pdMS_TO_TICKS(TOUCH_POLL_MS));
        touch_sample_t s = touch_read();
        if (s.down) {
            release_debounce = 0;
            if (tracking && s.has_pos) {
                if (!tracker_started) {
                    gesture_begin(&tracker, s.x, s.y);
                    tracker_started = true;
                } else {
                    // Every step the finger covered since the last read, not
                    // just one: a fast swipe, or one read late because the
                    // previous step was still being sent, keeps its length
                    gesture_t g;
                    while ((g = gesture_feed(&tracker, s.x, s.y)) != GESTURE_NONE) {
                        retry_pending = false; // a swipe never sends '`'
                        handle_swipe(g, mode, swipes > 0, &alt_held);
                        swipes++;
                    }
                }
            }
            if (retry_pending) {
                int64_t held_us = esp_timer_get_time() - down_us;
                bool resting = !(tracker_started && s.has_pos) ||
                               gesture_distance(&tracker, s.x, s.y) < PLAY_RETRY_STILL_PX;
                if ((resting && held_us >= PLAY_RETRY_STILL_US) || held_us >= PLAY_SWIPE_WINDOW_US) {
                    retry_pending = false;
                    retry_held = true;
                    tracking = false; // a held Quick Retry, not a swipe
                    usb_hid_set_touch_retry(true);
                    ESP_LOGI(TAG, "Touch held -> Quick Retry pressed ('`')");
                }
            }
            continue;
        }
        if (++release_debounce >= TOUCH_RELEASE_READS) {
            pressed = false;
            s_touch_pressed = false;
            tracking = false;
            if (retry_pending) {
                retry_pending = false;
                tap_retry();
                ESP_LOGI(TAG, "Touch tap -> Quick Retry pressed and released");
            }
            if (retry_held) {
                usb_hid_set_touch_retry(false);
                retry_held = false;
                ESP_LOGI(TAG, "Touch up -> Quick Retry released");
            }
            release_alt(&alt_held);
            // An INT edge seen during the press leaves a notification pending;
            // it costs one confirming read on the next loop and nothing more.
        }
    }
}

void touch_retry_set_host_status(bool tosu_connected, bool osu_active)
{
    // Without tosu the host cannot say when a map is played either
    atomic_store(&s_host_seen_us, tosu_connected ? esp_timer_get_time() : 0);
    atomic_store(&s_osu_active, osu_active);
}

void touch_retry_set_swipe_actions(const uint8_t actions[4], const uint8_t keys[4],
                                   const uint8_t mods[4])
{
    unsigned a = 0;
    unsigned k = 0;
    unsigned m = 0;
    for (unsigned i = 0; i < 4; i++) {
        a |= (unsigned)actions[i] << (8 * i);
        k |= (unsigned)keys[i] << (8 * i);
        m |= (unsigned)mods[i] << (8 * i);
    }
    atomic_store(&s_swipe_actions, a);
    atomic_store(&s_swipe_keys, k);
    atomic_store(&s_swipe_mods, m);
}

esp_err_t touch_retry_init(void)
{
    // 1. Configure INT pin as input with pull-up
    gpio_config_t int_cfg = {
        .pin_bit_mask = (1ULL << BOARD_TOUCH_INT_GPIO),
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_NEGEDGE,
    };
    esp_err_t err = gpio_config(&int_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to configure INT GPIO%d: %s", BOARD_TOUCH_INT_GPIO, esp_err_to_name(err));
    } else {
        // Same shared ISR service the keypad uses (already installed is fine).
        // If any of this fails the task falls back to polling.
        err = gpio_install_isr_service(ESP_INTR_FLAG_IRAM);
        if (err == ESP_OK || err == ESP_ERR_INVALID_STATE) {
            err = gpio_isr_handler_add(BOARD_TOUCH_INT_GPIO, touch_int_isr, NULL);
        }
        if (err == ESP_OK) {
            s_int_wakeup = true;
        } else {
            ESP_LOGW(TAG, "INT wake unavailable (%s); polling at 100 Hz instead", esp_err_to_name(err));
        }
    }

    // 2. Configure I2C bus
    i2c_master_bus_config_t bus_cfg = {
        .i2c_port = CST816_I2C_PORT,
        .sda_io_num = BOARD_I2C_SDA_GPIO,
        .scl_io_num = BOARD_I2C_SCL_GPIO,
        .clk_source = I2C_CLK_SRC_DEFAULT,
        .glitch_ignore_cnt = 7,
        .flags.enable_internal_pullup = true,
    };
    err = i2c_new_master_bus(&bus_cfg, &s_bus);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_new_master_bus failed: %s", esp_err_to_name(err));
        return err;
    }

    i2c_device_config_t dev_cfg = {
        .dev_addr_length = I2C_ADDR_BIT_LEN_7,
        .device_address = CST816_I2C_ADDR,
        .scl_speed_hz = CST816_I2C_FREQ_HZ,
    };
    err = i2c_master_bus_add_device(s_bus, &dev_cfg, &s_dev);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "i2c_master_bus_add_device failed: %s", esp_err_to_name(err));
        return err;
    }

#if CONFIG_OSUPAD_DEBUG_I2C_SCAN
    // 3. Scan I2C bus to discover devices (debug only: up to ~1.3 s of boot)
    ESP_LOGI(TAG, "Scanning I2C bus (SDA: GPIO%d, SCL: GPIO%d)...", BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO);
    int devices_found = 0;
    for (uint16_t addr = 1; addr < 127; addr++) {
        if (i2c_master_probe(s_bus, addr, 10) == ESP_OK) {
            ESP_LOGI(TAG, " - Found I2C device at 0x%02X", addr);
            devices_found++;
        }
    }
    ESP_LOGI(TAG, "I2C scan complete (%d devices found)", devices_found);
#endif

    // 4. Try reading CST816 Chip ID
    uint8_t chip_id = 0;
    esp_err_t id_err = cst816_read_reg(CST816_REG_CHIP_ID, &chip_id, 1);
    if (id_err == ESP_OK) {
        ESP_LOGI(TAG, "CST816 Touch Controller detected! Chip ID: 0x%02X", chip_id);
    } else {
        ESP_LOGI(TAG, "CST816 touch controller standby/idle on boot (INT=GPIO%d, INT level=%d)",
                 BOARD_TOUCH_INT_GPIO, gpio_get_level(BOARD_TOUCH_INT_GPIO));
    }

    // Try disabling auto-sleep upfront
    cst816_disable_auto_sleep();

    // 5. Pinned to Core 1 so it never interferes with Core 0 keypad ISR / TinyUSB
    BaseType_t res = xTaskCreatePinnedToCore(
        touch_retry_task,
        "touch_retry",
        3072,
        NULL,
        2, // Low priority
        &s_touch_task,
        1  // Core 1
    );

    if (res != pdPASS) {
        ESP_LOGE(TAG, "Failed to create touch_retry task");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "Touch Retry initialized (I2C SDA:%d, SCL:%d, INT:%d, Address:0x%02X)",
             BOARD_I2C_SDA_GPIO, BOARD_I2C_SCL_GPIO, BOARD_TOUCH_INT_GPIO, CST816_I2C_ADDR);
    return ESP_OK;
}

bool touch_retry_is_pressed(void)
{
    return s_touch_pressed;
}
