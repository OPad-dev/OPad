#include "runtime.h"
#include "counters/counters.h"
#include "counters/counter_sync_rules.h"
#include "input/keypad.h"
#include "config/device_config.h"
#include "ui/ui_store.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_timer.h"
#include "esp_log.h"
#include <stdatomic.h>

static const char *TAG = "runtime";

static atomic_int s_state = ATOMIC_VAR_INIT(OSUPAD_STATE_IDLE);
static int64_t s_boot_time_us = 0;
static int64_t s_cooldown_start_us = 0;
static int64_t s_last_checkpoint_us = 0;

static atomic_llong s_last_gameplay_us = ATOMIC_VAR_INIT(0);

static const int64_t COOLDOWN_DURATION_US = 5000000; // 5 seconds
// Host streams gameplay at >=1 Hz; leave PLAYING if it stops (daemon/tosu died)
static const int64_t GAMEPLAY_TIMEOUT_US = 3000000; // 3 seconds

static void flush_layouts(void)
{
    esp_err_t err = ui_store_flush_dirty();
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Deferred layout write failed: %s (kept, retrying later)", esp_err_to_name(err));
    }
}

static void runtime_supervisor_task(void *pvParameters)
{
    (void)pvParameters;
    ESP_LOGI(TAG, "Runtime supervisor task started");

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(100));
        int64_t now = esp_timer_get_time();
        osupad_state_t current = (osupad_state_t)atomic_load(&s_state);

        if (current == OSUPAD_STATE_PLAYING) {
            if ((now - atomic_load(&s_last_gameplay_us)) >= GAMEPLAY_TIMEOUT_US) {
                ESP_LOGI(TAG, "No gameplay updates from host, leaving PLAYING");
                runtime_notify_gameplay(false);
            }
        } else if (current == OSUPAD_STATE_COOLDOWN) {
            if ((now - s_cooldown_start_us) >= COOLDOWN_DURATION_US) {
                atomic_store(&s_state, OSUPAD_STATE_IDLE);
                ESP_LOGI(TAG, "State transition: COOLDOWN -> IDLE");

                // Checkpoint dirty counters, config, and layouts safely now that gameplay has ended
                counters_checkpoint(false);
                device_config_flush();
                flush_layouts();
                s_last_checkpoint_us = now;
            }
        } else if (current == OSUPAD_STATE_IDLE) {
            device_config_flush();
            flush_layouts();
            int64_t k1 = keypad_get_last_press_us(KEY_ID_1);
            int64_t k2 = keypad_get_last_press_us(KEY_ID_2);
            int64_t last_press = k1 > k2 ? k1 : k2;
            // Host suspend (PC sleeping, possibly losing power) writes at once
            bool requested = counters_take_checkpoint_request();
            if (requested ||
                counters_checkpoint_due(counters_is_dirty(), now, last_press, s_last_checkpoint_us)) {
                counters_checkpoint(false);
                s_last_checkpoint_us = now;
            }
        }
    }
}

esp_err_t runtime_init(void)
{
    s_boot_time_us = esp_timer_get_time();
    s_last_checkpoint_us = s_boot_time_us;

    // Core 1: counter checkpoints write NVS, which must stay away from the key/USB core
    BaseType_t res = xTaskCreatePinnedToCore(
        runtime_supervisor_task,
        "runtime_task",
        4096,
        NULL,
        tskIDLE_PRIORITY + 1,
        NULL,
        1
    );
    if (res != pdPASS) {
        ESP_LOGE(TAG, "Failed to create runtime supervisor task");
        return ESP_FAIL;
    }

    ESP_LOGI(TAG, "Runtime state machine initialized (Initial state: IDLE)");
    return ESP_OK;
}

osupad_state_t runtime_get_state(void)
{
    return (osupad_state_t)atomic_load(&s_state);
}

void runtime_notify_gameplay(bool is_playing)
{
    osupad_state_t current = (osupad_state_t)atomic_load(&s_state);

    if (is_playing) {
        atomic_store(&s_last_gameplay_us, esp_timer_get_time());
        if (current != OSUPAD_STATE_PLAYING) {
            atomic_store(&s_state, OSUPAD_STATE_PLAYING);
            ESP_LOGI(TAG, "State transition: %d -> PLAYING", current);
        }
    } else {
        if (current == OSUPAD_STATE_PLAYING) {
            s_cooldown_start_us = esp_timer_get_time();
            atomic_store(&s_state, OSUPAD_STATE_COOLDOWN);
            ESP_LOGI(TAG, "State transition: PLAYING -> COOLDOWN (5s window)");
        }
    }
}

uint32_t runtime_get_uptime_seconds(void)
{
    return (uint32_t)((esp_timer_get_time() - s_boot_time_us) / 1000000);
}
