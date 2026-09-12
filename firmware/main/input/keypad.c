#include "keypad.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "ui/ui.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_timer.h"
#include "esp_log.h"
#include <stdatomic.h>
#include "input/latency_stats.h"

static const char *TAG = "keypad";

static keypad_config_t s_config = {
    .keycode1 = 0x1D,       // 'z'
    .keycode2 = 0x1B,       // 'x'
    .debounce_ms = 5,
};

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
    int64_t debounce_us = (int64_t)s_config.debounce_ms * 1000;

    bool current_pressed = (key_index == 0) ? board_key1_read() : board_key2_read();

    if (current_pressed != s_key_state[key_index]) {
        if ((now - s_last_transition_us[key_index]) >= debounce_us) {
            s_key_state[key_index] = current_pressed;
            s_last_transition_us[key_index] = now;

            if (current_pressed) {
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
    }
}

static void keypad_task(void *pvParameters)
{
    ESP_LOGI(TAG, "Keypad high-priority processing task started on core %d", xPortGetCoreID());

    bool reported_state[KEY_ID_COUNT] = {false, false};

    while (1) {
        ulTaskNotifyTake(pdTRUE, portMAX_DELAY);

        for (int i = 0; i < KEY_ID_COUNT; i++) {
            bool current = s_key_state[i];
            if (current != reported_state[i]) {
                reported_state[i] = current;
                // HID report first; activity bookkeeping only after it is submitted
                if (s_callback) {
                    int64_t edge_us = s_last_transition_us[i];
                    if (s_callback(i, current)) {
                        latency_stats_record((uint32_t)(esp_timer_get_time() - edge_us));
                    } else {
                        latency_stats_record_drop();
                    }
                }
                if (current) {
                    ui_notify_activity();
                }
            }
        }
    }
}

esp_err_t keypad_init(const keypad_config_t *config)
{
    if (config) {
        s_config = *config;
    }

    // Initial state read
    s_key_state[0] = board_key1_read();
    s_key_state[1] = board_key2_read();

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
    esp_err_t err = board_keys_register_isr(gpio_isr_handler, NULL);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "Failed to register keypad ISR: %s", esp_err_to_name(err));
        return err;
    }

    ESP_LOGI(TAG, "Keypad initialized (debouncing=%ums, Key1: 0x%02X, Key2: 0x%02X)",
             s_config.debounce_ms, s_config.keycode1, s_config.keycode2);
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
    if (config) {
        s_config = *config;
    }
}

void keypad_get_config(keypad_config_t *out_config)
{
    if (out_config) {
        *out_config = s_config;
    }
}
