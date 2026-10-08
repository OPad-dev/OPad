#include "board_test.h"
#include "input/keypad.h"
#include "driver/gpio.h"
#include "esp_rom_sys.h"
#include "esp_log.h"

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
        for (int i = 0; i < KEY_ID_COUNT; i++) {
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

    // GPIO8 is never a key pin (not in KEY_GPIO_ALLOWED), so ID is always free
    board_read_module_id(&r.id_mv, &r.id_loaded_mv);
    r.module = board_module_from_id(r.id_mv, r.id_loaded_mv);

    if (r.module == BOARD_MODULE_NONE &&
        r.key1_gpio != BOARD_REVERSE_PROBE_GPIO && r.key2_gpio != BOARD_REVERSE_PROBE_GPIO) {
        board_probe_reversed_cable(&r.reverse_probe_mv);
    }

    // Spare lines only with a module answering: the wiring is then known.
    // A hand-wired pad may have anything on GPIO2/4/6.
    if (r.module != BOARD_MODULE_NONE) {
        test_spares(&r);
    }

    ESP_LOGI(TAG, "Module %d (boot %d), ID %d mV / %d mV loaded, reverse probe %d mV, "
             "tested 0x%llx up 0x%llx down 0x%llx bridged 0x%llx",
             r.module, r.boot_module, r.id_mv, r.id_loaded_mv, r.reverse_probe_mv,
             (unsigned long long)r.tested_gpios, (unsigned long long)r.high_with_pullup,
             (unsigned long long)r.high_with_pulldown, (unsigned long long)r.bridged_gpios);
    *out = r;
    return ESP_OK;
}
