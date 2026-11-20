#include "board_test.h"
#include "input/keypad.h"
#include "input/he_backend.h"
#include "driver/gpio.h"
#include "esp_rom_sys.h"
#include "esp_log.h"
#include <stdio.h>

static const char *TAG = "board_test";

// The spare module lines in connector order (pins 6, 7, 8). Neither module
// uses them in v1: on MX they go nowhere, on Hall Effect to bare test pads.
static const uint8_t SPARE_GPIOS[] = {
    BOARD_MODULE_IO6_GPIO, BOARD_MODULE_IO4_GPIO, BOARD_MODULE_IO2_GPIO,
};
#define SPARE_COUNT (sizeof(SPARE_GPIOS) / sizeof(SPARE_GPIOS[0]))

#define SPARE_SETTLE_US 300

static bool read_with_pull(uint8_t pin, gpio_pull_mode_t pull)
{
    gpio_set_pull_mode(pin, pull);
    esp_rom_delay_us(SPARE_SETTLE_US);
    return gpio_get_level(pin) != 0;
}

/*
 * Each spare line must follow its own pull both ways: low with a pull-up is a
 * short to GND (or to ID, pin 5, beside pin 6, whose 10k wins over the pull),
 * high with a pull-down a short to 3V3. Then each pair of neighbours on the
 * connector: one held low by the pad, the other pulled up; if that one reads
 * low the two are bridged. Only lines that passed on their own are driven,
 * so nothing is ever driven into a short.
 */
static void test_spares(board_test_result_t *r)
{
    uint64_t ok = 0;
    for (size_t i = 0; i < SPARE_COUNT; i++) {
        uint8_t pin = SPARE_GPIOS[i];
        if (pin == r->key1_gpio || pin == r->key2_gpio) {
            continue;
        }
        uint64_t bit = 1ULL << pin;
        gpio_set_direction(pin, GPIO_MODE_INPUT);
        bool up = read_with_pull(pin, GPIO_PULLUP_ONLY);
        bool down = read_with_pull(pin, GPIO_PULLDOWN_ONLY);
        r->tested_gpios |= bit;
        if (up) {
            r->high_with_pullup |= bit;
        }
        if (down) {
            r->high_with_pulldown |= bit;
        }
        if (up && !down) {
            ok |= bit;
        }
    }

    for (size_t i = 0; i + 1 < SPARE_COUNT; i++) {
        uint8_t a = SPARE_GPIOS[i];
        uint8_t b = SPARE_GPIOS[i + 1];
        if (!(ok & (1ULL << a)) || !(ok & (1ULL << b))) {
            continue;
        }
        gpio_set_level(b, 0);
        gpio_set_direction(b, GPIO_MODE_OUTPUT);
        bool a_high = read_with_pull(a, GPIO_PULLUP_ONLY);
        gpio_set_direction(b, GPIO_MODE_INPUT);
        if (!a_high) {
            r->bridged_gpios |= (1ULL << a) | (1ULL << b);
        }
    }

    for (size_t i = 0; i < SPARE_COUNT; i++) {
        if (r->tested_gpios & (1ULL << SPARE_GPIOS[i])) {
            gpio_reset_pin(SPARE_GPIOS[i]);
        }
    }
}

typedef struct {
    bool cable_reversed;
    he_adc_test_t *out;
} he_adc_args_t;

static void run_he_adc_test(void *arg)
{
    he_adc_args_t *a = arg;
    he_adc_test_run(a->cable_reversed, a->out);
}

esp_err_t board_test_run(board_test_result_t *out)
{
    if (!out) {
        return ESP_ERR_INVALID_ARG;
    }
    board_test_result_t r = {
        .boot_module = board_boot_module(),
        .module = BOARD_MODULE_NONE,
        .id_mv = -1,
        .id_loaded_mv = -1,
        .reverse_probe_mv = -1,
    };

    keypad_line_test_t lines;
    esp_err_t err = keypad_line_test(&lines);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Key line test did not run: %s", esp_err_to_name(err));
        return err;
    }
    r.key1_gpio = lines.gpio[KEY_ID_1];
    r.key2_gpio = lines.gpio[KEY_ID_2];
    r.keys_enabled = lines.ran;
    if (lines.ran) {
        for (int i = 0; i < lines.count; i++) {
            uint64_t bit = 1ULL << lines.gpio[i];
            r.tested_gpios |= bit;
            if (lines.high_with_pullup[i]) {
                r.high_with_pullup |= bit;
            }
            if (lines.high_with_pulldown[i]) {
                r.high_with_pulldown |= bit;
            }
        }
    }

    if (he_backend_running() && he_backend_source() == HE_SOURCE_ESP_ADC) {
        // ADC1 belongs to the V1 HE input: a oneshot read would fail, and once
        // stopped the conversions. The module is what boot found.
        r.module = r.boot_module;
    } else {
        // GPIO8 is never a key pin (not in KEY_GPIO_ALLOWED), so ID is always free
        board_read_module_id(&r.id_mv, &r.id_loaded_mv);
        r.module = board_module_from_id(r.id_mv, r.id_loaded_mv);
    }

    if (r.module == BOARD_MODULE_NONE &&
        r.key1_gpio != BOARD_REVERSE_PROBE_GPIO && r.key2_gpio != BOARD_REVERSE_PROBE_GPIO) {
        board_probe_reversed_cable(&r.reverse_probe_mv);
    }

    r.cable_reversed = board_boot_cable_reversed();

    // Spare lines only with a V1 module answering: the wiring is then known.
    // A hand-wired pad may have anything on GPIO2/4/6, and on a V3 carrier
    // they are the HE module's SPI lines or the MX module's key 3.
    if (r.module == BOARD_MODULE_MX || r.module == BOARD_MODULE_HE) {
        test_spares(&r);
    }

    if (r.module == BOARD_MODULE_HE_V3) {
        r.he_adc_present = true;
        if (he_backend_running() && he_backend_source() == HE_SOURCE_ADS7028) {
            // The keys read this ADC: release them, stop the stream, test, restart
            he_adc_args_t a = {.cable_reversed = r.cable_reversed, .out = &r.he_adc};
            esp_err_t perr = he_backend_run_paused(run_he_adc_test, &a);
            if (perr != ESP_OK) {
                r.he_adc = (he_adc_test_t){.status_after_reset = -1, .status_after_clear = -1, .status_end = -1};
                snprintf(r.he_adc.message, sizeof(r.he_adc.message), "key input busy: %s",
                         esp_err_to_name(perr));
            }
        } else if (lines.ran) {
            // CS is GPIO10: with the keys armed, selecting the ADC would press one
            snprintf(r.he_adc.message, sizeof(r.he_adc.message),
                     "key input is on: restart the pad");
            r.he_adc.status_after_reset = -1;
            r.he_adc.status_after_clear = -1;
            r.he_adc.status_end = -1;
        } else {
            he_adc_test_run(r.cable_reversed, &r.he_adc);
        }
    }

    ESP_LOGI(TAG, "Module %d (boot %d), ID %d mV / %d mV loaded, reverse probe %d mV, "
             "tested 0x%llx up 0x%llx down 0x%llx bridged 0x%llx",
             r.module, r.boot_module, r.id_mv, r.id_loaded_mv, r.reverse_probe_mv,
             (unsigned long long)r.tested_gpios, (unsigned long long)r.high_with_pullup,
             (unsigned long long)r.high_with_pulldown, (unsigned long long)r.bridged_gpios);
    *out = r;
    return ESP_OK;
}
