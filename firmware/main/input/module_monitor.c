/*
 * Input module swapped while powered: keys off at once, then the LCD says what
 * changed. module_watch.c decides; this task feeds it readings and acts.
 *
 * Every FAST_POLL_MS, one conversion tells whether anything is on the ID line
 * (board_module_present_fast, ~50 us). When that disagrees with the boot
 * module (something appeared on a pad that booted with none, or the module
 * went), the keys go off on that reading: a pad booted with nothing on ID
 * keeps its keys on (a hand-wired pad reads the same), so this is what stops
 * an HE module's analog outputs on its digital key pins from typing. It can
 * only turn keys off, as it cannot tell MX from HE.
 *
 * Every FULL_POLL_EVERY fast polls, the full reading (board_read_module_id,
 * ~8 ms, mostly sleeping) names the module: the notice, and the keys back on
 * when the boot module is seated again. A different module needs a restart
 * (the app's Restart, or a replug); the key input is only chosen at boot.
 *
 * Core 1 at priority 1, below the touch and LVGL tasks, so it never delays a
 * touch; a fast poll waits out a frame at worst.
 */
#include "module_monitor.h"
#include "input/module_watch.h"
#include "input/keypad.h"
#include "boards/waveshare_esp32s3_touch_lcd_2/board.h"
#include "ui/ui.h"
#include "esp_log.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"

static const char *TAG = "module";

#define FAST_POLL_MS     5
#define FULL_POLL_EVERY  40   // 200 ms

static void show_module_notice(board_module_type_t now)
{
    if (now == BOARD_MODULE_NONE) {
        ui_show_notice("Input module unplugged.\nKeys are off until it is back.");
    } else if (now == BOARD_MODULE_HE || now == BOARD_MODULE_MX) {
        ui_show_notice(now == BOARD_MODULE_HE
                           ? "Hall Effect module connected.\nRestart the pad to use it\n(Restart in the OPad app, or replug USB)."
                           : "MX module connected.\nRestart the pad to use it\n(Restart in the OPad app, or replug USB).");
    }
    // MODULE_WATCH_UNKNOWN: named by the next full reading
}

static void act(module_watch_t *watch, uint8_t now, int open_mv)
{
    board_module_type_t boot = board_boot_module();
    switch (module_watch_step(watch, now)) {
    case MODULE_WATCH_KEYS_OFF:
        keypad_set_input_enabled(false);
        ESP_LOGW(TAG, "Input module changed (ID %d mV): keys off", open_mv);
        show_module_notice((board_module_type_t)now);
        break;
    case MODULE_WATCH_CHANGED:
        if (now != boot) {
            show_module_notice((board_module_type_t)now);
        }
        break;
    case MODULE_WATCH_KEYS_ON:
        if (boot == BOARD_MODULE_HE) {
            // Back to how it booted: this firmware cannot read an HE module
            ui_show_notice("Hall Effect module needs firmware v2.\nKeys are disabled.");
        } else {
            keypad_set_input_enabled(true);
            ui_clear_notice();
            ESP_LOGI(TAG, "Boot module back: keys on");
        }
        break;
    case MODULE_WATCH_KEEP:
        break;
    }
}

static void module_monitor_task(void *arg)
{
    (void)arg;
    board_module_type_t boot = board_boot_module();
    bool boot_present = boot != BOARD_MODULE_NONE;
    module_watch_t watch;
    module_watch_init(&watch, (uint8_t)boot);
    uint32_t polls = 0;

    while (1) {
        vTaskDelay(pdMS_TO_TICKS(FAST_POLL_MS));

        bool present;
        if (!watch.keys_off && board_module_present_fast(&present) == ESP_OK && present != boot_present) {
            act(&watch, present ? MODULE_WATCH_UNKNOWN : BOARD_MODULE_NONE, -1);
        }

        if (++polls % FULL_POLL_EVERY == 0) {
            int open_mv = -1;
            int loaded_mv = -1;
            if (board_read_module_id(&open_mv, &loaded_mv) == ESP_OK) {
                act(&watch, (uint8_t)board_module_from_id(open_mv, loaded_mv), open_mv);
            }
        }
    }
}

esp_err_t module_monitor_start(void)
{
    BaseType_t res = xTaskCreatePinnedToCore(module_monitor_task, "module_mon", 3072, NULL,
                                             tskIDLE_PRIORITY + 1, NULL, 1);
    return res == pdPASS ? ESP_OK : ESP_ERR_NO_MEM;
}
