---
title: Device Settings Reference
description: Complete catalog of OPad configuration settings, data types, valid ranges, default values, and NVS storage.
---

This page documents every setting stored by the OPad firmware, including keyboard mappings, debounce timings, display preferences, swipe gesture bindings, and Hall Effect rapid trigger parameters.
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto -->

All settings are applied immediately to volatile memory (RAM). To protect the critical keypress path, non-volatile storage (NVS) writes are strictly forbidden during active gameplay (`PLAYING` and `COOLDOWN` states). Changes made while a map is active are held in RAM and automatically flushed to flash once the device returns to `IDLE`.
<!-- src: /firmware/main/config/device_config.c#write_to_nvs, /firmware/main/config/device_config.c#device_config_flush -->

---

## Core Key & Hardware Settings

Stored in the `opad_cfg` NVS namespace under the key `"cfg"` within the `device_config_data_t` structure.
<!-- src: /firmware/main/config/device_config.c#NVS_KEY -->

| Setting | Protobuf Field | C Type | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Key 1 HID Usage | `key1_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1D` ('Z') | USB HID Keyboard usage code transmitted for physical Key 1. |
| Key 2 HID Usage | `key2_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1B` ('X') | USB HID Keyboard usage code transmitted for physical Key 2. |
| Key 1 GPIO | `key1_gpio` | `uint32_t` | Allowed GPIO list | `14` (Header P2, pin 11) | Microcontroller GPIO input pin assigned to Key 1 switch. |
| Key 2 GPIO | `key2_gpio` | `uint32_t` | Allowed GPIO list | `9` (Header P2, pin 12) | Microcontroller GPIO input pin assigned to Key 2 switch. |
| Debounce Lockout | `debounce_us` | `uint32_t` | `500` – `20000` µs | `5000` µs (5 ms) | Eager debounce lockout period in microseconds. An edge triggers an instant report; subsequent bounces within this window are ignored. |
| Display Brightness | `brightness` | `uint32_t` | `0` – `100` % | `100` % | LCD backlight brightness percentage. Controlled via hardware PWM. |
| Sleep Timeout | `display_sleep_seconds` | `uint32_t` | `0` or `10` – `86400` s | `600` s (10 min) | Screen inactivity timeout in seconds. Setting `0` disables sleep entirely. |
| Gameplay Refresh Cap | `gameplay_display_hz` | `uint32_t` | `0` or `1` – `60` Hz | `10` Hz | Maximum display refresh rate while playing a beatmap, reducing Core 1 CPU load. |
<!-- src: /firmware/main/config/device_config.h, /firmware/main/config/config_validate.c -->

### Allowed Switch GPIO Pins

Keys wired directly to the board headers with a switch to ground must use one of the supported GPIO pins:
`2`, `4`, `6`, `7`, `9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `18`, `21`.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Key 1 and Key 2 cannot share the same GPIO pin. Pins used by USB (`19`, `20`), UART0 console (`43`, `44`), touch/IMU I2C (`47`, `48`), camera powerdown (`17`), and the module ID divider (`8`) are reserved and rejected by configuration validation.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED, /firmware/main/config/config_validate.c#device_config_validate -->

---

## Extended Settings (Three-Key & Game Key Selection)

Stored in the `opad_cfg` NVS namespace under the key `"cfg_ext"` within the `device_config_ext_t` structure. This separate NVS record ensures backwards compatibility if firmware is downgraded to earlier versions.
<!-- src: /firmware/main/config/device_config.c#NVS_EXT_KEY, /firmware/main/config/device_config.h#device_config_ext_t -->

| Setting | Protobuf Field | C Type | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Key 3 HID Usage | `key3_hid_usage` | `uint8_t` | `0` or `0x04` – `0xE7` | `0x06` ('C') | USB HID Keyboard usage code for Key 3 on three-key modules (Hall Effect V3). Setting `0` disables key transmission. |
| Game Keys | `game_keys` | `uint8_t` | Bitmask (`1` – `6`) | `0x03` (Keys 1 & 2) | Bitmask selecting which keys count toward session totals, KPM tap rate, and map stats (bit 0 = Key 1, bit 1 = Key 2, bit 2 = Key 3). Exactly one or two keys must be active; all three active is rejected. |
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_validate_ext -->

Every physical key maintains its own lifetime counter regardless of whether it is selected in `game_keys`.
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_GAME_KEYS -->

---

## Touchscreen Swipe Actions

The touchscreen acts as the third button. When not playing a map, four configurable directional swipe gestures are supported.
<!-- src: /protocol/opad.proto#SwipeAction, /firmware/main/config/device_config.h -->

| Gesture | Default Action | Action Values | Modifier Bits |
|---|---|---|---|
| Swipe Left | `SWIPE_ACTION_PREV_TRACK` (2) | `1` = None, `2` = Prev Track, `3` = Next Track, `4` = Play/Pause, `5` = Mute, `6` = Key, `7` = Volume Up, `8` = Volume Down | `0x01` = Ctrl, `0x02` = Shift, `0x04` = Alt, `0x08` = GUI |
| Swipe Right | `SWIPE_ACTION_NEXT_TRACK` (3) | Same as above | Same as above |
| Swipe Up | `SWIPE_ACTION_VOLUME_UP` (7) | Same as above | Same as above |
| Swipe Down | `SWIPE_ACTION_VOLUME_DOWN` (8) | Same as above | Same as above |
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

When an action is set to `SWIPE_ACTION_KEY` (6), the corresponding `swipe_*_key` defines the HID usage code transmitted, accompanied by `swipe_*_modifiers`. During gameplay (`PLAYING` state), swipe key reports are suppressed to prevent interference; only Key 1, Key 2, and Quick Retry can be reported.
<!-- src: /protocol/opad.proto#SwipeAction, /firmware/main/config/device_config.c#device_config_apply -->

---

## Hall Effect & Rapid Trigger Settings

Stored in the `opad_cfg` NVS namespace under `"he_cfg"` (Keys 1 & 2) and `"he_cfg3"` (Key 3) as fixed packed records (`he_config_record_t`) with CRC-32 integrity validation.
<!-- src: /firmware/main/config/device_config.c#NVS_HE_KEY, /firmware/main/config/he_config_record.h -->

Travel distances are specified in normalized units of 0.1% of the key's calibrated travel span (`0` – `1000`, where `1000` = 100.0% full stroke).
<!-- src: /firmware/main/input/he_params.h#HE_PCT_FULL -->

| Setting | Protobuf Field | Unit | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Actuation Depth | `actuation` | 0.1% | `0` – `1000` | `500` (50.0%) | Distance from rest required to trigger the initial key press. |
| Release Hysteresis | `release_hyst` | 0.1% | `0` – `1000` | `100` (10.0%) | Distance the stem must move upward past actuation to release in plain mode. |
| Top Reset Distance | `top_reset` | 0.1% | `0` – `1000` | `100` (10.0%) | Distance from rest where continuous Rapid Trigger deactivates and resets to initial actuation. |
| Rapid Trigger Press | `rt_press` | 0.1% | `0` – `1000` | `150` (15.0%) | Downward travel required to register an immediate re-press while depressed. |
| Rapid Trigger Release | `rt_release` | 0.1% | `0` – `1000` | `150` (15.0%) | Upward travel required to register an immediate release while depressed. |
| Bottom Dead Zone | `bottom_dead_zone` | 0.1% | `0` – `1000` | `0` (Disabled) | Clamp distance at bottom-out where sensor readings stop tracking travel. |
| Rapid Trigger Mode | `rt_mode` | Enum | `1` – `3` | `1` (`OFF`) | `1` = Plain mode (fixed actuation/hysteresis), `2` = Normal Rapid Trigger (with top reset), `3` = Continuous Rapid Trigger. |
| Allow Below Noise Floor | `allow_below_noise_floor` | Boolean | `true` / `false` | `false` | When enabled, bypasses conservative raw count safety floors. Setting this true may cause phantom presses from ambient sensor noise. |
<!-- src: /firmware/main/config/he_config_record.h, /firmware/main/input/he_params.h, /protocol/opad.proto#HeKeySettings -->

### Noise Floor Clamping

By default, the firmware enforces raw ADC count noise floors: movement thresholds must exceed the sensor's measured rest noise (`noise_floor_rest`) and movement noise (`noise_floor_movement`). If configured settings fall below these physical thresholds, the parameter builder automatically clamps them to safe limits to prevent phantom presses.
<!-- src: /firmware/main/input/he_params.h, /firmware/main/input/he_params.c -->
