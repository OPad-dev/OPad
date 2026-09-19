#include "keypad.h"
#include "debounce.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "ui/ui.h"
#include "usb/usb_hid.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_timer.h"
#include "esp_log.h"
#include <stdatomic.h>
#include "input/latency_stats.h"
#include "driver/gpio.h"

static const char *TAG = "keypad";

static portMUX_TYPE s_keypad_spinlock = portMUX_INITIALIZER_UNLOCKED;
static debounce_state_t s_key_debounce[KEY_ID_COUNT];

#define KEYPAD_DEFAULT_KEY1_GPIO 14
#define KEYPAD_DEFAULT_KEY2_GPIO 9

static keypad_config_t s_config = {
    .keycode1 = 0x1D,       // 'z'
    .keycode2 = 0x1B,       // 'x'
    .debounce_us = DEBOUNCE_DEFAULT_US,
    .key1_gpio = KEYPAD_DEFAULT_KEY1_GPIO,
    .key2_gpio = KEYPAD_DEFAULT_KEY2_GPIO,
};

static keypad_config_t s_staged_config;
static volatile bool s_config_staged = false;

static volatile bool s_key_state[KEY_ID_COUNT] = {false, false};
static volatile int64_t s_last_transition_us[KEY_ID_COUNT] = {0, 0};
static volatile int64_t s_key1_last_press_us = 0;
static volatile int64_t s_key2_last_press_us = 0;

static atomic_uint_least64_t s_key1_lifetime_presses = 0;
static atomic_uint_least64_t s_key2_lifetime_presses = 0;
// Presses in the current osu! attempt; zeroed by the host on every new play
static atomic_uint_least32_t s_key1_map_presses = 0;
static atomic_uint_least32_t s_key2_map_presses = 0;

static TaskHandle_t s_input_task_handle = NULL;
static keypad_state_callback_t s_callback = NULL;

static void IRAM_ATTR gpio_isr_handler(void *arg)
{
    int key_index = (intptr_t)arg - 1;
    if (key_index < 0 || key_index >= KEY_ID_COUNT) {
        return;
    }

    int64_t now = esp_timer_get_time();
    bool current_pressed = (key_index == 0) ? board_key1_read() : board_key2_read();

    portENTER_CRITICAL_ISR(&s_keypad_spinlock);
    debounce_action_t action = debounce_step(
        &s_key_debounce[key_index],
        now,
        current_pressed,
        DEBOUNCE_SOURCE_EDGE,
        s_config.debounce_us
    );

    if (action != DEBOUNCE_ACTION_NONE) {
        s_key_state[key_index] = s_key_debounce[key_index].accepted_pressed;
        s_last_transition_us[key_index] = s_key_debounce[key_index].last_transition_us;

        // Counting rule: only accepted press transitions increment counters.
        if (action == DEBOUNCE_ACTION_PRESS) {
            if (key_index == 0) {
                atomic_fetch_add_explicit(&s_key1_lifetime_presses, 1, memory_order_relaxed);
                atomic_fetch_add_explicit(&s_key1_map_presses, 1, memory_order_relaxed);
                s_key1_last_press_us = now;
            } else {
                atomic_fetch_add_explicit(&s_key2_lifetime_presses, 1, memory_order_relaxed);
                atomic_fetch_add_explicit(&s_key2_map_presses, 1, memory_order_relaxed);
                s_key2_last_press_us = now;
            }
        }

        BaseType_t high_task_wakeup = pdFALSE;
        if (s_input_task_handle != NULL) {
            vTaskNotifyGiveFromISR(s_input_task_handle, &high_task_wakeup);
            if (high_task_wakeup) {
                portYIELD_FROM_ISR();
            }
        }
    }
    portEXIT_CRITICAL_ISR(&s_keypad_spinlock);
}

static void keypad_task(void *pvParameters)
{
    (void)pvParameters;
    ESP_LOGI(TAG, "Keypad high-priority processing task started on core %d", xPortGetCoreID());

    bool reported_state[KEY_ID_COUNT] = {false, false};

    while (1) {
        // Calculate wait timeout based on smallest remaining lockout
        int64_t now_us = esp_timer_get_time();
        TickType_t wait_ticks = portMAX_DELAY;
        int64_t min_rem_us = -1;

        portENTER_CRITICAL(&s_keypad_spinlock);
        for (int i = 0; i < KEY_ID_COUNT; i++) {
            if (!s_key_debounce[i].resampled && s_key_debounce[i].lockout_end_us > 0) {
                int64_t rem = s_key_debounce[i].lockout_end_us - now_us;
                if (rem <= 0) {
                    min_rem_us = 0;
                    break;
                }
                if (min_rem_us < 0 || rem < min_rem_us) {
                    min_rem_us = rem;
                }
            }
        }
        portEXIT_CRITICAL(&s_keypad_spinlock);

        if (min_rem_us >= 0) {
            if (min_rem_us == 0) {
                wait_ticks = 0;
            } else {
                uint32_t ms = (uint32_t)((min_rem_us + 999) / 1000);
                wait_ticks = pdMS_TO_TICKS(ms);
                if (wait_ticks == 0) {
                    wait_ticks = 1;
                }
            }
        }

        ulTaskNotifyTake(pdTRUE, wait_ticks);

        now_us = esp_timer_get_time();

        // 1. Re-sample keys whose lockout has expired
        for (int i = 0; i < KEY_ID_COUNT; i++) {
            bool need_resample = false;
            portENTER_CRITICAL(&s_keypad_spinlock);
            if (!s_key_debounce[i].resampled && s_key_debounce[i].lockout_end_us > 0 &&
                now_us >= s_key_debounce[i].lockout_end_us) {
                need_resample = true;
            }
            portEXIT_CRITICAL(&s_keypad_spinlock);

            if (need_resample) {
                bool pin_level = (i == 0) ? board_key1_read() : board_key2_read();
                portENTER_CRITICAL(&s_keypad_spinlock);
                debounce_action_t action = debounce_step(
                    &s_key_debounce[i],
                    now_us,
                    pin_level,
                    DEBOUNCE_SOURCE_RESAMPLE,
                    s_config.debounce_us
                );

                if (action != DEBOUNCE_ACTION_NONE) {
                    s_key_state[i] = s_key_debounce[i].accepted_pressed;
                    s_last_transition_us[i] = s_key_debounce[i].last_transition_us;

                    // Counting rule: only accepted press transitions increment counters,
                    // including presses applied by resample. A glitch press that is later
                    // corrected will have been counted once; that is acceptable.
                    if (action == DEBOUNCE_ACTION_PRESS) {
                        if (i == 0) {
                            atomic_fetch_add_explicit(&s_key1_lifetime_presses, 1, memory_order_relaxed);
                            atomic_fetch_add_explicit(&s_key1_map_presses, 1, memory_order_relaxed);
                            s_key1_last_press_us = now_us;
                        } else {
                            atomic_fetch_add_explicit(&s_key2_lifetime_presses, 1, memory_order_relaxed);
                            atomic_fetch_add_explicit(&s_key2_map_presses, 1, memory_order_relaxed);
                            s_key2_last_press_us = now_us;
                        }
                    }
                }
                portEXIT_CRITICAL(&s_keypad_spinlock);
            }
        }

        // 2. Submit state changes to USB HID
        for (int i = 0; i < KEY_ID_COUNT; i++) {
            bool current;
            int64_t edge_us;

            portENTER_CRITICAL(&s_keypad_spinlock);
            current = s_key_state[i];
            edge_us = s_last_transition_us[i];
            portEXIT_CRITICAL(&s_keypad_spinlock);

            if (current != reported_state[i]) {
                reported_state[i] = current;
                // HID report first; activity bookkeeping only after it is submitted
                if (s_callback) {
                    if (s_callback(i, current, edge_us)) {
                        latency_stats_record((uint32_t)(esp_timer_get_time() - edge_us));
                    } else {
                        // Resent by the HID layer, which records the latency then
                        latency_stats_record_deferred();
                    }
                }
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO) || defined(OSUPAD_BENCH_DEBUG_GPIO)
                gpio_set_level(CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM, current ? 1 : 0);
#endif
                if (current) {
                    ui_notify_activity();
                }
            }
        }

        // 3. If both keys are currently released, apply any staged config change
        bool move_pins = false;
        keypad_config_t applied;
        portENTER_CRITICAL(&s_keypad_spinlock);
        if (s_config_staged && !s_key_state[0] && !s_key_state[1] &&
            !reported_state[0] && !reported_state[1]) {
            move_pins = s_staged_config.key1_gpio != s_config.key1_gpio ||
                        s_staged_config.key2_gpio != s_config.key2_gpio;
            s_config = s_staged_config;
            applied = s_config;
            usb_hid_set_keycodes(s_config.keycode1, s_config.keycode2);
            s_config_staged = false;
        }
        portEXIT_CRITICAL(&s_keypad_spinlock);

        if (move_pins) {
            // gpio_config and ISR (de)registration cannot run inside the spinlock
            esp_err_t err = board_keys_set_gpio(applied.key1_gpio, applied.key2_gpio);
            if (err != ESP_OK) {
                ESP_LOGE(TAG, "Failed to move keys to GPIO%u/GPIO%u: %s",
                         applied.key1_gpio, applied.key2_gpio, esp_err_to_name(err));
            }
            // Start debouncing from the new pins; a switch already held down is reported
            bool levels[KEY_ID_COUNT] = {board_key1_read(), board_key2_read()};
            portENTER_CRITICAL(&s_keypad_spinlock);
            for (int i = 0; i < KEY_ID_COUNT; i++) {
                debounce_init(&s_key_debounce[i], levels[i]);
                s_key_state[i] = levels[i];
                s_last_transition_us[i] = esp_timer_get_time();
            }
            portEXIT_CRITICAL(&s_keypad_spinlock);
            if (levels[0] || levels[1]) {
                xTaskNotifyGive(s_input_task_handle);
            }
        }
    }
}

esp_err_t keypad_init(const keypad_config_t *config)
{
    if (config) {
        keypad_set_config(config);
    } else {
        s_config.keycode1 = 0x1D;
        s_config.keycode2 = 0x1B;
        s_config.debounce_us = DEBOUNCE_DEFAULT_US;
        s_config.key1_gpio = KEYPAD_DEFAULT_KEY1_GPIO;
        s_config.key2_gpio = KEYPAD_DEFAULT_KEY2_GPIO;
    }

    esp_err_t err = board_keys_set_gpio(s_config.key1_gpio, s_config.key2_gpio);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Key GPIO%u/GPIO%u rejected (%s), using GPIO%d/GPIO%d",
                 s_config.key1_gpio, s_config.key2_gpio, esp_err_to_name(err),
                 KEYPAD_DEFAULT_KEY1_GPIO, KEYPAD_DEFAULT_KEY2_GPIO);
        s_config.key1_gpio = KEYPAD_DEFAULT_KEY1_GPIO;
        s_config.key2_gpio = KEYPAD_DEFAULT_KEY2_GPIO;
        err = board_keys_set_gpio(s_config.key1_gpio, s_config.key2_gpio);
        if (err != ESP_OK) {
            return err;
        }
    }

#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO) || defined(OSUPAD_BENCH_DEBUG_GPIO)
    gpio_config_t dbg_io_conf = {
        .pin_bit_mask = (1ULL << CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM),
        .mode = GPIO_MODE_OUTPUT,
        .pull_up_en = GPIO_PULLUP_DISABLE,
        .pull_down_en = GPIO_PULLDOWN_ENABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    gpio_config(&dbg_io_conf);
    gpio_set_level(CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM, 0);
    ESP_LOGI(TAG, "Debug GPIO initialized on pin %d", CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM);
#endif

    // Initial state read
    bool k1_init = board_key1_read();
    bool k2_init = board_key2_read();
    debounce_init(&s_key_debounce[0], k1_init);
    debounce_init(&s_key_debounce[1], k2_init);
    s_key_state[0] = k1_init;
    s_key_state[1] = k2_init;

    // Spawn high-priority task (configMAX_PRIORITIES - 1) pinned to Core 0
    BaseType_t res = xTaskCreatePinnedToCore(
        keypad_task,
        "keypad_task",
        4096,
        NULL,
        configMAX_PRIORITIES - 1,
        &s_input_task_handle,
        0
    );
    if (res != pdPASS) {
        ESP_LOGE(TAG, "Failed to create keypad task");
        return ESP_FAIL;
    }

    // Register board GPIO ISR
    err = board_keys_register_isr(gpio_isr_handler);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to register keypad ISR: %s", esp_err_to_name(err));
        return err;
    }

    ESP_LOGI(TAG, "Keypad initialized (debouncing=%lu us, Key1: 0x%02X@GPIO%u, Key2: 0x%02X@GPIO%u)",
             (unsigned long)s_config.debounce_us, s_config.keycode1, s_config.key1_gpio,
             s_config.keycode2, s_config.key2_gpio);
    return ESP_OK;
}

void keypad_set_state_callback(keypad_state_callback_t cb)
{
    s_callback = cb;
}

bool keypad_is_pressed(keypad_key_id_t key_id)
{
    if (key_id < KEY_ID_COUNT) {
        return s_key_state[key_id];
    }
    return false;
}

void keypad_get_lifetime_presses(uint64_t *key1_presses, uint64_t *key2_presses)
{
    if (key1_presses) {
        *key1_presses = atomic_load_explicit(&s_key1_lifetime_presses, memory_order_relaxed);
    }
    if (key2_presses) {
        *key2_presses = atomic_load_explicit(&s_key2_lifetime_presses, memory_order_relaxed);
    }
}

void keypad_set_lifetime_presses(uint64_t key1_presses, uint64_t key2_presses)
{
    atomic_store_explicit(&s_key1_lifetime_presses, key1_presses, memory_order_relaxed);
    atomic_store_explicit(&s_key2_lifetime_presses, key2_presses, memory_order_relaxed);
}

void keypad_add_lifetime_presses(uint64_t key1_presses, uint64_t key2_presses)
{
    atomic_fetch_add_explicit(&s_key1_lifetime_presses, key1_presses, memory_order_relaxed);
    atomic_fetch_add_explicit(&s_key2_lifetime_presses, key2_presses, memory_order_relaxed);
}

void keypad_get_map_presses(uint32_t *key1_presses, uint32_t *key2_presses)
{
    if (key1_presses) {
        *key1_presses = atomic_load_explicit(&s_key1_map_presses, memory_order_relaxed);
    }
    if (key2_presses) {
        *key2_presses = atomic_load_explicit(&s_key2_map_presses, memory_order_relaxed);
    }
}

void keypad_reset_map_presses(void)
{
    atomic_store_explicit(&s_key1_map_presses, 0, memory_order_relaxed);
    atomic_store_explicit(&s_key2_map_presses, 0, memory_order_relaxed);
}

int64_t keypad_get_last_press_us(keypad_key_id_t key_id)
{
    if (key_id == KEY_ID_1) return s_key1_last_press_us;
    if (key_id == KEY_ID_2) return s_key2_last_press_us;
    return 0;
}

void keypad_set_config(const keypad_config_t *config)
{
    if (!config) {
        return;
    }

    keypad_config_t clamped = *config;
    if (clamped.debounce_us < DEBOUNCE_MIN_US) {
        clamped.debounce_us = DEBOUNCE_MIN_US;
    } else if (clamped.debounce_us > DEBOUNCE_MAX_US) {
        clamped.debounce_us = DEBOUNCE_MAX_US;
    }

    if (s_input_task_handle == NULL) {
        // Before keypad_init: pins are configured from s_config there
        s_config = clamped;
        usb_hid_set_keycodes(clamped.keycode1, clamped.keycode2);
        return;
    }

    bool pins_changed;
    portENTER_CRITICAL(&s_keypad_spinlock);
    pins_changed = clamped.key1_gpio != s_config.key1_gpio || clamped.key2_gpio != s_config.key2_gpio;
    if (!pins_changed && !s_key_state[0] && !s_key_state[1]) {
        s_config = clamped;
        usb_hid_set_keycodes(clamped.keycode1, clamped.keycode2);
        s_config_staged = false;
    } else {
        // Pin moves always go through the keypad task, the only task touching key GPIOs
        s_staged_config = clamped;
        s_config_staged = true;
    }
    portEXIT_CRITICAL(&s_keypad_spinlock);

    if (pins_changed) {
        xTaskNotifyGive(s_input_task_handle);
    }
}

void keypad_get_config(keypad_config_t *out_config)
{
    if (out_config) {
        portENTER_CRITICAL(&s_keypad_spinlock);
        *out_config = s_config;
        portEXIT_CRITICAL(&s_keypad_spinlock);
    }
}

static const uint8_t SCAN_PINS[] = {2, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 21};

int keypad_detect_pressed_pin(uint32_t timeout_ms, uint32_t exclude_gpio)
{
    ESP_LOGI(TAG, "Starting interactive pin detection (timeout=%lu ms, exclude=GPIO%lu)",
             (unsigned long)timeout_ms, (unsigned long)exclude_gpio);

    if (timeout_ms == 0) {
        timeout_ms = 8000;
    }

    // Configure candidate pins with internal pull-ups
    uint64_t pin_mask = 0;
    for (size_t i = 0; i < sizeof(SCAN_PINS); i++) {
        if (SCAN_PINS[i] != exclude_gpio) {
            pin_mask |= (1ULL << SCAN_PINS[i]);
        }
    }

    gpio_config_t io_conf = {
        .pin_bit_mask = pin_mask,
        .mode = GPIO_MODE_INPUT,
        .pull_up_en = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type = GPIO_INTR_DISABLE,
    };
    gpio_config(&io_conf);
    esp_rom_delay_us(500);

    int64_t deadline = esp_timer_get_time() + (int64_t)timeout_ms * 1000;
    int detected_pin = -1;

    while (esp_timer_get_time() < deadline) {
        for (size_t i = 0; i < sizeof(SCAN_PINS); i++) {
            uint8_t pin = SCAN_PINS[i];
            if (pin == exclude_gpio) continue;
            if (gpio_get_level(pin) == 0) {
                // Confirm debounced LOW level (held for 15ms)
                vTaskDelay(pdMS_TO_TICKS(15));
                if (gpio_get_level(pin) == 0) {
                    detected_pin = pin;
                    ESP_LOGI(TAG, "Pin detected: GPIO%d pulled LOW", pin);
                    break;
                }
            }
        }
        if (detected_pin >= 0) {
            break;
        }
        vTaskDelay(pdMS_TO_TICKS(10));
    }

    // Restore active key GPIOs with pull-ups and interrupts
    portENTER_CRITICAL(&s_keypad_spinlock);
    keypad_config_t cfg = s_config;
    portEXIT_CRITICAL(&s_keypad_spinlock);

    for (size_t i = 0; i < sizeof(SCAN_PINS); i++) {
        if (SCAN_PINS[i] != cfg.key1_gpio && SCAN_PINS[i] != cfg.key2_gpio) {
            gpio_reset_pin(SCAN_PINS[i]);
        }
    }
    board_keys_set_gpio(cfg.key1_gpio, cfg.key2_gpio);

    return detected_pin;
}

