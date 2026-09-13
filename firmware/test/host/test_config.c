#include <stdio.h>
#include <assert.h>
#include <string.h>
#include "config/device_config.h"

static void test_null_config_rejected(void)
{
    char err[64] = {0};
    bool ok = device_config_validate(NULL, err, sizeof(err));
    assert(!ok);
    assert(strcmp(err, "null config") == 0);
    printf("✓ test_null_config_rejected passed\n");
}

static void test_valid_config_accepted(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D, // 'Z'
        .key2_usage = 0x1B, // 'X'
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 3000,
        .brightness = 80,
        .sleep_s = 600,
    };
    char err[64] = {0};
    bool ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);
    assert(err[0] == '\0');
    printf("✓ test_valid_config_accepted passed\n");
}

static void test_invalid_key_usages_rejected(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x03, // Below 0x04
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 3000,
        .brightness = 80,
        .sleep_s = 600,
    };
    char err[64] = {0};
    bool ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "invalid key1 usage") != NULL);

    cfg.key1_usage = 0x1D;
    cfg.key2_usage = 0xE8; // Above 0xE7
    memset(err, 0, sizeof(err));
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "invalid key2 usage") != NULL);
    printf("✓ test_invalid_key_usages_rejected passed\n");
}

static void test_debounce_bounds(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 499, // Below 500
        .brightness = 80,
        .sleep_s = 600,
    };
    char err[64] = {0};
    bool ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "debounce") != NULL);

    cfg.debounce_us = 20001; // Above 20000
    memset(err, 0, sizeof(err));
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "debounce") != NULL);

    cfg.debounce_us = 500; // Exact min
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);

    cfg.debounce_us = 20000; // Exact max
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);
    printf("✓ test_debounce_bounds passed\n");
}

static void test_brightness_and_sleep_bounds(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 3000,
        .brightness = 101, // Above 100
        .sleep_s = 600,
    };
    char err[64] = {0};
    bool ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "brightness") != NULL);

    cfg.brightness = 100;
    cfg.sleep_s = 5; // Below 10
    memset(err, 0, sizeof(err));
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "sleep") != NULL);

    cfg.sleep_s = 86401; // Above 86400
    memset(err, 0, sizeof(err));
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "sleep") != NULL);

    cfg.sleep_s = 0; // Disabled is allowed
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);
    printf("✓ test_brightness_and_sleep_bounds passed\n");
}

static void test_gameplay_display_hz_bounds(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 3000,
        .brightness = 100,
        .sleep_s = 600,
        .gameplay_display_hz = 61, // Above 60
    };
    char err[64] = {0};
    bool ok = device_config_validate(&cfg, err, sizeof(err));
    assert(!ok);
    assert(strstr(err, "gameplay display hz") != NULL);

    cfg.gameplay_display_hz = 60; // Max allowed
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);

    cfg.gameplay_display_hz = 1; // Min allowed
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);

    cfg.gameplay_display_hz = 0; // 0 = default (allowed)
    ok = device_config_validate(&cfg, err, sizeof(err));
    assert(ok);
    printf("✓ test_gameplay_display_hz_bounds passed\n");
}

static void test_key_gpio_allow_list(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .debounce_us = 3000,
        .brightness = 100,
        .sleep_s = 600,
        .key1_gpio = 14,
        .key2_gpio = 9,
    };
    char err[64] = {0};
    assert(device_config_validate(&cfg, err, sizeof(err)));

    // Every header pin free on the Waveshare board
    const uint32_t allowed[] = {2, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 21};
    for (size_t i = 0; i < sizeof(allowed) / sizeof(allowed[0]); i++) {
        assert(device_config_key_gpio_supported(allowed[i]));
    }
    // Unset, USB, UART0 console, I2C, CAM_PWDN pull-down, LCD, strapping, out of range
    const uint32_t rejected[] = {0, 1, 3, 5, 17, 19, 20, 38, 39, 43, 44, 45, 46, 47, 48, 49, 255};
    for (size_t i = 0; i < sizeof(rejected) / sizeof(rejected[0]); i++) {
        assert(!device_config_key_gpio_supported(rejected[i]));
    }

    cfg.key1_gpio = 19;
    memset(err, 0, sizeof(err));
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "key1 gpio") != NULL);

    cfg.key1_gpio = 14;
    cfg.key2_gpio = 43;
    memset(err, 0, sizeof(err));
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "key2 gpio") != NULL);

    cfg.key2_gpio = 14; // Same pin for both keys
    memset(err, 0, sizeof(err));
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "share gpio") != NULL);

    cfg.key1_gpio = 2;
    cfg.key2_gpio = 21;
    assert(device_config_validate(&cfg, err, sizeof(err)));
    printf("✓ test_key_gpio_allow_list passed\n");
}

int main(void)
{
    test_null_config_rejected();
    test_valid_config_accepted();
    test_invalid_key_usages_rejected();
    test_debounce_bounds();
    test_brightness_and_sleep_bounds();
    test_gameplay_display_hz_bounds();
    test_key_gpio_allow_list();
    printf("All config validation unit tests passed successfully!\n");
    return 0;
}
