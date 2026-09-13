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

int main(void)
{
    test_null_config_rejected();
    test_valid_config_accepted();
    test_invalid_key_usages_rejected();
    test_debounce_bounds();
    test_brightness_and_sleep_bounds();
    test_gameplay_display_hz_bounds();
    printf("All config validation unit tests passed successfully!\n");
    return 0;
}
