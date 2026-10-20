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
        .debounce_us = 5000,
        .brightness = 80,
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
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
        .debounce_us = 5000,
        .brightness = 80,
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
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
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
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
        .debounce_us = 5000,
        .brightness = 101, // Above 100
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
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
        .debounce_us = 5000,
        .brightness = 100,
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
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
        .debounce_us = 5000,
        .brightness = 100,
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
        .key1_gpio = 14,
        .key2_gpio = 9,
    };
    char err[64] = {0};
    assert(device_config_validate(&cfg, err, sizeof(err)));

    // Every header pin free on the Waveshare board, and the shared list is exactly those
    const uint32_t allowed[] = {2, 4, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 18, 21};
    assert(KEY_GPIO_ALLOWED_COUNT == sizeof(allowed) / sizeof(allowed[0]));
    for (size_t i = 0; i < sizeof(allowed) / sizeof(allowed[0]); i++) {
        assert(KEY_GPIO_ALLOWED[i] == allowed[i]);
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO)
        if (allowed[i] == CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM) {
            continue;
        }
#endif
        assert(device_config_key_gpio_supported(allowed[i]));
    }
#if defined(CONFIG_OSUPAD_BENCH_DEBUG_GPIO)
    // The bench debug output is driven by the firmware, never a key
    assert(!device_config_key_gpio_supported(CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM));
    cfg.key1_gpio = CONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM;
    memset(err, 0, sizeof(err));
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "key1 gpio") != NULL);
    cfg.key1_gpio = 14;
#endif
    // Unset, USB, module-ID ADC, UART0 console, I2C, CAM_PWDN pull-down, LCD, strapping,
    // out of range
    const uint32_t rejected[] = {0, 1, 3, 5, 8, 17, 19, 20, 38, 39, 43, 44, 45, 46, 47, 48, 49, 255};
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

    cfg.key1_gpio = 8; // Module-ID divider holds it LOW: would read as a held key
    cfg.key2_gpio = 9;
    memset(err, 0, sizeof(err));
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "key1 gpio") != NULL);

    cfg.key1_gpio = 2;
    cfg.key2_gpio = 21;
    assert(device_config_validate(&cfg, err, sizeof(err)));
    printf("✓ test_key_gpio_allow_list passed\n");
}

static void test_swipe_actions(void)
{
    device_config_data_t cfg = {
        .version = DEVICE_CONFIG_VERSION,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 5000,
        .brightness = 80,
        .sleep_s = 600,
        .swipe_left_action = SWIPE_ACTION_NONE,
        .swipe_right_action = SWIPE_ACTION_MUTE,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
    };
    char err[64] = {0};
    assert(device_config_validate(&cfg, err, sizeof(err)));

    // 0 is "keep current" on the wire, never a stored action
    cfg.swipe_left_action = 0;
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "left swipe") != NULL);
    cfg.swipe_left_action = SWIPE_ACTION_VOLUME_DOWN + 1;
    assert(!device_config_validate(&cfg, err, sizeof(err)));

    // A key action needs a key
    cfg.swipe_left_action = SWIPE_ACTION_NONE;
    cfg.swipe_right_action = SWIPE_ACTION_KEY;
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "right swipe") != NULL);
    cfg.swipe_right_key = 0x3B; // F2
    assert(device_config_validate(&cfg, err, sizeof(err)));
    cfg.swipe_right_key = 0x03;
    assert(!device_config_validate(&cfg, err, sizeof(err)));

    // A key kept from an earlier KEY action is fine under another action
    cfg.swipe_right_action = SWIPE_ACTION_NEXT_TRACK;
    cfg.swipe_right_key = 0x3B;
    assert(device_config_validate(&cfg, err, sizeof(err)));
    // Up/down take any action, volume included on left/right too
    cfg.swipe_up_action = SWIPE_ACTION_VOLUME_DOWN;
    cfg.swipe_down_action = SWIPE_ACTION_KEY;
    cfg.swipe_down_key = 0x29; // Esc
    cfg.swipe_left_action = SWIPE_ACTION_VOLUME_UP;
    assert(device_config_validate(&cfg, err, sizeof(err)));
    cfg.swipe_down_key = 0;
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "down swipe") != NULL);
    cfg.swipe_down_action = SWIPE_ACTION_NEXT_TRACK;
    cfg.swipe_up_action = SWIPE_ACTION_VOLUME_DOWN + 1;
    assert(!device_config_validate(&cfg, err, sizeof(err)));
    assert(strstr(err, "up swipe") != NULL);
    printf("✓ test_swipe_actions passed\n");
}

static void test_v7_blob_with_no_he_key(void)
{
    // A v7 blob in NVS has DEVICE_CONFIG_VERSION 7 and ends after swipe_mods
    device_config_data_t v7 = {
        .version = 7,
        .key1_usage = 0x1D,
        .key2_usage = 0x1B,
        .key1_gpio = DEVICE_CONFIG_DEFAULT_KEY1_GPIO,
        .key2_gpio = DEVICE_CONFIG_DEFAULT_KEY2_GPIO,
        .debounce_us = 5000,
        .brightness = 80,
        .sleep_s = 600,
        .gameplay_display_hz = 10,
        .swipe_left_action = SWIPE_ACTION_PREV_TRACK,
        .swipe_right_action = SWIPE_ACTION_NEXT_TRACK,
        .swipe_up_action = SWIPE_ACTION_VOLUME_UP,
        .swipe_down_action = SWIPE_ACTION_VOLUME_DOWN,
        .swipe_mods = {0, 0, 0, 0},
    };
    char err[64] = {0};
    assert(device_config_validate(&v7, err, sizeof(err)));
    assert(sizeof(device_config_data_t) == offsetof(device_config_data_t, swipe_mods) + 4);

    // Older firmware reads its v7 blob unchanged. When no "he_cfg" key is present,
    // he_config_default_settings populates default HE settings:
    he_key_settings_t he_keys[2];
    he_config_default_settings(he_keys);
    assert(device_config_validate_he(he_keys, err, sizeof(err)));
    assert(he_keys[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);
    assert(he_keys[0].release_hyst == HE_CONFIG_DEFAULT_HYSTERESIS);
    assert(he_keys[0].top_reset == HE_CONFIG_DEFAULT_TOP_RESET);
    assert(he_keys[0].rt_press == HE_CONFIG_DEFAULT_RT_PRESS);
    assert(he_keys[0].rt_release == HE_CONFIG_DEFAULT_RT_RELEASE);
    assert(he_keys[0].bottom_dz == HE_CONFIG_DEFAULT_BOTTOM_DZ);
    assert(!he_keys[0].rapid_trigger);
    assert(!he_keys[0].continuous);

    printf("✓ test_v7_blob_with_no_he_key passed\n");
}

static void test_corrupt_or_short_he_record(void)
{
    he_key_settings_t keys[2];
    he_config_record_t rec;

    // Default settings
    he_config_default_settings(keys);
    keys[0].actuation = 450;
    he_config_record_pack(&rec, keys);

    // NULL buffer
    he_key_settings_t out[2];
    assert(!he_config_record_unpack(NULL, sizeof(rec), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Short buffer
    assert(!he_config_record_unpack(&rec, sizeof(rec) - 1, out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Oversized buffer
    uint8_t long_buf[sizeof(rec) + 4];
    memcpy(long_buf, &rec, sizeof(rec));
    assert(!he_config_record_unpack(long_buf, sizeof(long_buf), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Corrupted magic
    he_config_record_t bad_magic = rec;
    bad_magic.magic = 0xDEADBEEFu;
    assert(!he_config_record_unpack(&bad_magic, sizeof(bad_magic), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Corrupted version
    he_config_record_t bad_ver = rec;
    bad_ver.version = 99;
    assert(!he_config_record_unpack(&bad_ver, sizeof(bad_ver), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Corrupted CRC32
    he_config_record_t bad_crc = rec;
    bad_crc.crc32 ^= 0xFFFFFFFFu;
    assert(!he_config_record_unpack(&bad_crc, sizeof(bad_crc), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    // Corrupted field with valid CRC
    he_config_record_t bad_field = rec;
    bad_field.keys[0].actuation = 1001; // out of range
    bad_field.crc32 = he_config_crc32(&bad_field, offsetof(he_config_record_t, crc32));
    assert(!he_config_record_unpack(&bad_field, sizeof(bad_field), out));
    assert(out[0].actuation == HE_CONFIG_DEFAULT_ACTUATION);

    printf("✓ test_corrupt_or_short_he_record passed\n");
}

static void test_he_record_round_trip(void)
{
    he_key_settings_t in[2] = {
        {
            .actuation = 400,
            .release_hyst = 80,
            .top_reset = 50,
            .rt_press = 120,
            .rt_release = 120,
            .bottom_dz = 30,
            .rapid_trigger = true,
            .continuous = true,
        },
        {
            .actuation = 600,
            .release_hyst = 150,
            .top_reset = 100,
            .rt_press = 200,
            .rt_release = 200,
            .bottom_dz = 0,
            .rapid_trigger = true,
            .continuous = false,
        },
    };

    he_config_record_t rec;
    he_config_record_pack(&rec, in);

    assert(rec.magic == HE_CONFIG_RECORD_MAGIC);
    assert(rec.version == HE_CONFIG_RECORD_VERSION);
    assert(rec.crc32 == he_config_crc32(&rec, offsetof(he_config_record_t, crc32)));

    he_key_settings_t out[2];
    bool ok = he_config_record_unpack(&rec, sizeof(rec), out);
    assert(ok);

    for (int k = 0; k < 2; k++) {
        assert(out[k].actuation == in[k].actuation);
        assert(out[k].release_hyst == in[k].release_hyst);
        assert(out[k].top_reset == in[k].top_reset);
        assert(out[k].rt_press == in[k].rt_press);
        assert(out[k].rt_release == in[k].rt_release);
        assert(out[k].bottom_dz == in[k].bottom_dz);
        assert(out[k].rapid_trigger == in[k].rapid_trigger);
        assert(out[k].continuous == in[k].continuous);
    }

    printf("✓ test_he_record_round_trip passed\n");
}

static void test_he_settings_bounds(void)
{
    he_key_settings_t keys[2];
    he_config_default_settings(keys);

    char err[64] = {0};
    assert(device_config_validate_he(keys, err, sizeof(err)));

    // Actuation > 1000
    keys[0].actuation = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "actuation") != NULL);
    keys[0].actuation = 500;

    // Hysteresis > 1000
    keys[1].release_hyst = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "hysteresis") != NULL);
    keys[1].release_hyst = 100;

    // Top reset > 1000
    keys[0].top_reset = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "top_reset") != NULL);
    keys[0].top_reset = 100;

    // rt_press > 1000
    keys[1].rt_press = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "rt_press") != NULL);
    keys[1].rt_press = 150;

    // rt_release > 1000
    keys[0].rt_release = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "rt_release") != NULL);
    keys[0].rt_release = 150;

    // bottom_dz > 1000
    keys[1].bottom_dz = 1001;
    assert(!device_config_validate_he(keys, err, sizeof(err)));
    assert(strstr(err, "bottom_dz") != NULL);
    keys[1].bottom_dz = 0;

    assert(device_config_validate_he(keys, err, sizeof(err)));

    printf("✓ test_he_settings_bounds passed\n");
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
    test_swipe_actions();
    test_v7_blob_with_no_he_key();
    test_corrupt_or_short_he_record();
    test_he_record_round_trip();
    test_he_settings_bounds();
    printf("All config validation unit tests passed successfully!\n");
    return 0;
}
